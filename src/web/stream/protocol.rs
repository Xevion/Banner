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

impl StreamKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::ScrapeJobs => "scrape_jobs",
            Self::AuditLog => "audit_log",
            Self::ScraperStats => "scraper_stats",
            Self::ScraperTimeseries => "scraper_timeseries",
            Self::ScraperSubjects => "scraper_subjects",
        }
    }
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

impl StreamClientMessage {
    pub fn kind_label(&self) -> &'static str {
        match self {
            Self::Subscribe { .. } => "subscribe",
            Self::Modify { .. } => "modify",
            Self::Unsubscribe { .. } => "unsubscribe",
            Self::Ping { .. } => "ping",
        }
    }
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

impl StreamServerMessage {
    pub fn kind_label(&self) -> &'static str {
        match self {
            Self::Ready { .. } => "ready",
            Self::Subscribed { .. } => "subscribed",
            Self::Modified { .. } => "modified",
            Self::Unsubscribed { .. } => "unsubscribed",
            Self::Snapshot { .. } => "snapshot",
            Self::Delta { .. } => "delta",
            Self::Error { .. } => "error",
            Self::Pong { .. } => "pong",
        }
    }
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_kind_label_every_variant_is_distinct() {
        let kinds = [
            StreamKind::ScrapeJobs,
            StreamKind::AuditLog,
            StreamKind::ScraperStats,
            StreamKind::ScraperTimeseries,
            StreamKind::ScraperSubjects,
        ];
        let labels: std::collections::HashSet<_> = kinds.iter().map(|k| k.label()).collect();
        assert_eq!(labels.len(), kinds.len());
    }

    #[test]
    fn test_client_message_kind_label_matches_variant() {
        let subscribe = StreamClientMessage::Subscribe {
            request_id: "1".to_string(),
            stream: StreamKind::ScrapeJobs,
            filter: None,
        };
        let modify = StreamClientMessage::Modify {
            request_id: "1".to_string(),
            subscription_id: "1".to_string(),
            filter: None,
        };
        let unsubscribe = StreamClientMessage::Unsubscribe {
            request_id: "1".to_string(),
            subscription_id: "1".to_string(),
        };
        let ping = StreamClientMessage::Ping {
            request_id: None,
            timestamp: None,
        };

        assert_eq!(subscribe.kind_label(), "subscribe");
        assert_eq!(modify.kind_label(), "modify");
        assert_eq!(unsubscribe.kind_label(), "unsubscribe");
        assert_eq!(ping.kind_label(), "ping");
    }

    #[test]
    fn test_server_message_kind_label_matches_variant() {
        assert_eq!(
            StreamServerMessage::Ready {
                protocol_version: 1
            }
            .kind_label(),
            "ready"
        );
        assert_eq!(
            StreamServerMessage::Pong {
                request_id: None,
                timestamp: None
            }
            .kind_label(),
            "pong"
        );
        assert_eq!(
            StreamServerMessage::Error {
                request_id: None,
                code: StreamErrorCode::InternalError,
                message: "x".to_string(),
            }
            .kind_label(),
            "error"
        );
    }
}
