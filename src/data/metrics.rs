//! Database query functions for course enrollment metrics.

use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::PgPool;

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
    sqlx::query_as::<_, MetricRow>(&format!(
        "{METRIC_SELECT} WHERE course_id = $1 AND timestamp >= $2 ORDER BY timestamp DESC LIMIT $3"
    ))
    .bind(course_id)
    .bind(since)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(anyhow::Error::from)
}

/// Fetch metrics across all courses since a given timestamp.
pub async fn list_all(pool: &PgPool, since: DateTime<Utc>, limit: i32) -> Result<Vec<MetricRow>> {
    sqlx::query_as::<_, MetricRow>(&format!(
        "{METRIC_SELECT} WHERE timestamp >= $1 ORDER BY timestamp DESC LIMIT $2"
    ))
    .bind(since)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(anyhow::Error::from)
}
