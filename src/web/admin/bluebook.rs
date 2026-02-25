//! Admin API handlers for BlueBook instructor linking.

use std::sync::atomic::Ordering;

use axum::extract::Query;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::Json;
use serde::{Deserialize, Serialize};
use tracing::{info, instrument};
use ts_rs::TS;

use crate::data::admin_bluebook::{self, BluebookError, ListBluebookLinksFilter};
use crate::state::AppState;
use crate::web::auth::extractors::AdminUser;
use crate::web::error::{ApiError, db_error};

pub use crate::data::admin_bluebook::{
    BluebookLinkDetail, BluebookMatchResponse, ListBluebookLinksResponse,
};

/// Check if an `anyhow::Error` chain contains a [`BluebookError`] variant that
/// indicates a "not found" condition, and return the appropriate 404 response.
/// Falls back to a generic 500 via [`db_error`].
fn bluebook_not_found_or_db(context: &str, e: anyhow::Error) -> ApiError {
    if let Some(bb) = e.downcast_ref::<BluebookError>() {
        ApiError::not_found(bb.to_string())
    } else {
        db_error(context, e)
    }
}

/// Response for `POST /api/admin/bluebook/sync`.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BluebookSyncTriggerResponse {
    pub message: String,
}

/// Query params for `GET /api/admin/bluebook/links`.
#[derive(Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ListBluebookLinksParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub per_page: Option<i32>,
}

/// Body for `POST /api/admin/bluebook/links/{id}/assign`.
#[derive(Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AssignBody {
    pub instructor_id: i32,
}

/// Simple acknowledgement response for mutating operations.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BluebookOkResponse {
    pub ok: bool,
}

/// `POST /api/admin/bluebook/sync` -- Trigger a BlueBook evaluation sync.
#[instrument(skip_all)]
pub async fn sync_bluebook(
    AdminUser(_user): AdminUser,
    State(state): State<AppState>,
) -> (StatusCode, Json<BluebookSyncTriggerResponse>) {
    info!("Admin triggered BlueBook sync");
    state.bluebook_force_flag.store(true, Ordering::Relaxed);
    state.bluebook_sync_notify.notify_one();
    (
        StatusCode::ACCEPTED,
        Json(BluebookSyncTriggerResponse {
            message: "BlueBook sync triggered".to_string(),
        }),
    )
}

/// `GET /api/admin/bluebook/links` -- List BlueBook links with filtering and pagination.
#[instrument(skip_all)]
pub async fn list_links(
    AdminUser(_user): AdminUser,
    State(state): State<AppState>,
    Query(params): Query<ListBluebookLinksParams>,
) -> Result<Json<ListBluebookLinksResponse>, ApiError> {
    let filter = ListBluebookLinksFilter {
        status: params.status,
        search: params.search,
        page: params.page.unwrap_or(1),
        per_page: params.per_page.unwrap_or(50),
    };

    let response = admin_bluebook::list_links(&state.db_pool, &filter)
        .await
        .map_err(|e| db_error("list bluebook links", e))?;

    Ok(Json(response))
}

/// `GET /api/admin/bluebook/links/{id}` -- Detail for a specific BlueBook link.
#[instrument(skip_all, fields(link_id = id))]
pub async fn get_link(
    AdminUser(_user): AdminUser,
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<Json<BluebookLinkDetail>, ApiError> {
    let response = admin_bluebook::get_link_detail(&state.db_pool, id)
        .await
        .map_err(|e| bluebook_not_found_or_db("get bluebook link", e))?;

    Ok(Json(response))
}

/// `POST /api/admin/bluebook/links/{id}/approve` -- Approve a pending link.
#[instrument(skip_all, fields(link_id = id))]
pub async fn approve_link(
    AdminUser(_user): AdminUser,
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<Json<BluebookOkResponse>, ApiError> {
    admin_bluebook::approve_link(&state.db_pool, id)
        .await
        .map_err(|e| bluebook_not_found_or_db("approve bluebook link", e))?;

    info!(link_id = id, "BlueBook link approved");

    Ok(Json(BluebookOkResponse { ok: true }))
}

/// `POST /api/admin/bluebook/links/{id}/reject` -- Reject a pending link.
#[instrument(skip_all, fields(link_id = id))]
pub async fn reject_link(
    AdminUser(_user): AdminUser,
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<Json<BluebookOkResponse>, ApiError> {
    admin_bluebook::reject_link(&state.db_pool, id)
        .await
        .map_err(|e| bluebook_not_found_or_db("reject bluebook link", e))?;

    info!(link_id = id, "BlueBook link rejected");

    Ok(Json(BluebookOkResponse { ok: true }))
}

/// `POST /api/admin/bluebook/links/{id}/assign` -- Manually assign an instructor to a link.
#[instrument(skip_all, fields(link_id = id))]
pub async fn assign_link(
    AdminUser(_user): AdminUser,
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(body): Json<AssignBody>,
) -> Result<Json<BluebookOkResponse>, ApiError> {
    admin_bluebook::assign_link(&state.db_pool, id, body.instructor_id)
        .await
        .map_err(|e| bluebook_not_found_or_db("assign bluebook link", e))?;

    info!(
        link_id = id,
        instructor_id = body.instructor_id,
        "BlueBook link manually assigned"
    );

    Ok(Json(BluebookOkResponse { ok: true }))
}

/// `POST /api/admin/bluebook/match` -- Trigger auto-matching pipeline.
#[instrument(skip_all)]
pub async fn run_matching(
    AdminUser(_user): AdminUser,
    State(state): State<AppState>,
) -> Result<Json<BluebookMatchResponse>, ApiError> {
    let response = admin_bluebook::run_auto_matching(&state.db_pool)
        .await
        .map_err(|e| db_error("bluebook auto-matching", e))?;

    info!(
        total_names = response.total_names,
        auto_matched = response.auto_matched,
        pending_review = response.pending_review,
        no_match = response.no_match,
        deleted_stale = response.deleted_stale,
        skipped_manual = response.skipped_manual,
        "BlueBook auto-matching triggered"
    );

    Ok(Json(response))
}
