//! Database operations for the `reference_data` table (code->description lookups).

use crate::data::models::ReferenceData;
use anyhow::{Context, Result};
use html_escape::decode_html_entities;
use sqlx::PgPool;

/// Batch upsert reference data entries.
pub async fn batch_upsert(pool: &PgPool, entries: &[ReferenceData]) -> Result<()> {
    if entries.is_empty() {
        return Ok(());
    }

    let categories: Vec<&str> = entries.iter().map(|e| e.category.as_str()).collect();
    let codes: Vec<&str> = entries.iter().map(|e| e.code.as_str()).collect();
    let descriptions: Vec<String> = entries
        .iter()
        .map(|e| decode_html_entities(&e.description).into_owned())
        .collect();

    sqlx::query(
        r#"
        INSERT INTO reference_data (category, code, description)
        SELECT * FROM UNNEST($1::text[], $2::text[], $3::text[])
        ON CONFLICT (category, code)
        DO UPDATE SET description = EXCLUDED.description
        "#,
    )
    .bind(&categories)
    .bind(&codes)
    .bind(&descriptions)
    .execute(pool)
    .await
    .context("failed to batch upsert reference data")?;

    Ok(())
}

/// Get all reference data entries for a category.
pub async fn get_by_category(pool: &PgPool, category: &str) -> Result<Vec<ReferenceData>> {
    let rows = sqlx::query_as::<_, ReferenceData>(
        "SELECT category, code, description FROM reference_data WHERE category = $1 ORDER BY description",
    )
    .bind(category)
    .fetch_all(pool)
    .await
    .context("failed to fetch reference data by category")?;
    Ok(rows)
}

/// Get all reference data entries (for cache initialization).
pub async fn get_all(pool: &PgPool) -> Result<Vec<ReferenceData>> {
    let rows = sqlx::query_as::<_, ReferenceData>(
        "SELECT category, code, description FROM reference_data ORDER BY category, description",
    )
    .fetch_all(pool)
    .await
    .context("failed to fetch all reference data")?;
    Ok(rows)
}
