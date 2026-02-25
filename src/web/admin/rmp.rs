//! Admin API handlers for RMP instructor matching management.
//!
//! Thin HTTP wrappers over data-layer operations in [`crate::data::admin_rmp`].
//! All SQL lives in the data layer; handlers handle HTTP concerns only.

use axum::extract::{Path, Query, State};
use axum::response::Json;
use serde::{Deserialize, Serialize};
use tracing::{info, instrument};
use ts_rs::TS;

use crate::data::admin_rmp::{self, ListInstructorsFilter};
use crate::state::AppState;
use crate::web::auth::extractors::AdminUser;
use crate::web::error::{ApiError, db_error};

// Re-export response types so existing imports from `web::admin::rmp::*` still work.
pub use crate::data::admin_rmp::{
    InstructorDetailResponse, ListInstructorsResponse, RescoreResponse,
};

#[derive(Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ListInstructorsParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub per_page: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort: Option<String>,
}

#[derive(Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MatchBody {
    pub rmp_legacy_id: i32,
}

#[derive(Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RejectCandidateBody {
    pub rmp_legacy_id: i32,
}

/// Simple acknowledgement response for mutating operations.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OkResponse {
    pub ok: bool,
}

/// Body for unmatch -- optional `rmpLegacyId` to remove a specific link.
/// If omitted (or null), all links are removed.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UnmatchBody {
    rmp_legacy_id: Option<i32>,
}

/// `GET /api/admin/instructors` -- List instructors with filtering and pagination.
#[instrument(skip_all)]
pub async fn list_instructors(
    AdminUser(_user): AdminUser,
    State(state): State<AppState>,
    Query(params): Query<ListInstructorsParams>,
) -> Result<Json<ListInstructorsResponse>, ApiError> {
    let filter = ListInstructorsFilter {
        status: params.status,
        search: params.search,
        page: params.page.unwrap_or(1),
        per_page: params.per_page.unwrap_or(50),
        sort: params.sort,
    };

    let response = admin_rmp::list_instructors(&state.db_pool, &filter)
        .await
        .map_err(|e| db_error("list instructors", e))?;

    Ok(Json(response))
}

/// `GET /api/admin/instructors/{id}` -- Full instructor detail with candidates.
#[instrument(skip_all, fields(instructor_id = id))]
pub async fn get_instructor(
    AdminUser(_user): AdminUser,
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<Json<InstructorDetailResponse>, ApiError> {
    let response = admin_rmp::get_instructor_detail(&state.db_pool, id)
        .await
        .map_err(|e| {
            if format!("{e:#}").contains("instructor not found") {
                ApiError::not_found("instructor not found")
            } else {
                db_error("get instructor", e)
            }
        })?;

    Ok(Json(response))
}

/// `POST /api/admin/instructors/{id}/match` -- Accept a candidate match.
#[instrument(skip_all, fields(instructor_id = id))]
pub async fn match_instructor(
    AdminUser(user): AdminUser,
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(body): Json<MatchBody>,
) -> Result<Json<InstructorDetailResponse>, ApiError> {
    admin_rmp::accept_candidate(&state.db_pool, id, body.rmp_legacy_id, user.discord_id)
        .await
        .map_err(|e| {
            let msg = format!("{e:#}");
            if msg.contains("pending candidate not found") {
                ApiError::not_found("pending candidate not found for this instructor")
            } else if msg.contains("already linked to instructor") {
                ApiError::conflict("RMP profile already linked to another instructor")
            } else {
                db_error("match instructor", e)
            }
        })?;

    info!(
        instructor_id = id,
        rmp_legacy_id = body.rmp_legacy_id,
        "instructor matched to RMP profile"
    );

    let detail = admin_rmp::get_instructor_detail(&state.db_pool, id)
        .await
        .map_err(|e| db_error("get instructor after match", e))?;

    Ok(Json(detail))
}

/// `POST /api/admin/instructors/{id}/reject-candidate` -- Reject a single candidate.
#[instrument(skip_all, fields(instructor_id = id))]
pub async fn reject_candidate(
    AdminUser(user): AdminUser,
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(body): Json<RejectCandidateBody>,
) -> Result<Json<OkResponse>, ApiError> {
    let found =
        admin_rmp::reject_candidate(&state.db_pool, id, body.rmp_legacy_id, user.discord_id)
            .await
            .map_err(|e| db_error("reject candidate", e))?;

    if !found {
        return Err(ApiError::not_found("pending candidate not found"));
    }

    info!(
        instructor_id = id,
        rmp_legacy_id = body.rmp_legacy_id,
        "RMP candidate rejected"
    );

    Ok(Json(OkResponse { ok: true }))
}

/// `POST /api/admin/instructors/{id}/reject-all` -- Mark instructor as having no valid RMP match.
#[instrument(skip_all, fields(instructor_id = id))]
pub async fn reject_all(
    AdminUser(user): AdminUser,
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<Json<OkResponse>, ApiError> {
    admin_rmp::reject_all_candidates(&state.db_pool, id, user.discord_id)
        .await
        .map_err(|e| {
            let msg = format!("{e:#}");
            if msg.contains("instructor not found") {
                ApiError::not_found("instructor not found")
            } else if msg.contains("cannot reject instructor with confirmed matches") {
                ApiError::conflict(
                    "cannot reject instructor with confirmed matches -- unmatch first",
                )
            } else {
                db_error("reject all candidates", e)
            }
        })?;

    info!(instructor_id = id, "all RMP candidates rejected");

    Ok(Json(OkResponse { ok: true }))
}

/// `POST /api/admin/instructors/{id}/unmatch` -- Remove RMP link(s).
///
/// Send `{ "rmpLegacyId": N }` to remove a specific link, or an empty body / `{}`
/// to remove all links for the instructor.
#[instrument(skip_all, fields(instructor_id = id))]
pub async fn unmatch_instructor(
    AdminUser(_user): AdminUser,
    State(state): State<AppState>,
    Path(id): Path<i32>,
    body: Option<Json<UnmatchBody>>,
) -> Result<Json<OkResponse>, ApiError> {
    let rmp_legacy_id = body.and_then(|b| b.rmp_legacy_id);

    if !admin_rmp::instructor_exists(&state.db_pool, id)
        .await
        .map_err(|e| db_error("check instructor", e))?
    {
        return Err(ApiError::not_found("instructor not found"));
    }

    crate::data::rmp::unmatch_instructor(&state.db_pool, id, rmp_legacy_id)
        .await
        .map_err(|e| db_error("unmatch instructor", e))?;

    info!(
        instructor_id = id,
        ?rmp_legacy_id,
        "instructor unmatched from RMP"
    );

    Ok(Json(OkResponse { ok: true }))
}

/// `POST /api/admin/rmp/rescore` -- Re-run RMP candidate generation.
#[instrument(skip_all)]
pub async fn rescore(
    AdminUser(_user): AdminUser,
    State(state): State<AppState>,
) -> Result<Json<RescoreResponse>, ApiError> {
    let stats = admin_rmp::rescore(&state.db_pool)
        .await
        .map_err(|e| db_error("rescore", e))?;

    info!(
        total_processed = stats.total_processed,
        deleted_pending_candidates = stats.deleted_pending_candidates,
        deleted_auto_links = stats.deleted_auto_links,
        candidates_created = stats.candidates_created,
        auto_matched = stats.auto_matched,
        pending_review = stats.pending_review,
        "RMP candidates rescored"
    );

    Ok(Json(stats))
}
