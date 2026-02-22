//! Scrape job database operations with automatic event emission.

use std::collections::HashSet;

use chrono::{DateTime, Utc};
use tracing::debug;

use super::context::DbContext;
use super::events::DomainEvent;
use crate::data::models::{
    ScrapeJob, ScrapeJobStatus, ScrapePriority, SubjectResultStats, TargetType, UpsertCounts,
};
use crate::web::ws::{ScrapeJobDto, ScrapeJobEvent};
use anyhow::Result;

/// Lock expiry duration in seconds.
const LOCK_EXPIRY_SECS: i32 = 10 * 60;

/// Scrape job operations.
pub struct ScrapeJobOps<'a> {
    ctx: &'a DbContext,
}

impl<'a> ScrapeJobOps<'a> {
    pub(crate) fn new(ctx: &'a DbContext) -> Self {
        Self { ctx }
    }

    /// Fetch and lock the next available job.
    ///
    /// Emits a `ScrapeJobEvent::Locked` event on success.
    pub async fn lock_next(&self) -> Result<Option<ScrapeJob>> {
        let mut tx = self.ctx.pool().begin().await?;

        let job = sqlx::query_as::<_, ScrapeJob>(
            "SELECT * FROM scrape_jobs \
             WHERE (locked_at IS NULL OR locked_at < NOW() - make_interval(secs => $1::double precision)) \
             AND execute_at <= NOW() \
             ORDER BY priority DESC, execute_at ASC \
             LIMIT 1 \
             FOR UPDATE SKIP LOCKED",
        )
        .bind(LOCK_EXPIRY_SECS)
        .fetch_optional(&mut *tx)
        .await?;

        if let Some(ref job) = job {
            sqlx::query("UPDATE scrape_jobs SET locked_at = NOW() WHERE id = $1")
                .bind(job.id)
                .execute(&mut *tx)
                .await?;
        }

        tx.commit().await?;

        // Emit event after successful commit
        if let Some(ref job) = job {
            let locked_at = Utc::now().to_rfc3339();
            self.ctx
                .events()
                .publish(DomainEvent::ScrapeJob(ScrapeJobEvent::Locked {
                    id: job.id,
                    locked_at,
                    status: job.status(),
                }));
        }

        Ok(job)
    }

    /// Delete a job by ID.
    ///
    /// Emits a `ScrapeJobEvent::Deleted` event on success.
    pub async fn delete(&self, job_id: i32) -> Result<()> {
        sqlx::query("DELETE FROM scrape_jobs WHERE id = $1")
            .bind(job_id)
            .execute(self.ctx.pool())
            .await?;

        self.ctx
            .events()
            .publish(DomainEvent::ScrapeJob(ScrapeJobEvent::Deleted {
                id: job_id,
            }));

        Ok(())
    }

    /// Mark a job as completed (deletes it).
    ///
    /// Emits a `ScrapeJobEvent::Completed` event with the subject extracted from
    /// the job's target payload.
    pub async fn complete(&self, job_id: i32) -> Result<()> {
        let subject: Option<String> = sqlx::query_scalar(
            "DELETE FROM scrape_jobs WHERE id = $1 RETURNING target_payload->>'subject'",
        )
        .bind(job_id)
        .fetch_optional(self.ctx.pool())
        .await?
        .flatten();

        self.ctx
            .events()
            .publish(DomainEvent::ScrapeJob(ScrapeJobEvent::Completed {
                id: job_id,
                subject,
            }));

        Ok(())
    }

    /// Unlock a job for retry.
    ///
    /// Emits a `ScrapeJobEvent::Retried` event.
    pub async fn retry(
        &self,
        job_id: i32,
        retry_count: i32,
        execute_at: DateTime<Utc>,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE scrape_jobs SET locked_at = NULL, retry_count = $2, queued_at = NOW(), execute_at = $3 WHERE id = $1",
        )
        .bind(job_id)
        .bind(retry_count)
        .bind(execute_at)
        .execute(self.ctx.pool())
        .await?;

        let queued_at = Utc::now().to_rfc3339();
        self.ctx
            .events()
            .publish(DomainEvent::ScrapeJob(ScrapeJobEvent::Retried {
                id: job_id,
                retry_count,
                queued_at,
                status: ScrapeJobStatus::Pending,
            }));

        Ok(())
    }

    /// Mark a job as exhausted (max retries exceeded).
    ///
    /// Emits `ScrapeJobEvent::Exhausted` then `ScrapeJobEvent::Deleted`.
    pub async fn exhaust(&self, job_id: i32) -> Result<()> {
        sqlx::query("DELETE FROM scrape_jobs WHERE id = $1")
            .bind(job_id)
            .execute(self.ctx.pool())
            .await?;

        self.ctx
            .events()
            .publish(DomainEvent::ScrapeJob(ScrapeJobEvent::Exhausted {
                id: job_id,
            }));
        self.ctx
            .events()
            .publish(DomainEvent::ScrapeJob(ScrapeJobEvent::Deleted {
                id: job_id,
            }));

        Ok(())
    }

    /// Force-unlock all jobs that have a non-NULL `locked_at`.
    ///
    /// Intended to be called once at startup to recover jobs left locked by
    /// a previous unclean shutdown.
    pub async fn force_unlock_all(&self) -> Result<u64> {
        let result = sqlx::query(
            "UPDATE scrape_jobs SET locked_at = NULL, queued_at = NOW() WHERE locked_at IS NOT NULL",
        )
        .execute(self.ctx.pool())
        .await?;
        Ok(result.rows_affected())
    }

    /// Unlock a scrape job by clearing its `locked_at` timestamp.
    ///
    /// Used to release a job back to the queue during graceful shutdown.
    pub async fn unlock(&self, job_id: i32) -> Result<()> {
        sqlx::query("UPDATE scrape_jobs SET locked_at = NULL WHERE id = $1")
            .bind(job_id)
            .execute(self.ctx.pool())
            .await?;
        Ok(())
    }

    /// Insert a scrape job result log entry.
    #[allow(clippy::too_many_arguments)]
    pub async fn insert_result(
        &self,
        target_type: TargetType,
        payload: serde_json::Value,
        priority: ScrapePriority,
        queued_at: DateTime<Utc>,
        started_at: DateTime<Utc>,
        duration_ms: i32,
        success: bool,
        error_message: Option<&str>,
        retry_count: i32,
        counts: Option<&UpsertCounts>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO scrape_job_results (
                target_type, payload, priority,
                queued_at, started_at, duration_ms,
                success, error_message, retry_count,
                courses_fetched, courses_changed, courses_unchanged,
                audits_generated, metrics_generated
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
            "#,
        )
        .bind(target_type)
        .bind(&payload)
        .bind(priority)
        .bind(queued_at)
        .bind(started_at)
        .bind(duration_ms)
        .bind(success)
        .bind(error_message)
        .bind(retry_count)
        .bind(counts.map(|c| c.courses_fetched))
        .bind(counts.map(|c| c.courses_changed))
        .bind(counts.map(|c| c.courses_unchanged))
        .bind(counts.map(|c| c.audits_generated))
        .bind(counts.map(|c| c.metrics_generated))
        .execute(self.ctx.pool())
        .await?;

        Ok(())
    }

    /// Find existing job payloads matching the given target type and candidates.
    ///
    /// Returns a set of stringified JSON payloads that already exist in the queue,
    /// used for deduplication when scheduling new jobs.
    pub async fn find_existing_payloads(
        &self,
        target_type: TargetType,
        candidate_payloads: &[serde_json::Value],
    ) -> Result<HashSet<String>> {
        let existing_jobs: Vec<(serde_json::Value,)> = sqlx::query_as(
            "SELECT target_payload FROM scrape_jobs
             WHERE target_type = $1 AND target_payload = ANY($2)",
        )
        .bind(target_type)
        .bind(candidate_payloads)
        .fetch_all(self.ctx.pool())
        .await?;

        let existing_payloads = existing_jobs
            .into_iter()
            .map(|(payload,)| payload.to_string())
            .collect();

        Ok(existing_payloads)
    }

    /// Fetch aggregated per-(subject, term) statistics from the last 24 hours of results.
    ///
    /// For past/archived terms, results older than 24h are still relevant since they
    /// scrape on 48h intervals. We use a 72h window to capture at least one full cycle.
    pub async fn fetch_subject_stats(&self) -> Result<Vec<SubjectResultStats>> {
        let rows = sqlx::query_as::<_, SubjectResultStats>(
            r#"
            WITH recent AS (
                SELECT payload->>'subject' AS subject,
                       COALESCE(payload->>'term', '') AS term,
                       success,
                       COALESCE(courses_fetched, 0) AS courses_fetched,
                       COALESCE(courses_changed, 0) AS courses_changed,
                       completed_at,
                       ROW_NUMBER() OVER (PARTITION BY payload->>'subject', payload->>'term' ORDER BY completed_at DESC) AS rn
                FROM scrape_job_results
                WHERE target_type = 'Subject' AND completed_at > NOW() - INTERVAL '72 hours'
            ),
            filtered AS (SELECT * FROM recent WHERE rn <= 20),
            zero_break AS (
                SELECT subject, term,
                       MIN(rn) FILTER (WHERE courses_changed > 0 AND success) AS first_nonzero_rn,
                       MIN(rn) FILTER (WHERE courses_fetched > 0 AND success) AS first_nonempty_rn
                FROM filtered GROUP BY subject, term
            )
            SELECT
                f.subject::TEXT AS subject,
                f.term::TEXT AS term,
                COUNT(*)::BIGINT AS recent_runs,
                COALESCE(AVG(CASE WHEN f.success AND f.courses_fetched > 0
                     THEN f.courses_changed::FLOAT / f.courses_fetched ELSE NULL END), 0.0)::FLOAT8 AS avg_change_ratio,
                COALESCE(zb.first_nonzero_rn - 1, COUNT(*) FILTER (WHERE f.success AND f.courses_changed = 0))::BIGINT AS consecutive_zero_changes,
                COALESCE(zb.first_nonempty_rn - 1, COUNT(*) FILTER (WHERE f.success AND f.courses_fetched = 0))::BIGINT AS consecutive_empty_fetches,
                COUNT(*) FILTER (WHERE NOT f.success)::BIGINT AS recent_failure_count,
                COUNT(*) FILTER (WHERE f.success)::BIGINT AS recent_success_count,
                MAX(f.completed_at) AS last_completed
            FROM filtered f
            LEFT JOIN zero_break zb ON f.subject = zb.subject AND f.term = zb.term
            GROUP BY f.subject, f.term, zb.first_nonzero_rn, zb.first_nonempty_rn
            "#,
        )
        .fetch_all(self.ctx.pool())
        .await?;

        Ok(rows)
    }

    /// Batch insert scrape jobs using UNNEST for a single round-trip.
    ///
    /// All jobs are inserted with `execute_at` set to the current time.
    /// Emits a `ScrapeJobEvent::Created` event for each inserted job.
    pub async fn batch_insert(
        &self,
        jobs: &[(serde_json::Value, TargetType, ScrapePriority)],
    ) -> Result<Vec<ScrapeJob>> {
        if jobs.is_empty() {
            return Ok(Vec::new());
        }

        let mut target_types: Vec<String> = Vec::with_capacity(jobs.len());
        let mut payloads: Vec<serde_json::Value> = Vec::with_capacity(jobs.len());
        let mut priorities: Vec<String> = Vec::with_capacity(jobs.len());

        for (payload, target_type, priority) in jobs {
            target_types.push(format!("{target_type:?}"));
            payloads.push(payload.clone());
            priorities.push(format!("{priority:?}"));
        }

        let inserted = sqlx::query_as::<_, ScrapeJob>(
            r#"
            INSERT INTO scrape_jobs (target_type, target_payload, priority, execute_at, queued_at)
            SELECT v.target_type::target_type, v.payload, v.priority::scrape_priority, NOW(), NOW()
            FROM UNNEST($1::text[], $2::jsonb[], $3::text[])
                AS v(target_type, payload, priority)
            RETURNING *
            "#,
        )
        .bind(&target_types)
        .bind(&payloads)
        .bind(&priorities)
        .fetch_all(self.ctx.pool())
        .await?;

        for job in &inserted {
            debug!(job_id = job.id, "Emitting JobCreated event");
            self.ctx
                .events()
                .publish(DomainEvent::ScrapeJob(ScrapeJobEvent::Created {
                    job: ScrapeJobDto::from(job),
                }));
        }

        Ok(inserted)
    }
}
