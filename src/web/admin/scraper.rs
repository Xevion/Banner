//! Admin API handlers for scraper observability.
//!
//! All endpoints require the `AdminUser` extractor, returning 401/403 as needed.

use std::time::{Duration, Instant};

use crate::utils::log_if_slow;
use axum::extract::{Path, Query, State};
use axum::response::Json;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tracing::{error, instrument, trace};
use ts_rs::TS;

use crate::banner::models::terms::Term;
use crate::data::DbContext;
use crate::data::unsigned::{Count, DurationMs};
use crate::scraper::adaptive::{self, SubjectSchedule, SubjectStats};
use crate::state::{AppState, ReferenceCache};
use crate::web::auth::extractors::AdminUser;
use crate::web::error::ApiError;

const SLOW_OP_THRESHOLD: Duration = Duration::from_secs(1);

fn parse_period(period: &str) -> Result<chrono::Duration, ApiError> {
    match period {
        "1h" => Ok(chrono::Duration::hours(1)),
        "6h" => Ok(chrono::Duration::hours(6)),
        "24h" => Ok(chrono::Duration::hours(24)),
        "7d" => Ok(chrono::Duration::days(7)),
        "30d" => Ok(chrono::Duration::days(30)),
        _ => Err(ApiError::bad_request(format!(
            "Invalid period '{period}'. Valid: 1h, 6h, 24h, 7d, 30d"
        ))),
    }
}

fn period_to_interval_str(period: &str) -> &'static str {
    match period {
        "1h" => "1 hour",
        "6h" => "6 hours",
        "24h" => "24 hours",
        "7d" => "7 days",
        "30d" => "30 days",
        _ => "24 hours",
    }
}

fn parse_bucket(bucket: &str) -> Result<&'static str, ApiError> {
    match bucket {
        "1m" => Ok("1 minute"),
        "5m" => Ok("5 minutes"),
        "15m" => Ok("15 minutes"),
        "1h" => Ok("1 hour"),
        "6h" => Ok("6 hours"),
        _ => Err(ApiError::bad_request(format!(
            "Invalid bucket '{bucket}'. Valid: 1m, 5m, 15m, 1h, 6h"
        ))),
    }
}

pub fn default_bucket_for_period(period: &str) -> &'static str {
    match period {
        "1h" => "1m",
        "6h" => "5m",
        "24h" => "15m",
        "7d" => "1h",
        "30d" => "6h",
        _ => "15m",
    }
}

#[derive(Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StatsParams {
    #[serde(default = "default_period")]
    pub period: String,
    /// Optional term code to filter stats (e.g., "202510"). If omitted, includes all terms.
    pub term: Option<String>,
}

fn default_period() -> String {
    "24h".to_string()
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ScraperStatsResponse {
    pub period: String,
    /// The term filter applied, or null if showing all terms.
    pub term: Option<String>,
    #[ts(type = "number")]
    pub total_scrapes: i64,
    #[ts(type = "number")]
    pub successful_scrapes: i64,
    #[ts(type = "number")]
    pub failed_scrapes: i64,
    pub success_rate: Option<f64>,
    pub avg_duration_ms: Option<f64>,
    #[ts(type = "number")]
    pub total_courses_changed: i64,
    #[ts(type = "number")]
    pub total_courses_fetched: i64,
    #[ts(type = "number")]
    pub total_audits_generated: i64,
    #[ts(type = "number")]
    pub pending_jobs: i64,
    #[ts(type = "number")]
    pub locked_jobs: i64,
}

#[instrument(skip_all, fields(period = %params.period))]
pub async fn scraper_stats(
    _admin: AdminUser,
    State(state): State<AppState>,
    Query(params): Query<StatsParams>,
) -> Result<Json<ScraperStatsResponse>, ApiError> {
    let start = Instant::now();
    let _ = parse_period(&params.period)?;

    let result = compute_stats(&state.db_pool, &params.period, params.term.as_deref())
        .await
        .map_err(|e| {
            error!(error = %e, "failed to fetch scraper stats");
            ApiError::internal_error("Failed to fetch scraper stats")
        })?;

    log_if_slow(start, SLOW_OP_THRESHOLD, "scraper_stats");

    trace!(
        total_scrapes = result.total_scrapes,
        "fetched scraper stats"
    );

    Ok(Json(result))
}

#[derive(Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TimeseriesParams {
    #[serde(default = "default_period")]
    pub period: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bucket: Option<String>,
    /// Optional term code to filter timeseries (e.g., "202510"). If omitted, includes all terms.
    pub term: Option<String>,
}

#[derive(Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct TimeseriesResponse {
    pub period: String,
    pub bucket: String,
    pub points: Vec<TimeseriesPoint>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct TimeseriesPoint {
    /// ISO-8601 UTC timestamp for this data point (e.g., "2024-01-15T10:00:00Z")
    #[ts(type = "string")]
    pub timestamp: DateTime<Utc>,
    #[ts(type = "number")]
    pub scrape_count: i64,
    #[ts(type = "number")]
    pub success_count: i64,
    #[ts(type = "number")]
    pub error_count: i64,
    #[ts(type = "number")]
    pub courses_changed: i64,
    pub avg_duration_ms: f64,
}

#[instrument(skip_all, fields(period = %params.period))]
pub async fn scraper_timeseries(
    _admin: AdminUser,
    State(state): State<AppState>,
    Query(params): Query<TimeseriesParams>,
) -> Result<Json<TimeseriesResponse>, ApiError> {
    let start = Instant::now();
    let _ = parse_period(&params.period)?;
    // Validate bucket if provided
    if let Some(ref b) = params.bucket {
        parse_bucket(b)?;
    }

    let (points, period, bucket) = compute_timeseries(
        &state.db_pool,
        &params.period,
        params.bucket.as_deref(),
        params.term.as_deref(),
    )
    .await
    .map_err(|e| {
        error!(error = %e, "failed to fetch scraper timeseries");
        ApiError::internal_error("Failed to fetch scraper timeseries")
    })?;

    log_if_slow(start, SLOW_OP_THRESHOLD, "scraper_timeseries");

    trace!(point_count = points.len(), "fetched scraper timeseries");

    Ok(Json(TimeseriesResponse {
        period,
        bucket,
        points,
    }))
}

#[derive(Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct SubjectsResponse {
    subjects: Vec<SubjectSummary>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct SubjectSummary {
    pub subject: String,
    pub subject_description: Option<String>,
    #[ts(type = "number")]
    pub tracked_course_count: i64,
    pub schedule_state: String,
    #[ts(type = "number")]
    pub current_interval_secs: u64,
    pub time_multiplier: u32,
    /// ISO-8601 UTC timestamp of last scrape (e.g., "2024-01-15T10:30:00Z")
    #[ts(type = "string")]
    pub last_scraped: DateTime<Utc>,
    /// ISO-8601 UTC timestamp when next scrape is eligible (e.g., "2024-01-15T11:00:00Z")
    #[ts(type = "string | null")]
    pub next_eligible_at: Option<DateTime<Utc>>,
    #[ts(type = "number | null")]
    pub cooldown_remaining_secs: Option<u64>,
    pub avg_change_ratio: f64,
    #[ts(type = "number")]
    pub consecutive_zero_changes: i64,
    #[ts(type = "number")]
    pub recent_runs: i64,
    #[ts(type = "number")]
    pub recent_failures: i64,
}

impl PartialEq for SubjectSummary {
    fn eq(&self, other: &Self) -> bool {
        // Exclude cooldown_remaining_secs and next_eligible_at from comparison
        // since they change every second and clients derive cooldowns from next_eligible_at
        self.subject == other.subject
            && self.subject_description == other.subject_description
            && self.tracked_course_count == other.tracked_course_count
            && self.schedule_state == other.schedule_state
            && self.current_interval_secs == other.current_interval_secs
            && self.time_multiplier == other.time_multiplier
            && self.last_scraped == other.last_scraped
            && self.avg_change_ratio == other.avg_change_ratio
            && self.consecutive_zero_changes == other.consecutive_zero_changes
            && self.recent_runs == other.recent_runs
            && self.recent_failures == other.recent_failures
    }
}

#[instrument(skip_all)]
pub async fn scraper_subjects(
    _admin: AdminUser,
    State(state): State<AppState>,
) -> Result<Json<SubjectsResponse>, ApiError> {
    let start = Instant::now();
    let ref_cache = state.reference_cache.read().await;

    let subjects = compute_subjects(&state.db_pool, &state.events, &ref_cache)
        .await
        .map_err(|e| {
            error!(error = %e, "failed to fetch subject stats");
            ApiError::internal_error("Failed to fetch subject stats")
        })?;

    log_if_slow(start, SLOW_OP_THRESHOLD, "scraper_subjects");

    trace!(count = subjects.len(), "fetched scraper subjects");

    Ok(Json(SubjectsResponse { subjects }))
}

#[derive(Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SubjectDetailParams {
    #[serde(default = "default_detail_limit")]
    pub limit: i32,
}

fn default_detail_limit() -> i32 {
    50
}

#[derive(Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct SubjectDetailResponse {
    subject: String,
    results: Vec<SubjectResultEntry>,
}

#[derive(Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct SubjectResultEntry {
    #[ts(type = "number")]
    id: i64,
    /// ISO-8601 UTC timestamp when the scrape job completed (e.g., "2024-01-15T10:30:00Z")
    #[ts(type = "string")]
    completed_at: DateTime<Utc>,
    duration_ms: DurationMs,
    success: bool,
    error_message: Option<String>,
    courses_fetched: Option<Count>,
    courses_changed: Option<Count>,
    courses_unchanged: Option<Count>,
    audits_generated: Option<Count>,
    metrics_generated: Option<Count>,
}

#[instrument(skip_all, fields(%subject))]
pub async fn scraper_subject_detail(
    _admin: AdminUser,
    State(state): State<AppState>,
    Path(subject): Path<String>,
    Query(params): Query<SubjectDetailParams>,
) -> Result<Json<SubjectDetailResponse>, ApiError> {
    let start = Instant::now();
    let limit = params.limit.clamp(1, 200);

    let rows =
        crate::data::scrape_jobs::list_results_for_subject(&state.db_pool, &subject, limit as i64)
            .await
            .map_err(|e| {
                error!(error = %e, "failed to fetch subject detail");
                ApiError::internal_error("Failed to fetch subject detail")
            })?;

    log_if_slow(start, SLOW_OP_THRESHOLD, "scraper_subject_detail");

    let results: Vec<SubjectResultEntry> = rows
        .into_iter()
        .map(|row| SubjectResultEntry {
            id: row.id as i64,
            completed_at: row.completed_at,
            duration_ms: DurationMs::new(row.duration_ms.max(0) as u32),
            success: row.success,
            error_message: row.error_message,
            courses_fetched: row.courses_fetched.and_then(|v| Count::try_from(v).ok()),
            courses_changed: row.courses_changed.and_then(|v| Count::try_from(v).ok()),
            courses_unchanged: row.courses_unchanged.and_then(|v| Count::try_from(v).ok()),
            audits_generated: row.audits_generated.and_then(|v| Count::try_from(v).ok()),
            metrics_generated: row.metrics_generated.and_then(|v| Count::try_from(v).ok()),
        })
        .collect();

    trace!(count = results.len(), "fetched subject detail");

    Ok(Json(SubjectDetailResponse { subject, results }))
}

/// Validate a period string and return the corresponding interval SQL string.
pub fn validate_period(period: &str) -> Option<&'static str> {
    match period {
        "1h" | "6h" | "24h" | "7d" | "30d" => Some(period_to_interval_str(period)),
        _ => None,
    }
}

/// Validate a bucket string.
pub fn validate_bucket(bucket: &str) -> Option<&'static str> {
    match bucket {
        "1m" => Some("1 minute"),
        "5m" => Some("5 minutes"),
        "15m" => Some("15 minutes"),
        "1h" => Some("1 hour"),
        "6h" => Some("6 hours"),
        _ => None,
    }
}

/// Compute scraper stats from the database.
pub async fn compute_stats(
    pool: &PgPool,
    period: &str,
    term: Option<&str>,
) -> anyhow::Result<ScraperStatsResponse> {
    let interval_str =
        validate_period(period).ok_or_else(|| anyhow::anyhow!("Invalid period: {period}"))?;

    let stats = crate::data::admin_scraper::compute_stats(pool, interval_str, term).await?;

    let success_rate = if stats.total_scrapes > 0 {
        Some(stats.successful_scrapes as f64 / stats.total_scrapes as f64)
    } else {
        None
    };

    Ok(ScraperStatsResponse {
        period: period.to_string(),
        term: term.map(|t| t.to_string()),
        total_scrapes: stats.total_scrapes,
        successful_scrapes: stats.successful_scrapes,
        failed_scrapes: stats.failed_scrapes,
        success_rate,
        avg_duration_ms: stats.avg_duration_ms,
        total_courses_changed: stats.total_courses_changed,
        total_courses_fetched: stats.total_courses_fetched,
        total_audits_generated: stats.total_audits_generated,
        pending_jobs: stats.pending_jobs,
        locked_jobs: stats.locked_jobs,
    })
}

/// Compute timeseries data from the database.
pub async fn compute_timeseries(
    pool: &PgPool,
    period: &str,
    bucket: Option<&str>,
    term: Option<&str>,
) -> anyhow::Result<(Vec<TimeseriesPoint>, String, String)> {
    let period_interval =
        validate_period(period).ok_or_else(|| anyhow::anyhow!("Invalid period: {period}"))?;

    let bucket_code = bucket.unwrap_or_else(|| default_bucket_for_period(period));
    let bucket_interval = validate_bucket(bucket_code)
        .ok_or_else(|| anyhow::anyhow!("Invalid bucket: {bucket_code}"))?;

    let raw = crate::data::admin_scraper::compute_timeseries(
        pool,
        bucket_interval,
        period_interval,
        term,
    )
    .await?;

    let points: Vec<TimeseriesPoint> = raw
        .into_iter()
        .map(|p| TimeseriesPoint {
            timestamp: p.timestamp,
            scrape_count: p.scrape_count,
            success_count: p.success_count,
            error_count: p.error_count,
            courses_changed: p.courses_changed,
            avg_duration_ms: p.avg_duration_ms,
        })
        .collect();

    Ok((points, period.to_string(), bucket_code.to_string()))
}

/// Compute subject summaries from the database.
pub async fn compute_subjects(
    pool: &PgPool,
    events: &std::sync::Arc<crate::data::events::EventBuffer>,
    ref_cache: &ReferenceCache,
) -> anyhow::Result<Vec<SubjectSummary>> {
    let db = DbContext::new(pool.clone(), events.clone());
    let all_stats = db.scrape_jobs().fetch_subject_stats().await?;

    let now = Utc::now();
    let multiplier = adaptive::time_of_day_multiplier(now);

    let term = Term::get_current().inner().to_string();

    // Filter to current term stats only for the admin dashboard
    let raw_stats: Vec<_> = all_stats.into_iter().filter(|s| s.term == term).collect();
    let course_counts = crate::data::courses::count_by_subject(pool, &term).await?;

    let subjects: Vec<SubjectSummary> = raw_stats
        .into_iter()
        .map(|row| {
            let stats: SubjectStats = row.into();
            let schedule = adaptive::evaluate_subject(&stats, now, adaptive::TermCategory::Current);
            let base_interval = adaptive::compute_base_interval(&stats);

            let schedule_state = match &schedule {
                SubjectSchedule::Eligible(_) => "eligible",
                SubjectSchedule::Cooldown(_) => "cooldown",
                SubjectSchedule::Paused => "paused",
            };

            let current_interval_secs = base_interval.as_secs() * multiplier as u64;

            let (next_eligible_at, cooldown_remaining_secs) = match &schedule {
                SubjectSchedule::Eligible(_) => (Some(now), Some(0)),
                SubjectSchedule::Cooldown(remaining) => {
                    let remaining_secs = remaining.as_secs();
                    (
                        Some(now + chrono::Duration::seconds(remaining_secs as i64)),
                        Some(remaining_secs),
                    )
                }
                SubjectSchedule::Paused => (None, None),
            };

            let subject_description = ref_cache
                .lookup("subject", &stats.subject)
                .map(|s| s.to_string());

            let tracked_course_count = course_counts.get(&stats.subject).copied().unwrap_or(0);

            SubjectSummary {
                subject: stats.subject,
                subject_description,
                tracked_course_count,
                schedule_state: schedule_state.to_string(),
                current_interval_secs,
                time_multiplier: multiplier,
                last_scraped: stats.last_completed,
                next_eligible_at,
                cooldown_remaining_secs,
                avg_change_ratio: stats.avg_change_ratio,
                consecutive_zero_changes: stats.consecutive_zero_changes,
                recent_runs: stats.recent_runs,
                recent_failures: stats.recent_failure_count,
            }
        })
        .collect();

    Ok(subjects)
}
