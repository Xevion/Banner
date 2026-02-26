//! TTL cache for pre-rendered sitemap XML strings.
//!
//! Mirrors the `SearchOptionsCache` pattern: DashMap entries with a 15-minute TTL
//! and singleflight dedup per cache key to prevent thundering-herd on cache miss.

use dashmap::DashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tracing::debug;

const TTL: Duration = Duration::from_secs(15 * 60);

#[derive(Clone, Default)]
pub struct SitemapCache {
    entries: Arc<DashMap<String, (Instant, Arc<String>)>>,
    inflight: Arc<DashMap<String, Arc<AtomicBool>>>,
}

impl SitemapCache {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Return a cached XML string if it exists and is fresh.
    pub(crate) fn get(&self, key: &str) -> Option<Arc<String>> {
        let entry = self.entries.get(key)?;
        let (cached_at, ref value) = *entry;
        if cached_at.elapsed() < TTL {
            Some(value.clone())
        } else {
            None
        }
    }

    /// Return a cached XML string even if stale (for singleflight contention fallback).
    pub(crate) fn get_stale(&self, key: &str) -> Option<Arc<String>> {
        self.entries.get(key).map(|e| e.1.clone())
    }

    /// Store a fresh XML string for the given key.
    pub(crate) fn insert(&self, key: String, value: String) {
        self.entries.insert(key, (Instant::now(), Arc::new(value)));
    }

    /// Try to claim the singleflight slot for a key.
    /// Returns `true` if this caller should build the response.
    pub(crate) fn try_claim(&self, key: &str) -> bool {
        let flag = self
            .inflight
            .entry(key.to_owned())
            .or_insert_with(|| Arc::new(AtomicBool::new(false)))
            .clone();
        flag.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    /// Release the singleflight slot for a key (call after insert or on error).
    pub(crate) fn release(&self, key: &str) {
        if let Some(flag) = self.inflight.get(key) {
            flag.store(false, Ordering::Release);
        }
        debug!(key, "sitemap cache slot released");
    }
}
