//! Business logic for scraper statistics -- period/bucket validation and
//! query orchestration. The actual SQL lives in [`crate::data::admin_scraper`].

use std::sync::Arc;

use sqlx::PgPool;

use crate::banner::models::terms::Term;
use crate::data::DbContext;
use crate::data::admin_scraper;
use crate::data::events::EventBuffer;
use crate::scraper::adaptive::{self, SubjectSchedule, SubjectStats};
use crate::state::ReferenceCache;

/// A scraper subject with computed schedule state, suitable for admin dashboards.
#[derive(Debug, Clone)]
pub struct SubjectData {
    pub subject: String,
    pub subject_description: Option<String>,
    pub tracked_course_count: i64,
    pub schedule_state: String,
    pub current_interval_secs: u64,
    pub time_multiplier: u32,
    pub last_scraped: chrono::DateTime<chrono::Utc>,
    pub next_eligible_at: Option<chrono::DateTime<chrono::Utc>>,
    pub cooldown_remaining_secs: Option<u64>,
    pub avg_change_ratio: f64,
    pub consecutive_zero_changes: i64,
    pub recent_runs: i64,
    pub recent_failures: i64,
}

/// Validate a period string and return the corresponding PostgreSQL interval literal.
pub fn validate_period(period: &str) -> Option<&'static str> {
    match period {
        "1h" => Some("1 hour"),
        "6h" => Some("6 hours"),
        "24h" => Some("24 hours"),
        "7d" => Some("7 days"),
        "30d" => Some("30 days"),
        _ => None,
    }
}

/// Validate a bucket string and return the corresponding PostgreSQL interval literal.
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

/// Return the default bucket code for a given period code.
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

/// Fetch aggregate scraper stats for the given period, with optional term filter.
pub async fn compute_stats(
    pool: &PgPool,
    period: &str,
    term: Option<&str>,
) -> anyhow::Result<admin_scraper::ScraperStats> {
    let interval_str =
        validate_period(period).ok_or_else(|| anyhow::anyhow!("Invalid period: {period}"))?;
    admin_scraper::compute_stats(pool, interval_str, term).await
}

/// Fetch timeseries scraper data for the given period and bucket, with optional term filter.
///
/// Returns `(points, period_code, bucket_code)`.
pub async fn compute_timeseries(
    pool: &PgPool,
    period: &str,
    bucket: Option<&str>,
    term: Option<&str>,
) -> anyhow::Result<(Vec<admin_scraper::TimeseriesPoint>, String, String)> {
    let period_interval =
        validate_period(period).ok_or_else(|| anyhow::anyhow!("Invalid period: {period}"))?;
    let bucket_code = bucket.unwrap_or_else(|| default_bucket_for_period(period));
    let bucket_interval = validate_bucket(bucket_code)
        .ok_or_else(|| anyhow::anyhow!("Invalid bucket: {bucket_code}"))?;
    let raw =
        admin_scraper::compute_timeseries(pool, bucket_interval, period_interval, term).await?;
    Ok((raw, period.to_string(), bucket_code.to_string()))
}

/// Compute subject schedule summaries for the current term.
pub async fn compute_subjects(
    pool: &PgPool,
    events: &Arc<EventBuffer>,
    ref_cache: &ReferenceCache,
) -> anyhow::Result<Vec<SubjectData>> {
    let db = DbContext::new(pool.clone(), events.clone());
    let all_stats = db.scrape_jobs().fetch_subject_stats().await?;

    let now = chrono::Utc::now();
    let multiplier = adaptive::time_of_day_multiplier(now);

    let term = Term::get_current().inner().to_string();

    // Filter to current term stats only for the admin dashboard.
    let raw_stats: Vec<_> = all_stats.into_iter().filter(|s| s.term == term).collect();
    let course_counts = crate::data::courses::count_by_subject(pool, &term).await?;

    let subjects: Vec<SubjectData> = raw_stats
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

            SubjectData {
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
