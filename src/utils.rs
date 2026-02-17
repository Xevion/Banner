/// Format a `Duration` as a human-readable string with automatic unit scaling.
///
/// Produces output like `1.94ms`, `2.34s`, `150.00µs` using Rust's Debug format.
pub fn fmt_duration(d: std::time::Duration) -> String {
    format!("{d:.2?}")
}
