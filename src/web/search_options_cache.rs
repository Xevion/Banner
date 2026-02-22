//! TTL cache for search-options responses, one snapshot per term.
//!
//! Stores typed `Arc<SearchOptionsResponse>` — no JSON round-trip on reads.
//! Singleflight per term key prevents thundering-herd on cache miss.

use crate::web::routes::SearchOptionsResponse;
use dashmap::DashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tracing::debug;

const TTL: Duration = Duration::from_secs(10 * 60);

#[derive(Clone, Default)]
pub struct SearchOptionsCache {
    /// term_code → (cached_at, value)
    entries: Arc<DashMap<String, (Instant, Arc<SearchOptionsResponse>)>>,
    /// term_code → in-flight flag (singleflight guard)
    inflight: Arc<DashMap<String, Arc<AtomicBool>>>,
}

impl SearchOptionsCache {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Return a cached entry if it exists and is fresh.
    pub(crate) fn get(&self, term_code: &str) -> Option<Arc<SearchOptionsResponse>> {
        let entry = self.entries.get(term_code)?;
        let (cached_at, ref value) = *entry;
        if cached_at.elapsed() < TTL {
            Some(value.clone())
        } else {
            None
        }
    }

    /// Store a fresh response for the given term.
    pub(crate) fn insert(&self, term_code: String, value: SearchOptionsResponse) {
        self.entries
            .insert(term_code, (Instant::now(), Arc::new(value)));
    }

    /// Try to claim the singleflight slot for a term.
    /// Returns `true` if this caller should build the response; `false` if another is already building it.
    pub(crate) fn try_claim(&self, term_code: &str) -> bool {
        let flag = self
            .inflight
            .entry(term_code.to_owned())
            .or_insert_with(|| Arc::new(AtomicBool::new(false)))
            .clone();
        flag.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    /// Release the singleflight slot for a term (call after insert or on error).
    pub(crate) fn release(&self, term_code: &str) {
        if let Some(flag) = self.inflight.get(term_code) {
            flag.store(false, Ordering::Release);
        }
        debug!(term = term_code, "search-options cache slot released");
    }
}
