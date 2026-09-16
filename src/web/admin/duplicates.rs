//! Admin endpoints for reviewing and merging duplicate instructor records.

use axum::extract::State;
use axum::response::Json;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tracing::{info, instrument, trace, warn};
use ts_rs::TS;

use crate::data::admin_audits::{AdminAction, Target};
use crate::data::instructor_merge::{self, DuplicatePair, MergeError, MergeStats};
use crate::state::AppState;
use crate::web::admin::action_log;
use crate::web::auth::extractors::AdminUser;
use crate::web::error::{ApiError, db_error};

/// Every merge failure names something the caller can correct, so all of them
/// are 400s; anything else is a genuine fault and stays a generic 500.
fn merge_error(context: &str, e: anyhow::Error) -> ApiError {
    match e.downcast_ref::<MergeError>() {
        Some(err) => ApiError::bad_request(err.to_string()),
        None => db_error(context, e),
    }
}

/// Read the dismissal list before a merge, since deleting the loser cascades
/// its dismissals away and they would otherwise vanish unrecorded.
async fn dismissals_before_merge(pool: &PgPool) -> Vec<DuplicatePair> {
    instructor_merge::find_dismissed_pairs(pool)
        .await
        .unwrap_or_else(|e| {
            warn!(error = %e, "Failed to read dismissals before merge");
            Vec::new()
        })
}

/// The instructors whose dismissals against `loser_id` the merge will erase.
fn dismissed_partners(pairs: &[DuplicatePair], loser_id: i32) -> Vec<i32> {
    pairs
        .iter()
        .filter_map(|pair| {
            let (first, second) = (pair.survivor.id, pair.loser.id);
            match loser_id {
                _ if first == loser_id => Some(second),
                _ if second == loser_id => Some(first),
                _ => None,
            }
        })
        .collect()
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DuplicatesResponse {
    pub pairs: Vec<DuplicatePair>,
    /// Pairs already judged to be different people, kept so review can undo one.
    pub dismissed: Vec<DuplicatePair>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeBody {
    survivor_id: i32,
    loser_id: i32,
    /// Set when the reviewer has confirmed two differing names are one person.
    #[serde(default)]
    names_confirmed: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DismissBody {
    first_id: i32,
    second_id: i32,
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

    let dismissed = instructor_merge::find_dismissed_pairs(&state.db_pool)
        .await
        .map_err(|e| db_error("list dismissed instructor pairs", e))?;

    trace!(
        count = pairs.len(),
        dismissed = dismissed.len(),
        "Listed duplicate instructor pairs"
    );

    Ok(Json(DuplicatesResponse { pairs, dismissed }))
}

/// `POST /api/admin/instructors/merge` -- Fold one instructor record into another.
#[instrument(skip_all, fields(survivor_id, loser_id))]
pub async fn merge(
    AdminUser(user): AdminUser,
    State(state): State<AppState>,
    Json(body): Json<MergeBody>,
) -> Result<Json<MergeResponse>, ApiError> {
    let dismissals = dismissals_before_merge(&state.db_pool).await;

    instructor_merge::merge_instructors(
        &state.db_pool,
        body.survivor_id,
        body.loser_id,
        Some(user.discord_id),
        body.names_confirmed,
    )
    .await
    .map_err(|e| merge_error("merge instructors", e))?;

    action_log::record(
        &state.db_pool,
        &user,
        AdminAction::InstructorMerge,
        Target::pair(body.survivor_id, body.loser_id),
        serde_json::json!({
            "survivorId": body.survivor_id,
            "loserId": body.loser_id,
            "namesConfirmed": body.names_confirmed,
            "erasedDismissalsWith": dismissed_partners(&dismissals, body.loser_id),
        }),
    )
    .await;

    crate::data::rmp::refresh_rmp_summary(&state.db_pool)
        .await
        .map_err(|e| db_error("refresh rmp summary", e))?;

    info!(
        survivor_id = body.survivor_id,
        loser_id = body.loser_id,
        actor = user.discord_id,
        "Merged duplicate instructors"
    );

    Ok(Json(MergeResponse { ok: true }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeClaimantBody {
    rmp_legacy_id: i32,
}

/// `POST /api/admin/instructors/{id}/merge-claimant` -- Resolve a blocked
/// candidate by merging this record into the one already holding the profile.
#[instrument(skip_all, fields(instructor_id = id))]
pub async fn merge_claimant(
    AdminUser(user): AdminUser,
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<i32>,
    Json(body): Json<MergeClaimantBody>,
) -> Result<Json<MergeResponse>, ApiError> {
    let dismissals = dismissals_before_merge(&state.db_pool).await;

    let (survivor, loser) = instructor_merge::merge_with_claimant(
        &state.db_pool,
        id,
        body.rmp_legacy_id,
        Some(user.discord_id),
    )
    .await
    .map_err(|e| merge_error("merge with claimant", e))?;

    action_log::record(
        &state.db_pool,
        &user,
        AdminAction::InstructorMergeClaimant,
        Target::pair(survivor, loser),
        serde_json::json!({
            "survivorId": survivor,
            "loserId": loser,
            "rmpLegacyId": body.rmp_legacy_id,
            "erasedDismissalsWith": dismissed_partners(&dismissals, loser),
        }),
    )
    .await;

    crate::data::rmp::refresh_rmp_summary(&state.db_pool)
        .await
        .map_err(|e| db_error("refresh rmp summary", e))?;

    info!(
        survivor,
        loser,
        actor = user.discord_id,
        "Resolved blocked candidate by merging records"
    );

    Ok(Json(MergeResponse { ok: true }))
}

/// `POST /api/admin/instructors/merge-duplicates` -- Merge every unambiguous pair.
#[instrument(skip_all)]
pub async fn merge_all(
    AdminUser(user): AdminUser,
    State(state): State<AppState>,
) -> Result<Json<MergeStats>, ApiError> {
    let stats = instructor_merge::auto_merge_duplicates(&state.db_pool, Some(user.discord_id))
        .await
        .map_err(|e| db_error("merge duplicate instructors", e))?;

    action_log::record(
        &state.db_pool,
        &user,
        AdminAction::InstructorMergeAll,
        Target::all(),
        serde_json::json!({ "merged": stats.merged, "skipped": stats.skipped }),
    )
    .await;

    crate::data::rmp::refresh_rmp_summary(&state.db_pool)
        .await
        .map_err(|e| db_error("refresh rmp summary", e))?;

    info!(
        merged = stats.merged,
        skipped = stats.skipped,
        actor = user.discord_id,
        "Merged duplicate instructor records"
    );

    Ok(Json(stats))
}

/// `POST /api/admin/instructors/dismiss` -- Record that a pair is two people.
#[instrument(skip_all, fields(first_id, second_id))]
pub async fn dismiss(
    AdminUser(user): AdminUser,
    State(state): State<AppState>,
    Json(body): Json<DismissBody>,
) -> Result<Json<MergeResponse>, ApiError> {
    instructor_merge::dismiss_pair(
        &state.db_pool,
        body.first_id,
        body.second_id,
        Some(user.discord_id),
    )
    .await
    .map_err(|e| merge_error("dismiss instructor pair", e))?;

    action_log::record(
        &state.db_pool,
        &user,
        AdminAction::InstructorDismiss,
        Target::pair(body.first_id, body.second_id),
        serde_json::json!({ "firstId": body.first_id, "secondId": body.second_id }),
    )
    .await;

    info!(
        first_id = body.first_id,
        second_id = body.second_id,
        actor = user.discord_id,
        "Dismissed a duplicate instructor pair"
    );

    Ok(Json(MergeResponse { ok: true }))
}

/// `POST /api/admin/instructors/undismiss` -- Return a dismissed pair to review.
#[instrument(skip_all, fields(first_id, second_id))]
pub async fn undismiss(
    AdminUser(user): AdminUser,
    State(state): State<AppState>,
    Json(body): Json<DismissBody>,
) -> Result<Json<MergeResponse>, ApiError> {
    let removed = instructor_merge::undismiss_pair(&state.db_pool, body.first_id, body.second_id)
        .await
        .map_err(|e| merge_error("undismiss instructor pair", e))?;

    action_log::record(
        &state.db_pool,
        &user,
        AdminAction::InstructorUndismiss,
        Target::pair(body.first_id, body.second_id),
        serde_json::json!({
            "firstId": body.first_id,
            "secondId": body.second_id,
            "removed": removed,
        }),
    )
    .await;

    info!(
        first_id = body.first_id,
        second_id = body.second_id,
        removed,
        actor = user.discord_id,
        "Restored a dismissed instructor pair"
    );

    Ok(Json(MergeResponse { ok: true }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::instructor_merge::{DuplicateSide, DuplicateTier};
    use crate::web::error::ApiErrorCode;
    use assert2::check;

    fn side(id: i32) -> DuplicateSide {
        DuplicateSide {
            id,
            display_name: format!("Fictional, {id}"),
            email: None,
            course_count: 0,
            subjects: Vec::new(),
            rmp_legacy_ids: Vec::new(),
            match_status: "unmatched".to_string(),
        }
    }

    fn pair(survivor: i32, loser: i32) -> DuplicatePair {
        DuplicatePair {
            tier: DuplicateTier::MissingEmail,
            survivor: side(survivor),
            loser: side(loser),
            subjects_overlap: false,
        }
    }

    /// Merging deletes the loser, taking its dismissals with it, so the entry
    /// has to name them whichever side of the pair the loser sat on.
    #[test]
    fn test_dismissed_partners_reads_both_sides_of_a_pair() {
        let pairs = [pair(10, 42), pair(42, 11), pair(50, 51)];

        check!(dismissed_partners(&pairs, 42) == vec![10, 11]);
        check!(dismissed_partners(&pairs, 99) == Vec::<i32>::new());
    }

    #[test]
    fn test_self_merge_maps_to_bad_request() {
        let api = merge_error("merge instructors", MergeError::SelfMerge.into());

        check!(api.code == ApiErrorCode::BadRequest);
        check!(api.message == "cannot merge an instructor into itself");
    }

    /// The message must survive the data layer annotating the path it failed on.
    #[test]
    fn test_missing_instructor_maps_to_bad_request() {
        let err = anyhow::Error::from(MergeError::MissingInstructor).context("merging");
        let api = merge_error("merge instructors", err);

        check!(api.code == ApiErrorCode::BadRequest);
        check!(api.message == "both instructors must exist to merge");
    }

    #[test]
    fn test_claimant_errors_map_to_bad_request() {
        let api = merge_error("merge with claimant", MergeError::NoClaimant.into());
        check!(api.code == ApiErrorCode::BadRequest);
        check!(api.message == "no other instructor holds this RMP profile");

        let api = merge_error("merge with claimant", MergeError::DifferentPeople.into());
        check!(api.code == ApiErrorCode::BadRequest);
        check!(
            api.message
                == "records name different people; merge them manually if they are the same"
        );
    }

    /// One failure reads the same whichever endpoint surfaced it.
    #[test]
    fn test_missing_instructor_reads_the_same_from_either_endpoint() {
        let direct = merge_error("merge instructors", MergeError::MissingInstructor.into());
        let claimant = merge_error("merge with claimant", MergeError::MissingInstructor.into());

        check!(direct.code == claimant.code);
        check!(direct.message == claimant.message);
        check!(claimant.code == ApiErrorCode::BadRequest);
    }

    #[test]
    fn test_untyped_failure_stays_internal() {
        let api = merge_error("merge with claimant", anyhow::anyhow!("connection reset"));

        check!(api.code == ApiErrorCode::InternalError);
        check!(api.message == "merge with claimant failed");
    }
}
