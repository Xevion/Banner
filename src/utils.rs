use std::time::{Duration, Instant};

/// Format a `Duration` as a human-readable string with automatic unit scaling.
///
/// Produces output like `1.94ms`, `2.34s`, `150.00uss` using Rust's Debug format.
pub fn fmt_duration(d: Duration) -> String {
    format!("{d:.2?}")
}

/// Log a warning if the elapsed time since `start` exceeds `threshold`.
pub fn log_if_slow(start: Instant, threshold: Duration, label: &str) {
    let elapsed = start.elapsed();
    if elapsed > threshold {
        tracing::warn!(duration = fmt_duration(elapsed), "slow operation: {label}");
    }
}
