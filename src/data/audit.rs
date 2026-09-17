//! Database query functions for the course audit log.

use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::PgPool;

use crate::data::models::AuditRow;

/// Fetch the most recent audit log entries, newest first.
pub async fn list_recent(pool: &PgPool, limit: i32) -> Result<Vec<AuditRow>> {
    let rows = sqlx::query_as!(
        AuditRow,
        r#"
        SELECT a.id AS "id!", a.course_id AS "course_id!", a.timestamp AS "timestamp!",
               a.field_changed AS "field_changed!", a.old_value, a.new_value AS "new_value!",
               c.subject AS "subject?", c.course_number AS "course_number?",
               c.crn AS "crn?", c.title AS "title?", c.term_code AS "term_code?"
        FROM course_audits a
        LEFT JOIN courses c ON c.id = a.course_id
        ORDER BY a.timestamp DESC
        LIMIT $1
        "#,
        i64::from(limit),
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Fetch audit log entries with optional filters applied in SQL.
///
/// All filter parameters are nullable -- passing `None` disables that filter.
pub async fn list_filtered(
    pool: &PgPool,
    since_dt: Option<DateTime<Utc>>,
    field_changed: Option<&[String]>,
    subject: Option<&[String]>,
    term: Option<&str>,
    limit: i32,
) -> Result<Vec<AuditRow>> {
    let rows = sqlx::query_as!(
        AuditRow,
        r#"
        SELECT a.id AS "id!", a.course_id AS "course_id!", a.timestamp AS "timestamp!",
               a.field_changed AS "field_changed!", a.old_value, a.new_value AS "new_value!",
               c.subject AS "subject?", c.course_number AS "course_number?",
               c.crn AS "crn?", c.title AS "title?", c.term_code AS "term_code?"
        FROM course_audits a
        LEFT JOIN courses c ON c.id = a.course_id
        WHERE ($1::timestamptz IS NULL OR a.timestamp > $1)
          AND ($2::text[] IS NULL OR a.field_changed = ANY($2))
          AND ($3::text[] IS NULL OR c.subject = ANY($3))
          AND ($4::text IS NULL OR c.term_code = $4)
        ORDER BY a.timestamp DESC
        LIMIT $5
        "#,
        since_dt,
        field_changed,
        subject,
        term,
        i64::from(limit),
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
