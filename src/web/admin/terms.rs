//! Admin API handlers for term management.
//!
//! All endpoints require the `AdminUser` extractor, returning 401/403 as needed.

use std::time::{Duration, Instant};

use crate::utils::fmt_duration;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::Json;
use serde::Serialize;
use serde_json::{Value, json};
use tracing::{error, info, instrument, trace, warn};
use ts_rs::TS;

use crate::data::terms::{self, DbTerm, SyncResult};
use crate::state::AppState;
use crate::web::auth::extractors::AdminUser;

type ApiError = (StatusCode, Json<Value>);

const SLOW_OP_THRESHOLD: Duration = Duration::from_secs(1);

fn db_error(context: &str, e: anyhow::Error) -> ApiError {
    error!(error = %e, "{context}");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error": context})),
    )
}

/// Response for `GET /api/admin/terms`.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TermsListResponse {
    pub terms: Vec<DbTerm>,
}

/// Response for `POST /api/admin/terms/:code/enable` and `disable`.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TermUpdateResponse {
    pub success: bool,
    pub term: Option<DbTerm>,
}

/// Response for `POST /api/admin/terms/sync`.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TermSyncResponse {
    pub inserted: usize,
    pub updated: usize,
}

impl From<SyncResult> for TermSyncResponse {
    fn from(result: SyncResult) -> Self {
        Self {
            inserted: result.inserted,
            updated: result.updated,
        }
    }
}

/// `GET /api/admin/terms` -- List all terms with their scraping status.
#[instrument(skip_all)]
pub async fn list_terms(
    _admin: AdminUser,
    State(state): State<AppState>,
) -> Result<Json<TermsListResponse>, ApiError> {
    let start = Instant::now();

    let terms = terms::get_all_terms(&state.db_pool)
        .await
        .map_err(|e| db_error("Failed to fetch terms", e))?;

    let elapsed = start.elapsed();
    if elapsed > SLOW_OP_THRESHOLD {
        warn!(
            duration = fmt_duration(elapsed),
            "slow operation: list_terms"
        );
    }

    trace!(count = terms.len(), "listed terms");
    Ok(Json(TermsListResponse { terms }))
}

/// `POST /api/admin/terms/:code/enable` -- Enable scraping for a term.
#[instrument(skip_all, fields(term_code = %code))]
pub async fn enable_term(
    _admin: AdminUser,
    State(state): State<AppState>,
    Path(code): Path<String>,
) -> Result<Json<TermUpdateResponse>, ApiError> {
    let start = Instant::now();

    let found = terms::enable_scraping(&state.db_pool, &code)
        .await
        .map_err(|e| db_error("Failed to enable scraping", e))?;

    if !found {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Term not found"})),
        ));
    }

    let term = terms::get_term_by_code(&state.db_pool, &code)
        .await
        .map_err(|e| db_error("Failed to fetch updated term", e))?;

    let elapsed = start.elapsed();
    if elapsed > SLOW_OP_THRESHOLD {
        warn!(
            duration = fmt_duration(elapsed),
            "slow operation: enable_term"
        );
    }

    info!(term_code = %code, "term scraping enabled");

    Ok(Json(TermUpdateResponse {
        success: true,
        term,
    }))
}

/// `POST /api/admin/terms/:code/disable` -- Disable scraping for a term.
#[instrument(skip_all, fields(term_code = %code))]
pub async fn disable_term(
    _admin: AdminUser,
    State(state): State<AppState>,
    Path(code): Path<String>,
) -> Result<Json<TermUpdateResponse>, ApiError> {
    let start = Instant::now();

    let found = terms::disable_scraping(&state.db_pool, &code)
        .await
        .map_err(|e| db_error("Failed to disable scraping", e))?;

    if !found {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Term not found"})),
        ));
    }

    let term = terms::get_term_by_code(&state.db_pool, &code)
        .await
        .map_err(|e| db_error("Failed to fetch updated term", e))?;

    let elapsed = start.elapsed();
    if elapsed > SLOW_OP_THRESHOLD {
        warn!(
            duration = fmt_duration(elapsed),
            "slow operation: disable_term"
        );
    }

    info!(term_code = %code, "term scraping disabled");

    Ok(Json(TermUpdateResponse {
        success: true,
        term,
    }))
}

/// `POST /api/admin/terms/sync` -- Manually sync terms from the Banner API.
#[instrument(skip_all)]
pub async fn sync_terms(
    _admin: AdminUser,
    State(state): State<AppState>,
) -> Result<Json<TermSyncResponse>, ApiError> {
    let start = Instant::now();

    let banner_terms = state.banner_api.get_terms("", 1, 500).await.map_err(|e| {
        error!(error = %e, "failed to fetch terms from Banner API");
        (
            StatusCode::BAD_GATEWAY,
            Json(json!({"error": "Failed to fetch terms from Banner API"})),
        )
    })?;

    let result = terms::sync_terms_from_banner(&state.db_pool, banner_terms)
        .await
        .map_err(|e| db_error("Failed to sync terms to database", e))?;

    let elapsed = start.elapsed();
    if elapsed > SLOW_OP_THRESHOLD {
        warn!(
            duration = fmt_duration(elapsed),
            "slow operation: sync_terms"
        );
    }

    info!(
        inserted = result.inserted,
        updated = result.updated,
        "terms synced from Banner API"
    );

    Ok(Json(result.into()))
}
