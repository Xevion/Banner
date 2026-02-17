//! Data-layer operations for RMP instructor matching admin features.
//!
//! Extracts all SQL from the web admin handlers into pure data functions
//! that return `anyhow::Result`. The web layer handles HTTP concerns only.

use anyhow::{Context, Result, anyhow};
use serde::Serialize;
use sqlx::PgPool;
use ts_rs::TS;

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
#[derive(Debug, Clone, Serialize, sqlx::FromRow, TS)]
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
#[derive(Debug, Clone, Serialize, sqlx::FromRow, TS)]
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

/// Full instructor detail with candidates and linked profiles.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct InstructorDetailResponse {
    pub instructor: InstructorDetail,
    pub current_matches: Vec<LinkedRmpProfile>,
    pub candidates: Vec<CandidateResponse>,
}

/// Response for the paginated instructor list.
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

/// Response for the rescore operation.
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

/// Filter/sort/pagination params for listing instructors.
pub struct ListInstructorsFilter {
    pub status: Option<String>,
    pub search: Option<String>,
    pub page: i32,
    pub per_page: i32,
    pub sort: Option<String>,
}

/// List instructors with filtering, sorting, and pagination.
pub async fn list_instructors(
    pool: &PgPool,
    filter: &ListInstructorsFilter,
) -> Result<ListInstructorsResponse> {
    let page = filter.page.max(1);
    let per_page = filter.per_page.clamp(1, 100);
    let offset = (page - 1) * per_page;

    let sort_clause = match filter.sort.as_deref() {
        Some("name_asc") => "i.display_name ASC",
        Some("name_desc") => "i.display_name DESC",
        Some("status") => "i.rmp_match_status ASC, i.display_name ASC",
        _ => "tc.score DESC NULLS LAST, i.display_name ASC",
    };

    // Build WHERE clause
    let mut conditions = Vec::new();
    let mut bind_idx = 0u32;

    if filter.status.is_some() {
        bind_idx += 1;
        conditions.push(format!("i.rmp_match_status = ${bind_idx}"));
    }
    if filter.search.is_some() {
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

    let mut query = sqlx::query_as::<_, InstructorRow>(&query_str);
    if let Some(ref status) = filter.status {
        query = query.bind(status);
    }
    if let Some(ref search) = filter.search {
        query = query.bind(format!("%{search}%"));
    }

    let rows = query
        .fetch_all(pool)
        .await
        .context("failed to list instructors")?;

    // Count total with filters
    let count_query_str = format!("SELECT COUNT(*) FROM instructors i {where_clause}");
    let mut count_query = sqlx::query_as::<_, (i64,)>(&count_query_str);
    if let Some(ref status) = filter.status {
        count_query = count_query.bind(status);
    }
    if let Some(ref search) = filter.search {
        count_query = count_query.bind(format!("%{search}%"));
    }

    let (total,) = count_query
        .fetch_one(pool)
        .await
        .context("failed to count instructors")?;

    // Aggregate stats (unfiltered)
    let stats_rows = sqlx::query_as::<_, StatusCount>(
        "SELECT rmp_match_status, COUNT(*) as count FROM instructors GROUP BY rmp_match_status",
    )
    .fetch_all(pool)
    .await
    .context("failed to get instructor stats")?;

    let (with_candidates,): (i64,) =
        sqlx::query_as("SELECT COUNT(DISTINCT instructor_id) FROM rmp_match_candidates")
            .fetch_one(pool)
            .await
            .context("failed to count instructors with candidates")?;

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

    let instructors = rows
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

    Ok(ListInstructorsResponse {
        instructors,
        total,
        page,
        per_page,
        stats,
    })
}

/// Fetch full instructor detail with candidates and linked profiles.
pub async fn get_instructor_detail(pool: &PgPool, id: i32) -> Result<InstructorDetailResponse> {
    let instructor: Option<(i32, String, String, String)> = sqlx::query_as(
        "SELECT id, display_name, email, rmp_match_status FROM instructors WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .context("failed to fetch instructor")?;

    let (inst_id, display_name, email, rmp_match_status) = instructor
        .ok_or_else(|| anyhow!("instructor not found"))
        .context("instructor lookup")?;

    let subjects: Vec<(String,)> = sqlx::query_as(
        "SELECT DISTINCT c.subject FROM course_instructors ci JOIN courses c ON c.id = ci.course_id WHERE ci.instructor_id = $1 ORDER BY c.subject",
    )
    .bind(inst_id)
    .fetch_all(pool)
    .await
    .context("failed to fetch subjects")?;

    let (course_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(DISTINCT ci.course_id) FROM course_instructors ci WHERE ci.instructor_id = $1",
    )
    .bind(inst_id)
    .fetch_one(pool)
    .await
    .context("failed to count courses")?;

    let candidates = sqlx::query_as::<_, CandidateResponse>(
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
    .fetch_all(pool)
    .await
    .context("failed to fetch candidates")?;

    let current_matches = sqlx::query_as::<_, LinkedRmpProfile>(
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
    .fetch_all(pool)
    .await
    .context("failed to fetch linked rmp profiles")?;

    Ok(InstructorDetailResponse {
        instructor: InstructorDetail {
            id: inst_id,
            display_name,
            email,
            rmp_match_status,
            subjects_taught: subjects.into_iter().map(|(s,)| s).collect(),
            course_count,
        },
        current_matches,
        candidates,
    })
}

/// Accept a candidate match for an instructor.
///
/// Returns `Ok(())` on success, `Err` with context on failure.
/// Returns `MatchConflict` details via the error if the RMP profile
/// is already linked to a different instructor.
pub async fn accept_candidate(
    pool: &PgPool,
    instructor_id: i32,
    rmp_legacy_id: i32,
    resolved_by: i64,
) -> Result<()> {
    // Verify the candidate exists and is pending
    let candidate: Option<(i32,)> = sqlx::query_as(
        "SELECT id FROM rmp_match_candidates WHERE instructor_id = $1 AND rmp_legacy_id = $2 AND status = 'pending'",
    )
    .bind(instructor_id)
    .bind(rmp_legacy_id)
    .fetch_optional(pool)
    .await
    .context("failed to check candidate")?;

    if candidate.is_none() {
        return Err(anyhow!("pending candidate not found for this instructor"));
    }

    // Check if this RMP profile is already linked to a different instructor
    let conflict: Option<(i32,)> = sqlx::query_as(
        "SELECT instructor_id FROM instructor_rmp_links WHERE rmp_legacy_id = $1 AND instructor_id != $2",
    )
    .bind(rmp_legacy_id)
    .bind(instructor_id)
    .fetch_optional(pool)
    .await
    .context("failed to check rmp uniqueness")?;

    if let Some((other_id,)) = conflict {
        return Err(anyhow!(
            "RMP profile already linked to instructor {other_id}"
        ))
        .context("conflict");
    }

    let mut tx = pool.begin().await.context("failed to begin transaction")?;

    sqlx::query(
        "INSERT INTO instructor_rmp_links (instructor_id, rmp_legacy_id, created_by, source) VALUES ($1, $2, $3, 'manual') ON CONFLICT (rmp_legacy_id) DO NOTHING",
    )
    .bind(instructor_id)
    .bind(rmp_legacy_id)
    .bind(resolved_by)
    .execute(&mut *tx)
    .await
    .context("failed to insert rmp link")?;

    sqlx::query("UPDATE instructors SET rmp_match_status = 'confirmed' WHERE id = $1")
        .bind(instructor_id)
        .execute(&mut *tx)
        .await
        .context("failed to update instructor match status")?;

    sqlx::query(
        "UPDATE rmp_match_candidates SET status = 'accepted', resolved_at = NOW(), resolved_by = $1 WHERE instructor_id = $2 AND rmp_legacy_id = $3",
    )
    .bind(resolved_by)
    .bind(instructor_id)
    .bind(rmp_legacy_id)
    .execute(&mut *tx)
    .await
    .context("failed to accept candidate")?;

    tx.commit().await.context("failed to commit transaction")?;

    Ok(())
}

/// Reject a single candidate for an instructor.
///
/// Returns `true` if a candidate was rejected, `false` if no pending candidate was found.
pub async fn reject_candidate(
    pool: &PgPool,
    instructor_id: i32,
    rmp_legacy_id: i32,
    resolved_by: i64,
) -> Result<bool> {
    let result = sqlx::query(
        "UPDATE rmp_match_candidates SET status = 'rejected', resolved_at = NOW(), resolved_by = $1 WHERE instructor_id = $2 AND rmp_legacy_id = $3 AND status = 'pending'",
    )
    .bind(resolved_by)
    .bind(instructor_id)
    .bind(rmp_legacy_id)
    .execute(pool)
    .await
    .context("failed to reject candidate")?;

    Ok(result.rows_affected() > 0)
}

/// Reject all pending candidates for an instructor and mark them as having no valid match.
///
/// Returns an error if the instructor has confirmed matches (must unmatch first).
pub async fn reject_all_candidates(
    pool: &PgPool,
    instructor_id: i32,
    resolved_by: i64,
) -> Result<()> {
    let mut tx = pool.begin().await.context("failed to begin transaction")?;

    let current_status: Option<(String,)> =
        sqlx::query_as("SELECT rmp_match_status FROM instructors WHERE id = $1")
            .bind(instructor_id)
            .fetch_optional(&mut *tx)
            .await
            .context("failed to fetch instructor status")?;

    let (status,) = current_status.ok_or_else(|| anyhow!("instructor not found"))?;

    if status == "confirmed" {
        return Err(anyhow!(
            "cannot reject instructor with confirmed matches — unmatch first"
        ));
    }

    sqlx::query("UPDATE instructors SET rmp_match_status = 'rejected' WHERE id = $1")
        .bind(instructor_id)
        .execute(&mut *tx)
        .await
        .context("failed to update instructor status")?;

    sqlx::query(
        "UPDATE rmp_match_candidates SET status = 'rejected', resolved_at = NOW(), resolved_by = $1 WHERE instructor_id = $2 AND status = 'pending'",
    )
    .bind(resolved_by)
    .bind(instructor_id)
    .execute(&mut *tx)
    .await
    .context("failed to reject candidates")?;

    tx.commit().await.context("failed to commit transaction")?;

    Ok(())
}

/// Check if an instructor exists.
pub async fn instructor_exists(pool: &PgPool, id: i32) -> Result<bool> {
    let exists: Option<(i32,)> = sqlx::query_as("SELECT id FROM instructors WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await
        .context("failed to check instructor")?;

    Ok(exists.is_some())
}

/// Re-run RMP candidate generation and return scoring statistics.
pub async fn rescore(pool: &PgPool) -> Result<RescoreResponse> {
    let stats = crate::data::rmp_matching::generate_candidates(pool)
        .await
        .context("candidate generation failed")?;

    Ok(RescoreResponse {
        total_unmatched: stats.total_unmatched,
        candidates_created: stats.candidates_created,
        candidates_rescored: stats.candidates_rescored,
        auto_matched: stats.auto_matched,
        skipped_unparseable: stats.skipped_unparseable,
        skipped_no_candidates: stats.skipped_no_candidates,
    })
}
