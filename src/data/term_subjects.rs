//! Database operations for the `term_subjects` table (cached per-term subject lists).

use anyhow::Result;
use sqlx::PgPool;

use crate::banner::models::common::Pair;

/// Returns cached subjects for a term, or empty vec if none cached.
pub async fn get_cached(term_code: &str, pool: &PgPool) -> Result<Vec<Pair>> {
    let rows = sqlx::query_as::<_, (String, String)>(
        r#"
        SELECT ts.subject_code, COALESCE(rd.description, ts.subject_code)
        FROM term_subjects ts
        LEFT JOIN reference_data rd ON rd.category = 'subject' AND rd.code = ts.subject_code
        WHERE ts.term_code = $1
        ORDER BY ts.subject_code
        "#,
    )
    .bind(term_code)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(code, description)| Pair { code, description })
        .collect())
}

/// Cache the subject list for a term. Replaces any existing cached subjects.
pub async fn cache(term_code: &str, subjects: &[Pair], pool: &PgPool) -> Result<()> {
    if subjects.is_empty() {
        return Ok(());
    }

    let mut tx = pool.begin().await?;

    sqlx::query("DELETE FROM term_subjects WHERE term_code = $1")
        .bind(term_code)
        .execute(&mut *tx)
        .await?;

    let term_codes: Vec<&str> = subjects.iter().map(|_| term_code).collect();
    let subject_codes: Vec<&str> = subjects.iter().map(|s| s.code.as_str()).collect();

    sqlx::query(
        r#"
        INSERT INTO term_subjects (term_code, subject_code)
        SELECT * FROM UNNEST($1::text[], $2::text[])
        "#,
    )
    .bind(&term_codes)
    .bind(&subject_codes)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}
