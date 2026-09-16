//! Admin API for the log of admin actions.
//!
//! Handlers record through [`record`] after their mutation has committed.

use axum::extract::{Query, State};
use axum::response::Json;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tracing::{error, instrument};
use ts_rs::TS;

use crate::data::admin_audits::{
    self, AdminAction, AdminAuditFilter, AdminAuditPage, AdminEntity, Target,
};
use crate::data::models::User;
use crate::state::AppState;
use crate::web::auth::extractors::AdminUser;
use crate::web::error::{ApiError, db_error};

/// Append an audit entry, logging and continuing when the write fails.
///
/// The action being described has already committed, so failing the request now
/// would report a false failure; the error carries the whole entry instead.
pub async fn record(
    pool: &PgPool,
    actor: &User,
    action: AdminAction,
    target: Target,
    detail: serde_json::Value,
) {
    let described = format!("{target:?}");

    if let Err(e) = admin_audits::insert(pool, actor, action, target, detail.clone()).await {
        error!(
            error = %e,
            action = action.as_ref(),
            entity = action.entity().as_ref(),
            target = %described,
            actor = actor.discord_id,
            detail = %detail,
            "Failed to record admin action"
        );
    }
}

/// Query params for `GET /api/admin/action-log`.
#[derive(Debug, Default, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ActionLogParams {
    /// Discord ID of the acting admin, as a string to survive JSON precision.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<AdminAction>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entity_type: Option<AdminEntity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entity_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(type = "string | null")]
    pub since: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(type = "string | null")]
    pub until: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub per_page: Option<i32>,
}

/// Treat a blank query param as absent, so an empty filter control clears it.
fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|s| !s.trim().is_empty())
}

impl ActionLogParams {
    fn into_filter(self) -> Result<AdminAuditFilter, ApiError> {
        let defaults = AdminAuditFilter::default();
        let actor_discord_id = match non_empty(self.actor) {
            Some(raw) => Some(
                raw.parse::<i64>()
                    .map_err(|_| ApiError::bad_request("actor must be a Discord ID"))?,
            ),
            None => None,
        };

        Ok(AdminAuditFilter {
            actor_discord_id,
            action: self.action,
            entity_type: self.entity_type,
            entity_id: non_empty(self.entity_id),
            since: self.since,
            until: self.until,
            page: self.page.unwrap_or(defaults.page),
            per_page: self.per_page.unwrap_or(defaults.per_page),
        })
    }
}

/// `GET /api/admin/action-log` -- List recorded admin actions, newest first.
#[instrument(skip_all)]
pub async fn list_action_log(
    AdminUser(_user): AdminUser,
    State(state): State<AppState>,
    Query(params): Query<ActionLogParams>,
) -> Result<Json<AdminAuditPage>, ApiError> {
    let filter = params.into_filter()?;

    let page = admin_audits::list(&state.db_pool, &filter)
        .await
        .map_err(|e| db_error("list admin action log", e))?;

    Ok(Json(page))
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert2::check;
    use serde_json::json;

    fn params(query: serde_json::Value) -> ActionLogParams {
        serde_json::from_value(query).unwrap()
    }

    #[test]
    fn test_blank_filters_are_treated_as_absent() {
        let filter = params(json!({ "entityId": "  ", "actor": "" }))
            .into_filter()
            .unwrap();

        check!(filter.entity_id == None);
        check!(filter.actor_discord_id == None);
    }

    #[test]
    fn test_non_numeric_actor_is_a_bad_request() {
        let err = params(json!({ "actor": "xevion" })).into_filter();

        check!(err.is_err());
    }

    #[test]
    fn test_filters_parse_into_their_typed_form() {
        let filter = params(json!({ "action": "instructor_merge", "entityType": "instructor" }))
            .into_filter()
            .unwrap();

        check!(filter.action == Some(AdminAction::InstructorMerge));
        check!(filter.entity_type == Some(AdminEntity::Instructor));
    }
}
