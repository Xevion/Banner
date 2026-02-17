use crate::banner::{BannerApi, Term};
use crate::data::DbContext;
use crate::data::models::{ReferenceData, ScrapePriority, TargetType};
use crate::data::terms;
use crate::rmp::RmpClient;
use crate::scraper::adaptive::{SubjectSchedule, SubjectStats, evaluate_subject};
use crate::scraper::jobs::subject::SubjectJob;
use crate::state::ReferenceCache;
use crate::utils::fmt_duration;
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde_json::json;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, broadcast};
use tokio::time;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

/// How often reference data is re-scraped (6 hours).
const REFERENCE_DATA_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);

/// How often RMP data is synced (24 hours).
const RMP_SYNC_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

/// How often terms are synced from Banner API (8 hours).
const TERM_SYNC_INTERVAL: Duration = Duration::from_secs(8 * 60 * 60);

const SLOW_QUERY_THRESHOLD: Duration = Duration::from_millis(500);

/// Periodically analyzes data and enqueues prioritized scrape jobs.
pub struct Scheduler {
    db: DbContext,
    banner_api: Arc<BannerApi>,
    reference_cache: Arc<RwLock<ReferenceCache>>,
}

impl Scheduler {
    pub fn new(
        db: DbContext,
        banner_api: Arc<BannerApi>,
        reference_cache: Arc<RwLock<ReferenceCache>>,
    ) -> Self {
        Self {
            db,
            banner_api,
            reference_cache,
        }
    }

    /// Runs the scheduler's main loop with graceful shutdown support.
    ///
    /// The scheduler wakes up every 60 seconds to analyze data and enqueue jobs.
    /// When a shutdown signal is received:
    /// 1. Any in-progress scheduling work is gracefully cancelled via CancellationToken
    /// 2. The scheduler waits up to 5 seconds for work to complete
    /// 3. If timeout occurs, the task is abandoned (it will be aborted when dropped)
    ///
    /// This ensures that shutdown is responsive even if scheduling work is blocked.
    pub async fn run(&self, mut shutdown_rx: broadcast::Receiver<()>) {
        info!("Scheduler service started");

        let work_interval = Duration::from_secs(60);
        let mut next_run = time::Instant::now();
        let mut current_work: Option<(tokio::task::JoinHandle<()>, CancellationToken)> = None;
        // Scrape reference data immediately on first cycle
        let mut last_ref_scrape = Instant::now() - REFERENCE_DATA_INTERVAL;
        // Sync RMP data immediately on first cycle
        let mut last_rmp_sync = Instant::now() - RMP_SYNC_INTERVAL;
        // Sync terms immediately on first cycle (also done at startup, but scheduler handles periodic)
        let mut last_term_sync = Instant::now() - TERM_SYNC_INTERVAL;

        loop {
            tokio::select! {
                _ = time::sleep_until(next_run) => {
                    let cancel_token = CancellationToken::new();

                    let should_scrape_ref = last_ref_scrape.elapsed() >= REFERENCE_DATA_INTERVAL;
                    let should_sync_rmp = last_rmp_sync.elapsed() >= RMP_SYNC_INTERVAL;
                    let should_sync_terms = last_term_sync.elapsed() >= TERM_SYNC_INTERVAL;

                    // Spawn work in separate task to allow graceful cancellation during shutdown.
                    let work_handle = tokio::spawn({
                        let db = self.db.clone();
                        let banner_api = self.banner_api.clone();
                        let cancel_token = cancel_token.clone();
                        let reference_cache = self.reference_cache.clone();

                                async move {
                                    tokio::select! {
                                        _ = async {
                                            // Term sync, RMP sync, and reference data are independent —
                                            // run them concurrently so they don't wait behind each other.
                                            let term_fut = async {
                                                if should_sync_terms
                                                    && let Err(e) = Self::sync_terms(db.pool(), &banner_api).await
                                                {
                                                    error!(error = ?e, "Failed to sync terms");
                                                }
                                            };

                                            let rmp_fut = async {
                                                if should_sync_rmp
                                                    && let Err(e) = Self::sync_rmp_data(db.pool()).await
                                                {
                                                    error!(error = ?e, "Failed to sync RMP data");
                                                }
                                            };

                                            let ref_fut = async {
                                                if should_scrape_ref
                                                    && let Err(e) = Self::scrape_reference_data(db.pool(), &banner_api, &reference_cache).await
                                                {
                                                    error!(error = ?e, "Failed to scrape reference data");
                                                }
                                            };

                                            tokio::join!(term_fut, rmp_fut, ref_fut);

                                            if let Err(e) = Self::schedule_jobs_impl(&db, &banner_api).await {
                                                error!(error = ?e, "Failed to schedule jobs");
                                            }
                                        } => {}
                                        _ = cancel_token.cancelled() => {
                                            debug!("Scheduling work cancelled gracefully");
                                        }
                                    }
                                }
                    });

                    if should_scrape_ref {
                        last_ref_scrape = Instant::now();
                    }
                    if should_sync_rmp {
                        last_rmp_sync = Instant::now();
                    }
                    if should_sync_terms {
                        last_term_sync = Instant::now();
                    }

                    current_work = Some((work_handle, cancel_token));
                    next_run = time::Instant::now() + work_interval;
                }
                _ = shutdown_rx.recv() => {
                    info!("Scheduler received shutdown signal");

                    if let Some((handle, cancel_token)) = current_work.take() {
                        cancel_token.cancel();

                        // Wait briefly for graceful completion
                        if tokio::time::timeout(Duration::from_secs(5), handle).await.is_err() {
                            warn!("Scheduling work did not complete within 5s, abandoning");
                        } else {
                            debug!("Scheduling work completed gracefully");
                        }
                    }

                    info!("Scheduler exiting gracefully");
                    break;
                }
            }
        }
    }

    /// Core scheduling logic that analyzes data and creates scrape jobs.
    ///
    /// Queries all enabled terms from the `terms` table and schedules jobs for each.
    /// Uses adaptive scheduling to determine per-subject scrape intervals based
    /// on recent change rates, failure patterns, and time of day.
    ///
    /// This is a static method (not &self) to allow it to be called from spawned tasks.
    async fn schedule_jobs_impl(db: &DbContext, banner_api: &BannerApi) -> Result<()> {
        // Query enabled terms from database
        let start = Instant::now();
        let enabled_terms = terms::get_enabled_term_codes(db.pool()).await?;
        let elapsed = start.elapsed();
        if elapsed > SLOW_QUERY_THRESHOLD {
            warn!(
                duration = fmt_duration(elapsed),
                "Slow query: get_enabled_term_codes"
            );
        }

        if enabled_terms.is_empty() {
            debug!("No enabled terms to schedule");
            return Ok(());
        }

        info!(terms = ?enabled_terms, "Scheduling jobs for enabled terms");

        for term_code in enabled_terms {
            if let Err(e) = Self::schedule_term_jobs(db, banner_api, &term_code).await {
                error!(term = %term_code, error = ?e, "Failed to schedule jobs for term");
                // Continue with other terms
            }
        }

        debug!("Job scheduling complete");
        Ok(())
    }

    /// Schedule jobs for a single term.
    #[tracing::instrument(skip_all, fields(term = %term_code))]
    async fn schedule_term_jobs(
        db: &DbContext,
        banner_api: &BannerApi,
        term_code: &str,
    ) -> Result<()> {
        debug!("Enqueuing subject jobs for term");

        let subjects = banner_api.get_subjects("", term_code, 1, 500).await?;
        debug!(
            subject_count = subjects.len(),
            "Retrieved subjects from API"
        );

        // Fetch per-subject stats and build a lookup map
        // Note: Currently stats are not term-aware, so we use global stats.
        let start = Instant::now();
        let stats_rows = db.scrape_jobs().fetch_subject_stats().await?;
        let elapsed = start.elapsed();
        if elapsed > SLOW_QUERY_THRESHOLD {
            warn!(
                duration = fmt_duration(elapsed),
                "Slow query: fetch_subject_stats"
            );
        }
        let stats_map: HashMap<String, SubjectStats> = stats_rows
            .into_iter()
            .map(|row| {
                let subject = row.subject.clone();
                (subject, SubjectStats::from(row))
            })
            .collect();

        // Determine if this is a past term (for ReadOnly mode)
        let current_term_code = Term::get_current().inner().to_string();
        let is_past_term = term_code < current_term_code.as_str();

        // Evaluate each subject using adaptive scheduling
        let now = Utc::now();
        let mut eligible_subjects: Vec<String> = Vec::new();
        let mut cooldown_count: usize = 0;
        let mut paused_count: usize = 0;
        let mut read_only_count: usize = 0;

        for subject in &subjects {
            let stats = stats_map.get(&subject.code).cloned().unwrap_or_else(|| {
                // Cold start: no history for this subject
                SubjectStats {
                    subject: subject.code.clone(),
                    recent_runs: 0,
                    avg_change_ratio: 0.0,
                    consecutive_zero_changes: 0,
                    consecutive_empty_fetches: 0,
                    recent_failure_count: 0,
                    recent_success_count: 0,
                    last_completed: DateTime::<Utc>::MIN_UTC,
                }
            });

            match evaluate_subject(&stats, now, is_past_term) {
                SubjectSchedule::Eligible(_) => {
                    eligible_subjects.push(subject.code.clone());
                }
                SubjectSchedule::Cooldown(_) => cooldown_count += 1,
                SubjectSchedule::Paused => paused_count += 1,
                SubjectSchedule::ReadOnly => read_only_count += 1,
            }
        }

        info!(
            total = subjects.len(),
            eligible = eligible_subjects.len(),
            cooldown = cooldown_count,
            paused = paused_count,
            read_only = read_only_count,
            is_past_term,
            "Adaptive scheduling decisions"
        );

        if eligible_subjects.is_empty() {
            debug!("No eligible subjects to schedule");
            return Ok(());
        }

        // Create payloads with term field for eligible subjects
        let subject_payloads: Vec<_> = eligible_subjects
            .iter()
            .map(|code| json!({ "subject": code, "term": term_code }))
            .collect();

        // Query existing jobs for eligible subjects only
        let start = Instant::now();
        let existing_payloads = db
            .scrape_jobs()
            .find_existing_payloads(TargetType::Subject, &subject_payloads)
            .await?;
        let elapsed = start.elapsed();
        if elapsed > SLOW_QUERY_THRESHOLD {
            warn!(
                duration = fmt_duration(elapsed),
                "Slow query: find_existing_payloads"
            );
        }

        // Filter out subjects that already have pending jobs
        let mut skipped_count = 0;
        let new_jobs: Vec<_> = eligible_subjects
            .into_iter()
            .filter_map(|subject_code| {
                let job = SubjectJob::new(subject_code.clone(), term_code.to_string());
                let payload = serde_json::to_value(&job).unwrap();
                let payload_str = payload.to_string();

                if existing_payloads.contains(&payload_str) {
                    skipped_count += 1;
                    None
                } else {
                    Some((payload, subject_code))
                }
            })
            .collect();

        if skipped_count > 0 {
            debug!(count = skipped_count, "Skipped subjects with existing jobs");
        }

        // Insert all new jobs in a single batch (events emitted automatically)
        if !new_jobs.is_empty() {
            for (_, subject_code) in &new_jobs {
                debug!(subject = %subject_code, "New job enqueued for subject");
            }

            let jobs: Vec<_> = new_jobs
                .into_iter()
                .map(|(payload, _)| (payload, TargetType::Subject, ScrapePriority::Low))
                .collect();

            let start = Instant::now();
            db.scrape_jobs().batch_insert(&jobs).await?;
            let elapsed = start.elapsed();
            if elapsed > SLOW_QUERY_THRESHOLD {
                warn!(
                    duration = fmt_duration(elapsed),
                    count = jobs.len(),
                    "Slow query: batch_insert"
                );
            }
        }

        Ok(())
    }

    /// Sync terms from Banner API to database (periodic background job).
    #[tracing::instrument(skip_all)]
    async fn sync_terms(db_pool: &PgPool, banner_api: &BannerApi) -> Result<()> {
        info!("Starting term sync from Banner API");

        let banner_terms = banner_api.sessions.get_terms("", 1, 500).await?;
        let start = Instant::now();
        let result = terms::sync_terms_from_banner(db_pool, banner_terms).await?;
        let elapsed = start.elapsed();
        if elapsed > SLOW_QUERY_THRESHOLD {
            warn!(
                duration = fmt_duration(elapsed),
                "Slow query: sync_terms_from_banner"
            );
        }

        info!(
            inserted = result.inserted,
            updated = result.updated,
            "Term sync completed"
        );

        Ok(())
    }

    /// Fetch all RMP professors, upsert to DB, and auto-match against Banner instructors.
    #[tracing::instrument(skip_all)]
    async fn sync_rmp_data(db_pool: &PgPool) -> Result<()> {
        info!("Starting RMP data sync");

        let client = RmpClient::new();
        let professors = client.fetch_all_professors().await?;
        let total = professors.len();

        let start = Instant::now();
        crate::data::rmp::batch_upsert_rmp_professors(&professors, db_pool).await?;
        let elapsed = start.elapsed();
        if elapsed > SLOW_QUERY_THRESHOLD {
            warn!(
                duration = fmt_duration(elapsed),
                count = total,
                "Slow query: batch_upsert_rmp_professors"
            );
        }
        info!(total, "RMP professors upserted");

        let start = Instant::now();
        let stats = crate::data::rmp_matching::generate_candidates(db_pool).await?;
        let elapsed = start.elapsed();
        if elapsed > SLOW_QUERY_THRESHOLD {
            warn!(
                duration = fmt_duration(elapsed),
                "Slow query: generate_candidates"
            );
        }
        info!(
            total,
            stats.total_unmatched,
            stats.candidates_created,
            stats.candidates_rescored,
            stats.auto_matched,
            stats.skipped_unparseable,
            stats.skipped_no_candidates,
            "RMP sync complete"
        );

        Ok(())
    }

    /// Scrape all reference data categories from Banner and upsert to DB, then refresh cache.
    #[tracing::instrument(skip_all)]
    async fn scrape_reference_data(
        db_pool: &PgPool,
        banner_api: &BannerApi,
        reference_cache: &Arc<RwLock<ReferenceCache>>,
    ) -> Result<()> {
        let term = Term::get_current().inner().to_string();
        info!(term = %term, "Scraping reference data");

        let mut all_entries = Vec::new();

        // Terms (fetched via session pool, no active session needed)
        match banner_api.sessions.get_terms("", 1, 500).await {
            Ok(terms) => {
                debug!(count = terms.len(), "Fetched terms");
                all_entries.extend(terms.into_iter().map(|t| ReferenceData {
                    category: "term".to_string(),
                    code: t.code,
                    description: t.description,
                }));
            }
            Err(e) => warn!(error = ?e, "Failed to fetch terms"),
        }

        // Subjects
        match banner_api.get_subjects("", &term, 1, 500).await {
            Ok(pairs) => {
                debug!(count = pairs.len(), "Fetched subjects");
                all_entries.extend(pairs.into_iter().map(|p| ReferenceData {
                    category: "subject".to_string(),
                    code: p.code,
                    description: p.description,
                }));
            }
            Err(e) => warn!(error = ?e, "Failed to fetch subjects"),
        }

        // Campuses
        match banner_api.get_campuses(&term).await {
            Ok(pairs) => {
                debug!(count = pairs.len(), "Fetched campuses");
                all_entries.extend(pairs.into_iter().map(|p| ReferenceData {
                    category: "campus".to_string(),
                    code: p.code,
                    description: p.description,
                }));
            }
            Err(e) => warn!(error = ?e, "Failed to fetch campuses"),
        }

        // Instructional methods
        match banner_api.get_instructional_methods(&term).await {
            Ok(pairs) => {
                debug!(count = pairs.len(), "Fetched instructional methods");
                all_entries.extend(pairs.into_iter().map(|p| ReferenceData {
                    category: "instructional_method".to_string(),
                    code: p.code,
                    description: p.description,
                }));
            }
            Err(e) => warn!(error = ?e, "Failed to fetch instructional methods"),
        }

        // Parts of term
        match banner_api.get_parts_of_term(&term).await {
            Ok(pairs) => {
                debug!(count = pairs.len(), "Fetched parts of term");
                all_entries.extend(pairs.into_iter().map(|p| ReferenceData {
                    category: "part_of_term".to_string(),
                    code: p.code,
                    description: p.description,
                }));
            }
            Err(e) => warn!(error = ?e, "Failed to fetch parts of term"),
        }

        // Attributes
        match banner_api.get_attributes(&term).await {
            Ok(pairs) => {
                debug!(count = pairs.len(), "Fetched attributes");
                all_entries.extend(pairs.into_iter().map(|p| ReferenceData {
                    category: "attribute".to_string(),
                    code: p.code,
                    description: p.description,
                }));
            }
            Err(e) => warn!(error = ?e, "Failed to fetch attributes"),
        }

        // Batch upsert all entries
        let total = all_entries.len();
        let start = Instant::now();
        crate::data::reference::batch_upsert(&all_entries, db_pool).await?;
        let elapsed = start.elapsed();
        if elapsed > SLOW_QUERY_THRESHOLD {
            warn!(
                duration = fmt_duration(elapsed),
                count = total,
                "Slow query: reference::batch_upsert"
            );
        }
        info!(total_entries = total, "Reference data upserted to DB");

        // Refresh in-memory cache
        let start = Instant::now();
        let all = crate::data::reference::get_all(db_pool).await?;
        let elapsed = start.elapsed();
        if elapsed > SLOW_QUERY_THRESHOLD {
            warn!(
                duration = fmt_duration(elapsed),
                "Slow query: reference::get_all"
            );
        }
        let count = all.len();
        *reference_cache.write().await = ReferenceCache::from_entries(all);
        info!(entries = count, "Reference cache refreshed");

        Ok(())
    }
}
