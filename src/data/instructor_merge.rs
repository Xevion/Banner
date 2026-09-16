//! Detection and merging of duplicate instructor records.
//!
//! UTSA issues people both a student and a staff address, and the scrape keys
//! instructors on the exact email, so one person can occupy several rows.

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;
use tracing::{debug, info};
use ts_rs::TS;

/// How much evidence there is that two records are the same person.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum DuplicateTier {
    /// One UTSA account reached through both its student and staff domain.
    SameAccount,
    /// One record carries no email, so only the name ties them together.
    MissingEmail,
    /// Distinct local parts, which usually means distinct people.
    DifferentAccount,
}

impl DuplicateTier {
    /// Whether this tier may merge without a human confirming it.
    pub fn is_auto_mergeable(self) -> bool {
        matches!(self, Self::SameAccount)
    }
}

/// One side of a duplicate pair.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DuplicateSide {
    pub id: i32,
    pub display_name: String,
    pub email: Option<String>,
    pub course_count: i64,
    pub subjects: Vec<String>,
    pub rmp_legacy_ids: Vec<i32>,
    pub match_status: String,
}

/// Two instructor records that appear to describe the same person.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DuplicatePair {
    pub tier: DuplicateTier,
    pub survivor: DuplicateSide,
    pub loser: DuplicateSide,
    /// Both records teach at least one subject in common.
    pub subjects_overlap: bool,
}

/// Outcome of an automatic merge sweep.
#[derive(Debug, Default, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MergeStats {
    pub merged: usize,
    pub skipped: usize,
}

/// Reduce a UTSA address to the account it identifies.
///
/// `lilian.cano@my.utsa.edu` and `lilian.cano@utsa.edu` are one account, so the
/// `my.` prefix is dropped before comparison.
pub fn canonical_email(email: &str) -> String {
    let lowered = email.trim().to_lowercase();
    let Some((local, domain)) = lowered.split_once('@') else {
        return lowered;
    };
    format!("{local}@{}", domain.strip_prefix("my.").unwrap_or(domain))
}

/// Rank of a match status, so a merge keeps the most deliberate one.
fn status_rank(status: &str) -> u8 {
    match status {
        "confirmed" => 4,
        "auto" => 3,
        "pending" => 2,
        _ => 1,
    }
}

#[derive(sqlx::FromRow)]
struct InstructorRow {
    id: i32,
    display_name: String,
    email: Option<String>,
    rmp_match_status: String,
    course_count: i64,
    subjects: Vec<String>,
    rmp_legacy_ids: Vec<i32>,
}

impl From<&InstructorRow> for DuplicateSide {
    fn from(row: &InstructorRow) -> Self {
        Self {
            id: row.id,
            display_name: row.display_name.clone(),
            email: row.email.clone(),
            course_count: row.course_count,
            subjects: row.subjects.clone(),
            rmp_legacy_ids: row.rmp_legacy_ids.clone(),
            match_status: row.rmp_match_status.clone(),
        }
    }
}

/// Which of two records should absorb the other.
///
/// Prefers the one that actually teaches, then the staff address over the
/// student one, then the older row.
fn pick_survivor<'a>(
    a: &'a InstructorRow,
    b: &'a InstructorRow,
) -> (&'a InstructorRow, &'a InstructorRow) {
    let is_staff = |row: &InstructorRow| {
        row.email
            .as_deref()
            .is_some_and(|e| !e.to_lowercase().contains("@my."))
    };
    let key = |row: &InstructorRow| (row.course_count, is_staff(row), -row.id);
    if key(a) >= key(b) { (a, b) } else { (b, a) }
}

/// Classify a pair by how strongly the two records are tied together.
fn classify(a: &InstructorRow, b: &InstructorRow) -> DuplicateTier {
    match (a.email.as_deref(), b.email.as_deref()) {
        (Some(x), Some(y)) if canonical_email(x) == canonical_email(y) => {
            DuplicateTier::SameAccount
        }
        (None, _) | (_, None) => DuplicateTier::MissingEmail,
        _ => DuplicateTier::DifferentAccount,
    }
}

/// Find instructor records that share a display name, paired and classified.
pub async fn find_duplicate_pairs(pool: &PgPool) -> Result<Vec<DuplicatePair>> {
    let rows: Vec<InstructorRow> = sqlx::query_as(
        r#"
        SELECT i.id, i.display_name, i.email, i.rmp_match_status,
               COALESCE(ci.course_count, 0) AS course_count,
               COALESCE(ci.subjects, '{}') AS subjects,
               COALESCE(rl.legacy_ids, '{}') AS rmp_legacy_ids
        FROM instructors i
        LEFT JOIN LATERAL (
            SELECT COUNT(*) AS course_count,
                   ARRAY_AGG(DISTINCT c.subject) AS subjects
            FROM course_instructors x
            JOIN courses c ON c.id = x.course_id
            WHERE x.instructor_id = i.id
        ) ci ON TRUE
        LEFT JOIN LATERAL (
            SELECT ARRAY_AGG(l.rmp_legacy_id ORDER BY l.rmp_legacy_id) AS legacy_ids
            FROM instructor_rmp_links l
            WHERE l.instructor_id = i.id
        ) rl ON TRUE
        WHERE i.display_name IN (
            SELECT display_name FROM instructors GROUP BY display_name HAVING COUNT(*) > 1
        )
        ORDER BY i.display_name, i.id
        "#,
    )
    .fetch_all(pool)
    .await
    .context("failed to fetch duplicate instructor groups")?;

    let mut by_name: HashMap<&str, Vec<&InstructorRow>> = HashMap::new();
    for row in &rows {
        by_name.entry(&row.display_name).or_default().push(row);
    }

    let mut pairs = Vec::new();
    for group in by_name.values() {
        for (i, a) in group.iter().enumerate() {
            for b in &group[i + 1..] {
                let (survivor, loser) = pick_survivor(a, b);
                let subjects_overlap = survivor.subjects.iter().any(|s| loser.subjects.contains(s));
                pairs.push(DuplicatePair {
                    tier: classify(a, b),
                    survivor: survivor.into(),
                    loser: loser.into(),
                    subjects_overlap,
                });
            }
        }
    }

    pairs.sort_by(|x, y| {
        x.survivor
            .display_name
            .cmp(&y.survivor.display_name)
            .then(x.survivor.id.cmp(&y.survivor.id))
    });
    Ok(pairs)
}

/// Fold `loser_id` into `survivor_id` and delete the loser.
///
/// Every dependent row moves across; rows that would collide with one the
/// survivor already has are dropped rather than duplicated.
pub async fn merge_instructors(pool: &PgPool, survivor_id: i32, loser_id: i32) -> Result<()> {
    if survivor_id == loser_id {
        return Err(anyhow!("cannot merge an instructor into itself"));
    }

    let mut tx = pool.begin().await.context("failed to begin merge")?;

    let sides: Vec<(i32, String, Option<String>)> =
        sqlx::query_as("SELECT id, rmp_match_status, email FROM instructors WHERE id = ANY($1)")
            .bind(vec![survivor_id, loser_id])
            .fetch_all(&mut *tx)
            .await
            .context("failed to load merge participants")?;

    if sides.len() != 2 {
        return Err(anyhow!("both instructors must exist to merge"));
    }

    // Course links are keyed on (course_id, instructor_id); both records can
    // hold the same section, so move what is new and discard the rest.
    sqlx::query(
        "UPDATE course_instructors SET instructor_id = $1 \
         WHERE instructor_id = $2 \
           AND course_id NOT IN (SELECT course_id FROM course_instructors WHERE instructor_id = $1)",
    )
    .bind(survivor_id)
    .bind(loser_id)
    .execute(&mut *tx)
    .await
    .context("failed to move course links")?;

    sqlx::query("DELETE FROM course_instructors WHERE instructor_id = $1")
        .bind(loser_id)
        .execute(&mut *tx)
        .await
        .context("failed to drop leftover course links")?;

    // A single RMP profile is globally unique to one instructor, so these can
    // move wholesale; the summary view aggregates several profiles per person.
    for stmt in [
        "UPDATE instructor_rmp_links SET instructor_id = $1 WHERE instructor_id = $2",
        "UPDATE instructor_bluebook_links SET instructor_id = $1 WHERE instructor_id = $2",
        "UPDATE bluebook_evaluations SET instructor_id = $1 WHERE instructor_id = $2",
    ] {
        sqlx::query(stmt)
            .bind(survivor_id)
            .bind(loser_id)
            .execute(&mut *tx)
            .await
            .context("failed to move instructor links")?;
    }

    sqlx::query(
        "UPDATE rmp_match_candidates SET instructor_id = $1 \
         WHERE instructor_id = $2 \
           AND rmp_legacy_id NOT IN \
               (SELECT rmp_legacy_id FROM rmp_match_candidates WHERE instructor_id = $1)",
    )
    .bind(survivor_id)
    .bind(loser_id)
    .execute(&mut *tx)
    .await
    .context("failed to move match candidates")?;

    for stmt in [
        "DELETE FROM rmp_match_candidates WHERE instructor_id = $1",
        "DELETE FROM instructor_scores WHERE instructor_id = $1",
    ] {
        sqlx::query(stmt)
            .bind(loser_id)
            .execute(&mut *tx)
            .await
            .context("failed to clear loser rows")?;
    }

    let survivor = sides.iter().find(|(id, _, _)| *id == survivor_id);
    let loser = sides.iter().find(|(id, _, _)| *id == loser_id);
    if let (Some((_, s_status, s_email)), Some((_, l_status, l_email))) = (survivor, loser) {
        let status = if status_rank(l_status) > status_rank(s_status) {
            l_status
        } else {
            s_status
        };
        // Keep an address if the survivor lacked one.
        let email = s_email.clone().or_else(|| l_email.clone());

        sqlx::query("UPDATE instructors SET rmp_match_status = $1, email = $2 WHERE id = $3")
            .bind(status)
            .bind(email)
            .bind(survivor_id)
            .execute(&mut *tx)
            .await
            .context("failed to update survivor")?;
    }

    sqlx::query("DELETE FROM instructors WHERE id = $1")
        .bind(loser_id)
        .execute(&mut *tx)
        .await
        .context("failed to delete merged instructor")?;

    tx.commit().await.context("failed to commit merge")?;

    info!(survivor_id, loser_id, "Merged duplicate instructor records");
    Ok(())
}

/// Merge an instructor into whichever record already holds the RMP profile it
/// is competing for.
///
/// Returns the surviving and absorbed ids. Refuses unless the two records carry
/// the same name, so this can only resolve a duplicate, never fuse two people.
pub async fn merge_with_claimant(
    pool: &PgPool,
    instructor_id: i32,
    rmp_legacy_id: i32,
) -> Result<(i32, i32)> {
    let claimant: Option<(i32,)> = sqlx::query_as(
        "SELECT instructor_id FROM instructor_rmp_links \
         WHERE rmp_legacy_id = $1 AND instructor_id <> $2",
    )
    .bind(rmp_legacy_id)
    .bind(instructor_id)
    .fetch_optional(pool)
    .await
    .context("failed to find the claiming instructor")?;

    let Some((claimant_id,)) = claimant else {
        return Err(anyhow!("no other instructor holds this RMP profile"));
    };

    let names: Vec<(i32, String, Option<String>, i64)> = sqlx::query_as(
        "SELECT i.id, i.display_name, i.email, \
                (SELECT COUNT(*) FROM course_instructors ci WHERE ci.instructor_id = i.id) \
         FROM instructors i WHERE i.id = ANY($1)",
    )
    .bind(vec![instructor_id, claimant_id])
    .fetch_all(pool)
    .await
    .context("failed to load merge participants")?;

    if names.len() != 2 {
        return Err(anyhow!("both instructors must exist to merge"));
    }
    if names[0].1 != names[1].1 {
        return Err(anyhow!(
            "records name different people; merge them manually if they are the same"
        ));
    }

    let staff = |email: &Option<String>| {
        email
            .as_deref()
            .is_some_and(|e| !e.to_lowercase().contains("@my."))
    };
    let key = |r: &(i32, String, Option<String>, i64)| (r.3, staff(&r.2), -r.0);
    let (survivor, loser) = if key(&names[0]) >= key(&names[1]) {
        (names[0].0, names[1].0)
    } else {
        (names[1].0, names[0].0)
    };

    merge_instructors(pool, survivor, loser).await?;
    Ok((survivor, loser))
}

/// Merge every pair whose tier needs no human confirmation.
pub async fn auto_merge_duplicates(pool: &PgPool) -> Result<MergeStats> {
    let pairs = find_duplicate_pairs(pool).await?;
    let mut stats = MergeStats::default();

    for pair in pairs {
        if !pair.tier.is_auto_mergeable() {
            stats.skipped += 1;
            continue;
        }
        match merge_instructors(pool, pair.survivor.id, pair.loser.id).await {
            Ok(()) => stats.merged += 1,
            Err(e) => {
                stats.skipped += 1;
                debug!(
                    survivor = pair.survivor.id,
                    loser = pair.loser.id,
                    error = %e,
                    "Skipped duplicate merge"
                );
            }
        }
    }

    info!(
        merged = stats.merged,
        skipped = stats.skipped,
        "Duplicate instructor merge complete"
    );
    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canonical_email_strips_student_domain() {
        assert_eq!(
            canonical_email("First.Last@my.utsa.edu"),
            "first.last@utsa.edu"
        );
        assert_eq!(
            canonical_email("first.last@utsa.edu"),
            "first.last@utsa.edu"
        );
    }

    #[test]
    fn test_canonical_email_leaves_other_domains_alone() {
        assert_eq!(canonical_email("someone@gmail.com"), "someone@gmail.com");
        assert_eq!(canonical_email("not-an-email"), "not-an-email");
    }

    #[test]
    fn test_canonical_email_distinguishes_different_accounts() {
        assert_ne!(
            canonical_email("first.last2@utsa.edu"),
            canonical_email("first.last@utsa.edu")
        );
        assert_ne!(
            canonical_email("abc123@my.utsa.edu"),
            canonical_email("first.last@utsa.edu")
        );
    }

    #[test]
    fn test_only_same_account_merges_without_review() {
        assert!(DuplicateTier::SameAccount.is_auto_mergeable());
        assert!(!DuplicateTier::MissingEmail.is_auto_mergeable());
        assert!(!DuplicateTier::DifferentAccount.is_auto_mergeable());
    }

    #[test]
    fn test_status_rank_prefers_human_decisions() {
        assert!(status_rank("confirmed") > status_rank("auto"));
        assert!(status_rank("auto") > status_rank("pending"));
        assert!(status_rank("pending") > status_rank("unmatched"));
    }
}
