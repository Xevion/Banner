//! Audit log DTOs shared by HTTP and stream handlers.

use serde::Serialize;
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AuditLogEntry {
    pub id: i32,
    pub course_id: i32,
    pub timestamp: String,
    pub field_changed: String,
    pub old_value: String,
    pub new_value: String,
    pub subject: Option<String>,
    pub course_number: Option<String>,
    pub crn: Option<String>,
    pub course_title: Option<String>,
    pub term_code: Option<String>,
}

/// Row returned by audit-log queries (audit + joined course fields).
/// Shared across batch, admin, and stream handlers.
#[derive(sqlx::FromRow, Debug)]
pub struct AuditRow {
    pub id: i32,
    pub course_id: i32,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub field_changed: String,
    pub old_value: String,
    pub new_value: String,
    pub subject: Option<String>,
    pub course_number: Option<String>,
    pub crn: Option<String>,
    pub title: Option<String>,
    pub term_code: Option<String>,
}

impl From<AuditRow> for AuditLogEntry {
    fn from(row: AuditRow) -> Self {
        Self {
            id: row.id,
            course_id: row.course_id,
            timestamp: row.timestamp.to_rfc3339(),
            field_changed: row.field_changed,
            old_value: row.old_value,
            new_value: row.new_value,
            subject: row.subject,
            course_number: row.course_number,
            crn: row.crn,
            course_title: row.title,
            term_code: row.term_code,
        }
    }
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AuditLogResponse {
    pub entries: Vec<AuditLogEntry>,
}
