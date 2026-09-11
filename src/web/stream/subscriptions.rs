//! Subscription registry and helpers.

use std::collections::{HashMap, HashSet};

use crate::telemetry::WS_SUBSCRIPTIONS;
use crate::web::stream::filters::{
    AuditLogFilter, ScrapeJobsFilter, ScraperStatsFilter, ScraperTimeseriesFilter,
    parse_audit_log_filter, parse_scrape_jobs_filter, parse_scraper_stats_filter,
    parse_scraper_timeseries_filter,
};
use crate::web::stream::protocol::{StreamError, StreamFilter, StreamKind};

pub enum Subscription {
    ScrapeJobs {
        filter: ScrapeJobsFilter,
        known_ids: HashSet<i32>,
    },
    AuditLog {
        filter: AuditLogFilter,
    },
    ScraperStats {
        filter: ScraperStatsFilter,
    },
    ScraperTimeseries {
        filter: ScraperTimeseriesFilter,
    },
    ScraperSubjects,
}

impl Subscription {
    pub fn kind(&self) -> StreamKind {
        match self {
            Subscription::ScrapeJobs { .. } => StreamKind::ScrapeJobs,
            Subscription::AuditLog { .. } => StreamKind::AuditLog,
            Subscription::ScraperStats { .. } => StreamKind::ScraperStats,
            Subscription::ScraperTimeseries { .. } => StreamKind::ScraperTimeseries,
            Subscription::ScraperSubjects => StreamKind::ScraperSubjects,
        }
    }

    pub fn is_computed(&self) -> bool {
        matches!(
            self,
            Self::ScraperStats { .. } | Self::ScraperTimeseries { .. } | Self::ScraperSubjects
        )
    }
}

pub struct SubscriptionRegistry {
    subscriptions: HashMap<String, Subscription>,
    next_id: u64,
}

impl Default for SubscriptionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl SubscriptionRegistry {
    pub fn new() -> Self {
        Self {
            subscriptions: HashMap::new(),
            next_id: 1,
        }
    }

    pub fn allocate_id(&mut self) -> String {
        let id = self.next_id.to_string();
        self.next_id += 1;
        id
    }

    pub fn insert(&mut self, id: String, subscription: Subscription) {
        metrics::gauge!(WS_SUBSCRIPTIONS, "stream" => subscription.kind().label()).increment(1.0);
        if let Some(old) = self.subscriptions.insert(id, subscription) {
            metrics::gauge!(WS_SUBSCRIPTIONS, "stream" => old.kind().label()).decrement(1.0);
        }
    }

    pub fn remove(&mut self, id: &str) -> Option<Subscription> {
        let removed = self.subscriptions.remove(id);
        if let Some(sub) = &removed {
            metrics::gauge!(WS_SUBSCRIPTIONS, "stream" => sub.kind().label()).decrement(1.0);
        }
        removed
    }

    pub fn get(&self, id: &str) -> Option<&Subscription> {
        self.subscriptions.get(id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut Subscription> {
        self.subscriptions.get_mut(id)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &Subscription)> {
        self.subscriptions.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&String, &mut Subscription)> {
        self.subscriptions.iter_mut()
    }

    pub fn ids_for_kind(&self, kind: StreamKind) -> Vec<String> {
        self.subscriptions
            .iter()
            .filter_map(|(id, sub)| {
                if sub.kind() == kind {
                    Some(id.clone())
                } else {
                    None
                }
            })
            .collect()
    }
}

impl Drop for SubscriptionRegistry {
    /// Covers connection close/error/cancellation, where no explicit `remove` runs per entry.
    fn drop(&mut self) {
        for sub in self.subscriptions.values() {
            metrics::gauge!(WS_SUBSCRIPTIONS, "stream" => sub.kind().label()).decrement(1.0);
        }
    }
}

pub fn build_subscription(
    kind: StreamKind,
    filter: Option<StreamFilter>,
) -> Result<Subscription, StreamError> {
    match kind {
        StreamKind::ScrapeJobs => {
            let filter = parse_scrape_jobs_filter(filter)?;
            Ok(Subscription::ScrapeJobs {
                filter,
                known_ids: HashSet::new(),
            })
        }
        StreamKind::AuditLog => {
            let filter = parse_audit_log_filter(filter)?;
            Ok(Subscription::AuditLog { filter })
        }
        StreamKind::ScraperStats => {
            let filter = parse_scraper_stats_filter(filter)?;
            Ok(Subscription::ScraperStats { filter })
        }
        StreamKind::ScraperTimeseries => {
            let filter = parse_scraper_timeseries_filter(filter)?;
            Ok(Subscription::ScraperTimeseries { filter })
        }
        StreamKind::ScraperSubjects => Ok(Subscription::ScraperSubjects),
    }
}

#[cfg(test)]
mod tests {
    use metrics_util::debugging::{DebugValue, DebuggingRecorder};

    use super::*;

    fn gauge_value(snapshot: metrics_util::debugging::Snapshot, stream: &str) -> Option<f64> {
        snapshot
            .into_vec()
            .into_iter()
            .find_map(|(ck, _, _, value)| {
                let matches = ck.key().name() == WS_SUBSCRIPTIONS
                    && ck
                        .key()
                        .labels()
                        .any(|l| l.key() == "stream" && l.value() == stream);
                match (matches, value) {
                    (true, DebugValue::Gauge(g)) => Some(g.into_inner()),
                    _ => None,
                }
            })
    }

    #[test]
    fn test_registry_insert_increments_subscription_gauge() {
        let recorder = DebuggingRecorder::new();
        let snapshotter = recorder.snapshotter();
        metrics::with_local_recorder(&recorder, || {
            let mut registry = SubscriptionRegistry::new();
            registry.insert("1".to_string(), Subscription::ScraperSubjects);
            let snapshot = snapshotter.snapshot();
            assert_eq!(gauge_value(snapshot, "scraper_subjects"), Some(1.0));
        });
    }

    #[test]
    fn test_registry_remove_decrements_subscription_gauge() {
        let recorder = DebuggingRecorder::new();
        let snapshotter = recorder.snapshotter();
        metrics::with_local_recorder(&recorder, || {
            let mut registry = SubscriptionRegistry::new();
            registry.insert("1".to_string(), Subscription::ScraperSubjects);
            registry.remove("1");
            let snapshot = snapshotter.snapshot();
            assert_eq!(gauge_value(snapshot, "scraper_subjects"), Some(0.0));
        });
    }

    #[test]
    fn test_registry_drop_decrements_remaining_subscriptions_gauge() {
        let recorder = DebuggingRecorder::new();
        let snapshotter = recorder.snapshotter();
        metrics::with_local_recorder(&recorder, || {
            let mut registry = SubscriptionRegistry::new();
            registry.insert("1".to_string(), Subscription::ScraperSubjects);
            registry.insert("2".to_string(), Subscription::ScraperSubjects);
            drop(registry);
            let snapshot = snapshotter.snapshot();
            assert_eq!(gauge_value(snapshot, "scraper_subjects"), Some(0.0));
        });
    }
}
