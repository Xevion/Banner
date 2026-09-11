//! Standard `process_*` metrics, read from procfs. Most Grafana dashboards assume these exist.

use std::sync::OnceLock;

pub const CPU_SECONDS: &str = "process_cpu_seconds_total";
pub const RESIDENT_BYTES: &str = "process_resident_memory_bytes";
pub const VIRTUAL_BYTES: &str = "process_virtual_memory_bytes";
pub const OPEN_FDS: &str = "process_open_fds";
pub const START_TIME: &str = "process_start_time_seconds";

/// Parsed from `/proc/self/stat`, in clock ticks.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct StatFields {
    pub utime: u64,
    pub stime: u64,
    pub starttime: u64,
}

/// The `comm` field is unquoted and may itself contain spaces or parens, so fields are counted
/// from the last `)` rather than by splitting the whole line.
pub(crate) fn parse_stat(line: &str) -> Option<StatFields> {
    let after_comm = line.rsplit_once(')')?.1;
    let fields: Vec<&str> = after_comm.split_whitespace().collect();

    // Indices are procfs field numbers minus 3, since field 3 (state) is the first one here.
    Some(StatFields {
        utime: fields.get(11)?.parse().ok()?,
        stime: fields.get(12)?.parse().ok()?,
        starttime: fields.get(19)?.parse().ok()?,
    })
}

/// Resident and virtual page counts from `/proc/self/statm`.
pub(crate) fn parse_statm(line: &str) -> Option<(u64, u64)> {
    let mut fields = line.split_whitespace();
    let size = fields.next()?.parse().ok()?;
    let resident = fields.next()?.parse().ok()?;
    Some((size, resident))
}

/// Seconds since the epoch at which this process started, computed once from boot time.
fn start_time_seconds() -> Option<f64> {
    static START: OnceLock<Option<f64>> = OnceLock::new();

    *START.get_or_init(|| {
        let stat = parse_stat(&std::fs::read_to_string("/proc/self/stat").ok()?)?;
        let btime = std::fs::read_to_string("/proc/stat")
            .ok()?
            .lines()
            .find_map(|l| l.strip_prefix("btime ")?.trim().parse::<u64>().ok())?;

        Some(btime as f64 + stat.starttime as f64 / clock_ticks())
    })
}

fn clock_ticks() -> f64 {
    // SAFETY: sysconf with a valid name is always sound and returns -1 on failure.
    let ticks = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
    if ticks > 0 { ticks as f64 } else { 100.0 }
}

fn page_size() -> u64 {
    // SAFETY: sysconf with a valid name is always sound and returns -1 on failure.
    let size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    if size > 0 { size as u64 } else { 4096 }
}

fn open_fds() -> Option<u64> {
    Some(std::fs::read_dir("/proc/self/fd").ok()?.count() as u64)
}

/// Publishes one sample. Silently does nothing where procfs is unavailable.
pub fn sample() {
    if let Some(start) = start_time_seconds() {
        metrics::gauge!(START_TIME).set(start);
    }

    if let Some(stat) = std::fs::read_to_string("/proc/self/stat")
        .ok()
        .as_deref()
        .and_then(parse_stat)
    {
        let cpu = (stat.utime + stat.stime) as f64 / clock_ticks();
        // Gauge, not counter: the value is fractional and `Counter` is u64-only. Monotonic, so
        // rate() over it still behaves; the exposed TYPE line is the only inaccuracy.
        metrics::gauge!(CPU_SECONDS).set(cpu);
    }

    if let Some((size, resident)) = std::fs::read_to_string("/proc/self/statm")
        .ok()
        .as_deref()
        .and_then(parse_statm)
    {
        metrics::gauge!(VIRTUAL_BYTES).set((size * page_size()) as f64);
        metrics::gauge!(RESIDENT_BYTES).set((resident * page_size()) as f64);
    }

    if let Some(fds) = open_fds() {
        metrics::gauge!(OPEN_FDS).set(fds as f64);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_stat_reads_fields_after_comm() {
        let line = "1234 (banner) S 1 1234 1234 0 -1 4194304 100 0 0 0 \
                    11 22 0 0 20 0 8 0 999 0 0";

        let parsed = parse_stat(line).expect("well-formed stat line parses");

        assert_eq!(
            parsed,
            StatFields {
                utime: 11,
                stime: 22,
                starttime: 999,
            }
        );
    }

    /// A comm containing spaces and parens is the classic way naive field-splitting breaks.
    #[test]
    fn test_parse_stat_tolerates_parens_in_comm() {
        let line = "1234 (od) (weird name)) S 1 1234 1234 0 -1 4194304 100 0 0 0 \
                    11 22 0 0 20 0 8 0 999 0 0";

        let parsed = parse_stat(line).expect("parenthesised comm parses");

        assert_eq!(parsed.utime, 11);
        assert_eq!(parsed.starttime, 999);
    }

    #[test]
    fn test_parse_stat_rejects_truncated_line() {
        assert!(parse_stat("1234 (banner) S 1 2 3").is_none());
        assert!(parse_stat("no parens here").is_none());
    }

    #[test]
    fn test_parse_statm_reads_size_and_resident() {
        assert_eq!(parse_statm("2048 512 100 1 0 200 0"), Some((2048, 512)));
        assert_eq!(parse_statm(""), None);
    }

    #[test]
    fn test_sample_reads_live_procfs() {
        let handle = crate::telemetry::recorder();
        sample();

        let rendered = handle.render();
        assert!(
            rendered.contains(RESIDENT_BYTES),
            "expected {RESIDENT_BYTES} in exposition output"
        );
    }
}
