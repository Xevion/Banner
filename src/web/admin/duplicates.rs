//! Admin endpoints for reviewing and merging duplicate instructor records.

use axum::extract::State;
use axum::response::Json;
use serde::{Deserialize, Serialize};
use tracing::{info, instrument, trace};
use ts_rs::TS;

use crate::data::instructor_merge::{self, DuplicatePair, MergeStats};
use crate::state::AppState;
use crate::web::auth::extractors::AdminUser;
use crate::web::error::{ApiError, db_error};

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DuplicatesResponse {
    pub pairs: Vec<DuplicatePair>,
}

#[derive(Deserialize)]
pub struct MergeBody {
    survivor_id: i32,
    loser_id: i32,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MergeResponse {
    pub ok: bool,
}

/// `GET /api/admin/instructors/duplicates` -- List duplicate instructor pairs.
#[instrument(skip_all)]
pub async fn list_duplicates(
    AdminUser(_user): AdminUser,
    State(state): State<AppState>,
) -> Result<Json<DuplicatesResponse>, ApiError> {
    let pairs = instructor_merge::find_duplicate_pairs(&state.db_pool)
        .await
        .map_err(|e| db_error("list duplicate instructors", e))?;

    trace!(count = pairs.len(), "Listed duplicate instructor pairs");

    Ok(Json(DuplicatesResponse { pairs }))
}

/// `POST /api/admin/instructors/merge` -- Fold one instructor record into another.
#[instrument(skip_all, fields(survivor_id, loser_id))]
pub async fn merge(
    AdminUser(_user): AdminUser,
    State(state): State<AppState>,
    Json(body): Json<MergeBody>,
) -> Result<Json<MergeResponse>, ApiError> {
    instructor_merge::merge_instructors(&state.db_pool, body.survivor_id, body.loser_id)
        .await
        .map_err(|e| {
            let msg = format!("{e:#}");
            if msg.contains("must exist") || msg.contains("into itself") {
                ApiError::bad_request(msg)
            } else {
                db_error("merge instructors", e)
            }
        })?;

    crate::data::rmp::refresh_rmp_summary(&state.db_pool)
        .await
        .map_err(|e| db_error("refresh rmp summary", e))?;

    info!(
        survivor_id = body.survivor_id,
        loser_id = body.loser_id,
        "Merged duplicate instructors"
    );

    Ok(Json(MergeResponse { ok: true }))
}

#[derive(Deserialize)]
pub struct MergeClaimantBody {
    rmp_legacy_id: i32,
}

/// `POST /api/admin/instructors/{id}/merge-claimant` -- Resolve a blocked
/// candidate by merging this record into the one already holding the profile.
#[instrument(skip_all, fields(instructor_id = id))]
pub async fn merge_claimant(
    AdminUser(_user): AdminUser,
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<i32>,
    Json(body): Json<MergeClaimantBody>,
) -> Result<Json<MergeResponse>, ApiError> {
    let (survivor, loser) =
        instructor_merge::merge_with_claimant(&state.db_pool, id, body.rmp_legacy_id)
            .await
            .map_err(|e| {
                let msg = format!("{e:#}");
                if msg.contains("no other instructor") || msg.contains("different people") {
                    ApiError::bad_request(msg)
                } else {
                    db_error("merge with claimant", e)
                }
            })?;

    crate::data::rmp::refresh_rmp_summary(&state.db_pool)
        .await
        .map_err(|e| db_error("refresh rmp summary", e))?;

    info!(
        survivor,
        loser, "Resolved blocked candidate by merging records"
    );

    Ok(Json(MergeResponse { ok: true }))
}

/// `POST /api/admin/instructors/merge-duplicates` -- Merge every unambiguous pair.
#[instrument(skip_all)]
pub async fn merge_all(
    AdminUser(_user): AdminUser,
    State(state): State<AppState>,
) -> Result<Json<MergeStats>, ApiError> {
    let stats = instructor_merge::auto_merge_duplicates(&state.db_pool)
        .await
        .map_err(|e| db_error("merge duplicate instructors", e))?;

    crate::data::rmp::refresh_rmp_summary(&state.db_pool)
        .await
        .map_err(|e| db_error("refresh rmp summary", e))?;

    info!(
        merged = stats.merged,
        skipped = stats.skipped,
        "Merged duplicate instructor records"
    );

    Ok(Json(stats))
}
