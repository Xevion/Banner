//! Adaptive scraping interval computation.
//!
//! Assigns per-subject scrape intervals based on recent change rates,
//! consecutive zero-change runs, failure patterns, and time of day.

use chrono::{DateTime, Datelike, Timelike, Utc};
use chrono_tz::US::Central;
use std::time::Duration;

use crate::data::models::SubjectResultStats;

const FLOOR_INTERVAL: Duration = Duration::from_secs(3 * 60);
const MODERATE_HIGH_INTERVAL: Duration = Duration::from_secs(5 * 60);
const MODERATE_LOW_INTERVAL: Duration = Duration::from_secs(15 * 60);
const LOW_CHANGE_INTERVAL: Duration = Duration::from_secs(30 * 60);
const ZERO_5_INTERVAL: Duration = Duration::from_secs(60 * 60);
const ZERO_10_INTERVAL: Duration = Duration::from_secs(2 * 60 * 60);
const CEILING_INTERVAL: Duration = Duration::from_secs(4 * 60 * 60);
const COLD_START_INTERVAL: Duration = FLOOR_INTERVAL;
pub(crate) const ARCHIVED_INTERVAL: Duration = Duration::from_secs(48 * 60 * 60);
const PAUSE_PROBE_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);
const EMPTY_FETCH_PAUSE_THRESHOLD: i64 = 3;
const FAILURE_PAUSE_THRESHOLD: i64 = 5;

/// Scheduling tier for a term based on its temporal status and archive flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TermCategory {
    /// Active term — full adaptive scheduling.
    Current,
    /// Upcoming term — full adaptive scheduling (same as current).
    Future,
    /// Banner "View Only" term — fixed 48-hour interval.
    Archived,
    /// Past non-archived term — fixed 48-hour interval (same as Archived).
    Past,
}

/// Aggregated per-subject statistics derived from recent scrape results.
#[derive(Debug, Clone)]
pub struct SubjectStats {
    pub subject: String,
    pub term: String,
    pub recent_runs: i64,
    pub avg_change_ratio: f64,
    pub consecutive_zero_changes: i64,
    pub consecutive_empty_fetches: i64,
    pub recent_failure_count: i64,
    pub recent_success_count: i64,
    pub last_completed: DateTime<Utc>,
}

/// Scheduling decision for a subject.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubjectSchedule {
    /// Subject is due for scraping, with the computed interval.
    Eligible(Duration),
    /// Subject was scraped recently; wait for the remaining cooldown.
    Cooldown(Duration),
    /// Subject is paused due to repeated empty fetches or failures.
    Paused,
}

impl From<SubjectResultStats> for SubjectStats {
    fn from(row: SubjectResultStats) -> Self {
        Self {
            subject: row.subject,
            term: row.term,
            recent_runs: row.recent_runs,
            avg_change_ratio: row.avg_change_ratio,
            consecutive_zero_changes: row.consecutive_zero_changes,
            consecutive_empty_fetches: row.consecutive_empty_fetches,
            recent_failure_count: row.recent_failure_count,
            recent_success_count: row.recent_success_count,
            last_completed: row.last_completed,
        }
    }
}

/// Compute the base interval tier from change-rate statistics.
pub fn compute_base_interval(stats: &SubjectStats) -> Duration {
    if stats.recent_runs == 0 {
        return COLD_START_INTERVAL;
    }

    // Consecutive-zero tiers take precedence when change ratio is near zero
    if stats.avg_change_ratio < 0.001 {
        return match stats.consecutive_zero_changes {
            0..5 => LOW_CHANGE_INTERVAL,
            5..10 => ZERO_5_INTERVAL,
            10..20 => ZERO_10_INTERVAL,
            _ => CEILING_INTERVAL,
        };
    }

    match stats.avg_change_ratio {
        r if r >= 0.10 => FLOOR_INTERVAL,
        r if r >= 0.05 => MODERATE_HIGH_INTERVAL,
        r if r >= 0.01 => MODERATE_LOW_INTERVAL,
        _ => LOW_CHANGE_INTERVAL,
    }
}

/// Return a time-of-day multiplier for the given UTC timestamp.
///
/// Peak hours (weekdays 8am-6pm CT) return 1; off-peak (weekdays 6pm-midnight CT)
/// return 2; night (midnight-8am CT) and weekends return 4.
pub fn time_of_day_multiplier(now: DateTime<Utc>) -> u32 {
    let ct = now.with_timezone(&Central);
    let weekday = ct.weekday();
    let hour = ct.hour();

    // Weekends get the slowest multiplier
    if matches!(weekday, chrono::Weekday::Sat | chrono::Weekday::Sun) {
        return 4;
    }

    match hour {
        8..18 => 1,  // peak
        18..24 => 2, // off-peak
        _ => 4,      // night (0..8)
    }
}

/// Evaluate whether a subject should be scraped now.
///
/// Combines base interval, time-of-day multiplier, pause detection (empty
/// fetches / consecutive failures), and term category scheduling tiers.
pub fn evaluate_subject(
    stats: &SubjectStats,
    now: DateTime<Utc>,
    category: TermCategory,
) -> SubjectSchedule {
    let elapsed = (now - stats.last_completed)
        .to_std()
        .unwrap_or(Duration::ZERO);

    // Past and Archived terms use a fixed long interval, bypassing adaptive logic entirely.
    if category == TermCategory::Past || category == TermCategory::Archived {
        return if elapsed >= ARCHIVED_INTERVAL {
            SubjectSchedule::Eligible(ARCHIVED_INTERVAL)
        } else {
            SubjectSchedule::Cooldown(ARCHIVED_INTERVAL - elapsed)
        };
    }

    let probe_due = elapsed >= PAUSE_PROBE_INTERVAL;

    // Pause on repeated empty fetches
    if stats.consecutive_empty_fetches >= EMPTY_FETCH_PAUSE_THRESHOLD {
        return if probe_due {
            SubjectSchedule::Eligible(PAUSE_PROBE_INTERVAL)
        } else {
            SubjectSchedule::Paused
        };
    }

    // Pause on all-failures
    if stats.recent_success_count == 0 && stats.recent_failure_count >= FAILURE_PAUSE_THRESHOLD {
        return if probe_due {
            SubjectSchedule::Eligible(PAUSE_PROBE_INTERVAL)
        } else {
            SubjectSchedule::Paused
        };
    }

    let base = compute_base_interval(stats);
    let multiplier = time_of_day_multiplier(now);
    let effective = base * multiplier;

    if elapsed >= effective {
        SubjectSchedule::Eligible(effective)
    } else {
        let remaining = effective - elapsed;
        SubjectSchedule::Cooldown(remaining)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    /// Create a default `SubjectStats` for testing. Callers mutate fields as needed.
    fn make_stats(subject: &str) -> SubjectStats {
        SubjectStats {
            subject: subject.to_string(),
            term: "202620".to_string(),
            recent_runs: 10,
            avg_change_ratio: 0.0,
            consecutive_zero_changes: 0,
            consecutive_empty_fetches: 0,
            recent_failure_count: 0,
            recent_success_count: 10,
            last_completed: Utc::now() - chrono::Duration::hours(1),
        }
    }

    // -- compute_base_interval tests --

    #[test]
    fn test_cold_start_returns_floor() {
        let mut stats = make_stats("CS");
        stats.recent_runs = 0;
        assert_eq!(compute_base_interval(&stats), COLD_START_INTERVAL);
    }

    #[test]
    fn test_high_change_rate() {
        let mut stats = make_stats("CS");
        stats.avg_change_ratio = 0.15;
        assert_eq!(compute_base_interval(&stats), FLOOR_INTERVAL);
    }

    #[test]
    fn test_moderate_high_change() {
        let mut stats = make_stats("CS");
        stats.avg_change_ratio = 0.07;
        assert_eq!(compute_base_interval(&stats), MODERATE_HIGH_INTERVAL);
    }

    #[test]
    fn test_moderate_low_change() {
        let mut stats = make_stats("CS");
        stats.avg_change_ratio = 0.03;
        assert_eq!(compute_base_interval(&stats), MODERATE_LOW_INTERVAL);
    }

    #[test]
    fn test_low_change() {
        let mut stats = make_stats("CS");
        stats.avg_change_ratio = 0.005;
        assert_eq!(compute_base_interval(&stats), LOW_CHANGE_INTERVAL);
    }

    #[test]
    fn test_zero_5_consecutive() {
        let mut stats = make_stats("CS");
        stats.avg_change_ratio = 0.0;
        stats.consecutive_zero_changes = 5;
        assert_eq!(compute_base_interval(&stats), ZERO_5_INTERVAL);
    }

    #[test]
    fn test_zero_10_consecutive() {
        let mut stats = make_stats("CS");
        stats.avg_change_ratio = 0.0;
        stats.consecutive_zero_changes = 10;
        assert_eq!(compute_base_interval(&stats), ZERO_10_INTERVAL);
    }

    #[test]
    fn test_zero_20_consecutive() {
        let mut stats = make_stats("CS");
        stats.avg_change_ratio = 0.0;
        stats.consecutive_zero_changes = 20;
        assert_eq!(compute_base_interval(&stats), CEILING_INTERVAL);
    }

    // -- evaluate_subject tests --

    #[test]
    fn test_pause_empty_fetches() {
        let mut stats = make_stats("CS");
        stats.consecutive_empty_fetches = 3;
        stats.last_completed = Utc::now() - chrono::Duration::minutes(10);
        let result = evaluate_subject(&stats, Utc::now(), TermCategory::Current);
        assert_eq!(result, SubjectSchedule::Paused);
    }

    #[test]
    fn test_pause_all_failures() {
        let mut stats = make_stats("CS");
        stats.recent_success_count = 0;
        stats.recent_failure_count = 5;
        stats.last_completed = Utc::now() - chrono::Duration::minutes(10);
        let result = evaluate_subject(&stats, Utc::now(), TermCategory::Current);
        assert_eq!(result, SubjectSchedule::Paused);
    }

    #[test]
    fn test_probe_after_pause() {
        let mut stats = make_stats("CS");
        stats.consecutive_empty_fetches = 5;
        stats.last_completed = Utc::now() - chrono::Duration::hours(7);
        let result = evaluate_subject(&stats, Utc::now(), TermCategory::Current);
        assert_eq!(result, SubjectSchedule::Eligible(PAUSE_PROBE_INTERVAL));
    }

    #[test]
    fn test_past_term_uses_archived_interval() {
        let mut stats = make_stats("CS");
        stats.last_completed = Utc::now() - chrono::Duration::hours(49);
        let result = evaluate_subject(&stats, Utc::now(), TermCategory::Past);
        assert_eq!(result, SubjectSchedule::Eligible(ARCHIVED_INTERVAL));
    }

    #[test]
    fn test_past_term_cooldown_before_48h() {
        let mut stats = make_stats("CS");
        stats.last_completed = Utc::now() - chrono::Duration::hours(24);
        let result = evaluate_subject(&stats, Utc::now(), TermCategory::Past);
        assert!(matches!(result, SubjectSchedule::Cooldown(_)));
    }

    #[test]
    fn test_archived_eligible_after_48h() {
        let mut stats = make_stats("CS");
        stats.last_completed = Utc::now() - chrono::Duration::hours(49);
        let result = evaluate_subject(&stats, Utc::now(), TermCategory::Archived);
        assert_eq!(result, SubjectSchedule::Eligible(ARCHIVED_INTERVAL));
    }

    #[test]
    fn test_archived_cooldown_before_48h() {
        let mut stats = make_stats("CS");
        stats.last_completed = Utc::now() - chrono::Duration::hours(24);
        let result = evaluate_subject(&stats, Utc::now(), TermCategory::Archived);
        assert!(matches!(result, SubjectSchedule::Cooldown(_)));
    }

    #[test]
    fn test_cooldown_not_elapsed() {
        let mut stats = make_stats("CS");
        stats.avg_change_ratio = 0.15; // floor = 3 min
        stats.last_completed = Utc::now() - chrono::Duration::seconds(30);
        // Use a peak-hours timestamp so multiplier = 1
        let peak = Utc.with_ymd_and_hms(2025, 7, 14, 15, 0, 0).unwrap(); // Mon 10am CT
        stats.last_completed = peak - chrono::Duration::seconds(30);
        let result = evaluate_subject(&stats, peak, TermCategory::Current);
        assert!(matches!(result, SubjectSchedule::Cooldown(_)));
    }

    #[test]
    fn test_eligible_elapsed() {
        let mut stats = make_stats("CS");
        stats.avg_change_ratio = 0.15; // floor = 3 min
        let peak = Utc.with_ymd_and_hms(2025, 7, 14, 15, 0, 0).unwrap(); // Mon 10am CT
        stats.last_completed = peak - chrono::Duration::minutes(5);
        let result = evaluate_subject(&stats, peak, TermCategory::Current);
        assert!(matches!(result, SubjectSchedule::Eligible(_)));
    }

    // -- time_of_day_multiplier tests --

    #[test]
    fn test_time_multiplier_peak() {
        // Monday 10am CT = 15:00 UTC
        let dt = Utc.with_ymd_and_hms(2025, 7, 14, 15, 0, 0).unwrap();
        assert_eq!(time_of_day_multiplier(dt), 1);
    }

    #[test]
    fn test_time_multiplier_offpeak() {
        // Monday 8pm CT = 01:00 UTC next day, but let's use Tuesday 01:00 UTC = Mon 8pm CT
        let dt = Utc.with_ymd_and_hms(2025, 7, 15, 1, 0, 0).unwrap();
        assert_eq!(time_of_day_multiplier(dt), 2);
    }

    #[test]
    fn test_time_multiplier_night() {
        // 3am CT = 08:00 UTC
        let dt = Utc.with_ymd_and_hms(2025, 7, 14, 8, 0, 0).unwrap();
        assert_eq!(time_of_day_multiplier(dt), 4);
    }

    #[test]
    fn test_time_multiplier_weekend() {
        // Saturday noon CT = 17:00 UTC
        let dt = Utc.with_ymd_and_hms(2025, 7, 12, 17, 0, 0).unwrap();
        assert_eq!(time_of_day_multiplier(dt), 4);
    }
}
