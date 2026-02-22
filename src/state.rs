//! Application state shared across components (bot, web, scheduler).

use crate::banner::BannerApi;
use crate::banner::Course;
use crate::data::events::EventBuffer;
use crate::data::models::ReferenceData;
use crate::web::auth::session::{OAuthStateStore, SessionCache};
use crate::web::schedule_cache::ScheduleCache;
use crate::web::search_options_cache::SearchOptionsCache;
use crate::web::stream::computed::ComputedStreamManager;
use anyhow::Result;
use dashmap::DashMap;
use serde::Serialize;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use ts_rs::TS;

/// Health status of a service.
#[derive(Debug, Clone, Serialize, PartialEq, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum ServiceStatus {
    #[allow(dead_code)]
    Starting,
    Active,
    Connected,
    Disabled,
    Error,
}

/// A timestamped status entry for a service.
#[derive(Debug, Clone)]
pub struct StatusEntry {
    pub status: ServiceStatus,
    #[allow(dead_code)]
    pub updated_at: Instant,
}

/// Thread-safe registry for services to self-report their health status.
#[derive(Debug, Clone, Default)]
pub struct ServiceStatusRegistry {
    inner: Arc<DashMap<String, StatusEntry>>,
}

impl ServiceStatusRegistry {
    /// Creates a new empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts or updates the status for a named service.
    pub fn set(&self, name: &str, status: ServiceStatus) {
        self.inner.insert(
            name.to_owned(),
            StatusEntry {
                status,
                updated_at: Instant::now(),
            },
        );
    }

    /// Returns the current status of a named service, if present.
    #[allow(dead_code)]
    pub fn get(&self, name: &str) -> Option<ServiceStatus> {
        self.inner.get(name).map(|entry| entry.status.clone())
    }

    /// Returns a snapshot of all service statuses.
    pub fn all(&self) -> Vec<(String, ServiceStatus)> {
        self.inner
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().status.clone()))
            .collect()
    }
}

/// In-memory cache for reference data (code→description lookups).
///
/// Loaded from the `reference_data` table on startup and refreshed periodically.
/// Uses a two-level HashMap so lookups take `&str` without allocating.
pub struct ReferenceCache {
    /// category → (code → description)
    data: HashMap<String, HashMap<String, String>>,
}

impl Default for ReferenceCache {
    fn default() -> Self {
        Self::new()
    }
}

impl ReferenceCache {
    /// Create an empty cache.
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    /// Build cache from a list of reference data entries.
    pub fn from_entries(entries: Vec<ReferenceData>) -> Self {
        let mut data: HashMap<String, HashMap<String, String>> = HashMap::new();
        for e in entries {
            data.entry(e.category)
                .or_default()
                .insert(e.code, e.description);
        }
        Self { data }
    }

    /// Look up a description by category and code. Zero allocations.
    pub fn lookup(&self, category: &str, code: &str) -> Option<&str> {
        self.data
            .get(category)
            .and_then(|codes| codes.get(code))
            .map(|s| s.as_str())
    }

    /// Get all `(code, description)` pairs for a category, sorted by description.
    pub fn entries_for_category(&self, category: &str) -> Vec<(&str, &str)> {
        let Some(codes) = self.data.get(category) else {
            return Vec::new();
        };
        let mut entries: Vec<(&str, &str)> = codes
            .iter()
            .map(|(code, desc)| (code.as_str(), desc.as_str()))
            .collect();
        entries.sort_by(|a, b| a.1.cmp(b.1));
        entries
    }
}

#[derive(Clone)]
pub struct AppState {
    pub banner_api: Arc<BannerApi>,
    pub db_pool: PgPool,
    pub service_statuses: ServiceStatusRegistry,
    pub reference_cache: Arc<RwLock<ReferenceCache>>,
    pub session_cache: SessionCache,
    pub oauth_state_store: OAuthStateStore,
    pub schedule_cache: ScheduleCache,
    pub events: Arc<EventBuffer>,
    pub search_options_cache: SearchOptionsCache,
    pub computed_streams: ComputedStreamManager,
}

impl AppState {
    pub fn new(banner_api: Arc<BannerApi>, db_pool: PgPool) -> Self {
        let events = Arc::new(EventBuffer::new(1024));
        let schedule_cache = ScheduleCache::new(db_pool.clone());
        let reference_cache = Arc::new(RwLock::new(ReferenceCache::new()));
        let computed_streams =
            ComputedStreamManager::new(events.clone(), db_pool.clone(), reference_cache.clone());
        Self {
            session_cache: SessionCache::new(db_pool.clone()),
            oauth_state_store: OAuthStateStore::new(),
            banner_api,
            db_pool,
            service_statuses: ServiceStatusRegistry::new(),
            reference_cache,
            schedule_cache,
            events,
            search_options_cache: SearchOptionsCache::new(),
            computed_streams,
        }
    }

    /// Spawn a background task that refreshes the reference cache every `interval`.
    /// The task runs until the process exits.
    pub fn spawn_reference_cache_refresh(&self, interval: std::time::Duration) {
        let state = self.clone();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            ticker.tick().await; // skip the immediate first tick
            loop {
                ticker.tick().await;
                match crate::data::reference::get_all(&state.db_pool).await {
                    Ok(entries) => {
                        let count = entries.len();
                        let cache = ReferenceCache::from_entries(entries);
                        *state.reference_cache.write().await = cache;
                        tracing::info!(entries = count, "Reference cache refreshed");
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "Failed to refresh reference cache");
                    }
                }
            }
        });
    }

    /// Initialize the reference cache from the database.
    pub async fn load_reference_cache(&self) -> Result<()> {
        let entries = crate::data::reference::get_all(&self.db_pool).await?;
        let count = entries.len();
        let cache = ReferenceCache::from_entries(entries);
        *self.reference_cache.write().await = cache;
        tracing::info!(entries = count, "Reference cache loaded");
        Ok(())
    }

    /// Get a course by CRN directly from Banner API
    pub async fn get_course_or_fetch(&self, term: &str, crn: &str) -> Result<Course> {
        self.banner_api
            .get_course_by_crn(term, crn)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Course not found for CRN {crn}"))
    }

    /// Get the total number of courses in the database
    pub async fn get_course_count(&self) -> Result<i64> {
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM courses")
            .fetch_one(&self.db_pool)
            .await?;
        Ok(count.0)
    }
}
