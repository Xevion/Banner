//! Data-layer operations for BlueBook instructor linking admin features.
//!
//! Pure data functions returning `anyhow::Result`. The web layer handles HTTP
//! concerns only; all SQL lives here.

use anyhow::{Context, Result};
use serde::Serialize;
use sqlx::PgPool;
use tracing::info;
use ts_rs::TS;

use crate::data::names::{MatchCandidate, NameMatchQuality, find_best_candidate};

/// Domain errors for BlueBook link operations.
///
/// The web layer downcasts `anyhow::Error` to this type to decide HTTP status codes
/// instead of fragile string matching.
#[derive(Debug, thiserror::Error)]
pub enum BluebookError {
    #[error("bluebook link not found")]
    NoSuchLink,
    #[error("approvable bluebook link not found (must be auto or pending)")]
    NotApprovable,
    #[error("rejectable bluebook link not found (must be auto or pending)")]
    NotRejectable,
    #[error("instructor not found")]
    NoSuchInstructor,
}

/// A BlueBook link row in the paginated list view.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BluebookLinkListItem {
    pub id: i32,
    pub instructor_name: String,
    pub subject: Option<String>,
    pub status: String,
    pub confidence: Option<f32>,
    pub instructor_id: Option<i32>,
    pub instructor_display_name: Option<String>,
    pub eval_count: i32,
}

/// Aggregate status counts for the link list.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BluebookLinkStats {
    pub total: i32,
    pub auto: i32,
    pub pending: i32,
    pub approved: i32,
    pub rejected: i32,
}

/// Response for the paginated link list.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ListBluebookLinksResponse {
    pub links: Vec<BluebookLinkListItem>,
    pub total: i32,
    pub page: i32,
    pub per_page: i32,
    pub stats: BluebookLinkStats,
}

/// Detail view for a single BlueBook link.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BluebookLinkDetail {
    pub id: i32,
    pub instructor_name: String,
    pub subject: Option<String>,
    pub status: String,
    pub confidence: Option<f32>,
    pub instructor_id: Option<i32>,
    pub instructor_display_name: Option<String>,
    pub eval_count: i32,
    pub courses: Vec<BluebookLinkCourse>,
}

/// A course associated with a BlueBook link (via evaluations).
#[derive(Debug, Clone, Serialize, sqlx::FromRow, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BluebookLinkCourse {
    pub subject: String,
    pub course_number: String,
    pub term: String,
    pub instructor_rating: Option<f32>,
    pub course_rating: Option<f32>,
}

/// Response for the auto-matching pipeline.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BluebookMatchResponse {
    /// Names processed in this run.
    pub total_names: usize,
    /// High-confidence matches (status = 'auto').
    pub auto_matched: usize,
    /// Lower-confidence matches needing review (status = 'pending').
    pub pending_review: usize,
    /// No match found at all (status = 'pending', no instructor_id).
    pub no_match: usize,
    /// Names with approved/rejected links that were left untouched.
    pub skipped_manual: usize,
    /// Stale auto/pending links deleted before re-matching.
    pub deleted_stale: usize,
}

/// Internal row type for the link list query.
#[derive(sqlx::FromRow)]
struct LinkListRow {
    id: i32,
    instructor_name: String,
    subject: Option<String>,
    status: String,
    confidence: Option<f32>,
    instructor_id: Option<i32>,
    instructor_display_name: Option<String>,
    eval_count: Option<i64>,
}

#[derive(sqlx::FromRow)]
struct StatusCount {
    status: String,
    count: i64,
}

/// Escape LIKE/ILIKE metacharacters so user input is treated as literal text.
fn escape_like(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

/// Filter/sort/pagination params for listing links.
pub struct ListBluebookLinksFilter {
    pub status: Option<String>,
    pub search: Option<String>,
    pub page: i32,
    pub per_page: i32,
}

/// List BlueBook links with filtering and pagination.
pub async fn list_links(
    pool: &PgPool,
    filter: &ListBluebookLinksFilter,
) -> Result<ListBluebookLinksResponse> {
    let page = filter.page.max(1);
    let per_page = filter.per_page.clamp(1, 100);
    let offset = (page - 1) * per_page;

    let mut conditions = Vec::new();
    let mut bind_idx = 0u32;

    if filter.status.is_some() {
        bind_idx += 1;
        conditions.push(format!("bl.status = ${bind_idx}"));
    }
    if filter.search.is_some() {
        bind_idx += 1;
        conditions.push(format!(
            "(bl.instructor_name ILIKE ${bind_idx} OR i.display_name ILIKE ${bind_idx})"
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
            bl.id,
            bl.instructor_name,
            bl.subject,
            bl.status,
            bl.confidence,
            bl.instructor_id,
            i.display_name AS instructor_display_name,
            (SELECT COUNT(*) FROM bluebook_evaluations be
             WHERE be.instructor_name = bl.instructor_name
               AND (bl.subject IS NULL OR be.subject = bl.subject)
            ) AS eval_count
        FROM instructor_bluebook_links bl
        LEFT JOIN instructors i ON i.id = bl.instructor_id
        {where_clause}
        ORDER BY
            CASE bl.status WHEN 'pending' THEN 0 WHEN 'auto' THEN 1 WHEN 'approved' THEN 2 ELSE 3 END,
            bl.instructor_name ASC
        LIMIT {per_page} OFFSET {offset}
        "#
    );

    let mut query = sqlx::query_as::<_, LinkListRow>(&query_str);
    if let Some(ref status) = filter.status {
        query = query.bind(status);
    }
    if let Some(ref search) = filter.search {
        query = query.bind(format!("%{}%", escape_like(search)));
    }

    let rows = query
        .fetch_all(pool)
        .await
        .context("failed to list bluebook links")?;

    // Count total with filters
    let count_query_str = format!(
        "SELECT COUNT(*) FROM instructor_bluebook_links bl LEFT JOIN instructors i ON i.id = bl.instructor_id {where_clause}"
    );
    let mut count_query = sqlx::query_as::<_, (i64,)>(&count_query_str);
    if let Some(ref status) = filter.status {
        count_query = count_query.bind(status);
    }
    if let Some(ref search) = filter.search {
        count_query = count_query.bind(format!("%{}%", escape_like(search)));
    }

    let (total,) = count_query
        .fetch_one(pool)
        .await
        .context("failed to count bluebook links")?;

    // Aggregate stats (unfiltered)
    let stats_rows = sqlx::query_as::<_, StatusCount>(
        "SELECT status, COUNT(*) AS count FROM instructor_bluebook_links GROUP BY status",
    )
    .fetch_all(pool)
    .await
    .context("failed to get bluebook link stats")?;

    let mut stats = BluebookLinkStats {
        total: 0,
        auto: 0,
        pending: 0,
        approved: 0,
        rejected: 0,
    };
    for row in &stats_rows {
        let count = row.count as i32;
        stats.total += count;
        match row.status.as_str() {
            "auto" => stats.auto = count,
            "pending" => stats.pending = count,
            "approved" => stats.approved = count,
            "rejected" => stats.rejected = count,
            _ => {}
        }
    }

    let links = rows
        .into_iter()
        .map(|r| BluebookLinkListItem {
            id: r.id,
            instructor_name: r.instructor_name,
            subject: r.subject,
            status: r.status,
            confidence: r.confidence,
            instructor_id: r.instructor_id,
            instructor_display_name: r.instructor_display_name,
            eval_count: r.eval_count.unwrap_or(0) as i32,
        })
        .collect();

    Ok(ListBluebookLinksResponse {
        links,
        total: total as i32,
        page,
        per_page,
        stats,
    })
}

/// Fetch detail for a single BlueBook link, including associated evaluations.
pub async fn get_link_detail(pool: &PgPool, link_id: i32) -> Result<BluebookLinkDetail> {
    let row: Option<LinkListRow> = sqlx::query_as(
        r#"
        SELECT
            bl.id,
            bl.instructor_name,
            bl.subject,
            bl.status,
            bl.confidence,
            bl.instructor_id,
            i.display_name AS instructor_display_name,
            (SELECT COUNT(*) FROM bluebook_evaluations be
             WHERE be.instructor_name = bl.instructor_name
               AND (bl.subject IS NULL OR be.subject = bl.subject)
            ) AS eval_count
        FROM instructor_bluebook_links bl
        LEFT JOIN instructors i ON i.id = bl.instructor_id
        WHERE bl.id = $1
        "#,
    )
    .bind(link_id)
    .fetch_optional(pool)
    .await
    .context("failed to fetch bluebook link")?;

    let r = row.ok_or(BluebookError::NoSuchLink)?;

    let courses = sqlx::query_as::<_, BluebookLinkCourse>(
        r#"
        SELECT DISTINCT be.subject, be.course_number, be.term,
               be.instructor_rating, be.course_rating
        FROM bluebook_evaluations be
        WHERE be.instructor_name = $1
          AND ($2::varchar IS NULL OR be.subject = $2)
        ORDER BY be.term DESC, be.subject, be.course_number
        "#,
    )
    .bind(&r.instructor_name)
    .bind(&r.subject)
    .fetch_all(pool)
    .await
    .context("failed to fetch bluebook link courses")?;

    Ok(BluebookLinkDetail {
        id: r.id,
        instructor_name: r.instructor_name,
        subject: r.subject,
        status: r.status,
        confidence: r.confidence,
        instructor_id: r.instructor_id,
        instructor_display_name: r.instructor_display_name,
        eval_count: r.eval_count.unwrap_or(0) as i32,
        courses,
    })
}

/// Approve an auto or pending BlueBook link.
pub async fn approve_link(pool: &PgPool, link_id: i32) -> Result<()> {
    let result = sqlx::query(
        "UPDATE instructor_bluebook_links SET status = 'approved', updated_at = NOW() WHERE id = $1 AND status IN ('auto', 'pending')",
    )
    .bind(link_id)
    .execute(pool)
    .await
    .context("failed to approve bluebook link")?;

    if result.rows_affected() == 0 {
        return Err(BluebookError::NotApprovable.into());
    }

    Ok(())
}

/// Reject an auto or pending BlueBook link.
pub async fn reject_link(pool: &PgPool, link_id: i32) -> Result<()> {
    let result = sqlx::query(
        "UPDATE instructor_bluebook_links SET status = 'rejected', updated_at = NOW() WHERE id = $1 AND status IN ('auto', 'pending')",
    )
    .bind(link_id)
    .execute(pool)
    .await
    .context("failed to reject bluebook link")?;

    if result.rows_affected() == 0 {
        return Err(BluebookError::NotRejectable.into());
    }

    Ok(())
}

/// Manually assign an instructor to a BlueBook link and approve it.
pub async fn assign_link(pool: &PgPool, link_id: i32, instructor_id: i32) -> Result<()> {
    // Verify instructor exists
    let exists: Option<(i32,)> = sqlx::query_as("SELECT id FROM instructors WHERE id = $1")
        .bind(instructor_id)
        .fetch_optional(pool)
        .await
        .context("failed to check instructor")?;

    if exists.is_none() {
        return Err(BluebookError::NoSuchInstructor.into());
    }

    let result = sqlx::query(
        r#"
        UPDATE instructor_bluebook_links
        SET instructor_id = $1,
            status = 'approved',
            updated_at = NOW()
        WHERE id = $2
          AND status IN ('auto', 'pending')
        "#,
    )
    .bind(instructor_id)
    .bind(link_id)
    .execute(pool)
    .await
    .context("failed to assign bluebook link")?;

    if result.rows_affected() == 0 {
        return Err(BluebookError::NoSuchLink.into());
    }

    Ok(())
}

/// Distinct instructor name from `bluebook_evaluations` not yet in the links table.
#[derive(sqlx::FromRow)]
struct UnlinkedName {
    instructor_name: String,
}

/// A candidate instructor found via CRN+term join.
#[derive(sqlx::FromRow)]
struct CrnCandidate {
    instructor_id: i32,
    display_name: String,
}

/// Idempotently refresh BlueBook instructor name matches.
///
/// Runs inside a transaction to prevent data loss if the process crashes mid-way.
/// Deletes all `auto` and `pending` links (algorithm-generated), then re-runs
/// the matching pipeline for every distinct `instructor_name` in
/// `bluebook_evaluations` that doesn't have an `approved` or `rejected` link.
///
/// Manual decisions (`approved`, `rejected`) are never touched.
///
/// Matching strategy per name:
///
/// 1. **CRN+term join**: Find evals with non-null `crn`, join to `courses`
///    via `(crn, term)`, then to `course_instructors` → `instructors`.
/// 2. **Name confirmation**: Compare via structured name parsing and matching keys.
/// 3. Insert into `instructor_bluebook_links` with appropriate status/confidence.
///
/// The matching uses [`find_best_candidate`] from the `names` module — pure
/// functions with no database dependency.
pub async fn run_auto_matching(pool: &PgPool) -> Result<BluebookMatchResponse> {
    let mut tx = pool.begin().await.context("failed to start transaction")?;

    // Step 0: Delete all algorithm-generated links so we can regenerate them.
    let deleted =
        sqlx::query("DELETE FROM instructor_bluebook_links WHERE status IN ('auto', 'pending')")
            .execute(&mut *tx)
            .await
            .context("failed to delete stale auto/pending links")?;
    let deleted_stale = deleted.rows_affected() as usize;

    // Count names with manual decisions that we'll skip.
    let (skipped_manual_count,): (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(DISTINCT be.instructor_name)
        FROM bluebook_evaluations be
        WHERE EXISTS (
            SELECT 1 FROM instructor_bluebook_links ibl
            WHERE ibl.instructor_name = be.instructor_name
              AND ibl.status IN ('approved', 'rejected')
        )
        "#,
    )
    .fetch_one(&mut *tx)
    .await
    .context("failed to count manually-decided links")?;

    // Fetch all names that need matching (no approved/rejected link exists).
    let unlinked: Vec<UnlinkedName> = sqlx::query_as(
        r#"
        SELECT DISTINCT be.instructor_name
        FROM bluebook_evaluations be
        WHERE NOT EXISTS (
            SELECT 1 FROM instructor_bluebook_links ibl
            WHERE ibl.instructor_name = be.instructor_name
        )
        "#,
    )
    .fetch_all(&mut *tx)
    .await
    .context("failed to fetch unlinked bluebook names")?;

    // Pre-fetch all instructors once for name-only fallback matching (avoids N+1).
    let all_instructors: Vec<(i32, String)> =
        sqlx::query_as("SELECT id, display_name FROM instructors")
            .fetch_all(&mut *tx)
            .await
            .context("failed to fetch instructors for name matching")?;

    let all_match_candidates: Vec<MatchCandidate> = all_instructors
        .into_iter()
        .map(|(id, dn)| MatchCandidate {
            instructor_id: id,
            display_name: dn,
        })
        .collect();

    let total_names = unlinked.len();
    let skipped_manual = skipped_manual_count as usize;
    let mut auto_matched = 0usize;
    let mut pending_review = 0usize;
    let mut no_match = 0usize;

    for row in &unlinked {
        let name = &row.instructor_name;

        // Step 1: CRN+term join — find instructor candidates via course matching
        let crn_candidates: Vec<CrnCandidate> = sqlx::query_as(
            r#"
            SELECT DISTINCT i.id AS instructor_id, i.display_name
            FROM bluebook_evaluations be
            JOIN courses c ON c.crn = be.crn AND c.term_code = be.term
            JOIN course_instructors ci ON ci.course_id = c.id
            JOIN instructors i ON i.id = ci.instructor_id
            WHERE be.instructor_name = $1
              AND be.crn IS NOT NULL
              AND be.crn != ''
            "#,
        )
        .bind(name)
        .fetch_all(&mut *tx)
        .await
        .context("failed to find CRN candidates")?;

        if !crn_candidates.is_empty() {
            // Step 2: Confirm name match among CRN candidates
            let match_candidates: Vec<MatchCandidate> = crn_candidates
                .iter()
                .map(|c| MatchCandidate {
                    instructor_id: c.instructor_id,
                    display_name: c.display_name.clone(),
                })
                .collect();

            let has_single_crn = crn_candidates.len() == 1;

            match find_best_candidate(name, &match_candidates) {
                Some(best) => {
                    // CRN evidence + name confirmation → auto
                    let confidence = match best.result.quality {
                        NameMatchQuality::Full => best.result.confidence,
                        NameMatchQuality::Partial if has_single_crn => 0.9 * best.result.confidence,
                        NameMatchQuality::Partial => 0.8 * best.result.confidence,
                        NameMatchQuality::None => unreachable!("find_best_candidate filters None"),
                    };
                    insert_link(
                        &mut *tx,
                        name,
                        Some(best.instructor_id),
                        "auto",
                        Some(confidence),
                    )
                    .await?;
                    auto_matched += 1;
                }
                None => {
                    // CRN candidates exist but no name match — pending review
                    insert_link(&mut *tx, name, None, "pending", Some(0.1)).await?;
                    pending_review += 1;
                }
            }
        } else {
            // No CRN join — try name-only matching against pre-fetched instructors
            match find_best_candidate(name, &all_match_candidates) {
                Some(best) if best.result.quality == NameMatchQuality::Full => {
                    // Exact name match but no CRN confirmation — pending
                    insert_link(
                        &mut *tx,
                        name,
                        Some(best.instructor_id),
                        "pending",
                        Some(0.5),
                    )
                    .await?;
                    pending_review += 1;
                }
                Some(best) => {
                    // Partial name match, no CRN — low confidence pending
                    insert_link(
                        &mut *tx,
                        name,
                        Some(best.instructor_id),
                        "pending",
                        Some(0.3),
                    )
                    .await?;
                    pending_review += 1;
                }
                None => {
                    insert_link(&mut *tx, name, None, "pending", None).await?;
                    no_match += 1;
                }
            }
        }
    }

    tx.commit()
        .await
        .context("failed to commit matching results")?;

    info!(
        total_names,
        auto_matched,
        pending_review,
        no_match,
        deleted_stale,
        skipped_manual,
        "BlueBook auto-matching complete"
    );

    Ok(BluebookMatchResponse {
        total_names,
        auto_matched,
        pending_review,
        no_match,
        skipped_manual,
        deleted_stale,
    })
}

/// Insert a new link into `instructor_bluebook_links`.
///
/// Uses `ON CONFLICT DO NOTHING` to handle race conditions with concurrent matching.
/// Accepts any SQLx executor (pool or transaction).
async fn insert_link(
    executor: impl sqlx::PgExecutor<'_>,
    instructor_name: &str,
    instructor_id: Option<i32>,
    status: &str,
    confidence: Option<f32>,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO instructor_bluebook_links
            (instructor_name, instructor_id, status, confidence)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (instructor_name, COALESCE(subject, '')) DO NOTHING
        "#,
    )
    .bind(instructor_name)
    .bind(instructor_id)
    .bind(status)
    .bind(confidence)
    .execute(executor)
    .await
    .context("failed to insert bluebook link")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_like_no_metacharacters() {
        assert_eq!(escape_like("John Smith"), "John Smith");
    }

    #[test]
    fn test_escape_like_percent() {
        assert_eq!(escape_like("100%"), "100\\%");
    }

    #[test]
    fn test_escape_like_underscore() {
        assert_eq!(escape_like("foo_bar"), "foo\\_bar");
    }

    #[test]
    fn test_escape_like_backslash() {
        assert_eq!(escape_like("path\\to"), "path\\\\to");
    }

    #[test]
    fn test_escape_like_all_metacharacters() {
        assert_eq!(escape_like("%_\\"), "\\%\\_\\\\");
    }

    #[test]
    fn test_escape_like_empty_string() {
        assert_eq!(escape_like(""), "");
    }

    #[test]
    fn test_bluebook_error_downcast() {
        let err: anyhow::Error = BluebookError::NoSuchLink.into();
        assert!(err.downcast_ref::<BluebookError>().is_some());
    }


}
