//! WebSocket event types and DTOs for admin streams.

use serde::Serialize;
use ts_rs::TS;

use crate::data::models::{ScrapeJob, ScrapeJobStatus, ScrapePriority, TargetType};
use crate::data::unsigned::Count;

/// A serializable DTO for `ScrapeJob` with computed `status`.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ScrapeJobDto {
    pub id: i32,
    pub target_type: TargetType,
    pub target_payload: serde_json::Value,
    pub priority: ScrapePriority,
    pub execute_at: String,
    pub created_at: String,
    pub locked_at: Option<String>,
    pub retry_count: Count,
    pub max_retries: Count,
    pub queued_at: String,
    pub status: ScrapeJobStatus,
}

impl From<&ScrapeJob> for ScrapeJobDto {
    fn from(job: &ScrapeJob) -> Self {
        Self {
            id: job.id,
            target_type: job.target_type,
            target_payload: job.target_payload.clone(),
            priority: job.priority,
            execute_at: job.execute_at.to_rfc3339(),
            created_at: job.created_at.to_rfc3339(),
            locked_at: job.locked_at.map(|t| t.to_rfc3339()),
            retry_count: job.retry_count,
            max_retries: job.max_retries,
            queued_at: job.queued_at.to_rfc3339(),
            status: job.status(),
        }
    }
}

/// Events broadcast when scrape job state changes.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export)]
pub enum ScrapeJobEvent {
    Created {
        job: ScrapeJobDto,
    },
    Locked {
        id: i32,
        #[serde(rename = "lockedAt")]
        locked_at: String,
        status: ScrapeJobStatus,
    },
    Completed {
        id: i32,
        subject: Option<String>,
    },
    Retried {
        id: i32,
        #[serde(rename = "retryCount")]
        retry_count: Count,
        #[serde(rename = "queuedAt")]
        queued_at: String,
        status: ScrapeJobStatus,
    },
    Exhausted {
        id: i32,
    },
    Deleted {
        id: i32,
    },
}
