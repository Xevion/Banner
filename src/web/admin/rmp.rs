//! Admin API handlers for RMP instructor matching management.
//!
//! Thin HTTP wrappers over data-layer operations in [`crate::data::admin_rmp`].
//! All SQL lives in the data layer; handlers handle HTTP concerns only.

use axum::extract::{Path, Query, State};
use axum::response::Json;
use serde::{Deserialize, Serialize};
use tracing::{info, instrument};
use ts_rs::TS;

use crate::data::admin_audits::{AdminAction, Target};
use crate::data::admin_rmp::{self, AdminRmpError, CandidateResponse, ListInstructorsFilter};
use crate::data::models::{RmpCandidateStatus, RmpMatchStatus};
use crate::state::AppState;
use crate::web::admin::action_log;
use crate::web::auth::extractors::AdminUser;
use crate::web::error::{ApiError, db_error};

// Re-export response types so existing imports from `web::admin::rmp::*` still work.
pub use crate::data::admin_rmp::{InstructorDetailResponse, ListInstructorsResponse, RescoreResponse};

/// Map an [`AdminRmpError`] to its HTTP status; anything else is a generic 500.
fn rmp_error(context: &str, e: anyhow::Error) -> ApiError {
    match e.downcast::<AdminRmpError>() {
        Ok(err) => match err {
            AdminRmpError::NoSuchInstructor | AdminRmpError::NoPendingCandidate => ApiError::not_found(err.to_string()),
            AdminRmpError::AlreadyLinked { .. } | AdminRmpError::ConfirmedMatches => {
                ApiError::conflict(err.to_string())
            }
        },
        Err(e) => db_error(context, e),
    }
}

/// Describe why a candidate is still waiting, in the reviewer's terms.
fn explain_block(candidate: &CandidateResponse) -> Option<String> {
    if candidate.status == RmpCandidateStatus::Accepted {
        return None;
    }
    if let Some(holder) = &candidate.claimed_by {
        return Some(format!("Already linked to {holder}"));
    }
    if candidate.score_breakdown.0.subject < 1.0 {
        return Some(
            "Subject not confirmed: neither the department nor the reviewed courses match what this instructor teaches"
                .to_string(),
        );
    }
    Some("Held for review: another professor shares this name".to_string())
}

/// Attach reviewer-facing explanations to every candidate in a detail response.
fn explain_candidates(mut detail: InstructorDetailResponse) -> InstructorDetailResponse {
    for candidate in &mut detail.candidates {
        candidate.blocked_reason = explain_block(candidate);
    }
    detail
}

#[derive(Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ListInstructorsParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<RmpMatchStatus>,
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
        .map_err(|e| rmp_error("get instructor", e))?;

    Ok(Json(explain_candidates(response)))
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
        .map_err(|e| rmp_error("match instructor", e))?;

    action_log::record(
        &state.db_pool,
        &user,
        AdminAction::RmpAcceptCandidate,
        Target::id(id),
        serde_json::json!({ "rmpLegacyId": body.rmp_legacy_id }),
    )
    .await;

    info!(
        instructor_id = id,
        rmp_legacy_id = body.rmp_legacy_id,
        "instructor matched to RMP profile"
    );

    let detail = admin_rmp::get_instructor_detail(&state.db_pool, id)
        .await
        .map_err(|e| db_error("get instructor after match", e))?;

    Ok(Json(explain_candidates(detail)))
}

/// `POST /api/admin/instructors/{id}/reject-candidate` -- Reject a single candidate.
#[instrument(skip_all, fields(instructor_id = id))]
pub async fn reject_candidate(
    AdminUser(user): AdminUser,
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(body): Json<RejectCandidateBody>,
) -> Result<Json<OkResponse>, ApiError> {
    let found = admin_rmp::reject_candidate(&state.db_pool, id, body.rmp_legacy_id, user.discord_id)
        .await
        .map_err(|e| db_error("reject candidate", e))?;

    if !found {
        return Err(ApiError::not_found("pending candidate not found"));
    }

    action_log::record(
        &state.db_pool,
        &user,
        AdminAction::RmpRejectCandidate,
        Target::id(id),
        serde_json::json!({ "rmpLegacyId": body.rmp_legacy_id }),
    )
    .await;

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
        .map_err(|e| rmp_error("reject all candidates", e))?;

    action_log::record(
        &state.db_pool,
        &user,
        AdminAction::RmpRejectAll,
        Target::id(id),
        serde_json::json!({}),
    )
    .await;

    info!(instructor_id = id, "all RMP candidates rejected");

    Ok(Json(OkResponse { ok: true }))
}

/// `POST /api/admin/instructors/{id}/unmatch` -- Remove RMP link(s).
///
/// Send `{ "rmpLegacyId": N }` to remove a specific link, or an empty body / `{}`
/// to remove all links for the instructor.
#[instrument(skip_all, fields(instructor_id = id))]
pub async fn unmatch_instructor(
    AdminUser(user): AdminUser,
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

    action_log::record(
        &state.db_pool,
        &user,
        AdminAction::RmpUnmatch,
        Target::id(id),
        serde_json::json!({ "rmpLegacyId": rmp_legacy_id }),
    )
    .await;

    info!(instructor_id = id, ?rmp_legacy_id, "instructor unmatched from RMP");

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::rmp_matching::ScoreBreakdown;
    use crate::web::error::ApiErrorCode;
    use assert2::check;

    fn candidate(status: RmpCandidateStatus, claimed_by: Option<&str>, subject: f32) -> CandidateResponse {
        CandidateResponse {
            id: 1,
            rmp_legacy_id: 42,
            first_name: "Test".to_string(),
            last_name: "Professor".to_string(),
            department: None,
            avg_rating: None,
            avg_difficulty: None,
            num_ratings: 0,
            would_take_again_pct: None,
            score: 0.0,
            score_breakdown: sqlx::types::Json(ScoreBreakdown {
                subject,
                ..ScoreBreakdown::default()
            }),
            status,
            review_subjects: Vec::new(),
            review_years: Vec::new(),
            claimed_by: claimed_by.map(str::to_string),
            blocked_reason: None,
        }
    }

    #[test]
    fn test_already_linked_maps_to_conflict_naming_the_holder() {
        let err = anyhow::Error::from(AdminRmpError::AlreadyLinked {
            instructor_id: 7,
            display_name: "Fictional, Bryn".to_string(),
            email: Some("bryn@utsa.edu".to_string()),
        });

        let api = rmp_error("match instructor", err);

        check!(api.code == ApiErrorCode::Conflict);
        check!(api.message == "RMP profile already linked to instructor Fictional, Bryn (bryn@utsa.edu, #7)");
    }

    #[test]
    fn test_already_linked_without_email_says_no_email() {
        let err = anyhow::Error::from(AdminRmpError::AlreadyLinked {
            instructor_id: 7,
            display_name: "Fictional, Bryn".to_string(),
            email: None,
        });

        let api = rmp_error("match instructor", err);

        check!(api.message == "RMP profile already linked to instructor Fictional, Bryn (no email, #7)");
    }

    #[test]
    fn test_typed_error_survives_added_context() {
        let err = anyhow::Error::from(AdminRmpError::NoSuchInstructor).context("instructor lookup");

        let api = rmp_error("get instructor", err);

        check!(api.code == ApiErrorCode::NotFound);
        check!(api.message == "instructor not found");
    }

    #[test]
    fn test_missing_candidate_maps_to_not_found() {
        let api = rmp_error("match instructor", AdminRmpError::NoPendingCandidate.into());

        check!(api.code == ApiErrorCode::NotFound);
        check!(api.message == "pending candidate not found for this instructor");
    }

    #[test]
    fn test_confirmed_matches_map_to_conflict() {
        let api = rmp_error("reject all candidates", AdminRmpError::ConfirmedMatches.into());

        check!(api.code == ApiErrorCode::Conflict);
        check!(api.message == "cannot reject instructor with confirmed matches -- unmatch first");
    }

    #[test]
    fn test_untyped_error_stays_internal() {
        let api = rmp_error("reject all candidates", anyhow::anyhow!("connection reset"));

        check!(api.code == ApiErrorCode::InternalError);
        check!(api.message == "reject all candidates failed");
    }

    #[test]
    fn test_accepted_candidate_has_no_blocked_reason() {
        check!(explain_block(&candidate(RmpCandidateStatus::Accepted, None, 0.0)) == None);
    }

    #[test]
    fn test_claimed_candidate_names_the_holder() {
        let reason = explain_block(&candidate(RmpCandidateStatus::Pending, Some("Fictional, Bryn"), 1.0));
        check!(reason == Some("Already linked to Fictional, Bryn".to_string()));
    }

    #[test]
    fn test_weak_subject_evidence_is_explained() {
        let reason = explain_block(&candidate(RmpCandidateStatus::Pending, None, 0.4));
        check!(reason.unwrap().starts_with("Subject not confirmed:"));
    }

    #[test]
    fn test_strong_subject_evidence_falls_back_to_shared_name() {
        let reason = explain_block(&candidate(RmpCandidateStatus::Pending, None, 1.0));
        check!(reason == Some("Held for review: another professor shares this name".to_string()));
    }
}
