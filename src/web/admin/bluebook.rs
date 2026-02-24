//! Admin API handler for triggering BlueBook evaluation sync.

use std::sync::atomic::Ordering;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Json;
use serde::Serialize;
use tracing::{info, instrument};
use ts_rs::TS;

use crate::state::AppState;
use crate::web::auth::extractors::AdminUser;

/// Response for `POST /api/admin/bluebook/sync`.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BlueBookSyncTriggerResponse {
    pub message: String,
}

/// `POST /api/admin/bluebook/sync` — Trigger a BlueBook evaluation sync.
#[instrument(skip_all)]
pub async fn sync_bluebook(
    AdminUser(_user): AdminUser,
    State(state): State<AppState>,
) -> (StatusCode, Json<BlueBookSyncTriggerResponse>) {
    info!("Admin triggered BlueBook sync");
    state.bluebook_force_flag.store(true, Ordering::Relaxed);
    state.bluebook_sync_notify.notify_one();
    (
        StatusCode::ACCEPTED,
        Json(BlueBookSyncTriggerResponse {
            message: "BlueBook sync triggered".to_string(),
        }),
    )
}
