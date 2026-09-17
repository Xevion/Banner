//! Append-only log of admin actions, and the queries that read it back.
//!
//! Distinct from [`crate::data::audit`], which trails field changes on courses.
//! Entries here record a person deciding something, across whichever table the
//! decision landed in, and are never updated or deleted: an undo is a new row.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize, Serializer};
use sqlx::PgPool;
use sqlx::types::Json;
use strum::{AsRefStr, VariantArray};
use ts_rs::TS;

use crate::data::models::{Page, User};
use crate::data::unsigned::Count;

const DEFAULT_PER_PAGE: i32 = 50;
const MAX_PER_PAGE: i32 = 200;

/// Serialize an `i64` as a string to avoid JavaScript precision loss for values exceeding 2^53.
fn serialize_i64_as_string<S: Serializer>(value: &i64, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&value.to_string())
}

/// What an admin did. Stored as text, so adding a variant needs no migration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, AsRefStr, VariantArray)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
#[ts(export)]
pub enum AdminAction {
    InstructorMerge,
    InstructorMergeClaimant,
    InstructorMergeAll,
    InstructorDismiss,
    InstructorUndismiss,
    RmpAcceptCandidate,
    RmpRejectCandidate,
    RmpRejectAll,
    RmpUnmatch,
    BluebookApproveLink,
    BluebookRejectLink,
    BluebookAssignLink,
    UserSetAdmin,
    TermEnable,
    TermDisable,
}

/// The kind of record an action targeted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, AsRefStr, VariantArray)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
#[ts(export)]
pub enum AdminEntity {
    Instructor,
    User,
    Term,
    BluebookLink,
}

impl AdminAction {
    /// The entity kind this action always targets.
    pub fn entity(self) -> AdminEntity {
        match self {
            Self::InstructorMerge
            | Self::InstructorMergeClaimant
            | Self::InstructorMergeAll
            | Self::InstructorDismiss
            | Self::InstructorUndismiss
            | Self::RmpAcceptCandidate
            | Self::RmpRejectCandidate
            | Self::RmpRejectAll
            | Self::RmpUnmatch => AdminEntity::Instructor,
            Self::BluebookApproveLink | Self::BluebookRejectLink | Self::BluebookAssignLink => {
                AdminEntity::BluebookLink
            }
            Self::UserSetAdmin => AdminEntity::User,
            Self::TermEnable | Self::TermDisable => AdminEntity::Term,
        }
    }
}

/// Which records an action touched: an anchor, plus any others of the same kind.
///
/// A merge or a dismissal names two instructors, and a search for either one
/// must find it, so the second is kept alongside rather than buried in detail.
#[derive(Debug, Clone, Default)]
pub struct Target {
    id: Option<String>,
    related: Vec<String>,
}

impl Target {
    /// One record.
    pub fn id(id: impl ToString) -> Self {
        Self {
            id: Some(id.to_string()),
            related: Vec::new(),
        }
    }

    /// Two records, anchored on the first.
    pub fn pair(anchor: impl ToString, other: impl ToString) -> Self {
        Self {
            id: Some(anchor.to_string()),
            related: vec![other.to_string()],
        }
    }

    /// Every record of the kind, as a sweep that names none of them.
    pub fn all() -> Self {
        Self::default()
    }
}

/// One recorded action, as stored and as served.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AdminAuditEntry {
    #[ts(as = "i32")]
    pub id: i64,
    #[ts(type = "string")]
    pub created_at: DateTime<Utc>,
    #[serde(serialize_with = "serialize_i64_as_string")]
    #[ts(type = "string")]
    pub actor_discord_id: i64,
    pub actor_username: String,
    #[ts(as = "AdminAction")]
    pub action: String,
    #[ts(as = "AdminEntity")]
    pub entity_type: String,
    pub entity_id: Option<String>,
    pub related_ids: Vec<String>,
    #[ts(as = "serde_json::Value")]
    pub detail: Json<serde_json::Value>,
}

/// Every filter is optional; `None` disables that clause.
#[derive(Debug, Clone)]
pub struct AdminAuditFilter {
    pub actor_discord_id: Option<i64>,
    pub action: Option<AdminAction>,
    pub entity_type: Option<AdminEntity>,
    /// Matches the anchor or any related id, so either half of a pair is found.
    pub entity_id: Option<String>,
    pub since: Option<DateTime<Utc>>,
    pub until: Option<DateTime<Utc>>,
    pub page: i32,
    pub per_page: i32,
}

impl Default for AdminAuditFilter {
    fn default() -> Self {
        Self {
            actor_discord_id: None,
            action: None,
            entity_type: None,
            entity_id: None,
            since: None,
            until: None,
            page: 1,
            per_page: DEFAULT_PER_PAGE,
        }
    }
}

/// Append one entry.
///
/// Callers audit an action that has already committed, so they must log a
/// failure here and carry on rather than propagating it to the client.
pub async fn insert(
    pool: &PgPool,
    actor: &User,
    action: AdminAction,
    target: Target,
    detail: serde_json::Value,
) -> Result<AdminAuditEntry> {
    let detail = Json(detail);
    let entity_type = action.entity();
    let entry = sqlx::query_as!(
        AdminAuditEntry,
        r#"
        INSERT INTO admin_audits
            (actor_discord_id, actor_username, action, entity_type, entity_id, related_ids, detail)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING id, created_at, actor_discord_id, actor_username, action, entity_type,
                  entity_id, related_ids, detail AS "detail: Json<serde_json::Value>"
        "#,
        actor.discord_id,
        actor.discord_username,
        action.as_ref(),
        entity_type.as_ref(),
        target.id,
        &target.related,
        detail as Json<serde_json::Value>,
    )
    .fetch_one(pool)
    .await
    .context("failed to record admin action")?;

    Ok(entry)
}

/// Fetch one page of entries, newest first.
pub async fn list(pool: &PgPool, filter: &AdminAuditFilter) -> Result<Page<AdminAuditEntry>> {
    let per_page = filter.per_page.clamp(1, MAX_PER_PAGE);
    let page = filter.page.max(1);
    let action = filter.action.as_ref().map(AsRef::as_ref);
    let entity_type = filter.entity_type.as_ref().map(AsRef::as_ref);

    let total = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) AS "total!"
        FROM admin_audits
        WHERE ($1::bigint IS NULL OR actor_discord_id = $1)
          AND ($2::text IS NULL OR action = $2)
          AND ($3::text IS NULL OR entity_type = $3)
          AND ($4::text IS NULL OR entity_id = $4 OR $4 = ANY(related_ids))
          AND ($5::timestamptz IS NULL OR created_at >= $5)
          AND ($6::timestamptz IS NULL OR created_at < $6)
        "#,
        filter.actor_discord_id,
        action,
        entity_type,
        filter.entity_id.as_deref(),
        filter.since,
        filter.until,
    )
    .fetch_one(pool)
    .await
    .context("failed to count admin audit entries")?;

    let entries = sqlx::query_as!(
        AdminAuditEntry,
        r#"
        SELECT id, created_at, actor_discord_id, actor_username, action, entity_type,
               entity_id, related_ids, detail AS "detail: Json<serde_json::Value>"
        FROM admin_audits
        WHERE ($1::bigint IS NULL OR actor_discord_id = $1)
          AND ($2::text IS NULL OR action = $2)
          AND ($3::text IS NULL OR entity_type = $3)
          AND ($4::text IS NULL OR entity_id = $4 OR $4 = ANY(related_ids))
          AND ($5::timestamptz IS NULL OR created_at >= $5)
          AND ($6::timestamptz IS NULL OR created_at < $6)
        ORDER BY created_at DESC, id DESC
        LIMIT $7 OFFSET $8
        "#,
        filter.actor_discord_id,
        action,
        entity_type,
        filter.entity_id.as_deref(),
        filter.since,
        filter.until,
        i64::from(per_page),
        i64::from(page - 1) * i64::from(per_page),
    )
    .fetch_all(pool)
    .await
    .context("failed to list admin audit entries")?;

    Ok(Page {
        items: entries,
        total: Count::try_from(total)?,
        page,
        per_page,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert2::check;

    /// The wire name is what the column stores, so the two must not drift.
    #[test]
    fn test_action_serde_name_matches_stored_name() {
        for action in AdminAction::VARIANTS {
            let wire = serde_json::to_string(action).unwrap();
            check!(wire == format!("\"{}\"", action.as_ref()));
        }
    }

    #[test]
    fn test_entity_serde_name_matches_stored_name() {
        for entity in AdminEntity::VARIANTS {
            let wire = serde_json::to_string(entity).unwrap();
            check!(wire == format!("\"{}\"", entity.as_ref()));
        }
    }

    #[test]
    fn test_pair_keeps_the_other_side_searchable() {
        let target = Target::pair(41, 42);

        check!(target.id == Some("41".to_string()));
        check!(target.related == vec!["42".to_string()]);
    }
}
