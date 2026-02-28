//! Application state shared across components (bot, web, scheduler).

use crate::banner::BannerApi;
use crate::data::events::EventBuffer;
use crate::data::models::ReferenceData;
use crate::web::auth::session::{OAuthStateStore, SessionCache};
use crate::web::middleware::rate_limit::{RateLimitState, SharedRateLimitState};
use crate::web::schedule_cache::ScheduleCache;
use crate::web::search_options_cache::SearchOptionsCache;
use crate::web::sitemap_cache::SitemapCache;
use crate::web::stream::computed::ComputedStreamManager;
use axum::extract::FromRef;
use dashmap::DashMap;
use serde::Serialize;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tokio::sync::{Notify, RwLock};
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

/// In-memory cache for reference data (code->description lookups).
///
/// Loaded from the `reference_data` table on startup and refreshed periodically.
/// Uses a two-level HashMap so lookups take `&str` without allocating.
pub struct ReferenceCache {
    /// category -> (code -> description)
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

#[derive(Clone, FromRef)]
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
    /// HTTP client for proxying requests to the SvelteKit SSR server.
    pub ssr_client: reqwest::Client,
    /// Base URL of the downstream SSR server (e.g. "http://localhost:3001").
    pub ssr_downstream: String,
    /// Notify handle to manually trigger a BlueBook sync from admin endpoints.
    pub bluebook_sync_notify: Arc<Notify>,
    /// When set to true before notifying, the next BlueBook sync will skip interval checks.
    pub bluebook_force_flag: Arc<AtomicBool>,
    /// Public origin for absolute URLs in sitemaps (e.g. "https://banner.xevion.dev").
    pub public_origin: Option<String>,
    /// In-memory cache for pre-rendered sitemap XML.
    pub sitemap_cache: SitemapCache,
    /// Shared rate limiting state for inbound HTTP requests.
    pub rate_limit: SharedRateLimitState,
}

impl AppState {
    /// The internal token that the SSR proxy injects to bypass rate limiting.
    pub fn internal_token(&self) -> &str {
        self.rate_limit.internal_token()
    }

    pub fn new(
        banner_api: Arc<BannerApi>,
        db_pool: PgPool,
        ssr_downstream: String,
        bluebook_sync_notify: Arc<Notify>,
        bluebook_force_flag: Arc<AtomicBool>,
        public_origin: Option<String>,
    ) -> Self {
        let events = Arc::new(EventBuffer::new(1024));
        let schedule_cache = ScheduleCache::new(db_pool.clone());
        let reference_cache = Arc::new(RwLock::new(ReferenceCache::new()));
        let computed_streams =
            ComputedStreamManager::new(events.clone(), db_pool.clone(), reference_cache.clone());
        let ssr_client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("Failed to create SSR proxy client");

        // Generate a random internal token for SSR -> API bypass.
        let internal_token = ulid::Ulid::new().to_string();
        let rate_limit = Arc::new(RateLimitState::new(internal_token));

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
            ssr_client,
            ssr_downstream,
            bluebook_sync_notify,
            bluebook_force_flag,
            public_origin,
            sitemap_cache: SitemapCache::new(),
            rate_limit,
        }
    }
}
