//! Axum extractors for authentication and authorization.

use axum::extract::FromRequestParts;
use axum::http::{StatusCode, header};
use axum::response::Json;
use http::request::Parts;
use serde_json::json;

use crate::data::models::User;
use crate::state::AppState;

/// Extractor that resolves the session cookie to an authenticated [`User`].
///
/// Returns 401 if no valid session cookie is present.
pub struct AuthUser(pub User);

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = (StatusCode, Json<serde_json::Value>);

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get(header::COOKIE)
            .and_then(|v| v.to_str().ok())
            .and_then(|cookies| {
                cookies
                    .split(';')
                    .find_map(|c| c.trim().strip_prefix("session=").map(|v| v.to_owned()))
            })
            .ok_or_else(|| {
                (
                    StatusCode::UNAUTHORIZED,
                    Json(json!({"error": "unauthorized", "message": "No session cookie"})),
                )
            })?;

        let user = state.session_cache.get_user(&token).await.ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "unauthorized", "message": "Invalid or expired session"})),
            )
        })?;

        Ok(AuthUser(user))
    }
}

/// Extractor that requires an authenticated admin user.
///
/// Returns 401 if not authenticated, 403 if not admin.
pub struct AdminUser(pub User);

impl FromRequestParts<AppState> for AdminUser {
    type Rejection = (StatusCode, Json<serde_json::Value>);

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let AuthUser(user) = AuthUser::from_request_parts(parts, state).await?;

        if !user.is_admin {
            return Err((
                StatusCode::FORBIDDEN,
                Json(json!({"error": "forbidden", "message": "Admin access required"})),
            ));
        }

        Ok(AdminUser(user))
    }
}
