use crate::banner::{BannerApi, Term};
use crate::bluebook::BlueBookClient;
use crate::data::DbContext;
use crate::data::models::{ReferenceData, ScrapePriority, TargetType};
use crate::data::unsigned::Count;
use crate::data::{kv, term_subjects, terms};
use crate::rmp::RmpClient;
use crate::scraper::adaptive::{
    ARCHIVED_INTERVAL, SubjectSchedule, SubjectStats, TermCategory, evaluate_subject,
};
use crate::scraper::jobs::subject::SubjectJob;
use crate::state::ReferenceCache;
use crate::utils::fmt_duration;
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde_json::json;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::{Notify, RwLock, broadcast};
use tokio::time;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, trace, warn};

/// How often reference data is re-scraped (6 hours).
const REFERENCE_DATA_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);

/// How often RMP data is synced (24 hours).
const RMP_SYNC_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

/// How often terms are synced from Banner API (8 hours).
const TERM_SYNC_INTERVAL: Duration = Duration::from_secs(8 * 60 * 60);

/// How often to check which BlueBook subjects need re-scraping (1 day).
///
/// Per-subject re-scrape frequency is governed by `RECENT_SUBJECT_INTERVAL` (14 days)
/// and `HISTORICAL_SUBJECT_INTERVAL` (90 days) in `src/bluebook.rs`.
const BLUEBOOK_SYNC_INTERVAL: Duration = Duration::from_secs(24 * 3600);

/// How often to check for professors eligible for review scraping (15 minutes).
const RMP_REVIEW_SCRAPE_INTERVAL: Duration = Duration::from_secs(15 * 60);

/// Max professors to scrape reviews for per cycle.
const RMP_REVIEW_SCRAPE_BATCH_SIZE: i64 = 50;

const SLOW_QUERY_THRESHOLD: Duration = Duration::from_millis(500);

// app_kv keys for persisting scheduler timestamps across restarts.
pub const KV_REF_SCRAPE: &str = "scheduler.ref_scrape";
pub const KV_RMP_SYNC: &str = "scheduler.rmp_sync";
pub const KV_TERM_SYNC: &str = "scheduler.term_sync";
pub const KV_BLUEBOOK_SYNC: &str = "scheduler.bluebook_sync";
pub const KV_RMP_REVIEW_SCRAPE: &str = "scheduler.rmp_review_scrape";

/// Convert a persisted UTC timestamp to an `Instant`, preserving remaining cooldown.
///
/// If the persisted time is older than `interval`, returns an `Instant` that
/// triggers immediate execution. If it's recent, the returned `Instant` reflects
/// how much time has actually elapsed so the scheduler respects the remaining cooldown.
fn persisted_to_instant(persisted: Option<DateTime<Utc>>, interval: Duration) -> Instant {
    match persisted {
        None => Instant::now() - interval,
        Some(ts) => {
            let elapsed = (Utc::now() - ts).to_std().unwrap_or(interval);
            if elapsed >= interval {
                Instant::now() - interval
            } else {
                Instant::now() - elapsed
            }
        }
    }
}

/// Periodically analyzes data and enqueues prioritized scrape jobs.
pub struct Scheduler {
    db: DbContext,
    banner_api: Arc<BannerApi>,
    reference_cache: Arc<RwLock<ReferenceCache>>,
    /// Tracks when each archived term was last evaluated, so we can skip
    /// the expensive `get_subjects()` API call when no subjects can possibly
    /// be eligible yet (archived interval is 48 hours).
    archived_eval_times: Arc<std::sync::Mutex<HashMap<String, Instant>>>,
    bluebook_notify: Arc<Notify>,
    /// When true, the next BlueBook sync ignores per-subject interval checks.
    bluebook_force_flag: Arc<AtomicBool>,
}

impl Scheduler {
    pub fn new(
        db: DbContext,
        banner_api: Arc<BannerApi>,
        reference_cache: Arc<RwLock<ReferenceCache>>,
        bluebook_notify: Arc<Notify>,
        bluebook_force_flag: Arc<AtomicBool>,
    ) -> Self {
        Self {
            db,
            banner_api,
            reference_cache,
            archived_eval_times: Arc::new(std::sync::Mutex::new(HashMap::new())),
            bluebook_notify,
            bluebook_force_flag,
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

        // Load persisted timestamps so we don't redo work that completed recently.
        let pool = self.db.pool();
        let persisted_ref = kv::get_timestamp(pool, KV_REF_SCRAPE).await.unwrap_or(None);
        let persisted_rmp = kv::get_timestamp(pool, KV_RMP_SYNC).await.unwrap_or(None);
        let persisted_term = kv::get_timestamp(pool, KV_TERM_SYNC).await.unwrap_or(None);
        let persisted_bb = kv::get_timestamp(pool, KV_BLUEBOOK_SYNC)
            .await
            .unwrap_or(None);
        let persisted_rmp_reviews = kv::get_timestamp(pool, KV_RMP_REVIEW_SCRAPE)
            .await
            .unwrap_or(None);

        if persisted_ref.is_some()
            || persisted_rmp.is_some()
            || persisted_term.is_some()
            || persisted_bb.is_some()
            || persisted_rmp_reviews.is_some()
        {
            info!(
                last_ref_scrape = ?persisted_ref,
                last_rmp_sync = ?persisted_rmp,
                last_term_sync = ?persisted_term,
                last_bluebook_sync = ?persisted_bb,
                last_rmp_review_scrape = ?persisted_rmp_reviews,
                "Loaded persisted scheduler timestamps"
            );
        }

        let mut last_ref_scrape = persisted_to_instant(persisted_ref, REFERENCE_DATA_INTERVAL);
        let mut last_rmp_sync = persisted_to_instant(persisted_rmp, RMP_SYNC_INTERVAL);
        let mut last_term_sync = persisted_to_instant(persisted_term, TERM_SYNC_INTERVAL);
        let mut last_bluebook_sync = persisted_to_instant(persisted_bb, BLUEBOOK_SYNC_INTERVAL);
        let mut last_rmp_review_scrape =
            persisted_to_instant(persisted_rmp_reviews, RMP_REVIEW_SCRAPE_INTERVAL);
        let mut bluebook_notified = false;

        loop {
            tokio::select! {
                _ = self.bluebook_notify.notified() => {
                    info!("BlueBook sync triggered manually via notify");
                    bluebook_notified = true;
                    // Fall through to let the next sleep_until cycle pick it up immediately.
                    next_run = time::Instant::now();
                    continue;
                }
                _ = time::sleep_until(next_run) => {
                    // Skip this cycle if the previous one is still running.
                    if let Some((ref handle, _)) = current_work
                        && !handle.is_finished()
                    {
                        trace!("Previous scheduling cycle still running, skipping");
                        next_run = time::Instant::now() + work_interval;
                        continue;
                    }

                    let cancel_token = CancellationToken::new();

                    let should_scrape_ref = last_ref_scrape.elapsed() >= REFERENCE_DATA_INTERVAL;
                    let should_sync_rmp = last_rmp_sync.elapsed() >= RMP_SYNC_INTERVAL;
                    let should_sync_terms = last_term_sync.elapsed() >= TERM_SYNC_INTERVAL;
                    let should_sync_bluebook = bluebook_notified
                        || last_bluebook_sync.elapsed() >= BLUEBOOK_SYNC_INTERVAL;
                    let should_scrape_rmp_reviews =
                        last_rmp_review_scrape.elapsed() >= RMP_REVIEW_SCRAPE_INTERVAL;
                    bluebook_notified = false;

                    // Read and clear the force flag before spawning so the flag
                    // state at decision time is used by the spawned task.
                    let bluebook_force = self
                        .bluebook_force_flag
                        .swap(false, Ordering::Relaxed);

                    // Spawn work in separate task to allow graceful cancellation during shutdown.
                    // Timestamps are persisted to DB on success so restarts don't redo recent work.
                    let work_handle = tokio::spawn({
                        let db = self.db.clone();
                        let banner_api = self.banner_api.clone();
                        let cancel_token = cancel_token.clone();
                        let reference_cache = self.reference_cache.clone();
                        let archived_eval_times = self.archived_eval_times.clone();

                                async move {
                                    tokio::select! {
                                        _ = async {
                                            // Term sync, RMP sync, and reference data are independent --
                                            // run them concurrently so they don't wait behind each other.
                                            let term_fut = async {
                                                if should_sync_terms {
                                                    match Self::sync_terms(db.pool(), &banner_api).await {
                                                        Ok(()) => {
                                                            if let Err(e) = kv::set_timestamp(db.pool(), KV_TERM_SYNC, Utc::now()).await {
                                                                warn!(error = ?e, "Failed to persist term sync timestamp");
                                                            }
                                                        }
                                                        Err(e) => error!(error = ?e, "Failed to sync terms"),
                                                    }
                                                }
                                            };

                                            let rmp_fut = async {
                                                if should_sync_rmp {
                                                    match Self::sync_rmp_data(db.pool()).await {
                                                        Ok(()) => {
                                                            if let Err(e) = kv::set_timestamp(db.pool(), KV_RMP_SYNC, Utc::now()).await {
                                                                warn!(error = ?e, "Failed to persist RMP sync timestamp");
                                                            }
                                                        }
                                                        Err(e) => error!(error = ?e, "Failed to sync RMP data"),
                                                    }
                                                }
                                            };

                                            let ref_fut = async {
                                                if should_scrape_ref {
                                                    match Self::scrape_reference_data(db.pool(), &banner_api, &reference_cache).await {
                                                        Ok(()) => {
                                                            if let Err(e) = kv::set_timestamp(db.pool(), KV_REF_SCRAPE, Utc::now()).await {
                                                                warn!(error = ?e, "Failed to persist ref scrape timestamp");
                                                            }
                                                        }
                                                        Err(e) => error!(error = ?e, "Failed to scrape reference data"),
                                                    }
                                                }
                                            };

                                            let bb_fut = async {
                                                if should_sync_bluebook {
                                                    match Self::sync_bluebook(db.pool(), bluebook_force).await {
                                                        Ok(()) => {
                                                            if let Err(e) = kv::set_timestamp(db.pool(), KV_BLUEBOOK_SYNC, Utc::now()).await {
                                                                warn!(error = ?e, "Failed to persist BlueBook sync timestamp");
                                                            }
                                                        }
                                                        Err(e) => error!(error = ?e, "Failed to sync BlueBook data"),
                                                    }
                                                }
                                            };

                                            let rmp_review_fut = async {
                                                if should_scrape_rmp_reviews {
                                                    match Self::sync_rmp_reviews(db.pool()).await {
                                                        Ok(()) => {
                                                            if let Err(e) = kv::set_timestamp(db.pool(), KV_RMP_REVIEW_SCRAPE, Utc::now()).await {
                                                                warn!(error = ?e, "Failed to persist RMP review scrape timestamp");
                                                            }
                                                        }
                                                        Err(e) => error!(error = ?e, "Failed to sync RMP reviews"),
                                                    }
                                                }
                                            };

                                            tokio::join!(term_fut, rmp_fut, ref_fut, bb_fut, rmp_review_fut);

                                            // Recompute instructor scores when rating data may have changed
                                            if should_sync_rmp || should_sync_bluebook || should_scrape_rmp_reviews {
                                                match crate::data::scoring::recompute_all_scores(db.pool()).await {
                                                    Ok(n) => info!(count = n, "Recomputed instructor scores after sync"),
                                                    Err(e) => error!(error = ?e, "Failed to recompute instructor scores after sync"),
                                                }
                                            }

                                            if let Err(e) = Self::schedule_jobs_impl(&db, &banner_api, &archived_eval_times).await {
                                                error!(error = ?e, "Failed to schedule jobs");
                                            }
                                        } => {}
                                        _ = cancel_token.cancelled() => {
                                            trace!("Scheduling work cancelled gracefully");
                                        }
                                    }
                                }
                    });

                    // Update in-memory timestamps to prevent re-triggering while
                    // the spawned task is still running. The DB is updated on
                    // success inside the task above.
                    if should_scrape_ref {
                        last_ref_scrape = Instant::now();
                    }
                    if should_sync_rmp {
                        last_rmp_sync = Instant::now();
                    }
                    if should_sync_terms {
                        last_term_sync = Instant::now();
                    }
                    if should_sync_bluebook {
                        last_bluebook_sync = Instant::now();
                    }
                    if should_scrape_rmp_reviews {
                        last_rmp_review_scrape = Instant::now();
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
                            trace!("Scheduling work completed gracefully");
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
    async fn schedule_jobs_impl(
        db: &DbContext,
        banner_api: &BannerApi,
        archived_eval_times: &std::sync::Mutex<HashMap<String, Instant>>,
    ) -> Result<()> {
        // Query enabled terms from database
        let start = Instant::now();
        let enabled_terms = terms::get_enabled_terms_for_scheduling(db.pool()).await?;
        let elapsed = start.elapsed();
        if elapsed > SLOW_QUERY_THRESHOLD {
            warn!(
                duration = fmt_duration(elapsed),
                "Slow query: get_enabled_terms_for_scheduling"
            );
        }

        if enabled_terms.is_empty() {
            trace!("No enabled terms to schedule");
            return Ok(());
        }

        // Compute categories up front so we can skip past terms entirely.
        let current_term_code = Term::get_current().inner().to_string();
        let categorized: Vec<_> = enabled_terms
            .into_iter()
            .map(|t| {
                let category = if t.code.as_str() < current_term_code.as_str() {
                    TermCategory::Past
                } else if t.code.as_str() > current_term_code.as_str() {
                    TermCategory::Future
                } else if t.is_archived {
                    TermCategory::Archived
                } else {
                    TermCategory::Current
                };
                (t, category)
            })
            .collect();

        // Filter out terms that don't need evaluation this cycle:
        // - Past and Archived terms only need evaluation every ARCHIVED_INTERVAL (48h).
        let active_terms: Vec<_> = {
            let eval_times = archived_eval_times.lock().unwrap();
            categorized
                .into_iter()
                .filter(|(t, cat)| match cat {
                    TermCategory::Past | TermCategory::Archived => {
                        // Skip if we evaluated this term recently
                        eval_times
                            .get(&t.code)
                            .is_none_or(|last| last.elapsed() >= ARCHIVED_INTERVAL)
                    }
                    _ => true,
                })
                .collect()
        };

        // Fetch per-(subject, term) stats once for the entire cycle.
        let start = Instant::now();
        let stats_rows = db.scrape_jobs().fetch_subject_stats().await?;
        let elapsed = start.elapsed();
        if elapsed > SLOW_QUERY_THRESHOLD {
            warn!(
                duration = fmt_duration(elapsed),
                "Slow query: fetch_subject_stats"
            );
        }
        let stats_map: HashMap<(String, String), SubjectStats> = stats_rows
            .into_iter()
            .map(|row| {
                let key = (row.subject.clone(), row.term.clone());
                (key, SubjectStats::from(row))
            })
            .collect();

        let active_count = active_terms.len();
        let current_future: Vec<&str> = active_terms
            .iter()
            .filter(|(_, c)| matches!(c, TermCategory::Current | TermCategory::Future))
            .map(|(t, _)| t.code.as_str())
            .collect();
        let past_count = active_count - current_future.len();

        if !current_future.is_empty() || past_count > 0 {
            info!(
                current_future = ?current_future,
                past_terms = past_count,
                "Scheduling cycle"
            );
        }

        for (term, category) in active_terms {
            if let Err(e) =
                Self::schedule_term_jobs(db, banner_api, &term.code, category, &stats_map).await
            {
                error!(term = %term.code, error = ?e, "Failed to schedule jobs for term");
                continue;
            }

            // Record evaluation time for past/archived terms so we skip them next cycle.
            if category == TermCategory::Past || category == TermCategory::Archived {
                archived_eval_times
                    .lock()
                    .unwrap()
                    .insert(term.code.clone(), Instant::now());
            }
        }

        trace!("Job scheduling complete");
        Ok(())
    }

    /// Schedule jobs for a single term.
    ///
    /// For past/archived terms, subjects are read from the database cache to avoid
    /// expensive Banner session creation. The cache is populated on first access.
    #[tracing::instrument(skip_all, fields(term = %term_code))]
    async fn schedule_term_jobs(
        db: &DbContext,
        banner_api: &BannerApi,
        term_code: &str,
        category: TermCategory,
        stats_map: &HashMap<(String, String), SubjectStats>,
    ) -> Result<()> {
        trace!(?category, "Enqueuing subject jobs for term");

        let subjects = match category {
            TermCategory::Past | TermCategory::Archived => {
                let cached = term_subjects::get_cached(term_code, db.pool()).await?;
                if !cached.is_empty() {
                    trace!(count = cached.len(), "Using cached subjects");
                    cached
                } else {
                    let fetched = banner_api.get_subjects("", term_code, 1, 500).await?;
                    trace!(
                        count = fetched.len(),
                        "Fetched subjects from API (cold cache)"
                    );
                    term_subjects::cache(term_code, &fetched, db.pool()).await?;
                    fetched
                }
            }
            _ => {
                let fetched = banner_api.get_subjects("", term_code, 1, 500).await?;
                trace!(count = fetched.len(), "Fetched subjects from API");
                term_subjects::cache(term_code, &fetched, db.pool()).await?;
                fetched
            }
        };

        // Evaluate each subject using adaptive scheduling
        let now = Utc::now();
        let mut eligible_subjects: Vec<String> = Vec::new();
        let mut cooldown_count: usize = 0;
        let mut paused_count: usize = 0;

        for subject in &subjects {
            let key = (subject.code.clone(), term_code.to_string());
            let stats = stats_map
                .get(&key)
                .cloned()
                .unwrap_or_else(|| SubjectStats {
                    subject: subject.code.clone(),
                    term: term_code.to_string(),
                    recent_runs: 0,
                    avg_change_ratio: 0.0,
                    consecutive_zero_changes: 0,
                    consecutive_empty_fetches: 0,
                    recent_failure_count: 0,
                    recent_success_count: 0,
                    last_completed: DateTime::<Utc>::MIN_UTC,
                });

            match evaluate_subject(&stats, now, category) {
                SubjectSchedule::Eligible(_) => {
                    eligible_subjects.push(subject.code.clone());
                }
                SubjectSchedule::Cooldown(_) => cooldown_count += 1,
                SubjectSchedule::Paused => paused_count += 1,
            }
        }

        if eligible_subjects.is_empty() {
            trace!(
                total = subjects.len(),
                cooldown = cooldown_count,
                paused = paused_count,
                ?category,
                "No eligible subjects"
            );
            return Ok(());
        }

        info!(
            total = subjects.len(),
            eligible = eligible_subjects.len(),
            cooldown = cooldown_count,
            paused = paused_count,
            ?category,
            "Scheduling subjects"
        );

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

        let banner_terms = banner_api.get_terms("", 1, 500).await?;
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
        crate::data::rmp::refresh_rmp_summary(db_pool).await?;

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
            stats.total_processed,
            stats.deleted_pending_candidates,
            stats.deleted_auto_links,
            stats.candidates_created,
            stats.auto_matched,
            stats.pending_review,
            stats.skipped_unparseable,
            stats.skipped_no_candidates,
            "RMP sync complete"
        );

        Ok(())
    }

    /// Scrape individual reviews for eligible professors.
    ///
    /// Fetches extended profile data and all reviews for each professor,
    /// stores them, and updates the scrape schedule based on review count.
    #[tracing::instrument(skip_all)]
    async fn sync_rmp_reviews(db_pool: &PgPool) -> Result<()> {
        let eligible = crate::data::rmp::get_professors_eligible_for_review_scrape(
            db_pool,
            RMP_REVIEW_SCRAPE_BATCH_SIZE,
        )
        .await?;

        if eligible.is_empty() {
            trace!("No professors eligible for RMP review scraping");
            return Ok(());
        }

        info!(
            count = eligible.len(),
            "Scraping RMP reviews for professors"
        );

        let client = RmpClient::new();
        let mut success_count = 0;

        for (legacy_id, graphql_id) in &eligible {
            match client.fetch_professor_with_reviews(graphql_id).await {
                Ok((detail, reviews)) => {
                    let num_reviews =
                        Count::try_from(reviews.len()).unwrap_or(Count::new(u32::MAX));

                    if let Err(e) =
                        crate::data::rmp::upsert_professor_detail(&detail, db_pool).await
                    {
                        warn!(legacy_id, error = ?e, "Failed to upsert professor detail");
                        continue;
                    }

                    if let Err(e) =
                        crate::data::rmp::replace_professor_reviews(*legacy_id, &reviews, db_pool)
                            .await
                    {
                        warn!(legacy_id, error = ?e, "Failed to replace professor reviews");
                        continue;
                    }

                    if let Err(e) = crate::data::rmp::mark_professor_reviews_scraped(
                        *legacy_id,
                        i32::try_from(num_reviews.get()).unwrap_or(i32::MAX),
                        db_pool,
                    )
                    .await
                    {
                        warn!(legacy_id, error = ?e, "Failed to mark professor reviews scraped");
                        continue;
                    }

                    debug!(legacy_id, num_reviews = %num_reviews, "Scraped professor reviews");
                    success_count += 1;
                }
                Err(e) => {
                    warn!(legacy_id, error = ?e, "Failed to fetch professor reviews from RMP");
                }
            }
        }

        if success_count > 0 {
            crate::data::rmp::refresh_rmp_summary(db_pool).await?;
        }

        info!(
            total = eligible.len(),
            success = success_count,
            failed = eligible.len() - success_count,
            "RMP review scrape cycle complete"
        );
        Ok(())
    }

    /// Scrape all BlueBook course evaluations and upsert to DB.
    ///
    /// When `force` is true, all subjects are scraped regardless of their per-subject timestamps.
    #[tracing::instrument(skip_all)]
    async fn sync_bluebook(db_pool: &PgPool, force: bool) -> Result<()> {
        info!(force, "Starting BlueBook evaluation sync");

        let client = BlueBookClient::new();
        let total = client.scrape_all(db_pool, force).await?;

        info!(total, "BlueBook evaluation sync complete");
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
        match banner_api.get_terms("", 1, 500).await {
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

        // Subjects -- also cache in term_subjects for scheduler use
        match banner_api.get_subjects("", &term, 1, 500).await {
            Ok(pairs) => {
                debug!(count = pairs.len(), "Fetched subjects");
                if let Err(e) = term_subjects::cache(&term, &pairs, db_pool).await {
                    warn!(error = ?e, "Failed to cache term subjects");
                }
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
