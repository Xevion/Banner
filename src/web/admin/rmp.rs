//! Admin API handlers for RMP instructor matching management.

use std::time::{Duration, Instant};

use crate::utils::fmt_duration;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::Json;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tracing::{debug, error, info, instrument, warn};
use ts_rs::TS;

use crate::state::AppState;
use crate::web::auth::extractors::AdminUser;

// ---------------------------------------------------------------------------
// Query / body types
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

/// Simple acknowledgement response for mutating operations.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OkResponse {
    pub ok: bool,
}

/// A top-candidate summary shown in the instructor list view.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TopCandidateResponse {
    pub rmp_legacy_id: i32,
    pub score: Option<f32>,
    #[ts(as = "Option<std::collections::HashMap<String, f32>>")]
    pub score_breakdown: Option<serde_json::Value>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub department: Option<String>,
    pub avg_rating: Option<f32>,
    pub num_ratings: Option<i32>,
}

/// An instructor row in the paginated list.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct InstructorListItem {
    pub id: i32,
    pub display_name: String,
    pub email: String,
    pub rmp_match_status: String,
    #[ts(as = "i32")]
    pub rmp_link_count: i64,
    #[ts(as = "i32")]
    pub candidate_count: i64,
    #[ts(as = "i32")]
    pub course_subject_count: i64,
    pub top_candidate: Option<TopCandidateResponse>,
}

/// Aggregate status counts for the instructor list.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct InstructorStats {
    #[ts(as = "i32")]
    pub total: i64,
    #[ts(as = "i32")]
    pub unmatched: i64,
    #[ts(as = "i32")]
    pub auto: i64,
    #[ts(as = "i32")]
    pub confirmed: i64,
    #[ts(as = "i32")]
    pub rejected: i64,
    #[ts(as = "i32")]
    pub with_candidates: i64,
}

/// Response for `GET /api/admin/instructors`.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ListInstructorsResponse {
    pub instructors: Vec<InstructorListItem>,
    #[ts(as = "i32")]
    pub total: i64,
    pub page: i32,
    pub per_page: i32,
    pub stats: InstructorStats,
}

/// Instructor summary in the detail view.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct InstructorDetail {
    pub id: i32,
    pub display_name: String,
    pub email: String,
    pub rmp_match_status: String,
    pub subjects_taught: Vec<String>,
    #[ts(as = "i32")]
    pub course_count: i64,
}

/// A linked RMP profile in the detail view.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LinkedRmpProfile {
    pub link_id: i32,
    pub legacy_id: i32,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub department: Option<String>,
    pub avg_rating: Option<f32>,
    pub avg_difficulty: Option<f32>,
    pub num_ratings: Option<i32>,
    pub would_take_again_pct: Option<f32>,
}

/// A match candidate in the detail view.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CandidateResponse {
    pub id: i32,
    pub rmp_legacy_id: i32,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub department: Option<String>,
    pub avg_rating: Option<f32>,
    pub avg_difficulty: Option<f32>,
    pub num_ratings: Option<i32>,
    pub would_take_again_pct: Option<f32>,
    pub score: Option<f32>,
    #[ts(as = "Option<std::collections::HashMap<String, f32>>")]
    pub score_breakdown: Option<serde_json::Value>,
    pub status: String,
}

/// Response for `GET /api/admin/instructors/{id}` and `POST .../match`.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct InstructorDetailResponse {
    pub instructor: InstructorDetail,
    pub current_matches: Vec<LinkedRmpProfile>,
    pub candidates: Vec<CandidateResponse>,
}

/// Response for `POST /api/admin/rmp/rescore`.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RescoreResponse {
    pub total_unmatched: usize,
    pub candidates_created: usize,
    pub candidates_rescored: usize,
    pub auto_matched: usize,
    pub skipped_unparseable: usize,
    pub skipped_no_candidates: usize,
}

// ---------------------------------------------------------------------------
// Helper: map sqlx errors to the standard admin error tuple
// ---------------------------------------------------------------------------

fn db_error(context: &str, e: sqlx::Error) -> (StatusCode, Json<Value>) {
    error!(error = %e, "{context}");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error": context})),
    )
}

const SLOW_OP_THRESHOLD: Duration = Duration::from_secs(1);

// ---------------------------------------------------------------------------
// Row types for SQL queries
// ---------------------------------------------------------------------------

#[derive(sqlx::FromRow)]
struct InstructorRow {
    id: i32,
    display_name: String,
    email: String,
    rmp_match_status: String,
    rmp_link_count: Option<i64>,
    top_candidate_rmp_id: Option<i32>,
    top_candidate_score: Option<f32>,
    top_candidate_breakdown: Option<serde_json::Value>,
    tc_first_name: Option<String>,
    tc_last_name: Option<String>,
    tc_department: Option<String>,
    tc_avg_rating: Option<f32>,
    tc_num_ratings: Option<i32>,
    candidate_count: Option<i64>,
    course_subject_count: Option<i64>,
}

#[derive(sqlx::FromRow)]
struct StatusCount {
    rmp_match_status: String,
    count: i64,
}

#[derive(sqlx::FromRow)]
struct CandidateRow {
    id: i32,
    rmp_legacy_id: i32,
    score: Option<f32>,
    score_breakdown: Option<serde_json::Value>,
    status: String,
    first_name: Option<String>,
    last_name: Option<String>,
    department: Option<String>,
    avg_rating: Option<f32>,
    avg_difficulty: Option<f32>,
    num_ratings: Option<i32>,
    would_take_again_pct: Option<f32>,
}

#[derive(sqlx::FromRow)]
struct LinkedRmpProfileRow {
    link_id: i32,
    legacy_id: i32,
    first_name: Option<String>,
    last_name: Option<String>,
    department: Option<String>,
    avg_rating: Option<f32>,
    avg_difficulty: Option<f32>,
    num_ratings: Option<i32>,
    would_take_again_pct: Option<f32>,
}

// ---------------------------------------------------------------------------
// 1. GET /api/admin/instructors — paginated list with filtering
// ---------------------------------------------------------------------------

/// `GET /api/admin/instructors` — List instructors with filtering and pagination.
#[instrument(skip_all)]
pub async fn list_instructors(
    AdminUser(_user): AdminUser,
    State(state): State<AppState>,
    Query(params): Query<ListInstructorsParams>,
) -> Result<Json<ListInstructorsResponse>, (StatusCode, Json<Value>)> {
    let start = Instant::now();
    let page = params.page.unwrap_or(1).max(1);
    let per_page = params.per_page.unwrap_or(50).clamp(1, 100);
    let offset = (page - 1) * per_page;

    let sort_clause = match params.sort.as_deref() {
        Some("name_asc") => "i.display_name ASC",
        Some("name_desc") => "i.display_name DESC",
        Some("status") => "i.rmp_match_status ASC, i.display_name ASC",
        _ => "tc.score DESC NULLS LAST, i.display_name ASC",
    };

    // Build WHERE clause
    let mut conditions = Vec::new();
    let mut bind_idx = 0u32;

    if params.status.is_some() {
        bind_idx += 1;
        conditions.push(format!("i.rmp_match_status = ${bind_idx}"));
    }
    if params.search.is_some() {
        bind_idx += 1;
        conditions.push(format!(
            "(i.display_name ILIKE ${bind_idx} OR i.email ILIKE ${bind_idx})"
        ));
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    let query_str = format!(
        r#"
        SELECT
            i.id, i.display_name, i.email, i.rmp_match_status,
            (SELECT COUNT(*) FROM instructor_rmp_links irl WHERE irl.instructor_id = i.id) as rmp_link_count,
            tc.rmp_legacy_id as top_candidate_rmp_id,
            tc.score as top_candidate_score,
            tc.score_breakdown as top_candidate_breakdown,
            rp.first_name as tc_first_name,
            rp.last_name as tc_last_name,
            rp.department as tc_department,
            rp.avg_rating as tc_avg_rating,
            rp.num_ratings as tc_num_ratings,
            (SELECT COUNT(*) FROM rmp_match_candidates mc WHERE mc.instructor_id = i.id AND mc.status = 'pending') as candidate_count,
            (SELECT COUNT(DISTINCT c.subject) FROM course_instructors ci JOIN courses c ON c.id = ci.course_id WHERE ci.instructor_id = i.id) as course_subject_count
        FROM instructors i
        LEFT JOIN LATERAL (
            SELECT mc.rmp_legacy_id, mc.score, mc.score_breakdown
            FROM rmp_match_candidates mc
            WHERE mc.instructor_id = i.id AND mc.status = 'pending'
            ORDER BY mc.score DESC
            LIMIT 1
        ) tc ON true
        LEFT JOIN rmp_professors rp ON rp.legacy_id = tc.rmp_legacy_id
        {where_clause}
        ORDER BY {sort_clause}
        LIMIT {per_page} OFFSET {offset}
        "#
    );

    // Build the query with dynamic binds
    let mut query = sqlx::query_as::<_, InstructorRow>(&query_str);
    if let Some(ref status) = params.status {
        query = query.bind(status);
    }
    if let Some(ref search) = params.search {
        query = query.bind(format!("%{search}%"));
    }

    let rows = query
        .fetch_all(&state.db_pool)
        .await
        .map_err(|e| db_error("failed to list instructors", e))?;

    // Count total with filters
    let count_query_str = format!("SELECT COUNT(*) FROM instructors i {where_clause}");
    let mut count_query = sqlx::query_as::<_, (i64,)>(&count_query_str);
    if let Some(ref status) = params.status {
        count_query = count_query.bind(status);
    }
    if let Some(ref search) = params.search {
        count_query = count_query.bind(format!("%{search}%"));
    }

    let (total,) = count_query
        .fetch_one(&state.db_pool)
        .await
        .map_err(|e| db_error("failed to count instructors", e))?;

    // Aggregate stats (unfiltered)
    let stats_rows = sqlx::query_as::<_, StatusCount>(
        "SELECT rmp_match_status, COUNT(*) as count FROM instructors GROUP BY rmp_match_status",
    )
    .fetch_all(&state.db_pool)
    .await
    .map_err(|e| db_error("failed to get instructor stats", e))?;

    // Count instructors with at least one candidate (for progress bar denominator)
    let (with_candidates,): (i64,) =
        sqlx::query_as("SELECT COUNT(DISTINCT instructor_id) FROM rmp_match_candidates")
            .fetch_one(&state.db_pool)
            .await
            .map_err(|e| db_error("failed to count instructors with candidates", e))?;

    let mut stats = InstructorStats {
        total: 0,
        unmatched: 0,
        auto: 0,
        confirmed: 0,
        rejected: 0,
        with_candidates,
    };
    for row in &stats_rows {
        stats.total += row.count;
        match row.rmp_match_status.as_str() {
            "unmatched" => stats.unmatched = row.count,
            "auto" => stats.auto = row.count,
            "confirmed" => stats.confirmed = row.count,
            "rejected" => stats.rejected = row.count,
            _ => {}
        }
    }

    let instructors: Vec<InstructorListItem> = rows
        .iter()
        .map(|r| {
            let top_candidate = r.top_candidate_rmp_id.map(|rmp_id| TopCandidateResponse {
                rmp_legacy_id: rmp_id,
                score: r.top_candidate_score,
                score_breakdown: r.top_candidate_breakdown.clone(),
                first_name: r.tc_first_name.clone(),
                last_name: r.tc_last_name.clone(),
                department: r.tc_department.clone(),
                avg_rating: r.tc_avg_rating,
                num_ratings: r.tc_num_ratings,
            });

            InstructorListItem {
                id: r.id,
                display_name: r.display_name.clone(),
                email: r.email.clone(),
                rmp_match_status: r.rmp_match_status.clone(),
                rmp_link_count: r.rmp_link_count.unwrap_or(0),
                candidate_count: r.candidate_count.unwrap_or(0),
                course_subject_count: r.course_subject_count.unwrap_or(0),
                top_candidate,
            }
        })
        .collect();

    let elapsed = start.elapsed();
    if elapsed > SLOW_OP_THRESHOLD {
        warn!(
            duration = fmt_duration(elapsed),
            "slow operation: list_instructors"
        );
    }

    debug!(count = instructors.len(), total, "listed instructors");

    Ok(Json(ListInstructorsResponse {
        instructors,
        total,
        page,
        per_page,
        stats,
    }))
}

// ---------------------------------------------------------------------------
// 2. GET /api/admin/instructors/{id} — full detail
// ---------------------------------------------------------------------------

/// `GET /api/admin/instructors/{id}` — Full instructor detail with candidates.
#[instrument(skip_all, fields(instructor_id = id))]
pub async fn get_instructor(
    AdminUser(_user): AdminUser,
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<Json<InstructorDetailResponse>, (StatusCode, Json<Value>)> {
    let start = Instant::now();
    let result = build_instructor_detail(&state, id).await;

    let elapsed = start.elapsed();
    if elapsed > SLOW_OP_THRESHOLD {
        warn!(
            duration = fmt_duration(elapsed),
            "slow operation: get_instructor"
        );
    }

    if result.is_ok() {
        debug!(instructor_id = id, "fetched instructor detail");
    }

    result
}

/// Shared helper that builds the full instructor detail response.
async fn build_instructor_detail(
    state: &AppState,
    id: i32,
) -> Result<Json<InstructorDetailResponse>, (StatusCode, Json<Value>)> {
    // Fetch instructor
    let instructor: Option<(i32, String, String, String)> = sqlx::query_as(
        "SELECT id, display_name, email, rmp_match_status FROM instructors WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.db_pool)
    .await
    .map_err(|e| db_error("failed to fetch instructor", e))?;

    let (inst_id, display_name, email, rmp_match_status) = instructor.ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "instructor not found"})),
        )
    })?;

    // Subjects taught
    let subjects: Vec<(String,)> = sqlx::query_as(
        "SELECT DISTINCT c.subject FROM course_instructors ci JOIN courses c ON c.id = ci.course_id WHERE ci.instructor_id = $1 ORDER BY c.subject",
    )
    .bind(inst_id)
    .fetch_all(&state.db_pool)
    .await
    .map_err(|e| db_error("failed to fetch subjects", e))?;

    // Course count
    let (course_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(DISTINCT ci.course_id) FROM course_instructors ci WHERE ci.instructor_id = $1",
    )
    .bind(inst_id)
    .fetch_one(&state.db_pool)
    .await
    .map_err(|e| db_error("failed to count courses", e))?;

    // Candidates with RMP professor info
    let candidates = sqlx::query_as::<_, CandidateRow>(
        r#"
        SELECT mc.id, mc.rmp_legacy_id, mc.score, mc.score_breakdown, mc.status,
               rp.first_name, rp.last_name, rp.department,
               rp.avg_rating, rp.avg_difficulty, rp.num_ratings, rp.would_take_again_pct
        FROM rmp_match_candidates mc
        JOIN rmp_professors rp ON rp.legacy_id = mc.rmp_legacy_id
        WHERE mc.instructor_id = $1
        ORDER BY mc.score DESC
        "#,
    )
    .bind(inst_id)
    .fetch_all(&state.db_pool)
    .await
    .map_err(|e| db_error("failed to fetch candidates", e))?;

    // Current matches (all linked RMP profiles)
    let current_matches = sqlx::query_as::<_, LinkedRmpProfileRow>(
        r#"
        SELECT irl.id as link_id,
               rp.legacy_id, rp.first_name, rp.last_name, rp.department,
               rp.avg_rating, rp.avg_difficulty, rp.num_ratings, rp.would_take_again_pct
        FROM instructor_rmp_links irl
        JOIN rmp_professors rp ON rp.legacy_id = irl.rmp_legacy_id
        WHERE irl.instructor_id = $1
        ORDER BY rp.num_ratings DESC NULLS LAST
        "#,
    )
    .bind(inst_id)
    .fetch_all(&state.db_pool)
    .await
    .map_err(|e| db_error("failed to fetch linked rmp profiles", e))?;

    let current_matches_resp: Vec<LinkedRmpProfile> = current_matches
        .into_iter()
        .map(|p| LinkedRmpProfile {
            link_id: p.link_id,
            legacy_id: p.legacy_id,
            first_name: p.first_name,
            last_name: p.last_name,
            department: p.department,
            avg_rating: p.avg_rating,
            avg_difficulty: p.avg_difficulty,
            num_ratings: p.num_ratings,
            would_take_again_pct: p.would_take_again_pct,
        })
        .collect();

    let candidates_resp: Vec<CandidateResponse> = candidates
        .into_iter()
        .map(|c| CandidateResponse {
            id: c.id,
            rmp_legacy_id: c.rmp_legacy_id,
            first_name: c.first_name,
            last_name: c.last_name,
            department: c.department,
            avg_rating: c.avg_rating,
            avg_difficulty: c.avg_difficulty,
            num_ratings: c.num_ratings,
            would_take_again_pct: c.would_take_again_pct,
            score: c.score,
            score_breakdown: c.score_breakdown,
            status: c.status,
        })
        .collect();

    Ok(Json(InstructorDetailResponse {
        instructor: InstructorDetail {
            id: inst_id,
            display_name,
            email,
            rmp_match_status,
            subjects_taught: subjects.into_iter().map(|(s,)| s).collect(),
            course_count,
        },
        current_matches: current_matches_resp,
        candidates: candidates_resp,
    }))
}

// ---------------------------------------------------------------------------
// 3. POST /api/admin/instructors/{id}/match — accept a candidate
// ---------------------------------------------------------------------------

/// `POST /api/admin/instructors/{id}/match` — Accept a candidate match.
#[instrument(skip_all, fields(instructor_id = id))]
pub async fn match_instructor(
    AdminUser(user): AdminUser,
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(body): Json<MatchBody>,
) -> Result<Json<InstructorDetailResponse>, (StatusCode, Json<Value>)> {
    let start = Instant::now();
    // Verify the candidate exists and is pending
    let candidate: Option<(i32,)> = sqlx::query_as(
        "SELECT id FROM rmp_match_candidates WHERE instructor_id = $1 AND rmp_legacy_id = $2 AND status = 'pending'",
    )
    .bind(id)
    .bind(body.rmp_legacy_id)
    .fetch_optional(&state.db_pool)
    .await
    .map_err(|e| db_error("failed to check candidate", e))?;

    if candidate.is_none() {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "pending candidate not found for this instructor"})),
        ));
    }

    // Check if this RMP profile is already linked to a different instructor
    let conflict: Option<(i32,)> = sqlx::query_as(
        "SELECT instructor_id FROM instructor_rmp_links WHERE rmp_legacy_id = $1 AND instructor_id != $2",
    )
    .bind(body.rmp_legacy_id)
    .bind(id)
    .fetch_optional(&state.db_pool)
    .await
    .map_err(|e| db_error("failed to check rmp uniqueness", e))?;

    if let Some((other_id,)) = conflict {
        return Err((
            StatusCode::CONFLICT,
            Json(json!({
                "error": "RMP profile already linked to another instructor",
                "conflictingInstructorId": other_id,
            })),
        ));
    }

    let mut tx = state
        .db_pool
        .begin()
        .await
        .map_err(|e| db_error("failed to begin transaction", e))?;

    // Insert link into instructor_rmp_links
    sqlx::query(
        "INSERT INTO instructor_rmp_links (instructor_id, rmp_legacy_id, created_by, source) VALUES ($1, $2, $3, 'manual') ON CONFLICT (rmp_legacy_id) DO NOTHING",
    )
    .bind(id)
    .bind(body.rmp_legacy_id)
    .bind(user.discord_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| db_error("failed to insert rmp link", e))?;

    // Update instructor match status
    sqlx::query("UPDATE instructors SET rmp_match_status = 'confirmed' WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| db_error("failed to update instructor match status", e))?;

    // Accept the candidate
    sqlx::query(
        "UPDATE rmp_match_candidates SET status = 'accepted', resolved_at = NOW(), resolved_by = $1 WHERE instructor_id = $2 AND rmp_legacy_id = $3",
    )
    .bind(user.discord_id)
    .bind(id)
    .bind(body.rmp_legacy_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| db_error("failed to accept candidate", e))?;

    tx.commit()
        .await
        .map_err(|e| db_error("failed to commit transaction", e))?;

    let elapsed = start.elapsed();
    if elapsed > SLOW_OP_THRESHOLD {
        warn!(
            duration = fmt_duration(elapsed),
            "slow operation: match_instructor"
        );
    }

    info!(
        instructor_id = id,
        rmp_legacy_id = body.rmp_legacy_id,
        "instructor matched to RMP profile"
    );

    build_instructor_detail(&state, id).await
}

// ---------------------------------------------------------------------------
// 4. POST /api/admin/instructors/{id}/reject-candidate — reject one candidate
// ---------------------------------------------------------------------------

/// `POST /api/admin/instructors/{id}/reject-candidate` — Reject a single candidate.
#[instrument(skip_all, fields(instructor_id = id))]
pub async fn reject_candidate(
    AdminUser(user): AdminUser,
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(body): Json<RejectCandidateBody>,
) -> Result<Json<OkResponse>, (StatusCode, Json<Value>)> {
    let start = Instant::now();
    let result = sqlx::query(
        "UPDATE rmp_match_candidates SET status = 'rejected', resolved_at = NOW(), resolved_by = $1 WHERE instructor_id = $2 AND rmp_legacy_id = $3 AND status = 'pending'",
    )
    .bind(user.discord_id)
    .bind(id)
    .bind(body.rmp_legacy_id)
    .execute(&state.db_pool)
    .await
    .map_err(|e| db_error("failed to reject candidate", e))?;

    if result.rows_affected() == 0 {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "pending candidate not found"})),
        ));
    }

    let elapsed = start.elapsed();
    if elapsed > SLOW_OP_THRESHOLD {
        warn!(
            duration = fmt_duration(elapsed),
            "slow operation: reject_candidate"
        );
    }

    info!(
        instructor_id = id,
        rmp_legacy_id = body.rmp_legacy_id,
        "RMP candidate rejected"
    );

    Ok(Json(OkResponse { ok: true }))
}

// ---------------------------------------------------------------------------
// 5. POST /api/admin/instructors/{id}/reject-all — no valid match
// ---------------------------------------------------------------------------

/// `POST /api/admin/instructors/{id}/reject-all` — Mark instructor as having no valid RMP match.
#[instrument(skip_all, fields(instructor_id = id))]
pub async fn reject_all(
    AdminUser(user): AdminUser,
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<Json<OkResponse>, (StatusCode, Json<Value>)> {
    let start = Instant::now();
    let mut tx = state
        .db_pool
        .begin()
        .await
        .map_err(|e| db_error("failed to begin transaction", e))?;

    // Check current status — cannot reject an instructor with confirmed matches
    let current_status: Option<(String,)> =
        sqlx::query_as("SELECT rmp_match_status FROM instructors WHERE id = $1")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| db_error("failed to fetch instructor status", e))?;

    let (status,) = current_status.ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "instructor not found"})),
        )
    })?;

    if status == "confirmed" {
        return Err((
            StatusCode::CONFLICT,
            Json(
                json!({"error": "cannot reject instructor with confirmed matches — unmatch first"}),
            ),
        ));
    }

    // Update instructor status
    sqlx::query("UPDATE instructors SET rmp_match_status = 'rejected' WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| db_error("failed to update instructor status", e))?;

    // Reject all pending candidates
    sqlx::query(
        "UPDATE rmp_match_candidates SET status = 'rejected', resolved_at = NOW(), resolved_by = $1 WHERE instructor_id = $2 AND status = 'pending'",
    )
    .bind(user.discord_id)
    .bind(id)
    .execute(&mut *tx)
    .await
    .map_err(|e| db_error("failed to reject candidates", e))?;

    tx.commit()
        .await
        .map_err(|e| db_error("failed to commit transaction", e))?;

    let elapsed = start.elapsed();
    if elapsed > SLOW_OP_THRESHOLD {
        warn!(
            duration = fmt_duration(elapsed),
            "slow operation: reject_all"
        );
    }

    info!(instructor_id = id, "all RMP candidates rejected");

    Ok(Json(OkResponse { ok: true }))
}

// ---------------------------------------------------------------------------
// 6. POST /api/admin/instructors/{id}/unmatch — remove current match
// ---------------------------------------------------------------------------

/// Body for unmatch — optional `rmpLegacyId` to remove a specific link.
/// If omitted (or null), all links are removed.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UnmatchBody {
    rmp_legacy_id: Option<i32>,
}

/// `POST /api/admin/instructors/{id}/unmatch` — Remove RMP link(s).
///
/// Send `{ "rmpLegacyId": N }` to remove a specific link, or an empty body / `{}`
/// to remove all links for the instructor.
#[instrument(skip_all, fields(instructor_id = id))]
pub async fn unmatch_instructor(
    AdminUser(_user): AdminUser,
    State(state): State<AppState>,
    Path(id): Path<i32>,
    body: Option<Json<UnmatchBody>>,
) -> Result<Json<OkResponse>, (StatusCode, Json<Value>)> {
    let start = Instant::now();
    let rmp_legacy_id = body.and_then(|b| b.rmp_legacy_id);

    // Verify instructor exists
    let exists: Option<(i32,)> = sqlx::query_as("SELECT id FROM instructors WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db_pool)
        .await
        .map_err(|e| db_error("failed to check instructor", e))?;

    if exists.is_none() {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "instructor not found"})),
        ));
    }

    // Use the data layer function to perform the unmatch
    crate::data::rmp::unmatch_instructor(&state.db_pool, id, rmp_legacy_id)
        .await
        .map_err(|e| {
            error!(error = %e, "failed to unmatch instructor");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "failed to unmatch instructor"})),
            )
        })?;

    let elapsed = start.elapsed();
    if elapsed > SLOW_OP_THRESHOLD {
        warn!(
            duration = fmt_duration(elapsed),
            "slow operation: unmatch_instructor"
        );
    }

    info!(
        instructor_id = id,
        ?rmp_legacy_id,
        "instructor unmatched from RMP"
    );

    Ok(Json(OkResponse { ok: true }))
}

// ---------------------------------------------------------------------------
// 7. POST /api/admin/rmp/rescore — re-run candidate generation
// ---------------------------------------------------------------------------

/// `POST /api/admin/rmp/rescore` — Re-run RMP candidate generation.
#[instrument(skip_all)]
pub async fn rescore(
    AdminUser(_user): AdminUser,
    State(state): State<AppState>,
) -> Result<Json<RescoreResponse>, (StatusCode, Json<Value>)> {
    let start = Instant::now();
    let stats = crate::data::rmp_matching::generate_candidates(&state.db_pool)
        .await
        .map_err(|e| {
            error!(error = %e, "failed to run candidate generation");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "candidate generation failed"})),
            )
        })?;

    let elapsed = start.elapsed();
    if elapsed > SLOW_OP_THRESHOLD {
        warn!(duration = fmt_duration(elapsed), "slow operation: rescore");
    }

    info!(
        total_unmatched = stats.total_unmatched,
        candidates_created = stats.candidates_created,
        auto_matched = stats.auto_matched,
        "RMP candidates rescored"
    );

    Ok(Json(RescoreResponse {
        total_unmatched: stats.total_unmatched,
        candidates_created: stats.candidates_created,
        candidates_rescored: stats.candidates_rescored,
        auto_matched: stats.auto_matched,
        skipped_unparseable: stats.skipped_unparseable,
        skipped_no_candidates: stats.skipped_no_candidates,
    }))
}
