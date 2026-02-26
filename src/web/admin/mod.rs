//! Admin API handlers.
//!
//! All endpoints require the `AdminUser` extractor, returning 401/403 as needed.

pub mod bluebook;
pub mod rmp;
pub mod scraper;
pub mod terms;

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Json, Response};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tracing::{error, info, instrument, trace};
use ts_rs::TS;

use crate::data::models::User;
use crate::state::AppState;
use crate::state::ServiceStatus;
use crate::web::audit::{AuditLogEntry, AuditLogResponse};
use crate::web::auth::extractors::AdminUser;
use crate::web::error::{ApiError, db_error};
use crate::web::ws::ScrapeJobDto;

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ScrapeJobsResponse {
    pub jobs: Vec<ScrapeJobDto>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AdminServiceInfo {
    name: String,
    status: ServiceStatus,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AdminStatusResponse {
    #[ts(type = "number")]
    user_count: i64,
    #[ts(type = "number")]
    session_count: i64,
    #[ts(type = "number")]
    course_count: i64,
    #[ts(type = "number")]
    scrape_job_count: i64,
    services: Vec<AdminServiceInfo>,
}

/// `GET /api/admin/status` -- Enhanced system status for admins.
#[instrument(skip_all)]
pub async fn admin_status(
    AdminUser(_user): AdminUser,
    State(state): State<AppState>,
) -> Result<Json<AdminStatusResponse>, ApiError> {
    let user_count = crate::data::users::count_all(&state.db_pool)
        .await
        .map_err(|e| db_error("count users", e))?;

    let session_count = crate::data::sessions::count_active(&state.db_pool)
        .await
        .map_err(|e| db_error("count sessions", e))?;

    let course_count = crate::data::courses::count_all(&state.db_pool)
        .await
        .map_err(|e| db_error("count courses", e))?;

    let scrape_job_count = crate::data::scrape_jobs::count_all(&state.db_pool)
        .await
        .map_err(|e| db_error("count scrape jobs", e))?;

    let services: Vec<AdminServiceInfo> = state
        .service_statuses
        .all()
        .into_iter()
        .map(|(name, status)| AdminServiceInfo { name, status })
        .collect();

    trace!(
        user_count,
        session_count,
        course_count,
        scrape_job_count,
        service_count = services.len(),
        "Fetched admin status"
    );

    Ok(Json(AdminStatusResponse {
        user_count,
        session_count,
        course_count,
        scrape_job_count,
        services,
    }))
}

/// `GET /api/admin/users` -- List all users.
#[instrument(skip_all)]
pub async fn list_users(
    AdminUser(_user): AdminUser,
    State(state): State<AppState>,
) -> Result<Json<Vec<User>>, (StatusCode, Json<Value>)> {
    let users = crate::data::users::list_users(&state.db_pool)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to list users");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "failed to list users"})),
            )
        })?;

    trace!(count = users.len(), "Listed users");

    Ok(Json(users))
}

#[derive(Deserialize)]
pub struct SetAdminBody {
    is_admin: bool,
}

/// `PUT /api/admin/users/{discord_id}/admin` -- Set admin status for a user.
#[instrument(skip_all, fields(discord_id))]
pub async fn set_user_admin(
    AdminUser(_user): AdminUser,
    State(state): State<AppState>,
    Path(discord_id): Path<i64>,
    Json(body): Json<SetAdminBody>,
) -> Result<Json<User>, (StatusCode, Json<Value>)> {
    let user = crate::data::users::set_admin(&state.db_pool, discord_id, body.is_admin)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to set admin status");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "failed to set admin status"})),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "user not found"})),
            )
        })?;

    state.session_cache.evict_user(discord_id);

    info!(
        discord_id,
        is_admin = body.is_admin,
        "Updated user admin status"
    );

    Ok(Json(user))
}

/// `GET /api/admin/scrape-jobs` -- List scrape jobs.
#[instrument(skip_all)]
pub async fn list_scrape_jobs(
    AdminUser(_user): AdminUser,
    State(state): State<AppState>,
) -> Result<Json<ScrapeJobsResponse>, ApiError> {
    let rows = crate::data::scrape_jobs::list_ordered(&state.db_pool, 100)
        .await
        .map_err(|e| db_error("list scrape jobs", e))?;

    let jobs: Vec<ScrapeJobDto> = rows.iter().map(ScrapeJobDto::from).collect();

    trace!(count = jobs.len(), "Listed scrape jobs");

    Ok(Json(ScrapeJobsResponse { jobs }))
}

/// Format a `DateTime<Utc>` as an HTTP-date (RFC 2822) for Last-Modified headers.
fn to_http_date(dt: &DateTime<Utc>) -> String {
    dt.format("%a, %d %b %Y %H:%M:%S GMT").to_string()
}

/// Parse an `If-Modified-Since` header value into a `DateTime<Utc>`.
fn parse_if_modified_since(headers: &HeaderMap) -> Option<DateTime<Utc>> {
    let val = headers.get(header::IF_MODIFIED_SINCE)?.to_str().ok()?;
    DateTime::parse_from_rfc2822(val)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

/// `GET /api/admin/audit-log` -- List recent audit entries.
///
/// Supports `If-Modified-Since`: returns 304 when the newest entry hasn't changed.
#[instrument(skip_all)]
pub async fn list_audit_log(
    AdminUser(_user): AdminUser,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Response, ApiError> {
    let rows = crate::data::audit::list_recent(&state.db_pool, 200)
        .await
        .map_err(|e| db_error("list audit log", e))?;

    // Determine the latest timestamp across all rows (query is DESC so first row is newest)
    let latest = rows.first().map(|r| r.timestamp);

    // If the client sent If-Modified-Since and our data hasn't changed, return 304
    if let (Some(since), Some(latest_ts)) = (parse_if_modified_since(&headers), latest) {
        // Truncate to seconds for comparison (HTTP dates have second precision)
        if latest_ts.timestamp() <= since.timestamp() {
            trace!("Audit log not modified, returning 304");
            let mut resp = StatusCode::NOT_MODIFIED.into_response();
            if let Ok(val) = to_http_date(&latest_ts).parse() {
                resp.headers_mut().insert(header::LAST_MODIFIED, val);
            }
            return Ok(resp);
        }
    }

    let entries: Vec<AuditLogEntry> = rows.into_iter().map(AuditLogEntry::from).collect();

    trace!(count = entries.len(), "Listed audit log entries");

    let mut resp = Json(AuditLogResponse { entries }).into_response();
    if let Some(latest_ts) = latest
        && let Ok(val) = to_http_date(&latest_ts).parse()
    {
        resp.headers_mut().insert(header::LAST_MODIFIED, val);
    }
    Ok(resp)
}
