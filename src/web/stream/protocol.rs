//! Stream WebSocket protocol types and messages.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::web::admin::scraper::{ScraperStatsResponse, SubjectSummary, TimeseriesPoint};
use crate::web::stream::filters::{
    AuditLogFilter, ScrapeJobsFilter, ScraperStatsFilter, ScraperTimeseriesFilter,
};
use crate::web::ws::{ScrapeJobDto, ScrapeJobEvent};

pub const STREAM_PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum StreamKind {
    ScrapeJobs,
    AuditLog,
    ScraperStats,
    ScraperTimeseries,
    ScraperSubjects,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "stream", rename_all = "camelCase")]
#[ts(export)]
pub enum StreamFilter {
    ScrapeJobs(ScrapeJobsFilter),
    AuditLog(AuditLogFilter),
    ScraperStats(ScraperStatsFilter),
    ScraperTimeseries(ScraperTimeseriesFilter),
    ScraperSubjects {},
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export)]
pub enum StreamClientMessage {
    Subscribe {
        request_id: String,
        stream: StreamKind,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        filter: Option<StreamFilter>,
    },
    Modify {
        request_id: String,
        subscription_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        filter: Option<StreamFilter>,
    },
    Unsubscribe {
        request_id: String,
        subscription_id: String,
    },
    Ping {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        request_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        timestamp: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum StreamErrorCode {
    InvalidMessage,
    InvalidFilter,
    UnknownSubscription,
    InternalError,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(tag = "stream", rename_all = "camelCase")]
#[ts(export)]
pub enum StreamSnapshot {
    ScrapeJobs {
        jobs: Vec<ScrapeJobDto>,
    },
    AuditLog {
        entries: Vec<crate::web::audit::AuditLogEntry>,
    },
    ScraperStats {
        stats: ScraperStatsResponse,
    },
    ScraperTimeseries {
        points: Vec<TimeseriesPoint>,
        period: String,
        bucket: String,
    },
    ScraperSubjects {
        subjects: Vec<SubjectSummary>,
    },
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(tag = "stream", rename_all = "camelCase")]
#[ts(export)]
pub enum StreamDelta {
    ScrapeJobs {
        event: ScrapeJobEvent,
    },
    AuditLog {
        entries: Vec<crate::web::audit::AuditLogEntry>,
    },
    ScraperStats {
        stats: ScraperStatsResponse,
    },
    ScraperTimeseries {
        changed: Vec<TimeseriesPoint>,
    },
    ScraperSubjects {
        changed: Vec<SubjectSummary>,
        removed: Vec<String>,
    },
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export)]
pub enum StreamServerMessage {
    Ready {
        protocol_version: u32,
    },
    Subscribed {
        request_id: String,
        subscription_id: String,
        stream: StreamKind,
    },
    Modified {
        request_id: String,
        subscription_id: String,
    },
    Unsubscribed {
        request_id: String,
        subscription_id: String,
    },
    Snapshot {
        subscription_id: String,
        snapshot: StreamSnapshot,
    },
    Delta {
        subscription_id: String,
        delta: StreamDelta,
    },
    Error {
        #[serde(skip_serializing_if = "Option::is_none")]
        request_id: Option<String>,
        code: StreamErrorCode,
        message: String,
    },
    Pong {
        #[serde(skip_serializing_if = "Option::is_none")]
        request_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        timestamp: Option<String>,
    },
}

#[derive(Debug, Clone)]
pub struct StreamError {
    pub code: StreamErrorCode,
    pub message: String,
}

impl StreamError {
    pub fn invalid_filter(message: impl Into<String>) -> Self {
        Self {
            code: StreamErrorCode::InvalidFilter,
            message: message.into(),
        }
    }

    #[allow(dead_code)]
    pub fn unknown_subscription() -> Self {
        Self {
            code: StreamErrorCode::UnknownSubscription,
            message: "Unknown subscription".to_string(),
        }
    }

    #[allow(dead_code)]
    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            code: StreamErrorCode::InternalError,
            message: message.into(),
        }
    }
}
