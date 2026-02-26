use crate::banner::{BannerApi, BannerApiError};
use crate::data::DbContext;
use crate::data::models::{ScrapeJob, UpsertCounts};
use crate::data::terms;
use crate::data::unsigned::{Count, DurationMs};
use crate::scraper::jobs::{JobError, JobType};
use crate::utils::fmt_duration;
use anyhow::Result;
use chrono::{DateTime, Utc};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::time;
use tracing::{Instrument, debug, error, info, trace, warn};

/// Maximum time a single job is allowed to run before being considered stuck.
const JOB_TIMEOUT: Duration = Duration::from_secs(5 * 60);

/// A single worker instance.
///
/// Each worker runs in its own asynchronous task and continuously polls the
/// database for scrape jobs to execute.
pub struct Worker {
    id: usize,
    db: DbContext,
    banner_api: Arc<BannerApi>,
}

impl Worker {
    pub fn new(id: usize, db: DbContext, banner_api: Arc<BannerApi>) -> Self {
        Self { id, db, banner_api }
    }

    /// Runs the worker's main loop.
    pub async fn run(&self, mut shutdown_rx: broadcast::Receiver<()>) {
        info!(worker_id = self.id, "Worker started");

        loop {
            // Fetch and lock a job, racing against shutdown signal
            let job = tokio::select! {
                _ = shutdown_rx.recv() => {
                    info!(worker_id = self.id, "Worker received shutdown signal, exiting gracefully");
                    break;
                }
                result = self.fetch_and_lock_job() => {
                    match result {
                        Ok(Some(job)) => job,
                        Ok(None) => {
                            trace!(worker_id = self.id, "No jobs available, waiting");
                            time::sleep(Duration::from_secs(5)).await;
                            continue;
                        }
                        Err(e) => {
                            warn!(worker_id = self.id, error = ?e, "Failed to fetch job, waiting");
                            time::sleep(Duration::from_secs(10)).await;
                            continue;
                        }
                    }
                }
            };

            let job_id = job.id;
            let retry_count = job.retry_count;
            let max_retries = job.max_retries;
            let target_type = job.target_type;
            let payload = job.target_payload.clone();
            let priority = job.priority;
            let queued_at = job.queued_at;
            let started_at = Utc::now();
            let start = std::time::Instant::now();

            // Locked event is emitted automatically by lock_next()

            // Process the job, racing against shutdown signal and timeout
            let process_result = tokio::select! {
                _ = shutdown_rx.recv() => {
                    self.handle_shutdown_during_processing(job_id).await;
                    break;
                }
                result = async {
                    match time::timeout(JOB_TIMEOUT, self.process_job(job)).await {
                        Ok(result) => result,
                        Err(_elapsed) => {
                            Err(JobError::Recoverable(anyhow::anyhow!(
                                "job timed out after {}s",
                                JOB_TIMEOUT.as_secs()
                            )))
                        }
                    }
                } => result
            };

            let duration = start.elapsed();

            // Handle the job processing result
            self.handle_job_result(
                job_id,
                retry_count,
                max_retries,
                process_result,
                duration,
                target_type,
                payload,
                priority,
                queued_at,
                started_at,
            )
            .await;
        }
    }

    /// Atomically fetches a job from the queue, locking it for processing.
    ///
    /// This uses a `FOR UPDATE SKIP LOCKED` query to ensure that multiple
    /// workers can poll the queue concurrently without conflicts.
    /// Emits a `ScrapeJobEvent::Locked` event automatically via DbContext.
    async fn fetch_and_lock_job(&self) -> Result<Option<ScrapeJob>> {
        self.db.scrape_jobs().lock_next().await
    }

    async fn process_job(&self, job: ScrapeJob) -> Result<UpsertCounts, JobError> {
        // Convert the database job to our job type
        let job_type = JobType::from_target_type_and_payload(job.target_type, job.target_payload)
            .map_err(|e| JobError::Unrecoverable(anyhow::anyhow!(e)))?; // Parse errors are unrecoverable

        // Get the job implementation
        let job_impl = job_type.boxed();

        // Create span with job context
        let span = tracing::info_span!("process_job", job_id = job.id);

        async move {
            debug!(worker_id = self.id, "Processing job");

            // Process the job - API errors are recoverable
            job_impl
                .process(&self.banner_api, &self.db)
                .await
                .map_err(JobError::Recoverable)
        }
        .instrument(span)
        .await
    }

    async fn unlock_job(&self, job_id: i32) -> Result<()> {
        self.db.scrape_jobs().unlock(job_id).await
    }

    /// Handle shutdown signal received during job processing
    async fn handle_shutdown_during_processing(&self, job_id: i32) {
        info!(
            worker_id = self.id,
            job_id, "Shutdown received during job processing"
        );

        if let Err(e) = self.unlock_job(job_id).await {
            warn!(
                worker_id = self.id,
                job_id,
                error = ?e,
                "Failed to unlock job during shutdown"
            );
        } else {
            debug!(worker_id = self.id, job_id, "Job unlocked during shutdown");
        }

        info!(worker_id = self.id, "Worker exiting gracefully");
    }

    /// Handle the result of job processing
    #[allow(clippy::too_many_arguments)]
    async fn handle_job_result(
        &self,
        job_id: i32,
        retry_count: Count,
        max_retries: Count,
        result: Result<UpsertCounts, JobError>,
        duration: std::time::Duration,
        target_type: crate::data::models::TargetType,
        payload: serde_json::Value,
        priority: crate::data::models::ScrapePriority,
        queued_at: DateTime<Utc>,
        started_at: DateTime<Utc>,
    ) {
        let duration_ms = DurationMs::new(u32::try_from(duration.as_millis()).unwrap_or(u32::MAX));

        const SLOW_THRESHOLD: Duration = Duration::from_secs(30);
        if duration > SLOW_THRESHOLD {
            warn!(
                worker_id = self.id,
                job_id,
                duration = fmt_duration(duration),
                "Slow job processing detected (likely rate limiting or network delays)"
            );
        }

        match result {
            Ok(counts) => {
                // Log at INFO if data changed, DEBUG if no changes
                let has_changes = counts.courses_changed > Count::default();
                if has_changes {
                    info!(
                        worker_id = self.id,
                        job_id,
                        duration = fmt_duration(duration),
                        courses_fetched = %counts.courses_fetched,
                        courses_changed = %counts.courses_changed,
                        courses_unchanged = %counts.courses_unchanged,
                        "Job completed with changes"
                    );
                } else {
                    debug!(
                        worker_id = self.id,
                        job_id,
                        duration = fmt_duration(duration),
                        courses_fetched = %counts.courses_fetched,
                        courses_changed = %counts.courses_changed,
                        courses_unchanged = %counts.courses_unchanged,
                        "Job completed (no changes)"
                    );
                }

                // Extract term code before payload is moved into insert_result
                let term_code = payload
                    .get("term")
                    .and_then(|v| v.as_str())
                    .map(String::from);

                // Log the result
                if let Err(e) = self
                    .db
                    .scrape_jobs()
                    .insert_result(
                        target_type,
                        payload,
                        priority,
                        queued_at,
                        started_at,
                        duration_ms,
                        true,
                        None,
                        retry_count,
                        Some(&counts),
                    )
                    .await
                {
                    error!(worker_id = self.id, job_id, error = ?e, "Failed to insert job result");
                }

                // Mark job as completed (deletes it and emits Completed event)
                if let Err(e) = self.db.scrape_jobs().complete(job_id).await {
                    error!(worker_id = self.id, job_id, error = ?e, "Failed to complete job");
                }

                // Update last_scraped_at for the term if this job has a term code
                if let Some(code) = term_code {
                    let _ = terms::update_last_scraped_at(self.db.pool(), &code).await
                        .map_err(|e| warn!(worker_id = self.id, job_id, term_code = code.as_str(), error = ?e, "Failed to update last_scraped_at"));
                }
            }
            Err(JobError::Recoverable(e)) => {
                self.handle_recoverable_error(
                    job_id,
                    retry_count,
                    max_retries,
                    e,
                    duration,
                    target_type,
                    payload,
                    priority,
                    queued_at,
                    started_at,
                )
                .await;
            }
            Err(JobError::Unrecoverable(e)) => {
                // Log the failed result
                let err_msg = format!("{e:#}");
                if let Err(log_err) = self
                    .db
                    .scrape_jobs()
                    .insert_result(
                        target_type,
                        payload,
                        priority,
                        queued_at,
                        started_at,
                        duration_ms,
                        false,
                        Some(&err_msg),
                        retry_count,
                        None,
                    )
                    .await
                {
                    error!(worker_id = self.id, job_id, error = ?log_err, "Failed to insert job result");
                }

                error!(
                    worker_id = self.id,
                    job_id,
                    duration = fmt_duration(duration),
                    error = ?e,
                    "Job corrupted, deleting"
                );
                // Delete job (emits Deleted event automatically)
                if let Err(e) = self.db.scrape_jobs().delete(job_id).await {
                    error!(worker_id = self.id, job_id, error = ?e, "Failed to delete corrupted job");
                }
            }
        }
    }

    /// Handle recoverable errors by logging appropriately and unlocking the job
    #[allow(clippy::too_many_arguments)]
    async fn handle_recoverable_error(
        &self,
        job_id: i32,
        retry_count: Count,
        max_retries: Count,
        e: anyhow::Error,
        duration: std::time::Duration,
        target_type: crate::data::models::TargetType,
        payload: serde_json::Value,
        priority: crate::data::models::ScrapePriority,
        queued_at: DateTime<Utc>,
        started_at: DateTime<Utc>,
    ) {
        let next_attempt = Count::new(retry_count.get().saturating_add(1));
        let remaining_retries = max_retries.get().saturating_sub(next_attempt.get());

        // Log the error appropriately based on type
        match e.downcast_ref::<BannerApiError>() {
            Some(BannerApiError::InvalidSession(_)) => {
                warn!(
                    worker_id = self.id,
                    job_id,
                    duration = fmt_duration(duration),
                    retry_attempt = %next_attempt,
                    max_retries = %max_retries,
                    remaining_retries,
                    "Invalid session detected, will retry"
                );
            }
            Some(BannerApiError::ParseFailed {
                status,
                url,
                source,
            }) => {
                error!(
                    worker_id = self.id,
                    job_id,
                    duration = fmt_duration(duration),
                    retry_attempt = %next_attempt,
                    max_retries = %max_retries,
                    remaining_retries,
                    status,
                    url,
                    error = ?source,
                    "Failed to parse search response"
                );
            }
            _ => {
                error!(
                    worker_id = self.id,
                    job_id,
                    duration = fmt_duration(duration),
                    retry_attempt = %next_attempt,
                    max_retries = %max_retries,
                    remaining_retries,
                    error = ?e,
                    "Failed to process job, will retry"
                );
            }
        }

        // Check if retries remain
        if next_attempt < max_retries {
            // Retry is allowed - unlock job with incremented retry count
            // Use immediate retry for now (execute_at = NOW)
            let execute_at = Utc::now();
            match self
                .db
                .scrape_jobs()
                .retry(job_id, next_attempt, execute_at)
                .await
            {
                Ok(()) => {
                    debug!(
                        worker_id = self.id,
                        job_id,
                        retry_attempt = %next_attempt,
                        remaining_retries,
                        "Job unlocked for retry"
                    );
                    // Retried event emitted automatically by retry()
                }
                Err(e) => {
                    error!(worker_id = self.id, job_id, error = ?e, "Failed to unlock job for retry");
                }
            }
        } else {
            // Max retries exceeded -- log final failure result
            let duration_ms =
                DurationMs::new(u32::try_from(duration.as_millis()).unwrap_or(u32::MAX));
            let err_msg = format!("{e:#}");
            if let Err(log_err) = self
                .db
                .scrape_jobs()
                .insert_result(
                    target_type,
                    payload,
                    priority,
                    queued_at,
                    started_at,
                    duration_ms,
                    false,
                    Some(&err_msg),
                    next_attempt,
                    None,
                )
                .await
            {
                error!(worker_id = self.id, job_id, error = ?log_err, "Failed to insert job result");
            }

            error!(
                worker_id = self.id,
                job_id,
                duration = fmt_duration(duration),
                retry_count = %next_attempt,
                max_retries = %max_retries,
                error = ?e,
                "Job failed permanently (max retries exceeded), deleting"
            );
            // Mark as exhausted (emits Exhausted + Deleted events automatically)
            if let Err(e) = self.db.scrape_jobs().exhaust(job_id).await {
                error!(worker_id = self.id, job_id, error = ?e, "Failed to exhaust job");
            }
        }
    }
}
