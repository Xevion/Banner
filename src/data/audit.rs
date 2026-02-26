//! Database query functions for the course audit log.

use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::PgPool;

use crate::data::models::AuditRow;

const AUDIT_SELECT: &str = "SELECT a.id, a.course_id, a.timestamp, a.field_changed, a.old_value, a.new_value, \
            c.subject, c.course_number, c.crn, c.title, c.term_code \
     FROM course_audits a \
     LEFT JOIN courses c ON c.id = a.course_id";

/// Fetch the most recent audit log entries, newest first.
pub async fn list_recent(pool: &PgPool, limit: i32) -> Result<Vec<AuditRow>> {
    let rows = sqlx::query_as::<_, AuditRow>(&format!(
        "{AUDIT_SELECT} ORDER BY a.timestamp DESC LIMIT $1"
    ))
    .bind(limit)
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
    let rows: Vec<AuditRow> = sqlx::query_as(&format!(
        "{AUDIT_SELECT} \
         WHERE ($1::timestamptz IS NULL OR a.timestamp > $1) \
           AND ($2::text[] IS NULL OR a.field_changed = ANY($2)) \
           AND ($3::text[] IS NULL OR c.subject = ANY($3)) \
           AND ($4::text IS NULL OR c.term_code = $4) \
         ORDER BY a.timestamp DESC LIMIT $5"
    ))
    .bind(since_dt)
    .bind(field_changed)
    .bind(subject)
    .bind(term)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
