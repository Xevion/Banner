//! Data-layer operations for RMP instructor matching admin features.
//!
//! Extracts all SQL from the web admin handlers into pure data functions
//! that return `anyhow::Result`. The web layer handles HTTP concerns only.

use anyhow::{Context, Result};
use serde::Serialize;
use sqlx::{AssertSqlSafe, PgPool};
use tracing::warn;
use ts_rs::TS;

use crate::data::escape_like;
use crate::data::models::{Page, RmpCandidateStatus, RmpMatchStatus};
use crate::data::rmp_matching::ScoreBreakdown;
use crate::data::unsigned::Count;

/// Domain errors for RMP matching admin operations.
///
/// The web layer downcasts `anyhow::Error` to this type to decide HTTP status codes
/// instead of fragile string matching.
#[derive(Debug, thiserror::Error)]
pub enum AdminRmpError {
    #[error("instructor not found")]
    NoSuchInstructor,
    #[error("pending candidate not found for this instructor")]
    NoPendingCandidate,
    #[error(
        "RMP profile already linked to instructor {display_name} ({}, #{instructor_id})",
        .email.as_deref().unwrap_or("no email")
    )]
    AlreadyLinked {
        instructor_id: i32,
        display_name: String,
        email: Option<String>,
    },
    #[error("cannot reject instructor with confirmed matches -- unmatch first")]
    ConfirmedMatches,
}

/// A top-candidate summary shown in the instructor list view.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TopCandidateResponse {
    pub rmp_legacy_id: i32,
    pub score: f32,
    pub score_breakdown: ScoreBreakdown,
    pub first_name: String,
    pub last_name: String,
    pub department: Option<String>,
    pub avg_rating: Option<f32>,
    pub num_ratings: i32,
    /// Instructor already holding this profile, when it is not this one.
    pub claimed_by: Option<String>,
}

/// An instructor row in the paginated list.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct InstructorListItem {
    pub id: i32,
    pub display_name: String,
    pub email: Option<String>,
    pub rmp_match_status: RmpMatchStatus,
    #[ts(as = "i32")]
    pub rmp_link_count: i64,
    #[ts(as = "i32")]
    pub candidate_count: i64,
    #[ts(as = "i32")]
    pub course_subject_count: i64,
    pub top_candidate: Option<TopCandidateResponse>,
    /// Sorted distinct academic years in which this instructor taught courses.
    pub teaching_years: Vec<i16>,
    /// Sorted distinct subject codes this instructor has taught.
    pub subjects_taught: Vec<String>,
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
    /// Instructors with algorithm-generated candidates below auto-accept threshold.
    #[ts(as = "i32")]
    pub pending: i64,
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
    pub email: Option<String>,
    pub rmp_match_status: RmpMatchStatus,
    pub subjects_taught: Vec<String>,
    #[ts(as = "i32")]
    pub course_count: i64,
    /// Sorted distinct academic years in which this instructor taught courses.
    pub teaching_years: Vec<i16>,
}

/// A linked RMP profile in the detail view.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LinkedRmpProfile {
    pub link_id: i32,
    pub legacy_id: i32,
    pub first_name: String,
    pub last_name: String,
    pub department: Option<String>,
    pub avg_rating: Option<f32>,
    pub avg_difficulty: Option<f32>,
    pub num_ratings: i32,
    pub would_take_again_pct: Option<f32>,
    /// Subject prefixes extracted from RMP reviews (queried live from rmp_reviews).
    pub review_subjects: Vec<String>,
    /// Distinct years in which this professor received reviews.
    pub review_years: Vec<i16>,
}

/// A match candidate in the detail view.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CandidateResponse {
    pub id: i32,
    pub rmp_legacy_id: i32,
    pub first_name: String,
    pub last_name: String,
    pub department: Option<String>,
    pub avg_rating: Option<f32>,
    pub avg_difficulty: Option<f32>,
    pub num_ratings: i32,
    pub would_take_again_pct: Option<f32>,
    pub score: f32,
    #[ts(as = "ScoreBreakdown")]
    pub score_breakdown: sqlx::types::Json<ScoreBreakdown>,
    pub status: RmpCandidateStatus,
    /// Subject prefixes extracted from RMP reviews (e.g. ["CS", "WRC"]).
    pub review_subjects: Vec<String>,
    /// Distinct years in which this professor received reviews.
    pub review_years: Vec<i16>,
    /// Instructor already holding this RMP profile, if any.
    pub claimed_by: Option<String>,
    /// Why this candidate was not linked automatically.
    pub blocked_reason: Option<String>,
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
    pub page: Page<InstructorListItem>,
    pub stats: InstructorStats,
}

/// Response for the rescore operation.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RescoreResponse {
    /// Total instructors processed (excludes confirmed/rejected).
    pub total_processed: usize,
    /// Pending candidate rows deleted before regeneration.
    pub deleted_pending_candidates: usize,
    /// Auto-generated links deleted before regeneration.
    pub deleted_auto_links: usize,
    /// Candidates inserted in this run.
    pub candidates_created: usize,
    /// Instructors auto-linked (score >= threshold).
    pub auto_matched: usize,
    /// Instructors with candidates below auto-accept threshold (status = 'pending').
    pub pending_review: usize,
    pub skipped_unparseable: usize,
    pub skipped_no_candidates: usize,
}

#[derive(sqlx::FromRow)]
struct InstructorRow {
    id: i32,
    display_name: String,
    email: Option<String>,
    rmp_match_status: RmpMatchStatus,
    rmp_link_count: i64,
    top_candidate_rmp_id: Option<i32>,
    top_candidate_score: Option<f32>,
    top_candidate_breakdown: Option<sqlx::types::Json<ScoreBreakdown>>,
    tc_first_name: Option<String>,
    tc_last_name: Option<String>,
    tc_department: Option<String>,
    tc_avg_rating: Option<f32>,
    tc_num_ratings: Option<i32>,
    tc_claimed_by: Option<String>,
    candidate_count: i64,
    course_subject_count: i64,
    teaching_years: Option<Vec<i16>>,
    subjects_taught: Option<Vec<String>>,
}

/// Filter/sort/pagination params for listing instructors.
pub struct ListInstructorsFilter {
    pub status: Option<RmpMatchStatus>,
    pub search: Option<String>,
    pub page: i32,
    pub per_page: i32,
    pub sort: Option<String>,
}

/// List instructors with filtering, sorting, and pagination.
pub async fn list_instructors(pool: &PgPool, filter: &ListInstructorsFilter) -> Result<ListInstructorsResponse> {
    let page = filter.page.max(1);
    let per_page = filter.per_page.clamp(1, 100);
    let offset = (page - 1) * per_page;

    let sort_clause = match filter.sort.as_deref() {
        Some("name_asc") => "i.display_name ASC",
        Some("name_desc") => "i.display_name DESC",
        Some("status") => "ms.status ASC, i.display_name ASC",
        _ => "tc.score DESC NULLS LAST, i.display_name ASC",
    };

    let status = filter.status.map(<&'static str>::from);
    let search_pattern = filter
        .search
        .as_ref()
        .map(|search| format!("%{}%", escape_like(search)));

    let query_str = format!(
        r#"
        SELECT
            i.id, i.display_name, i.email, ms.status AS rmp_match_status,
            (SELECT COUNT(*) FROM instructor_rmp_links irl WHERE irl.instructor_id = i.id) as rmp_link_count,
            tc.rmp_legacy_id as top_candidate_rmp_id,
            tc.score as top_candidate_score,
            tc.score_breakdown as top_candidate_breakdown,
            rp.first_name as tc_first_name,
            rp.last_name as tc_last_name,
            rp.department as tc_department,
            rp.avg_rating as tc_avg_rating,
            rp.num_ratings as tc_num_ratings,
            (SELECT i2.display_name FROM instructor_rmp_links l2
              JOIN instructors i2 ON i2.id = l2.instructor_id
             WHERE l2.rmp_legacy_id = tc.rmp_legacy_id AND l2.instructor_id <> i.id) as tc_claimed_by,
            (SELECT COUNT(*) FROM rmp_match_candidates mc WHERE mc.instructor_id = i.id AND mc.status = 'pending') as candidate_count,
            (SELECT COUNT(DISTINCT c.subject) FROM course_instructors ci JOIN courses c ON c.id = ci.course_id WHERE ci.instructor_id = i.id) as course_subject_count,
            (SELECT ARRAY_AGG(DISTINCT t.year ORDER BY t.year) FROM course_instructors ci JOIN courses c ON c.id = ci.course_id JOIN terms t ON t.code = c.term_code WHERE ci.instructor_id = i.id) as teaching_years,
            (SELECT ARRAY_AGG(DISTINCT c.subject ORDER BY c.subject) FROM course_instructors ci JOIN courses c ON c.id = ci.course_id WHERE ci.instructor_id = i.id) as subjects_taught
        FROM instructors i
        JOIN instructor_rmp_match_status ms ON ms.instructor_id = i.id
        LEFT JOIN LATERAL (
            SELECT mc.rmp_legacy_id, mc.score, mc.score_breakdown
            FROM rmp_match_candidates mc
            WHERE mc.instructor_id = i.id
            ORDER BY (mc.status = 'accepted') DESC, mc.score DESC
            LIMIT 1
        ) tc ON true
        LEFT JOIN rmp_professors rp ON rp.legacy_id = tc.rmp_legacy_id
        WHERE ($1::text IS NULL OR ms.status = $1)
          AND ($2::text IS NULL OR i.display_name ILIKE $2 OR i.email ILIKE $2)
        ORDER BY {sort_clause}
        LIMIT $3 OFFSET $4
        "#
    );

    // Only the sort is interpolated, and ORDER BY cannot be a bind parameter, so this
    // one stays runtime-checked. A NULL bind disables its own filter clause.
    let rows = sqlx::query_as::<_, InstructorRow>(AssertSqlSafe(query_str))
        .bind(status)
        .bind(search_pattern.as_deref())
        .bind(per_page)
        .bind(offset)
        .fetch_all(pool)
        .await
        .context("failed to list instructors")?;

    // The count repeats the page query's FROM and WHERE, and is macro-checked so that
    // an alias the count cannot resolve fails the build instead of the request.
    let total = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) AS "total!"
        FROM instructors i
        JOIN instructor_rmp_match_status ms ON ms.instructor_id = i.id
        WHERE ($1::text IS NULL OR ms.status = $1)
          AND ($2::text IS NULL OR i.display_name ILIKE $2 OR i.email ILIKE $2)
        "#,
        status,
        search_pattern.as_deref(),
    )
    .fetch_one(pool)
    .await
    .context("failed to count instructors")?;

    // Aggregate stats (unfiltered). Both overrides are the view's doing: sqlx reads
    // status through a CASE expression, and an aggregate, as nullable.
    let stats_rows = sqlx::query!(
        r#"
        SELECT status AS "status!: RmpMatchStatus", COUNT(*) AS "count!"
        FROM instructor_rmp_match_status
        GROUP BY status
        "#
    )
    .fetch_all(pool)
    .await
    .context("failed to get instructor stats")?;

    let with_candidates =
        sqlx::query_scalar!(r#"SELECT COUNT(DISTINCT instructor_id) AS "count!" FROM rmp_match_candidates"#)
            .fetch_one(pool)
            .await
            .context("failed to count instructors with candidates")?;

    let mut stats = InstructorStats {
        total: 0,
        unmatched: 0,
        pending: 0,
        auto: 0,
        confirmed: 0,
        rejected: 0,
        with_candidates,
    };
    for row in &stats_rows {
        stats.total += row.count;
        match row.status {
            RmpMatchStatus::Unmatched => stats.unmatched = row.count,
            RmpMatchStatus::Pending => stats.pending = row.count,
            RmpMatchStatus::Auto => stats.auto = row.count,
            RmpMatchStatus::Confirmed => stats.confirmed = row.count,
            RmpMatchStatus::Rejected => stats.rejected = row.count,
        }
    }

    let instructors: Vec<InstructorListItem> = rows
        .iter()
        .map(|r| {
            // The lateral join either produces a whole candidate row or none of it,
            // and its rmp_legacy_id is a foreign key, so the professor columns follow.
            let top_candidate = r
                .top_candidate_rmp_id
                .map(|rmp_id| -> Result<TopCandidateResponse> {
                    Ok(TopCandidateResponse {
                        rmp_legacy_id: rmp_id,
                        score: r.top_candidate_score.context("top candidate has no score")?,
                        score_breakdown: r
                            .top_candidate_breakdown
                            .as_ref()
                            .context("top candidate has no score breakdown")?
                            .0
                            .clone(),
                        first_name: r.tc_first_name.clone().context("top candidate has no rmp profile")?,
                        last_name: r.tc_last_name.clone().context("top candidate has no rmp profile")?,
                        department: r.tc_department.clone(),
                        avg_rating: r.tc_avg_rating,
                        num_ratings: r.tc_num_ratings.context("top candidate has no rmp profile")?,
                        claimed_by: r.tc_claimed_by.clone(),
                    })
                })
                .transpose()?;

            Ok(InstructorListItem {
                id: r.id,
                display_name: r.display_name.clone(),
                email: r.email.clone(),
                rmp_match_status: r.rmp_match_status,
                rmp_link_count: r.rmp_link_count,
                candidate_count: r.candidate_count,
                course_subject_count: r.course_subject_count,
                top_candidate,
                teaching_years: r.teaching_years.clone().unwrap_or_default(),
                subjects_taught: r.subjects_taught.clone().unwrap_or_default(),
            })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(ListInstructorsResponse {
        page: Page {
            items: instructors,
            total: Count::try_from(total)?,
            page,
            per_page,
        },
        stats,
    })
}

/// Fetch full instructor detail with candidates and linked profiles.
///
/// `blocked_reason` is left empty; the web layer fills in reviewer-facing copy.
pub async fn get_instructor_detail(pool: &PgPool, id: i32) -> Result<InstructorDetailResponse> {
    // The view reads status through a CASE expression, so sqlx calls it nullable.
    let instructor = sqlx::query!(
        r#"
        SELECT i.id, i.display_name, i.email, ms.status AS "status!: RmpMatchStatus"
        FROM instructors i
        JOIN instructor_rmp_match_status ms ON ms.instructor_id = i.id
        WHERE i.id = $1
        "#,
        id
    )
    .fetch_optional(pool)
    .await
    .context("failed to fetch instructor")?
    .ok_or(AdminRmpError::NoSuchInstructor)?;

    let inst_id = instructor.id;

    let subjects = sqlx::query_scalar!(
        r#"
        SELECT DISTINCT c.subject
        FROM course_instructors ci
        JOIN courses c ON c.id = ci.course_id
        WHERE ci.instructor_id = $1
        ORDER BY c.subject
        "#,
        inst_id
    )
    .fetch_all(pool)
    .await
    .context("failed to fetch subjects")?;

    let course_count = sqlx::query_scalar!(
        r#"SELECT COUNT(DISTINCT ci.course_id) AS "count!" FROM course_instructors ci WHERE ci.instructor_id = $1"#,
        inst_id
    )
    .fetch_one(pool)
    .await
    .context("failed to count courses")?;

    let teaching_years = sqlx::query_scalar!(
        r#"
        SELECT DISTINCT t.year
        FROM course_instructors ci
        JOIN courses c ON c.id = ci.course_id
        JOIN terms t ON t.code = c.term_code
        WHERE ci.instructor_id = $1
        ORDER BY t.year
        "#,
        inst_id
    )
    .fetch_all(pool)
    .await
    .context("failed to fetch teaching years")?;

    // Hand-mapped because blocked_reason is not a column: the web layer fills it in.
    let candidates = sqlx::query!(
        r#"
        SELECT mc.id, mc.rmp_legacy_id, mc.score,
               mc.score_breakdown AS "score_breakdown: sqlx::types::Json<ScoreBreakdown>",
               mc.status AS "status: RmpCandidateStatus",
               rp.first_name, rp.last_name, rp.department,
               rp.avg_rating, rp.avg_difficulty, rp.num_ratings, rp.would_take_again_pct,
               mc.review_subjects, mc.review_years,
               (SELECT i2.display_name
                  FROM instructor_rmp_links l2
                  JOIN instructors i2 ON i2.id = l2.instructor_id
                 WHERE l2.rmp_legacy_id = mc.rmp_legacy_id
                   AND l2.instructor_id <> $1) AS claimed_by
        FROM rmp_match_candidates mc
        JOIN rmp_professors rp ON rp.legacy_id = mc.rmp_legacy_id
        WHERE mc.instructor_id = $1
        ORDER BY (mc.status = 'accepted') DESC, mc.score DESC
        "#,
        inst_id
    )
    .fetch_all(pool)
    .await
    .context("failed to fetch candidates")?
    .into_iter()
    .map(|r| CandidateResponse {
        id: r.id,
        rmp_legacy_id: r.rmp_legacy_id,
        first_name: r.first_name,
        last_name: r.last_name,
        department: r.department,
        avg_rating: r.avg_rating,
        avg_difficulty: r.avg_difficulty,
        num_ratings: r.num_ratings,
        would_take_again_pct: r.would_take_again_pct,
        score: r.score,
        score_breakdown: r.score_breakdown,
        status: r.status,
        review_subjects: r.review_subjects,
        review_years: r.review_years,
        claimed_by: r.claimed_by,
        blocked_reason: None,
    })
    .collect::<Vec<_>>();

    let current_matches = sqlx::query_as!(
        LinkedRmpProfile,
        r#"
        SELECT irl.id as link_id,
               rp.legacy_id, rp.first_name, rp.last_name, rp.department,
               rp.avg_rating, rp.avg_difficulty, rp.num_ratings, rp.would_take_again_pct,
               COALESCE((
                   SELECT ARRAY_AGG(DISTINCT subj ORDER BY subj)
                   FROM (
                       SELECT UPPER(regexp_replace(r.class, '[^A-Za-z].*$', '')) as subj
                       FROM rmp_reviews r
                       WHERE r.rmp_legacy_id = rp.legacy_id AND r.class IS NOT NULL
                   ) sub
                   WHERE subj != ''
               ), '{}') as "review_subjects!",
               COALESCE((
                   SELECT ARRAY_AGG(DISTINCT EXTRACT(YEAR FROM r.posted_at)::SMALLINT ORDER BY EXTRACT(YEAR FROM r.posted_at)::SMALLINT)
                   FROM rmp_reviews r
                   WHERE r.rmp_legacy_id = rp.legacy_id AND r.posted_at IS NOT NULL
               ), '{}') as "review_years!"
        FROM instructor_rmp_links irl
        JOIN rmp_professors rp ON rp.legacy_id = irl.rmp_legacy_id
        WHERE irl.instructor_id = $1
        ORDER BY rp.num_ratings DESC NULLS LAST
        "#,
        inst_id
    )
    .fetch_all(pool)
    .await
    .context("failed to fetch linked rmp profiles")?;

    Ok(InstructorDetailResponse {
        instructor: InstructorDetail {
            id: inst_id,
            display_name: instructor.display_name,
            email: instructor.email,
            rmp_match_status: instructor.status,
            subjects_taught: subjects,
            course_count,
            teaching_years,
        },
        current_matches,
        candidates,
    })
}

/// Accept a candidate match for an instructor.
///
/// Fails with [`AdminRmpError::AlreadyLinked`] if the RMP profile already belongs
/// to a different instructor.
pub async fn accept_candidate(pool: &PgPool, instructor_id: i32, rmp_legacy_id: i32, resolved_by: i64) -> Result<()> {
    // Verify the candidate exists and is pending
    let candidate = sqlx::query_scalar!(
        "SELECT id FROM rmp_match_candidates WHERE instructor_id = $1 AND rmp_legacy_id = $2 AND status = 'pending'",
        instructor_id,
        rmp_legacy_id,
    )
    .fetch_optional(pool)
    .await
    .context("failed to check candidate")?;

    if candidate.is_none() {
        return Err(AdminRmpError::NoPendingCandidate.into());
    }

    // Check if this RMP profile is already linked to a different instructor
    let conflict = sqlx::query!(
        "SELECT i.id, i.display_name, i.email \
         FROM instructor_rmp_links l \
         JOIN instructors i ON i.id = l.instructor_id \
         WHERE l.rmp_legacy_id = $1 AND l.instructor_id != $2",
        rmp_legacy_id,
        instructor_id,
    )
    .fetch_optional(pool)
    .await
    .context("failed to check rmp uniqueness")?;

    if let Some(holder) = conflict {
        // Reaching this point means a candidate was offered that could never be
        // accepted, so the queue showed the reviewer a dead end.
        warn!(
            instructor_id,
            rmp_legacy_id,
            holder_id = holder.id,
            "Unacceptable RMP candidate was offered for review"
        );
        return Err(AdminRmpError::AlreadyLinked {
            instructor_id: holder.id,
            display_name: holder.display_name,
            email: holder.email,
        }
        .into());
    }

    let mut tx = pool.begin().await.context("failed to begin transaction")?;

    sqlx::query!(
        "INSERT INTO instructor_rmp_links (instructor_id, rmp_legacy_id, created_by, source) VALUES ($1, $2, $3, 'manual') ON CONFLICT (rmp_legacy_id) DO NOTHING",
        instructor_id,
        rmp_legacy_id,
        resolved_by,
    )
    .execute(&mut *tx)
    .await
    .context("failed to insert rmp link")?;

    sqlx::query!(
        "UPDATE rmp_match_candidates SET status = 'accepted', resolved_at = NOW(), resolved_by = $1 WHERE instructor_id = $2 AND rmp_legacy_id = $3",
        resolved_by,
        instructor_id,
        rmp_legacy_id,
    )
    .execute(&mut *tx)
    .await
    .context("failed to accept candidate")?;

    tx.commit().await.context("failed to commit transaction")?;
    crate::data::rmp::refresh_rmp_summary(pool)
        .await
        .context("failed to refresh rmp summary")?;

    Ok(())
}

/// Reject a single candidate for an instructor.
///
/// Returns `true` if a candidate was rejected, `false` if no pending candidate was found.
pub async fn reject_candidate(pool: &PgPool, instructor_id: i32, rmp_legacy_id: i32, resolved_by: i64) -> Result<bool> {
    let mut tx = pool.begin().await.context("failed to begin rejection")?;

    let result = sqlx::query!(
        "UPDATE rmp_match_candidates SET status = 'rejected', resolved_at = NOW(), resolved_by = $1 WHERE instructor_id = $2 AND rmp_legacy_id = $3 AND status = 'pending'",
        resolved_by,
        instructor_id,
        rmp_legacy_id,
    )
    .execute(&mut *tx)
    .await
    .context("failed to reject candidate")?;

    tx.commit().await.context("failed to commit rejection")?;

    Ok(result.rows_affected() > 0)
}

/// Reject all pending candidates for an instructor and mark them as having no valid match.
///
/// Returns an error if the instructor has confirmed matches (must unmatch first).
pub async fn reject_all_candidates(pool: &PgPool, instructor_id: i32, resolved_by: i64) -> Result<()> {
    let mut tx = pool.begin().await.context("failed to begin transaction")?;

    // The view reads status through a CASE expression, so sqlx calls it nullable.
    let current_status = sqlx::query_scalar!(
        r#"SELECT status AS "status!: RmpMatchStatus" FROM instructor_rmp_match_status WHERE instructor_id = $1"#,
        instructor_id
    )
    .fetch_optional(&mut *tx)
    .await
    .context("failed to fetch instructor status")?;

    let status = current_status.ok_or(AdminRmpError::NoSuchInstructor)?;

    if status == RmpMatchStatus::Confirmed {
        return Err(AdminRmpError::ConfirmedMatches.into());
    }

    sqlx::query!(
        "UPDATE rmp_match_candidates SET status = 'rejected', resolved_at = NOW(), resolved_by = $1 WHERE instructor_id = $2 AND status = 'pending'",
        resolved_by,
        instructor_id,
    )
    .execute(&mut *tx)
    .await
    .context("failed to reject candidates")?;

    tx.commit().await.context("failed to commit transaction")?;

    Ok(())
}

/// Check if an instructor exists.
pub async fn instructor_exists(pool: &PgPool, id: i32) -> Result<bool> {
    let exists = sqlx::query_scalar!("SELECT id FROM instructors WHERE id = $1", id)
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
        total_processed: stats.total_processed,
        deleted_pending_candidates: stats.deleted_pending_candidates,
        deleted_auto_links: stats.deleted_auto_links,
        candidates_created: stats.candidates_created,
        auto_matched: stats.auto_matched,
        pending_review: stats.pending_review,
        skipped_unparseable: stats.skipped_unparseable,
        skipped_no_candidates: stats.skipped_no_candidates,
    })
}
