//! Health, status, and metrics handlers.

use axum::extract::{Query, State};
use axum::response::Json;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use tracing::trace;
use ts_rs::TS;

use crate::state::{AppState, ServiceStatus};
use crate::web::error::{ApiError, ApiErrorCode, db_error};

fn default_metrics_limit() -> i32 {
    500
}

#[derive(Serialize, TS)]
#[ts(export)]
pub struct ServiceInfo {
    name: String,
    status: ServiceStatus,
}

#[derive(Serialize, TS)]
#[ts(export)]
pub struct StatusResponse {
    status: ServiceStatus,
    version: String,
    commit: String,
    services: BTreeMap<String, ServiceInfo>,
}

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MetricEntry {
    pub id: i32,
    pub course_id: i32,
    pub timestamp: String,
    pub enrollment: i32,
    pub wait_count: i32,
    pub seats_available: i32,
}

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MetricsResponse {
    pub metrics: Vec<MetricEntry>,
    pub count: usize,
    pub timestamp: String,
}

#[derive(Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MetricsParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub course_id: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub term: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crn: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<String>,
    #[serde(default = "default_metrics_limit")]
    pub limit: i32,
}

/// Health check endpoint
pub(super) async fn health() -> Json<Value> {
    trace!("health check requested");
    Json(json!({
        "status": "healthy",
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

/// Status endpoint showing bot and system status
pub(super) async fn status(State(state): State<AppState>) -> Json<StatusResponse> {
    let mut services = BTreeMap::new();

    for (name, svc_status) in state.service_statuses.all() {
        services.insert(
            name.clone(),
            ServiceInfo {
                name,
                status: svc_status,
            },
        );
    }

    let overall_status = if services
        .values()
        .any(|s| matches!(s.status, ServiceStatus::Error))
    {
        ServiceStatus::Error
    } else if !services.is_empty()
        && services
            .values()
            .all(|s| matches!(s.status, ServiceStatus::Active | ServiceStatus::Connected))
    {
        ServiceStatus::Active
    } else if services.is_empty() {
        ServiceStatus::Disabled
    } else {
        ServiceStatus::Active
    };

    Json(StatusResponse {
        status: overall_status,
        version: env!("CARGO_PKG_VERSION").to_string(),
        commit: env!("GIT_COMMIT_HASH").to_string(),
        services,
    })
}

/// Metrics endpoint for monitoring
pub(super) async fn metrics(
    State(state): State<AppState>,
    Query(params): Query<MetricsParams>,
) -> Result<Json<MetricsResponse>, ApiError> {
    let limit = params.limit.clamp(1, 5000);

    let range_str = params.range.as_deref().unwrap_or("24h");
    let duration = match range_str {
        "1h" => chrono::Duration::hours(1),
        "6h" => chrono::Duration::hours(6),
        "24h" => chrono::Duration::hours(24),
        "7d" => chrono::Duration::days(7),
        "30d" => chrono::Duration::days(30),
        _ => {
            return Err(ApiError::new(
                ApiErrorCode::InvalidRange,
                format!("Invalid range '{range_str}'. Valid: 1h, 6h, 24h, 7d, 30d"),
            ));
        }
    };
    let since = chrono::Utc::now() - duration;

    let course_id = if let Some(id) = params.course_id {
        Some(id)
    } else if let (Some(term), Some(crn)) = (params.term.as_deref(), params.crn.as_deref()) {
        use crate::banner::models::terms::Term;
        let resolved = Term::resolve_to_code(term).unwrap_or_else(|| term.to_string());
        crate::data::courses::get_id_by_crn(&state.db_pool, &resolved, crn)
            .await
            .map_err(|e| db_error("Course lookup for metrics", e))?
    } else {
        None
    };

    let metrics = if let Some(cid) = course_id {
        crate::data::metrics::list_for_course(&state.db_pool, cid, since, limit)
            .await
            .map_err(|e| db_error("Metrics query", e))?
    } else {
        crate::data::metrics::list_all(&state.db_pool, since, limit)
            .await
            .map_err(|e| db_error("Metrics query", e))?
    };

    let count = metrics.len();
    let metrics_entries: Vec<MetricEntry> = metrics
        .into_iter()
        .map(|row| MetricEntry {
            id: row.id,
            course_id: row.course_id,
            timestamp: row.timestamp.to_rfc3339(),
            enrollment: row.enrollment,
            wait_count: row.wait_count,
            seats_available: row.seats_available,
        })
        .collect();

    Ok(Json(MetricsResponse {
        metrics: metrics_entries,
        count,
        timestamp: chrono::Utc::now().to_rfc3339(),
    }))
}
