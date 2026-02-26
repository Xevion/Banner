//! Web API router construction and shared response utilities.

use axum::{
    Extension, Router,
    extract::{ConnectInfo, Request, State},
    http::HeaderValue,
    response::{IntoResponse, Response},
    routing::{get, post, put},
};

use std::net::SocketAddr;
use std::time::Duration;

use axum::http::StatusCode;
use axum::response::Json;

use crate::state::AppState;
use crate::web::auth::{self, AuthConfig};
use crate::web::middleware::request_id::RequestIdLayer;
use crate::web::middleware::security_headers::SecurityHeadersLayer;
use crate::web::{
    admin, calendar, courses, csp_report, instructors, search_options, status, stream, suggest,
    timeline,
};
use tower_http::{compression::CompressionLayer, timeout::TimeoutLayer};

#[cfg(feature = "embed-assets")]
use crate::web::assets::try_serve_asset_with_encoding;

/// Cache-Control presets for public endpoints.
///
/// Cloudflare respects `s-maxage` for edge caching and `stale-while-revalidate`
/// for serving stale content while re-fetching in the background.
pub mod cache {
    /// Reference data, search-options, suggest, instructor list.
    pub const REFERENCE: &str = "public, max-age=300, s-maxage=3600, stale-while-revalidate=300";
    /// Course search results.
    pub const SEARCH: &str = "public, max-age=60, s-maxage=300, stale-while-revalidate=120";
    /// Course/instructor detail (typically paired with ETag).
    pub const DETAIL: &str = "public, max-age=60, s-maxage=300, stale-while-revalidate=120";
    /// Admin endpoints -- never cache.
    pub const ADMIN: &str = "private, no-store, must-revalidate";
}

/// Wraps a JSON response with a `Cache-Control` header.
pub fn with_cache_control<T: serde::Serialize>(value: T, header: &'static str) -> Response {
    let mut response = Json(value).into_response();
    response.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        HeaderValue::from_static(header),
    );
    response
}

/// Creates the web server router
pub fn create_router(app_state: AppState, auth_config: AuthConfig) -> Router {
    let api_router = Router::new()
        .route("/health", get(status::health))
        .route("/status", get(status::status))
        .route("/metrics", get(status::metrics))
        .route("/courses/search", get(courses::search_courses))
        .route("/courses/{term}/{crn}", get(courses::get_course))
        .route(
            "/courses/{term}/{subject}/{course_number}/sections",
            get(courses::get_related_sections),
        )
        .route(
            "/courses/{term}/{crn}/calendar.ics",
            get(calendar::course_ics),
        )
        .route("/courses/{term}/{crn}/gcal", get(calendar::course_gcal))
        .route("/reference/{category}", get(search_options::get_reference))
        .route("/search-options", get(search_options::get_search_options))
        .route("/suggest", get(suggest::suggest))
        .route("/instructors/resolve", get(suggest::resolve_instructors))
        .route("/instructors/suggest", get(suggest::suggest_instructors))
        .route("/instructors", get(instructors::list_instructors))
        .route("/instructors/{slug}", get(instructors::get_instructor))
        .route(
            "/instructors/{slug}/sections",
            get(instructors::get_instructor_sections),
        )
        .route("/timeline", post(timeline::timeline))
        .route("/ws", get(stream::stream_ws))
        .route("/csp-report", post(csp_report::csp_report))
        .with_state(app_state.clone());

    let auth_router = Router::new()
        .route("/auth/login", get(auth::auth_login))
        .route("/auth/callback", get(auth::auth_callback))
        .route("/auth/logout", post(auth::auth_logout))
        .route("/auth/me", get(auth::auth_me))
        .layer(Extension(auth_config))
        .with_state(app_state.clone());

    let admin_router = Router::new()
        .route("/admin/status", get(admin::admin_status))
        .route("/admin/users", get(admin::list_users))
        .route(
            "/admin/users/{discord_id}/admin",
            put(admin::set_user_admin),
        )
        .route("/admin/scrape-jobs", get(admin::list_scrape_jobs))
        .route("/admin/audit-log", get(admin::list_audit_log))
        .route("/admin/instructors", get(admin::rmp::list_instructors))
        .route("/admin/instructors/{id}", get(admin::rmp::get_instructor))
        .route(
            "/admin/instructors/{id}/match",
            post(admin::rmp::match_instructor),
        )
        .route(
            "/admin/instructors/{id}/reject-candidate",
            post(admin::rmp::reject_candidate),
        )
        .route(
            "/admin/instructors/{id}/reject-all",
            post(admin::rmp::reject_all),
        )
        .route(
            "/admin/instructors/{id}/unmatch",
            post(admin::rmp::unmatch_instructor),
        )
        .route("/admin/rmp/rescore", post(admin::rmp::rescore))
        .route("/admin/scraper/stats", get(admin::scraper::scraper_stats))
        .route(
            "/admin/scraper/timeseries",
            get(admin::scraper::scraper_timeseries),
        )
        .route(
            "/admin/scraper/subjects",
            get(admin::scraper::scraper_subjects),
        )
        .route(
            "/admin/scraper/subjects/{subject}",
            get(admin::scraper::scraper_subject_detail),
        )
        .route("/admin/bluebook/sync", post(admin::bluebook::sync_bluebook))
        .route("/admin/bluebook/links", get(admin::bluebook::list_links))
        .route("/admin/bluebook/links/{id}", get(admin::bluebook::get_link))
        .route(
            "/admin/bluebook/links/{id}/approve",
            post(admin::bluebook::approve_link),
        )
        .route(
            "/admin/bluebook/links/{id}/reject",
            post(admin::bluebook::reject_link),
        )
        .route(
            "/admin/bluebook/links/{id}/assign",
            post(admin::bluebook::assign_link),
        )
        .route("/admin/bluebook/match", post(admin::bluebook::run_matching))
        .route("/admin/terms", get(admin::terms::list_terms))
        .route("/admin/terms/sync", post(admin::terms::sync_terms))
        .route(
            "/admin/terms/{code}/enable",
            post(admin::terms::enable_term),
        )
        .route(
            "/admin/terms/{code}/disable",
            post(admin::terms::disable_term),
        )
        .layer(axum::middleware::map_response(
            |mut resp: Response| async move {
                resp.headers_mut().insert(
                    axum::http::header::CACHE_CONTROL,
                    HeaderValue::from_static(cache::ADMIN),
                );
                resp
            },
        ))
        .with_state(app_state.clone());

    use crate::web::sitemap;

    let router = Router::new()
        .route("/robots.txt", get(robots_txt))
        .route("/sitemap.xml", get(sitemap::sitemap_index))
        .route("/sitemap-static.xml", get(sitemap::sitemap_static))
        .route(
            "/sitemap-instructors.xml",
            get(sitemap::sitemap_instructors),
        )
        .route("/sitemap-courses-{rest}", get(sitemap::sitemap_courses))
        .route("/sitemap-subjects.xml", get(sitemap::sitemap_subjects))
        .nest("/api", api_router)
        .nest("/api", auth_router)
        .nest("/api", admin_router)
        .fallback(ssr_fallback)
        .with_state(app_state);

    router.layer((
        // Outermost: per-request ID span + severity-proportional response logging.
        RequestIdLayer,
        // Security headers on every response (HSTS is prod-only).
        SecurityHeadersLayer,
        // Compress API responses (gzip/brotli/zstd). Pre-compressed static
        // assets already have Content-Encoding set, so tower-http skips them.
        CompressionLayer::new()
            .zstd(true)
            .br(true)
            .gzip(true)
            .quality(tower_http::CompressionLevel::Fastest),
        TimeoutLayer::new(Duration::from_secs(60)),
    ))
}

/// SSR fallback: try embedded static assets first, then proxy to the SSR server.
async fn ssr_fallback(
    State(state): State<AppState>,
    connect_info: ConnectInfo<SocketAddr>,
    request: Request,
) -> axum::response::Response {
    let method = request.method().clone();
    let uri = request.uri().clone();
    let path = uri.path();
    let query = uri.query();
    let mut headers = request.headers().clone();

    // Augment X-Forwarded-For so the SSR server (and its backend calls) see
    // the real client IP, not localhost. Append the peer address to any
    // existing value rather than replacing it.
    let client_ip = connect_info.0.ip().to_string();
    let xff_value = match headers.get("x-forwarded-for") {
        Some(existing) => {
            let existing = existing.to_str().unwrap_or("");
            format!("{existing}, {client_ip}")
        }
        None => client_ip,
    };
    if let Ok(value) = HeaderValue::from_str(&xff_value) {
        headers.insert("x-forwarded-for", value);
    }

    // Try serving embedded static assets (production only)
    #[cfg(feature = "embed-assets")]
    {
        if let Some(response) = try_serve_asset_with_encoding(path, &headers) {
            return response;
        }

        // SvelteKit assets under _app/ that don't exist are a hard 404
        let trimmed = path.trim_start_matches('/');
        if trimmed.starts_with("_app/") || trimmed.starts_with("assets/") {
            return (StatusCode::NOT_FOUND, "Asset not found").into_response();
        }
    }

    // Proxy to the downstream SSR server
    crate::web::proxy::proxy_to_ssr(&state, &method, path, query, headers).await
}

/// `GET /robots.txt`
///
/// Blocks crawlers from API and admin paths. Includes sitemap directive when
/// `PUBLIC_ORIGIN` is configured.
async fn robots_txt(State(state): State<AppState>) -> Response {
    let mut body = String::from(
        "User-agent: *\n\
         Disallow: /api/\n\
         Disallow: /admin/\n",
    );
    if let Some(ref origin) = state.public_origin {
        body.push_str(&format!("\nSitemap: {origin}/sitemap.xml\n"));
    }
    let mut resp = body.into_response();
    resp.headers_mut().insert(
        axum::http::header::CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    resp.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=86400"),
    );
    resp
}
