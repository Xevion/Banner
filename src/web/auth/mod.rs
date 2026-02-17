//! Discord OAuth2 authentication handlers.
//!
//! Provides login, callback, logout, and session introspection endpoints
//! for Discord OAuth2 authentication flow.

pub mod extractors;
pub mod session;

use axum::extract::{Extension, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Json, Redirect, Response};
use serde::Deserialize;
use serde_json::{Value, json};
use std::time::Duration;
use tracing::{error, info, instrument, warn};

use crate::state::AppState;

/// OAuth configuration passed as an Axum Extension.
#[derive(Clone)]
pub struct AuthConfig {
    pub client_id: String,
    pub client_secret: String,
    /// Optional base URL override (e.g. "https://banner.xevion.dev").
    /// When `None`, the redirect URI is derived from the request's Origin/Host header.
    pub redirect_base: Option<String>,
}

const CALLBACK_PATH: &str = "/api/auth/callback";

/// Derive the origin (scheme + host + port) the user's browser is actually on.
///
/// Priority:
/// 1. Configured `redirect_base` (production override)
/// 2. `Referer` header — preserves the real browser origin even through
///    reverse proxies that rewrite `Host` (e.g. Vite dev proxy with
///    `changeOrigin: true`)
/// 3. `Origin` header (present on POST / CORS requests)
/// 4. `Host` header (last resort, may be rewritten by proxies)
fn resolve_origin(auth_config: &AuthConfig, headers: &HeaderMap) -> String {
    if let Some(base) = &auth_config.redirect_base {
        return base.trim_end_matches('/').to_owned();
    }

    // Referer carries the full browser URL; extract just the origin.
    if let Some(referer) = headers.get(header::REFERER).and_then(|v| v.to_str().ok())
        && let Ok(parsed) = url::Url::parse(referer)
    {
        let origin = parsed.origin().unicode_serialization();
        if origin != "null" {
            return origin;
        }
    }

    if let Some(origin) = headers.get("origin").and_then(|v| v.to_str().ok()) {
        return origin.trim_end_matches('/').to_owned();
    }

    if let Some(host) = headers.get(header::HOST).and_then(|v| v.to_str().ok()) {
        return format!("http://{host}");
    }

    "http://localhost:8080".to_owned()
}

#[derive(Deserialize)]
pub struct CallbackParams {
    code: String,
    state: String,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
}

#[derive(Deserialize)]
struct DiscordUser {
    id: String,
    username: String,
    avatar: Option<String>,
}

/// Extract the `session` cookie value from request headers.
fn extract_session_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .find_map(|cookie| {
            let cookie = cookie.trim();
            cookie.strip_prefix("session=").map(|v| v.to_owned())
        })
}

/// Build a `Set-Cookie` header value for the session cookie.
fn session_cookie(token: &str, max_age: i64, secure: bool) -> String {
    let mut cookie = format!("session={token}; HttpOnly; SameSite=Lax; Path=/; Max-Age={max_age}");
    if secure {
        cookie.push_str("; Secure");
    }
    cookie
}

/// `GET /api/auth/login` — Redirect to Discord OAuth2 authorization page.
#[instrument(skip_all)]
pub async fn auth_login(
    State(state): State<AppState>,
    Extension(auth_config): Extension<AuthConfig>,
    headers: HeaderMap,
) -> Redirect {
    let origin = resolve_origin(&auth_config, &headers);
    let redirect_uri = format!("{origin}{CALLBACK_PATH}");
    let csrf_state = state.oauth_state_store.generate(origin);
    let redirect_uri_encoded = urlencoding::encode(&redirect_uri);

    let url = format!(
        "https://discord.com/oauth2/authorize\
         ?client_id={}\
         &redirect_uri={redirect_uri_encoded}\
         &response_type=code\
         &scope=identify\
         &state={csrf_state}",
        auth_config.client_id,
    );

    Redirect::temporary(&url)
}

/// `GET /api/auth/callback` — Handle Discord OAuth2 callback.
#[instrument(skip_all)]
pub async fn auth_callback(
    State(state): State<AppState>,
    Extension(auth_config): Extension<AuthConfig>,
    Query(params): Query<CallbackParams>,
) -> Result<Response, (StatusCode, Json<Value>)> {
    // 1. Validate CSRF state and recover the origin used during login
    let origin = state
        .oauth_state_store
        .validate(&params.state)
        .ok_or_else(|| {
            warn!("OAuth callback with invalid CSRF state");
            (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "Invalid OAuth state" })),
            )
        })?;

    // 2. Exchange authorization code for access token
    let redirect_uri = format!("{origin}{CALLBACK_PATH}");
    let client = reqwest::Client::new();
    let token_response = client
        .post("https://discord.com/api/oauth2/token")
        .form(&[
            ("client_id", auth_config.client_id.as_str()),
            ("client_secret", auth_config.client_secret.as_str()),
            ("grant_type", "authorization_code"),
            ("code", params.code.as_str()),
            ("redirect_uri", redirect_uri.as_str()),
        ])
        .send()
        .await
        .map_err(|e| {
            error!(error = %e, "failed to exchange OAuth code for token");
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({ "error": "Failed to exchange code with Discord" })),
            )
        })?;

    if !token_response.status().is_success() {
        let status = token_response.status();
        let body = token_response.text().await.unwrap_or_default();
        error!(%status, %body, "Discord token exchange returned error");
        return Err((
            StatusCode::BAD_GATEWAY,
            Json(json!({ "error": "Discord token exchange failed" })),
        ));
    }

    let token_data: TokenResponse = token_response.json().await.map_err(|e| {
        error!(error = %e, "failed to parse Discord token response");
        (
            StatusCode::BAD_GATEWAY,
            Json(json!({ "error": "Invalid token response from Discord" })),
        )
    })?;

    // 3. Fetch Discord user profile
    let discord_user: DiscordUser = client
        .get("https://discord.com/api/users/@me")
        .bearer_auth(&token_data.access_token)
        .send()
        .await
        .map_err(|e| {
            error!(error = %e, "failed to fetch Discord user profile");
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({ "error": "Failed to fetch Discord profile" })),
            )
        })?
        .json()
        .await
        .map_err(|e| {
            error!(error = %e, "failed to parse Discord user profile");
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({ "error": "Invalid user profile from Discord" })),
            )
        })?;

    let discord_id: i64 = discord_user.id.parse().map_err(|_| {
        error!(id = %discord_user.id, "Discord user ID is not a valid i64");
        (
            StatusCode::BAD_GATEWAY,
            Json(json!({ "error": "Invalid Discord user ID" })),
        )
    })?;

    // 4. Upsert user
    let user = crate::data::users::upsert_user(
        &state.db_pool,
        discord_id,
        &discord_user.username,
        discord_user.avatar.as_deref(),
    )
    .await
    .map_err(|e| {
        error!(error = %e, "failed to upsert user");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "Database error" })),
        )
    })?;

    info!(discord_id, username = %user.discord_username, "user authenticated via OAuth");

    // 5. Create session
    let session = crate::data::sessions::create_session(
        &state.db_pool,
        discord_id,
        Duration::from_secs(crate::data::sessions::SESSION_DURATION_SECS),
    )
    .await
    .map_err(|e| {
        error!(error = %e, "failed to create session");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "Failed to create session" })),
        )
    })?;

    // 6. Build response with session cookie
    let secure = redirect_uri.starts_with("https://");
    let cookie = session_cookie(
        &session.id,
        crate::data::sessions::SESSION_DURATION_SECS as i64,
        secure,
    );

    let redirect_to = if user.is_admin { "/admin" } else { "/" };

    Ok((
        [(header::SET_COOKIE, cookie)],
        Redirect::temporary(redirect_to),
    )
        .into_response())
}

/// `POST /api/auth/logout` — Destroy the current session.
#[instrument(skip_all)]
pub async fn auth_logout(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Some(token) = extract_session_token(&headers) {
        if let Err(e) = crate::data::sessions::delete_session(&state.db_pool, &token).await {
            warn!(error = %e, "failed to delete session from database");
        }
        state.session_cache.evict(&token);
    }

    let cookie = session_cookie("", 0, false);

    (
        StatusCode::OK,
        [(header::SET_COOKIE, cookie)],
        Json(json!({ "ok": true })),
    )
        .into_response()
}

/// `GET /api/auth/me` — Return the current authenticated user's info.
#[instrument(skip_all)]
pub async fn auth_me(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    let token = extract_session_token(&headers).ok_or(StatusCode::UNAUTHORIZED)?;

    let user = state
        .session_cache
        .get_user(&token)
        .await
        .ok_or(StatusCode::UNAUTHORIZED)?;

    Ok(Json(json!({
        "discordId": user.discord_id.to_string(),
        "username": user.discord_username,
        "avatarHash": user.discord_avatar_hash,
        "isAdmin": user.is_admin,
    })))
}
