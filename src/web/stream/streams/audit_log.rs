//! Audit log stream logic.

use sqlx::PgPool;

use crate::web::audit::{AuditLogEntry, AuditRow};
use crate::web::stream::filters::AuditLogFilter;

const DEFAULT_AUDIT_LIMIT: i32 = 200;
const MAX_AUDIT_LIMIT: i32 = 500;

pub async fn build_snapshot(
    db_pool: &PgPool,
    filter: &AuditLogFilter,
) -> Result<Vec<AuditLogEntry>, sqlx::Error> {
    let limit = filter
        .limit
        .unwrap_or(DEFAULT_AUDIT_LIMIT)
        .clamp(1, MAX_AUDIT_LIMIT);

    let field_changed: Option<&[String]> = filter
        .field_changed
        .as_deref()
        .filter(|v| !v.is_empty());
    let subject: Option<&[String]> = filter.subject.as_deref().filter(|v| !v.is_empty());
    let term: Option<&str> = filter.term.as_deref();

    let rows: Vec<AuditRow> = sqlx::query_as(
        "SELECT a.id, a.course_id, a.timestamp, a.field_changed, a.old_value, a.new_value, \
                c.subject, c.course_number, c.crn, c.title, c.term_code \
         FROM course_audits a \
         LEFT JOIN courses c ON c.id = a.course_id \
         WHERE ($1::timestamptz IS NULL OR a.timestamp > $1) \
           AND ($2::text[] IS NULL OR a.field_changed = ANY($2)) \
           AND ($3::text[] IS NULL OR c.subject = ANY($3)) \
           AND ($4::text IS NULL OR c.term_code = $4) \
         ORDER BY a.timestamp DESC LIMIT $5",
    )
    .bind(filter.since_dt)
    .bind(field_changed)
    .bind(subject)
    .bind(term)
    .bind(limit)
    .fetch_all(db_pool)
    .await?;

    Ok(rows.into_iter().map(AuditLogEntry::from).collect())
}

pub fn filter_entries(filter: &AuditLogFilter, entries: &[AuditLogEntry]) -> Vec<AuditLogEntry> {
    entries
        .iter()
        .filter(|entry| entry_matches(filter, entry))
        .cloned()
        .collect()
}

pub fn entry_matches(filter: &AuditLogFilter, entry: &AuditLogEntry) -> bool {
    if let Some(ref since) = filter.since_dt
        && let Ok(timestamp) = chrono::DateTime::parse_from_rfc3339(&entry.timestamp)
        && timestamp.with_timezone(&chrono::Utc) <= *since
    {
        return false;
    }

    filter_matches(
        filter,
        &entry.field_changed,
        entry.subject.as_deref(),
        entry.term_code.as_deref(),
    )
}

fn filter_matches(
    filter: &AuditLogFilter,
    field_changed: &str,
    subject: Option<&str>,
    term_code: Option<&str>,
) -> bool {
    if let Some(ref fields) = filter.field_changed
        && !fields.is_empty()
        && !fields.iter().any(|f| f == field_changed)
    {
        return false;
    }

    if let Some(ref subjects) = filter.subject
        && !subjects.is_empty()
    {
        let Some(subject) = subject else {
            return false;
        };
        if !subjects.iter().any(|f| f == subject) {
            return false;
        }
    }

    if let Some(ref term) = filter.term
        && term_code != Some(term.as_str())
    {
        return false;
    }

    true
}
