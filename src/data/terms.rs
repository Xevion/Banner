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
use crate::banner::models::terms::Season;
use anyhow::{Context, Result};

/// A term record from the database, synced from Banner.
///
/// Named `DbTerm` to avoid collision with `crate::banner::models::terms::Term`
/// which represents a parsed term code (year + season).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DbTerm {
    /// Term code, e.g., "202510"
    pub code: String,
    /// Description from Banner, e.g., "Fall 2024"
    pub description: String,
    /// Year extracted from code, e.g., 2024
    pub year: i16,
    /// Academic season the term falls in
    pub season: Season,
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
    let terms = sqlx::query_as!(
        DbTerm,
        r#"
        SELECT code, description, year, season AS "season: Season", scrape_enabled,
               is_archived, discovered_at, last_scraped_at, created_at, updated_at
        FROM terms
        ORDER BY code DESC
        "#,
    )
    .fetch_all(db_pool)
    .await
    .context("failed to fetch all terms")?;

    Ok(terms)
}

/// Get terms with scraping enabled, ordered by code descending.
pub async fn get_enabled_terms(db_pool: &PgPool) -> Result<Vec<DbTerm>> {
    let terms = sqlx::query_as!(
        DbTerm,
        r#"
        SELECT code, description, year, season AS "season: Season", scrape_enabled,
               is_archived, discovered_at, last_scraped_at, created_at, updated_at
        FROM terms
        WHERE scrape_enabled = true
        ORDER BY code DESC
        "#,
    )
    .fetch_all(db_pool)
    .await
    .context("failed to fetch enabled terms")?;

    Ok(terms)
}

/// A lightweight projection of an enabled term for scheduling decisions.
#[derive(Debug, Clone)]
pub struct EnabledTerm {
    pub code: String,
    pub is_archived: bool,
}

/// Get enabled terms with their archive status.
///
/// Returns lightweight projections for the scheduler to determine per-term
/// scheduling tiers without fetching full rows.
pub async fn get_enabled_terms_for_scheduling(db_pool: &PgPool) -> Result<Vec<EnabledTerm>> {
    let terms = sqlx::query_as!(
        EnabledTerm,
        "SELECT code, is_archived FROM terms WHERE scrape_enabled = true ORDER BY code DESC",
    )
    .fetch_all(db_pool)
    .await
    .context("failed to fetch enabled terms for scheduling")?;

    Ok(terms)
}

/// Get a single term by code.
pub async fn get_term_by_code(db_pool: &PgPool, code: &str) -> Result<Option<DbTerm>> {
    let term = sqlx::query_as!(
        DbTerm,
        r#"
        SELECT code, description, year, season AS "season: Season", scrape_enabled,
               is_archived, discovered_at, last_scraped_at, created_at, updated_at
        FROM terms
        WHERE code = $1
        "#,
        code,
    )
    .fetch_optional(db_pool)
    .await
    .context("failed to fetch term by code")?;

    Ok(term)
}

/// Get all existing term codes (for sync deduplication).
async fn get_existing_term_codes(db_pool: &PgPool) -> Result<HashSet<String>> {
    let codes = sqlx::query_scalar!("SELECT code FROM terms")
        .fetch_all(db_pool)
        .await
        .context("failed to fetch existing term codes")?;

    Ok(codes.into_iter().collect())
}

/// Enable scraping for a term.
///
/// Returns `true` if the term was found and updated, `false` if not found.
pub async fn enable_scraping(db_pool: &PgPool, code: &str) -> Result<bool> {
    let result = sqlx::query!(
        "UPDATE terms SET scrape_enabled = true, updated_at = now() WHERE code = $1",
        code
    )
    .execute(db_pool)
    .await
    .context("failed to enable scraping for term")?;

    Ok(result.rows_affected() > 0)
}

/// Disable scraping for a term.
///
/// Returns `true` if the term was found and updated, `false` if not found.
pub async fn disable_scraping(db_pool: &PgPool, code: &str) -> Result<bool> {
    let result = sqlx::query!(
        "UPDATE terms SET scrape_enabled = false, updated_at = now() WHERE code = $1",
        code
    )
    .execute(db_pool)
    .await
    .context("failed to disable scraping for term")?;

    Ok(result.rows_affected() > 0)
}

/// Update the `last_scraped_at` timestamp for a term.
///
/// Called when a subject scrape job completes for this term.
pub async fn update_last_scraped_at(db_pool: &PgPool, code: &str) -> Result<()> {
    sqlx::query!(
        "UPDATE terms SET last_scraped_at = now(), updated_at = now() WHERE code = $1",
        code
    )
    .execute(db_pool)
    .await
    .context("failed to update last scraped at for term")?;

    Ok(())
}

/// Parse a 6-digit term code into (`display_year`, season).
///
/// Returns the **display year** (the year shown in the term description), not the raw
/// code prefix. Banner encodes Fall as `(display_year + 1)10`, so "202610" (Fall 2025)
/// returns `(2025, "Fall")` rather than `(2026, "Fall")`.
///
/// Returns `None` for unrecognized formats (e.g., legacy terms with non-standard
/// season codes like "11"). This allows the sync to skip invalid terms gracefully.
///
/// # Examples
/// - "202510" -> Some((2024, `Season::Fall`))   // Fall 2024, code prefix is 2025
/// - "202520" -> Some((2025, `Season::Spring`))
/// - "202530" -> Some((2025, `Season::Summer`))
/// - "201411" -> None (invalid season code)
fn parse_term_code(code: &str) -> Option<(i16, Season)> {
    if code.len() != 6 {
        return None;
    }

    let code_year = code[0..4].parse::<i16>().ok()?;

    let (season, display_year) = match &code[4..6] {
        // Fall display year is one less than the code prefix (Banner convention)
        "10" => (Season::Fall, code_year - 1),
        "20" => (Season::Spring, code_year),
        "30" => (Season::Summer, code_year),
        _ => return None,
    };

    Some((display_year, season))
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
pub async fn sync_terms_from_banner(db_pool: &PgPool, banner_terms: Vec<BannerTerm>) -> Result<SyncResult> {
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
            sqlx::query!(
                r#"
                UPDATE terms
                SET description = $2, is_archived = $3, updated_at = now()
                WHERE code = $1
                "#,
                term.code,
                term.description,
                is_archived,
            )
            .execute(db_pool)
            .await
            .context("failed to update term metadata")?;

            result.updated += 1;
        } else {
            // New term - enable scraping ONLY if it's the latest
            let scrape_enabled = Some(&term.code) == latest_code.as_ref();

            sqlx::query!(
                r#"
                INSERT INTO terms (code, description, year, season, scrape_enabled, is_archived)
                VALUES ($1, $2, $3, $4, $5, $6)
                "#,
                term.code,
                term.description,
                year,
                season as Season,
                scrape_enabled,
                is_archived,
            )
            .execute(db_pool)
            .await
            .context("failed to insert new term")?;

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
        tracing::warn!(skipped = result.skipped, "Skipped terms with unrecognized format codes");
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_term_code_fall() {
        // "202510": code_year=2025, Fall -> display_year = 2025 - 1 = 2024
        let (year, season) = parse_term_code("202510").unwrap();
        assert_eq!(year, 2024);
        assert_eq!(season, Season::Fall);
    }

    #[test]
    fn test_parse_term_code_fall_earliest() {
        // Fall 2001: Banner code "200210", code_year=2002 -> display_year=2001
        let (year, season) = parse_term_code("200210").unwrap();
        assert_eq!(year, 2001);
        assert_eq!(season, Season::Fall);
    }

    #[test]
    fn test_parse_term_code_spring() {
        let (year, season) = parse_term_code("202520").unwrap();
        assert_eq!(year, 2025);
        assert_eq!(season, Season::Spring);
    }

    #[test]
    fn test_parse_term_code_summer() {
        let (year, season) = parse_term_code("202530").unwrap();
        assert_eq!(year, 2025);
        assert_eq!(season, Season::Summer);
    }

    /// The column stores the display name, so the write and read halves must agree.
    #[test]
    fn test_season_column_roundtrip() {
        for season in [Season::Fall, Season::Spring, Season::Summer] {
            assert_eq!(Season::from_slug(season.as_ref()), Some(season));
        }
        assert_eq!(Season::from_slug("Winter"), None);
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
