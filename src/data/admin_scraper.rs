//! Database query functions for the admin scraper dashboard.

use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row};

/// Aggregate statistics for a time period, optionally filtered by term.
#[derive(Debug)]
pub struct ScraperStats {
    pub total_scrapes: i64,
    pub successful_scrapes: i64,
    pub failed_scrapes: i64,
    pub avg_duration_ms: Option<f64>,
    pub total_courses_changed: i64,
    pub total_courses_fetched: i64,
    pub total_audits_generated: i64,
    pub pending_jobs: i64,
    pub locked_jobs: i64,
}

/// A single timeseries bucket of scraper activity.
#[derive(Debug, Clone)]
pub struct TimeseriesPoint {
    pub timestamp: DateTime<Utc>,
    pub scrape_count: i64,
    pub success_count: i64,
    pub error_count: i64,
    pub courses_changed: i64,
    pub avg_duration_ms: f64,
}

/// Fetch aggregate scraper stats for a period, with optional term filter.
///
/// `interval_str` is a validated PostgreSQL interval literal (e.g. `"24 hours"`).
pub async fn compute_stats(
    pool: &PgPool,
    interval_str: &str,
    term: Option<&str>,
) -> Result<ScraperStats> {
    let row = sqlx::query(
        "SELECT \
            COUNT(*) AS total_scrapes, \
            COUNT(*) FILTER (WHERE success) AS successful_scrapes, \
            COUNT(*) FILTER (WHERE NOT success) AS failed_scrapes, \
            (AVG(duration_ms) FILTER (WHERE success))::FLOAT8 AS avg_duration_ms, \
            COALESCE(SUM(courses_changed) FILTER (WHERE success), 0) AS total_courses_changed, \
            COALESCE(SUM(courses_fetched) FILTER (WHERE success), 0) AS total_courses_fetched, \
            COALESCE(SUM(audits_generated) FILTER (WHERE success), 0) AS total_audits_generated \
         FROM scrape_job_results \
         WHERE completed_at > NOW() - $1::interval \
           AND ($2::text IS NULL OR payload->>'term' = $2)",
    )
    .bind(interval_str)
    .bind(term)
    .fetch_one(pool)
    .await?;

    let queue_row = sqlx::query(
        "SELECT \
            COUNT(*) FILTER (WHERE locked_at IS NULL) AS pending_jobs, \
            COUNT(*) FILTER (WHERE locked_at IS NOT NULL) AS locked_jobs \
         FROM scrape_jobs",
    )
    .fetch_one(pool)
    .await?;

    Ok(ScraperStats {
        total_scrapes: row.get("total_scrapes"),
        successful_scrapes: row.get("successful_scrapes"),
        failed_scrapes: row.get("failed_scrapes"),
        avg_duration_ms: row.get("avg_duration_ms"),
        total_courses_changed: row.get("total_courses_changed"),
        total_courses_fetched: row.get("total_courses_fetched"),
        total_audits_generated: row.get("total_audits_generated"),
        pending_jobs: queue_row.get("pending_jobs"),
        locked_jobs: queue_row.get("locked_jobs"),
    })
}

/// Fetch timeseries scraper data for a period, bucketed by interval.
///
/// Both `bucket_interval` and `period_interval` are validated PostgreSQL interval
/// literals. Returns one point per bucket, with zero-filled gaps.
pub async fn compute_timeseries(
    pool: &PgPool,
    bucket_interval: &str,
    period_interval: &str,
    term: Option<&str>,
) -> Result<Vec<TimeseriesPoint>> {
    let rows = sqlx::query(
        "WITH buckets AS ( \
            SELECT generate_series( \
                date_bin($1::interval, NOW() - $2::interval, '2020-01-01'::timestamptz), \
                date_bin($1::interval, NOW(), '2020-01-01'::timestamptz), \
                $1::interval \
            ) AS bucket_start \
         ), \
         raw AS ( \
            SELECT date_bin($1::interval, completed_at, '2020-01-01'::timestamptz) AS bucket_start, \
                   COUNT(*)::BIGINT AS scrape_count, \
                   COUNT(*) FILTER (WHERE success)::BIGINT AS success_count, \
                   COUNT(*) FILTER (WHERE NOT success)::BIGINT AS error_count, \
                   COALESCE(SUM(courses_changed) FILTER (WHERE success), 0)::BIGINT AS courses_changed, \
                   COALESCE(AVG(duration_ms) FILTER (WHERE success), 0)::FLOAT8 AS avg_duration_ms \
            FROM scrape_job_results \
            WHERE completed_at > NOW() - $2::interval \
              AND ($3::text IS NULL OR payload->>'term' = $3) \
            GROUP BY 1 \
         ) \
         SELECT b.bucket_start, \
                COALESCE(r.scrape_count, 0) AS scrape_count, \
                COALESCE(r.success_count, 0) AS success_count, \
                COALESCE(r.error_count, 0) AS error_count, \
                COALESCE(r.courses_changed, 0) AS courses_changed, \
                COALESCE(r.avg_duration_ms, 0) AS avg_duration_ms \
         FROM buckets b \
         LEFT JOIN raw r ON b.bucket_start = r.bucket_start \
         ORDER BY b.bucket_start",
    )
    .bind(bucket_interval)
    .bind(period_interval)
    .bind(term)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .iter()
        .map(|row| TimeseriesPoint {
            timestamp: row.get("bucket_start"),
            scrape_count: row.get("scrape_count"),
            success_count: row.get("success_count"),
            error_count: row.get("error_count"),
            courses_changed: row.get("courses_changed"),
            avg_duration_ms: row.get("avg_duration_ms"),
        })
        .collect())
}
