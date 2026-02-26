//! Stream filter types and parsing helpers.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::data::models::{ScrapeJobStatus, ScrapePriority, TargetType};
use crate::data::scraper_stats::{validate_bucket, validate_period};
use crate::web::stream::protocol::{StreamError, StreamFilter};

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ScrapeJobsFilter {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<Vec<ScrapeJobStatus>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<Vec<ScrapePriority>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_type: Option<Vec<TargetType>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub term: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<Vec<String>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AuditLogFilter {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub term: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field_changed: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub since: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<i32>,
    #[serde(skip)]
    pub since_dt: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ScraperStatsFilter {
    #[serde(default = "default_period")]
    pub period: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub term: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ScraperTimeseriesFilter {
    #[serde(default = "default_period")]
    pub period: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bucket: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub term: Option<String>,
}

fn default_period() -> String {
    "24h".to_string()
}

pub fn parse_scrape_jobs_filter(
    filter: Option<StreamFilter>,
) -> Result<ScrapeJobsFilter, StreamError> {
    match filter {
        Some(StreamFilter::ScrapeJobs(filter)) => Ok(filter),
        Some(_) => Err(StreamError::invalid_filter("Invalid scrape jobs filter")),
        None => Ok(ScrapeJobsFilter::default()),
    }
}

pub fn parse_audit_log_filter(filter: Option<StreamFilter>) -> Result<AuditLogFilter, StreamError> {
    let mut filter = match filter {
        Some(StreamFilter::AuditLog(filter)) => filter,
        Some(_) => {
            return Err(StreamError::invalid_filter("Invalid audit log filter"));
        }
        None => AuditLogFilter::default(),
    };

    filter.since_dt = match filter.since.as_deref() {
        Some(val) => Some(
            chrono::DateTime::parse_from_rfc3339(val)
                .map_err(|_| StreamError::invalid_filter("Invalid audit log 'since'"))?
                .with_timezone(&chrono::Utc),
        ),
        None => None,
    };

    Ok(filter)
}

pub fn parse_scraper_stats_filter(
    filter: Option<StreamFilter>,
) -> Result<ScraperStatsFilter, StreamError> {
    let f = match filter {
        Some(StreamFilter::ScraperStats(f)) => f,
        Some(_) => return Err(StreamError::invalid_filter("Invalid scraper stats filter")),
        None => ScraperStatsFilter {
            period: "24h".to_string(),
            term: None,
        },
    };
    if validate_period(&f.period).is_none() {
        return Err(StreamError::invalid_filter(format!(
            "Invalid period '{}'. Valid: 1h, 6h, 24h, 7d, 30d",
            f.period
        )));
    }
    Ok(f)
}

pub fn parse_scraper_timeseries_filter(
    filter: Option<StreamFilter>,
) -> Result<ScraperTimeseriesFilter, StreamError> {
    let f = match filter {
        Some(StreamFilter::ScraperTimeseries(f)) => f,
        Some(_) => {
            return Err(StreamError::invalid_filter(
                "Invalid scraper timeseries filter",
            ));
        }
        None => ScraperTimeseriesFilter {
            period: "24h".to_string(),
            bucket: None,
            term: None,
        },
    };
    if validate_period(&f.period).is_none() {
        return Err(StreamError::invalid_filter(format!(
            "Invalid period '{}'. Valid: 1h, 6h, 24h, 7d, 30d",
            f.period
        )));
    }
    if let Some(ref b) = f.bucket
        && validate_bucket(b).is_none()
    {
        return Err(StreamError::invalid_filter(format!(
            "Invalid bucket '{}'. Valid: 1m, 5m, 15m, 1h, 6h",
            b
        )));
    }
    Ok(f)
}
