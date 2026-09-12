//! Database query functions for course enrollment metrics.

use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::{AssertSqlSafe, PgPool};

/// A single course metrics snapshot row.
#[derive(sqlx::FromRow, Debug)]
pub struct MetricRow {
    pub id: i32,
    pub course_id: i32,
    pub timestamp: DateTime<Utc>,
    pub enrollment: i32,
    pub wait_count: i32,
    pub seats_available: i32,
}

const METRIC_SELECT: &str = "SELECT id, course_id, timestamp, enrollment, wait_count, seats_available \
     FROM course_metrics";

/// Fetch metrics for a specific course since a given timestamp.
pub async fn list_for_course(
    pool: &PgPool,
    course_id: i32,
    since: DateTime<Utc>,
    limit: i32,
) -> Result<Vec<MetricRow>> {
    sqlx::query_as::<_, MetricRow>(AssertSqlSafe(format!(
        "{METRIC_SELECT} WHERE course_id = $1 AND timestamp >= $2 ORDER BY timestamp DESC LIMIT $3"
    )))
    .bind(course_id)
    .bind(since)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(anyhow::Error::from)
}

/// A downsampled trend sample for one section, identified by CRN.
#[derive(sqlx::FromRow, Debug)]
pub struct TrendRow {
    pub crn: String,
    pub enrollment: i32,
    pub wait_count: i32,
    pub seats_available: i32,
}

/// Fetch an evenly-spaced trend sample per section, oldest first.
///
/// Buckets each section's snapshots into `buckets` slices and keeps the last value in each,
/// so a section with 600 snapshots and one with 12 both come back the same size.
pub async fn list_trends_for_courses(
    pool: &PgPool,
    term_code: &str,
    crns: &[String],
    buckets: i32,
) -> Result<Vec<TrendRow>> {
    sqlx::query_as::<_, TrendRow>(
        r#"
        WITH target AS (
            SELECT id, crn FROM courses WHERE term_code = $1 AND crn = ANY($2)
        ),
        bucketed AS (
            SELECT t.crn,
                   m.timestamp,
                   m.enrollment,
                   m.wait_count,
                   m.seats_available,
                   ntile($3) OVER (PARTITION BY m.course_id ORDER BY m.timestamp) AS bucket
            FROM course_metrics m
            JOIN target t ON t.id = m.course_id
        )
        SELECT crn,
               (array_agg(enrollment ORDER BY timestamp DESC))[1] AS enrollment,
               (array_agg(wait_count ORDER BY timestamp DESC))[1] AS wait_count,
               (array_agg(seats_available ORDER BY timestamp DESC))[1] AS seats_available
        FROM bucketed
        GROUP BY crn, bucket
        ORDER BY crn, bucket
        "#,
    )
    .bind(term_code)
    .bind(crns)
    .bind(buckets)
    .fetch_all(pool)
    .await
    .map_err(anyhow::Error::from)
}

/// Fetch metrics across all courses since a given timestamp.
pub async fn list_all(pool: &PgPool, since: DateTime<Utc>, limit: i32) -> Result<Vec<MetricRow>> {
    sqlx::query_as::<_, MetricRow>(AssertSqlSafe(format!(
        "{METRIC_SELECT} WHERE timestamp >= $1 ORDER BY timestamp DESC LIMIT $2"
    )))
    .bind(since)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(anyhow::Error::from)
}
