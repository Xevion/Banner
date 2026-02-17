//! Background task for computing and broadcasting aggregated stream data.
//!
//! Watches domain events, debounces, recomputes aggregated data from the database,
//! diffs against cached values, and broadcasts changes to subscribers.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use sqlx::PgPool;
use tokio::sync::{RwLock, broadcast, mpsc};
use tokio::time::Instant;
use tracing::warn;

use crate::data::events::{DomainEvent, EventBuffer};
use crate::state::ReferenceCache;
use crate::web::admin::scraper::{
    ScraperStatsResponse, SubjectSummary, TimeseriesPoint, compute_stats, compute_subjects,
    compute_timeseries,
};
use crate::web::stream::protocol::StreamDelta;
use crate::web::ws::ScrapeJobEvent;

/// Cache key for computed stream data.
#[derive(Hash, Eq, PartialEq, Clone, Debug)]
pub enum ComputedCacheKey {
    Stats {
        period: String,
        term: Option<String>,
    },
    Timeseries {
        period: String,
        bucket: String,
        term: Option<String>,
    },
    Subjects,
}

/// Update message broadcast to WebSocket handlers.
#[derive(Clone, Debug)]
pub struct ComputedUpdate {
    pub key: ComputedCacheKey,
    pub delta: Option<StreamDelta>,
}

/// Registration message from WS handlers.
pub enum RegistrationMsg {
    Register(ComputedCacheKey),
    Deregister(ComputedCacheKey),
}

/// Cached entry with last computed value.
struct CacheEntry {
    stats: Option<ScraperStatsResponse>,
    timeseries: Option<Vec<TimeseriesPoint>>,
    subjects: Option<Vec<SubjectSummary>>,
    subscribers: usize,
    stale: bool,
}

/// Manager for computed stream data.
///
/// Spawns a background task that watches domain events, debounces changes,
/// recomputes aggregated data, and broadcasts updates to subscribers.
#[derive(Clone)]
pub struct ComputedStreamManager {
    update_tx: broadcast::Sender<ComputedUpdate>,
    reg_tx: mpsc::UnboundedSender<RegistrationMsg>,
}

impl ComputedStreamManager {
    /// Create manager and spawn background task.
    pub fn new(
        events: Arc<EventBuffer>,
        pool: PgPool,
        reference_cache: Arc<RwLock<ReferenceCache>>,
    ) -> Self {
        let (update_tx, _) = broadcast::channel(256);
        let (reg_tx, reg_rx) = mpsc::unbounded_channel();

        let manager = Self {
            update_tx: update_tx.clone(),
            reg_tx,
        };

        tokio::spawn(run_manager_loop(
            events,
            pool,
            reference_cache,
            update_tx,
            reg_rx,
        ));

        manager
    }

    /// Subscribe to updates (WS handlers call this).
    pub fn subscribe(&self) -> broadcast::Receiver<ComputedUpdate> {
        self.update_tx.subscribe()
    }

    /// Register interest in a cache key.
    pub fn register(&self, key: ComputedCacheKey) {
        let _ = self.reg_tx.send(RegistrationMsg::Register(key));
    }

    /// Deregister interest.
    pub fn deregister(&self, key: ComputedCacheKey) {
        let _ = self.reg_tx.send(RegistrationMsg::Deregister(key));
    }
}

async fn run_manager_loop(
    events: Arc<EventBuffer>,
    pool: PgPool,
    reference_cache: Arc<RwLock<ReferenceCache>>,
    update_tx: broadcast::Sender<ComputedUpdate>,
    mut reg_rx: mpsc::UnboundedReceiver<RegistrationMsg>,
) {
    const DEBOUNCE_DURATION: Duration = Duration::from_secs(1);

    let mut cache: HashMap<ComputedCacheKey, CacheEntry> = HashMap::new();
    let mut debounce_deadline: Option<Instant> = None;
    let (mut cursor, mut head_watch) = events.subscribe();

    loop {
        let sleep_future = async {
            if let Some(deadline) = debounce_deadline {
                tokio::time::sleep_until(deadline).await;
            } else {
                std::future::pending::<()>().await;
            }
        };

        tokio::select! {
            biased;

            msg = reg_rx.recv() => {
                match msg {
                    Some(RegistrationMsg::Register(key)) => {
                        let entry = cache.entry(key.clone()).or_insert_with(|| CacheEntry {
                            stats: None,
                            timeseries: None,
                            subjects: None,
                            subscribers: 0,
                            stale: true, // Mark stale so initial compute happens
                        });
                        entry.subscribers += 1;
                        if entry.stale && debounce_deadline.is_none() {
                            // Compute immediately for first subscriber
                            debounce_deadline = Some(Instant::now());
                        }
                    }
                    Some(RegistrationMsg::Deregister(key)) => {
                        if let Some(entry) = cache.get_mut(&key) {
                            entry.subscribers = entry.subscribers.saturating_sub(1);
                            if entry.subscribers == 0 {
                                cache.remove(&key);
                            }
                        }
                    }
                    None => break, // Channel closed
                }
            }

            _ = head_watch.changed() => {
                // Process new events, mark affected keys stale
                while let Some(event) = events.read(cursor) {
                    mark_stale_for_event(&mut cache, &event);
                    cursor += 1;
                }
                // Check if we lagged behind and reset
                let base = events.base_offset();
                if cursor < base {
                    tracing::warn!(cursor, base, "ComputedStreamManager lagged, resetting cursor");
                    cursor = base;
                    // Mark all keys stale since we missed events
                    for entry in cache.values_mut() {
                        entry.stale = true;
                    }
                }
                // Reset debounce for any stale keys with subscribers
                if cache.values().any(|e| e.stale && e.subscribers > 0) {
                    debounce_deadline = Some(Instant::now() + DEBOUNCE_DURATION);
                }
            }

            _ = sleep_future, if debounce_deadline.is_some() => {
                debounce_deadline = None;
                // Recompute all stale keys with subscribers
                recompute_stale(&mut cache, &pool, &events, &reference_cache, &update_tx).await;
            }
        }
    }
}

fn mark_stale_for_event(cache: &mut HashMap<ComputedCacheKey, CacheEntry>, event: &DomainEvent) {
    let DomainEvent::ScrapeJob(scrape_event) = event else {
        return; // AuditLog events don't affect computed streams
    };

    match scrape_event {
        ScrapeJobEvent::Completed { .. } => {
            // Stats: always stale
            // Timeseries: always stale
            // Subjects: stale (targeted if subject known)
            for entry in cache.values_mut() {
                entry.stale = true; // Simplified: mark all as stale
            }
        }
        ScrapeJobEvent::Created { .. }
        | ScrapeJobEvent::Locked { .. }
        | ScrapeJobEvent::Deleted { .. }
        | ScrapeJobEvent::Retried { .. }
        | ScrapeJobEvent::Exhausted { .. } => {
            // Only affects stats (queue counts)
            for (key, entry) in cache.iter_mut() {
                if matches!(key, ComputedCacheKey::Stats { .. }) {
                    entry.stale = true;
                }
            }
        }
    }
}

async fn recompute_stale(
    cache: &mut HashMap<ComputedCacheKey, CacheEntry>,
    pool: &PgPool,
    events: &Arc<EventBuffer>,
    reference_cache: &Arc<RwLock<ReferenceCache>>,
    update_tx: &broadcast::Sender<ComputedUpdate>,
) {
    let stale_keys: Vec<_> = cache
        .iter()
        .filter(|(_, e)| e.stale && e.subscribers > 0)
        .map(|(k, _)| k.clone())
        .collect();

    for key in stale_keys {
        let Some(entry) = cache.get_mut(&key) else {
            continue;
        };
        entry.stale = false;

        match &key {
            ComputedCacheKey::Stats { period, term } => {
                match compute_stats(pool, period, term.as_deref()).await {
                    Ok(new_stats) => {
                        let delta = if entry.stats.as_ref() != Some(&new_stats) {
                            Some(StreamDelta::ScraperStats {
                                stats: new_stats.clone(),
                            })
                        } else {
                            None
                        };
                        entry.stats = Some(new_stats.clone());
                        if delta.is_some() {
                            let _ = update_tx.send(ComputedUpdate {
                                key: key.clone(),
                                delta,
                            });
                        }
                    }
                    Err(e) => warn!(%e, %period, ?term, "Failed to compute stats"),
                }
            }
            ComputedCacheKey::Timeseries {
                period,
                bucket,
                term,
            } => {
                match compute_timeseries(pool, period, Some(bucket.as_str()), term.as_deref()).await
                {
                    Ok((new_points, _returned_period, _returned_bucket)) => {
                        let delta = compute_timeseries_delta(&entry.timeseries, &new_points);
                        let is_first = entry.timeseries.is_none();
                        entry.timeseries = Some(new_points.clone());
                        if delta.is_some() || is_first {
                            let _ = update_tx.send(ComputedUpdate {
                                key: key.clone(),
                                delta,
                            });
                        }
                    }
                    Err(e) => warn!(%e, %period, %bucket, ?term, "Failed to compute timeseries"),
                }
            }
            ComputedCacheKey::Subjects => {
                let ref_cache = reference_cache.read().await;
                match compute_subjects(pool, events, &ref_cache).await {
                    Ok(new_subjects) => {
                        let delta = compute_subjects_delta(&entry.subjects, &new_subjects);
                        let is_first = entry.subjects.is_none();
                        entry.subjects = Some(new_subjects.clone());
                        if delta.is_some() || is_first {
                            let _ = update_tx.send(ComputedUpdate {
                                key: key.clone(),
                                delta,
                            });
                        }
                    }
                    Err(e) => warn!(?e, "Failed to compute subjects"),
                }
            }
        }
    }
}

fn compute_timeseries_delta(
    old: &Option<Vec<TimeseriesPoint>>,
    new: &[TimeseriesPoint],
) -> Option<StreamDelta> {
    let Some(old_points) = old else {
        return None; // First computation, send snapshot not delta
    };

    let old_map: HashMap<_, _> = old_points.iter().map(|p| (&p.timestamp, p)).collect();
    let changed: Vec<_> = new
        .iter()
        .filter(|p| {
            old_map
                .get(&p.timestamp)
                .map(|&old| old != *p)
                .unwrap_or(true)
        })
        .cloned()
        .collect();

    if changed.is_empty() {
        None
    } else {
        Some(StreamDelta::ScraperTimeseries { changed })
    }
}

fn compute_subjects_delta(
    old: &Option<Vec<SubjectSummary>>,
    new: &[SubjectSummary],
) -> Option<StreamDelta> {
    let Some(old_subjects) = old else {
        return None;
    };

    let old_map: HashMap<_, _> = old_subjects.iter().map(|s| (&s.subject, s)).collect();
    let new_map: HashMap<_, _> = new.iter().map(|s| (&s.subject, s)).collect();

    let changed: Vec<_> = new
        .iter()
        .filter(|s| {
            old_map
                .get(&s.subject)
                .map(|&old| old != *s)
                .unwrap_or(true)
        })
        .cloned()
        .collect();

    let removed: Vec<_> = old_subjects
        .iter()
        .filter(|s| !new_map.contains_key(&s.subject))
        .map(|s| s.subject.clone())
        .collect();

    if changed.is_empty() && removed.is_empty() {
        None
    } else {
        Some(StreamDelta::ScraperSubjects { changed, removed })
    }
}
