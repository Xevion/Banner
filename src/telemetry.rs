//! Prometheus instrumentation. Not named `metrics`: `data::metrics` is course enrollment history.

pub mod process;

use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use metrics_util::MetricKindMask;
use sqlx::PgPool;
use std::sync::OnceLock;
use std::time::Duration;

pub const HTTP_REQUESTS: &str = "http_requests_total";
pub const HTTP_DURATION: &str = "http_request_duration_seconds";
pub const DB_POOL_CONNECTIONS: &str = "db_pool_connections";
pub const DB_FAILURES: &str = "db_failures_total";
pub const SCRAPE_JOBS: &str = "scrape_jobs_total";
pub const SCRAPE_COURSES: &str = "scrape_courses_total";
pub const BANNER_REQUESTS: &str = "banner_api_requests_total";
pub const BANNER_DURATION: &str = "banner_api_request_duration_seconds";
pub const BANNER_RATE_LIMIT_WAIT: &str = "banner_api_rate_limit_wait_seconds";
pub const BANNER_DECODE_FAILURES: &str = "banner_api_decode_failures_total";
pub const WS_CONNECTIONS: &str = "websocket_connections";
pub const WS_SUBSCRIPTIONS: &str = "websocket_subscriptions";
pub const WS_MESSAGES: &str = "websocket_messages_total";
pub const BUILD_INFO: &str = "build_info";

/// Spans sub-millisecond cache hits through the 60s request timeout, so neither end is a cliff.
/// Applied globally: an unmatched histogram would otherwise silently render as a summary.
const LATENCY_BUCKETS: &[f64] = &[
    0.0005, 0.001, 0.0025, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0,
    60.0,
];

/// Bounds series growth from label values we do not fully control.
const IDLE_TIMEOUT: Duration = Duration::from_secs(60 * 60);

static HANDLE: OnceLock<PrometheusHandle> = OnceLock::new();

/// Installs the global recorder once per process; a second install would fail.
pub fn recorder() -> &'static PrometheusHandle {
    HANDLE.get_or_init(|| {
        PrometheusBuilder::new()
            .set_buckets(LATENCY_BUCKETS)
            .expect("latency buckets are non-empty and ascending")
            // Counters and histograms only. A gauge tracks current state owned by a live RAII
            // guard, so reaping an idle series orphans that handle and the eventual decrement
            // lands on a fresh zero, under-counting for the life of the process.
            .idle_timeout(
                MetricKindMask::COUNTER | MetricKindMask::HISTOGRAM,
                Some(IDLE_TIMEOUT),
            )
            .install_recorder()
            .expect("no global metrics recorder was installed before this call")
    })
}

/// Standard verbs only. `http::Method` accepts arbitrary extension tokens and axum produces a 405
/// inside this layer, so an unfiltered method label lets any caller mint unbounded series.
pub fn method_label(method: &axum::http::Method) -> &'static str {
    use axum::http::Method;
    match *method {
        Method::GET => "GET",
        Method::POST => "POST",
        Method::PUT => "PUT",
        Method::PATCH => "PATCH",
        Method::DELETE => "DELETE",
        Method::HEAD => "HEAD",
        Method::OPTIONS => "OPTIONS",
        Method::TRACE => "TRACE",
        Method::CONNECT => "CONNECT",
        _ => "other",
    }
}

/// Publishes version and commit as a constant-1 gauge, so a dashboard can align a metric shift
/// against the deploy that caused it. Re-set on every sample tick to outlive the idle timeout.
pub fn set_build_info() {
    metrics::gauge!(
        BUILD_INFO,
        "version" => env!("CARGO_PKG_VERSION"),
        "commit" => env!("GIT_COMMIT_HASH"),
    )
    .set(1.0);
}

/// Counts courses seen by a completed scrape, so the backlog is visible as volume, not just outcome.
pub fn record_upsert_counts(fetched: u64, changed: u64, unchanged: u64) {
    metrics::counter!(SCRAPE_COURSES, "result" => "fetched").increment(fetched);
    metrics::counter!(SCRAPE_COURSES, "result" => "changed").increment(changed);
    metrics::counter!(SCRAPE_COURSES, "result" => "unchanged").increment(unchanged);
}

/// Publishes pool depth as gauges. Timer-driven: an exhausted pool has no acquisitions to hook.
pub fn sample_pool(pool: &PgPool) {
    set_build_info();

    let total = pool.size();
    let idle = u32::try_from(pool.num_idle()).unwrap_or(total);
    // The reads are not atomic, so idle can briefly exceed total.
    let in_use = total.saturating_sub(idle);

    metrics::gauge!(DB_POOL_CONNECTIONS, "state" => "total").set(f64::from(total));
    metrics::gauge!(DB_POOL_CONNECTIONS, "state" => "idle").set(f64::from(idle));
    metrics::gauge!(DB_POOL_CONNECTIONS, "state" => "in_use").set(f64::from(in_use));
}

/// Record the outcome of a scrape job.
///
/// Not a partition of jobs: `recoverable_error` fires per retry attempt, the others once per job,
/// so summing across outcomes overcounts.
pub fn record_scrape_job(outcome: &'static str) {
    metrics::counter!(SCRAPE_JOBS, "outcome" => outcome).increment(1);
}

/// Counts a database failure by kind. `pool_timeout` is the one that signals contention.
pub fn record_db_failure(error: &anyhow::Error) {
    metrics::counter!(DB_FAILURES, "kind" => classify_db_error(error)).increment(1);
}

/// Relies on `downcast_ref` walking the source chain: callers wrap errors in `anyhow` context.
fn classify_db_error(error: &anyhow::Error) -> &'static str {
    match error.downcast_ref::<sqlx::Error>() {
        Some(sqlx::Error::PoolTimedOut) => "pool_timeout",
        Some(sqlx::Error::PoolClosed) => "pool_closed",
        Some(sqlx::Error::RowNotFound) => "row_not_found",
        Some(sqlx::Error::Database(_)) => "database",
        Some(_) => "other_sqlx",
        None => "other",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recorder_is_idempotent_across_calls() {
        let first = recorder();
        let second = recorder();

        assert!(
            std::ptr::eq(first, second),
            "recorder() must reuse the installed handle; a second install would panic"
        );
    }

    #[test]
    fn test_classify_pool_timeout_is_distinct_from_other_failures() {
        let timeout = anyhow::Error::new(sqlx::Error::PoolTimedOut);
        let not_found = anyhow::Error::new(sqlx::Error::RowNotFound);

        assert_eq!(classify_db_error(&timeout), "pool_timeout");
        assert_eq!(classify_db_error(&not_found), "row_not_found");
    }

    #[test]
    fn test_classify_sees_through_anyhow_context() {
        use anyhow::Context;

        let wrapped = Err::<(), _>(sqlx::Error::PoolTimedOut)
            .context("failed to fetch courses")
            .context("Course lookup")
            .unwrap_err();

        assert_eq!(classify_db_error(&wrapped), "pool_timeout");
    }

    #[test]
    fn test_classify_non_sqlx_error_falls_back() {
        let other = anyhow::anyhow!("something unrelated");

        assert_eq!(classify_db_error(&other), "other");
    }

    #[test]
    fn test_render_includes_a_recorded_metric() {
        let handle = recorder();
        metrics::counter!(SCRAPE_JOBS, "outcome" => "test_probe").increment(1);

        let rendered = handle.render();

        assert!(
            rendered.contains(SCRAPE_JOBS),
            "expected {SCRAPE_JOBS} in exposition output, got: {rendered}"
        );
    }
}
