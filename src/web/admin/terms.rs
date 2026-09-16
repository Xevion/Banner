//! Admin API handlers for term management.
//!
//! All endpoints require the `AdminUser` extractor, returning 401/403 as needed.

use std::time::{Duration, Instant};

use crate::utils::log_if_slow;
use axum::extract::{Path, State};
use axum::response::Json;
use serde::Serialize;
use tracing::{error, info, instrument, trace};
use ts_rs::TS;

use crate::data::admin_audits::{AdminAction, Target};
use crate::data::terms::{self, DbTerm, SyncResult};
use crate::state::AppState;
use crate::web::admin::action_log;
use crate::web::auth::extractors::AdminUser;
use crate::web::error::{ApiError, DbResultExt};

const SLOW_OP_THRESHOLD: Duration = Duration::from_secs(1);

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
pub async fn list_terms(_admin: AdminUser, State(state): State<AppState>) -> Result<Json<TermsListResponse>, ApiError> {
    let start = Instant::now();

    let terms = terms::get_all_terms(&state.db_pool)
        .await
        .db_context("Failed to fetch terms")?;

    log_if_slow(start, SLOW_OP_THRESHOLD, "list_terms");

    trace!(count = terms.len(), "listed terms");
    Ok(Json(TermsListResponse { terms }))
}

/// `POST /api/admin/terms/:code/enable` -- Enable scraping for a term.
#[instrument(skip_all, fields(term_code = %code))]
pub async fn enable_term(
    AdminUser(user): AdminUser,
    State(state): State<AppState>,
    Path(code): Path<String>,
) -> Result<Json<TermUpdateResponse>, ApiError> {
    let start = Instant::now();

    let found = terms::enable_scraping(&state.db_pool, &code)
        .await
        .db_context("Failed to enable scraping")?;

    if !found {
        return Err(ApiError::not_found("Term not found"));
    }

    action_log::record(
        &state.db_pool,
        &user,
        AdminAction::TermEnable,
        Target::id(&code),
        serde_json::json!({ "scrapeEnabled": true }),
    )
    .await;

    let term = terms::get_term_by_code(&state.db_pool, &code)
        .await
        .db_context("Failed to fetch updated term")?;

    log_if_slow(start, SLOW_OP_THRESHOLD, "enable_term");

    info!(term_code = %code, "term scraping enabled");

    Ok(Json(TermUpdateResponse { success: true, term }))
}

/// `POST /api/admin/terms/:code/disable` -- Disable scraping for a term.
#[instrument(skip_all, fields(term_code = %code))]
pub async fn disable_term(
    AdminUser(user): AdminUser,
    State(state): State<AppState>,
    Path(code): Path<String>,
) -> Result<Json<TermUpdateResponse>, ApiError> {
    let start = Instant::now();

    let found = terms::disable_scraping(&state.db_pool, &code)
        .await
        .db_context("Failed to disable scraping")?;

    if !found {
        return Err(ApiError::not_found("Term not found"));
    }

    action_log::record(
        &state.db_pool,
        &user,
        AdminAction::TermDisable,
        Target::id(&code),
        serde_json::json!({ "scrapeEnabled": false }),
    )
    .await;

    let term = terms::get_term_by_code(&state.db_pool, &code)
        .await
        .db_context("Failed to fetch updated term")?;

    log_if_slow(start, SLOW_OP_THRESHOLD, "disable_term");

    info!(term_code = %code, "term scraping disabled");

    Ok(Json(TermUpdateResponse { success: true, term }))
}

/// `POST /api/admin/terms/sync` -- Manually sync terms from the Banner API.
#[instrument(skip_all)]
pub async fn sync_terms(_admin: AdminUser, State(state): State<AppState>) -> Result<Json<TermSyncResponse>, ApiError> {
    let start = Instant::now();

    let banner_terms = state.banner_api.get_terms("", 1, 500).await.map_err(|e| {
        error!(error = %e, "failed to fetch terms from Banner API");
        ApiError::internal_error("Failed to fetch terms from Banner API")
    })?;

    let result = terms::sync_terms_from_banner(&state.db_pool, banner_terms)
        .await
        .db_context("Failed to sync terms to database")?;

    log_if_slow(start, SLOW_OP_THRESHOLD, "sync_terms");

    info!(
        inserted = result.inserted,
        updated = result.updated,
        "terms synced from Banner API"
    );

    Ok(Json(result.into()))
}
