//! Database operations for term management.
//!
//! Terms represent academic periods (Fall 2024, Spring 2025, etc.) that can be
//! enabled or disabled for scraping. The scheduler queries enabled terms to
//! determine which courses to scrape.

use std::collections::HashSet;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use ts_rs::TS;

use crate::banner::BannerTerm;
use anyhow::Result;

/// A term record from the database, synced from Banner.
///
/// Named `DbTerm` to avoid collision with `crate::banner::models::terms::Term`
/// which represents a parsed term code (year + season).
#[derive(sqlx::FromRow, Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DbTerm {
    /// Term code, e.g., "202510"
    pub code: String,
    /// Description from Banner, e.g., "Fall 2024"
    pub description: String,
    /// Year extracted from code, e.g., 2024
    pub year: i16,
    /// Season name: "Fall", "Spring", or "Summer"
    pub season: String,
    /// Whether the scraper should process this term
    pub scrape_enabled: bool,
    /// Whether Banner marks this as "View Only"
    pub is_archived: bool,
    /// When we first discovered this term
    #[ts(type = "string")]
    pub discovered_at: DateTime<Utc>,
    /// When we last completed a full scrape of this term
    #[ts(type = "string | null")]
    pub last_scraped_at: Option<DateTime<Utc>>,
    /// Record creation timestamp
    #[ts(type = "string")]
    pub created_at: DateTime<Utc>,
    /// Record update timestamp
    #[ts(type = "string")]
    pub updated_at: DateTime<Utc>,
}

/// Result of a term sync operation.
#[derive(Debug, Default)]
pub struct SyncResult {
    /// Number of new terms inserted
    pub inserted: usize,
    /// Number of existing terms updated (metadata only)
    pub updated: usize,
    /// Number of terms skipped due to invalid/unrecognized format
    pub skipped: usize,
}

/// Get all terms, ordered by code descending (newest first).
pub async fn get_all_terms(db_pool: &PgPool) -> Result<Vec<DbTerm>> {
    let terms = sqlx::query_as::<_, DbTerm>("SELECT * FROM terms ORDER BY code DESC")
        .fetch_all(db_pool)
        .await?;

    Ok(terms)
}

/// Get terms with scraping enabled, ordered by code descending.
#[allow(dead_code)] // Used by admin API and future features
pub async fn get_enabled_terms(db_pool: &PgPool) -> Result<Vec<DbTerm>> {
    let terms = sqlx::query_as::<_, DbTerm>(
        "SELECT * FROM terms WHERE scrape_enabled = true ORDER BY code DESC",
    )
    .fetch_all(db_pool)
    .await?;

    Ok(terms)
}

/// A lightweight projection of an enabled term for scheduling decisions.
#[derive(sqlx::FromRow, Debug, Clone)]
pub struct EnabledTerm {
    pub code: String,
    pub is_archived: bool,
}

/// Get enabled terms with their archive status.
///
/// Returns lightweight projections for the scheduler to determine per-term
/// scheduling tiers without fetching full rows.
pub async fn get_enabled_terms_for_scheduling(db_pool: &PgPool) -> Result<Vec<EnabledTerm>> {
    let terms = sqlx::query_as::<_, EnabledTerm>(
        "SELECT code, is_archived FROM terms WHERE scrape_enabled = true ORDER BY code DESC",
    )
    .fetch_all(db_pool)
    .await?;

    Ok(terms)
}

/// Get a single term by code.
pub async fn get_term_by_code(db_pool: &PgPool, code: &str) -> Result<Option<DbTerm>> {
    let term = sqlx::query_as::<_, DbTerm>("SELECT * FROM terms WHERE code = $1")
        .bind(code)
        .fetch_optional(db_pool)
        .await?;

    Ok(term)
}

/// Get all existing term codes (for sync deduplication).
async fn get_existing_term_codes(db_pool: &PgPool) -> Result<HashSet<String>> {
    let codes: Vec<String> = sqlx::query_scalar("SELECT code FROM terms")
        .fetch_all(db_pool)
        .await?;

    Ok(codes.into_iter().collect())
}

/// Enable scraping for a term.
///
/// Returns `true` if the term was found and updated, `false` if not found.
pub async fn enable_scraping(db_pool: &PgPool, code: &str) -> Result<bool> {
    let result =
        sqlx::query("UPDATE terms SET scrape_enabled = true, updated_at = now() WHERE code = $1")
            .bind(code)
            .execute(db_pool)
            .await?;

    Ok(result.rows_affected() > 0)
}

/// Disable scraping for a term.
///
/// Returns `true` if the term was found and updated, `false` if not found.
pub async fn disable_scraping(db_pool: &PgPool, code: &str) -> Result<bool> {
    let result =
        sqlx::query("UPDATE terms SET scrape_enabled = false, updated_at = now() WHERE code = $1")
            .bind(code)
            .execute(db_pool)
            .await?;

    Ok(result.rows_affected() > 0)
}

/// Update the `last_scraped_at` timestamp for a term.
///
/// Called when a subject scrape job completes for this term.
pub async fn update_last_scraped_at(db_pool: &PgPool, code: &str) -> Result<()> {
    sqlx::query("UPDATE terms SET last_scraped_at = now(), updated_at = now() WHERE code = $1")
        .bind(code)
        .execute(db_pool)
        .await?;

    Ok(())
}

/// Parse a 6-digit term code into (year, season).
///
/// Returns `None` for unrecognized formats (e.g., legacy terms with non-standard
/// season codes like "11"). This allows the sync to skip invalid terms gracefully.
///
/// # Examples
/// - "202510" -> Some((2025, "Fall"))
/// - "202520" -> Some((2025, "Spring"))
/// - "202530" -> Some((2025, "Summer"))
/// - "201411" -> None (invalid season code)
fn parse_term_code(code: &str) -> Option<(i16, String)> {
    if code.len() != 6 {
        return None;
    }

    let year = code[0..4].parse::<i16>().ok()?;

    let season = match &code[4..6] {
        "10" => "Fall",
        "20" => "Spring",
        "30" => "Summer",
        _ => return None,
    };

    Some((year, season.to_string()))
}

/// Sync terms from Banner API to database.
///
/// # Rules
/// 1. **New terms**: Only the latest (highest code) gets `scrape_enabled = true`
/// 2. **Existing terms**: Never auto-change `scrape_enabled` - that's admin-controlled
/// 3. **Metadata updates**: Always sync `description`, `is_archived` from Banner
///
/// # Returns
/// A `SyncResult` with counts of inserted and updated terms.
pub async fn sync_terms_from_banner(
    db_pool: &PgPool,
    banner_terms: Vec<BannerTerm>,
) -> Result<SyncResult> {
    if banner_terms.is_empty() {
        return Ok(SyncResult::default());
    }

    let existing_codes = get_existing_term_codes(db_pool).await?;

    // Find the latest term (highest code = most recent)
    let latest_code = banner_terms.iter().map(|t| &t.code).max().cloned();

    let mut result = SyncResult::default();

    for term in &banner_terms {
        let is_archived = term.is_archived();

        // Skip terms with unrecognized format (e.g., legacy terms with non-standard season codes)
        let Some((year, season)) = parse_term_code(&term.code) else {
            tracing::debug!(
                code = %term.code,
                description = %term.description,
                is_archived,
                "Skipping term with unrecognized format"
            );
            result.skipped += 1;
            continue;
        };

        if existing_codes.contains(&term.code) {
            // Update metadata only - DO NOT touch scrape_enabled
            sqlx::query(
                r#"
                UPDATE terms 
                SET description = $2, is_archived = $3, updated_at = now()
                WHERE code = $1
                "#,
            )
            .bind(&term.code)
            .bind(&term.description)
            .bind(is_archived)
            .execute(db_pool)
            .await?;

            result.updated += 1;
        } else {
            // New term - enable scraping ONLY if it's the latest
            let scrape_enabled = Some(&term.code) == latest_code.as_ref();

            sqlx::query(
                r#"
                INSERT INTO terms (code, description, year, season, scrape_enabled, is_archived)
                VALUES ($1, $2, $3, $4, $5, $6)
                "#,
            )
            .bind(&term.code)
            .bind(&term.description)
            .bind(year)
            .bind(&season)
            .bind(scrape_enabled)
            .bind(is_archived)
            .execute(db_pool)
            .await?;

            result.inserted += 1;

            if scrape_enabled {
                tracing::info!(
                    term_code = %term.code,
                    description = %term.description,
                    "New term discovered and enabled for scraping"
                );
            } else {
                tracing::debug!(
                    term_code = %term.code,
                    description = %term.description,
                    "New term discovered (scraping disabled)"
                );
            }
        }
    }

    // Log summary of skipped terms if any
    if result.skipped > 0 {
        tracing::warn!(
            skipped = result.skipped,
            "Skipped terms with unrecognized format codes"
        );
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_term_code_fall() {
        let (year, season) = parse_term_code("202510").unwrap();
        assert_eq!(year, 2025);
        assert_eq!(season, "Fall");
    }

    #[test]
    fn test_parse_term_code_spring() {
        let (year, season) = parse_term_code("202520").unwrap();
        assert_eq!(year, 2025);
        assert_eq!(season, "Spring");
    }

    #[test]
    fn test_parse_term_code_summer() {
        let (year, season) = parse_term_code("202530").unwrap();
        assert_eq!(year, 2025);
        assert_eq!(season, "Summer");
    }

    #[test]
    fn test_parse_term_code_invalid_length() {
        assert!(parse_term_code("20251").is_none());
        assert!(parse_term_code("2025100").is_none());
    }

    #[test]
    fn test_parse_term_code_invalid_season() {
        assert!(parse_term_code("202540").is_none());
        assert!(parse_term_code("202500").is_none());
        assert!(parse_term_code("201411").is_none()); // Legacy term format
    }
}
