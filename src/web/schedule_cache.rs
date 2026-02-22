//! ISR-style schedule cache for timeline enrollment queries.
//!
//! Loads all courses with their meeting times from the database, parses the
//! JSONB meeting times into a compact in-memory representation, and caches
//! the result. The cache is refreshed in the background every hour using a
//! stale-while-revalidate pattern with singleflight deduplication — readers
//! always get the current cached value instantly, never blocking on a refresh.
//!
//! ## Optimizations
//!
//! - **SQL-side extraction**: The query uses a lateral join to unnest meeting
//!   times and extract only the 5 scalar fields needed per meeting directly in
//!   SQL, avoiding `serde_json::Value` materialization entirely.
//! - **Dual format support**: SQL COALESCE handles both legacy snake_case
//!   (`begin_time`, boolean day flags) and current camelCase (`timeRange`,
//!   `days` array) meeting time formats transparently.
//! - **Streaming**: Rows are processed one at a time via `sqlx::fetch()`,
//!   never materializing an intermediate `Vec<ScheduleRow>`.
//! - **Subject interning**: Subjects are stored as `Arc<str>` and deduplicated
//!   via a `HashSet`, eliminating per-request cloning in the timeline hot path.

use crate::utils::fmt_duration;
use chrono::NaiveDate;
use futures::TryStreamExt;
use sqlx::PgPool;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::watch;
use tracing::{debug, error, info};

const REFRESH_INTERVAL: std::time::Duration = std::time::Duration::from_secs(60 * 60);

/// A single meeting time block, pre-parsed for fast filtering.
#[derive(Debug, Clone)]
pub(crate) struct ParsedSchedule {
    /// Bitmask of days: bit 0 = Monday, bit 6 = Sunday.
    days: u8,
    /// Minutes since midnight for start (e.g. 600 = 10:00).
    begin_minutes: u16,
    /// Minutes since midnight for end (e.g. 650 = 10:50).
    end_minutes: u16,
    /// First day the meeting pattern is active.
    start_date: NaiveDate,
    /// Last day the meeting pattern is active.
    end_date: NaiveDate,
}

/// A course with its enrollment and pre-parsed schedule blocks.
#[derive(Debug, Clone)]
pub(crate) struct CachedCourse {
    pub(crate) subject: Arc<str>,
    pub(crate) enrollment: i32,
    pub(crate) schedules: Vec<ParsedSchedule>,
}

/// The immutable snapshot of all courses, swapped atomically on refresh.
#[derive(Debug, Clone)]
pub(crate) struct ScheduleSnapshot {
    pub(crate) courses: Vec<CachedCourse>,
    refreshed_at: std::time::Instant,
}

/// Shared schedule cache. Clone-cheap (all `Arc`-wrapped internals).
#[derive(Clone)]
pub struct ScheduleCache {
    /// Current snapshot, updated via `watch` channel for lock-free reads.
    rx: watch::Receiver<Arc<ScheduleSnapshot>>,
    /// Sender side, held to push new snapshots.
    tx: Arc<watch::Sender<Arc<ScheduleSnapshot>>>,
    /// Singleflight guard — true while a refresh task is in flight.
    refreshing: Arc<AtomicBool>,
    /// Database pool for refresh queries.
    pool: PgPool,
}

impl ScheduleCache {
    /// Create a new cache with an empty initial snapshot.
    pub(crate) fn new(pool: PgPool) -> Self {
        let empty = Arc::new(ScheduleSnapshot {
            courses: Vec::new(),
            refreshed_at: std::time::Instant::now(),
        });
        let (tx, rx) = watch::channel(empty);
        Self {
            rx,
            tx: Arc::new(tx),
            refreshing: Arc::new(AtomicBool::new(false)),
            pool,
        }
    }

    /// Get the current snapshot. Never blocks on refresh.
    pub(crate) fn snapshot(&self) -> Arc<ScheduleSnapshot> {
        self.rx.borrow().clone()
    }

    /// Check freshness and trigger a background refresh if stale.
    /// Always returns immediately — the caller uses the current snapshot.
    pub(crate) fn ensure_fresh(&self) {
        let snap = self.rx.borrow();
        if snap.refreshed_at.elapsed() < REFRESH_INTERVAL {
            return;
        }
        if self
            .refreshing
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            debug!("Schedule cache refresh already in flight, skipping");
            return;
        }
        let cache = self.clone();
        tokio::spawn(async move {
            match load_snapshot(&cache.pool).await {
                Ok(snap) => {
                    let count = snap.courses.len();
                    let _ = cache.tx.send(Arc::new(snap));
                    info!(courses = count, "Schedule cache refreshed");
                }
                Err(e) => {
                    error!(error = %e, "Failed to refresh schedule cache");
                }
            }
            cache.refreshing.store(false, Ordering::Release);
        });
    }

    /// Force an initial load (blocking). Call once at startup.
    pub(crate) async fn load(&self) -> anyhow::Result<()> {
        let snap = load_snapshot(&self.pool).await?;
        let count = snap.courses.len();
        let _ = self.tx.send(Arc::new(snap));
        info!(courses = count, "Schedule cache initially loaded");
        Ok(())
    }
}

/// SQL query that extracts meeting time scalars via lateral join.
///
/// Returns one row per meeting time per course (plus one NULL-meeting row for
/// courses with empty `meeting_times`). Handles both legacy and current JSON
/// formats via COALESCE:
///
/// - **Time**: `begin_time`/`end_time` (legacy "HHMM") or `timeRange.start`/`.end` (current "HH:MM:SS")
/// - **Date**: `start_date`/`end_date` (legacy "MM/DD/YYYY") or `dateRange.start`/`.end` (current "YYYY-MM-DD")
/// - **Days**: Boolean flags (legacy) or `days` string array (current) → computed as a bitmask
///
/// Ordered by `c.id` so the streaming consumer can group rows by course.
const SCHEDULE_QUERY: &str = r#"
SELECT
    c.id,
    c.subject,
    c.enrollment,
    COALESCE(mt.val->>'begin_time', mt.val->'timeRange'->>'start') as begin_time,
    COALESCE(mt.val->>'end_time', mt.val->'timeRange'->>'end') as end_time,
    COALESCE(mt.val->>'start_date', mt.val->'dateRange'->>'start') as start_date,
    COALESCE(mt.val->>'end_date', mt.val->'dateRange'->>'end') as end_date,
    COALESCE(
        (SELECT bit_or(
            CASE d
                WHEN 'monday' THEN 1 WHEN 'tuesday' THEN 2 WHEN 'wednesday' THEN 4
                WHEN 'thursday' THEN 8 WHEN 'friday' THEN 16 WHEN 'saturday' THEN 32
                WHEN 'sunday' THEN 64
            END
        ) FROM jsonb_array_elements_text(mt.val->'days') AS d),
        (CASE WHEN (mt.val->>'monday')::boolean THEN 1 ELSE 0 END |
         CASE WHEN (mt.val->>'tuesday')::boolean THEN 2 ELSE 0 END |
         CASE WHEN (mt.val->>'wednesday')::boolean THEN 4 ELSE 0 END |
         CASE WHEN (mt.val->>'thursday')::boolean THEN 8 ELSE 0 END |
         CASE WHEN (mt.val->>'friday')::boolean THEN 16 ELSE 0 END |
         CASE WHEN (mt.val->>'saturday')::boolean THEN 32 ELSE 0 END |
         CASE WHEN (mt.val->>'sunday')::boolean THEN 64 ELSE 0 END)
    )::smallint as day_bits
FROM courses c
LEFT JOIN LATERAL jsonb_array_elements(c.meeting_times) AS mt(val) ON true
ORDER BY c.id
"#;

/// One row from the lateral-join query. Each course produces one row per
/// meeting time element, with NULL meeting columns for courses that have
/// an empty `meeting_times` array.
#[derive(sqlx::FromRow)]
struct MeetingRow {
    id: i32,
    subject: String,
    enrollment: i32,
    begin_time: Option<String>,
    end_time: Option<String>,
    start_date: Option<String>,
    end_date: Option<String>,
    day_bits: Option<i16>,
}

/// Load all courses via streaming lateral join and build a snapshot.
///
/// Rows arrive ordered by course `id`. We accumulate schedules for the
/// current course and emit a `CachedCourse` when the id changes. Subject
/// strings are interned into `Arc<str>` for cheap cloning downstream.
async fn load_snapshot(pool: &PgPool) -> anyhow::Result<ScheduleSnapshot> {
    let start = std::time::Instant::now();

    let mut subject_intern: HashSet<Arc<str>> = HashSet::new();
    let mut courses: Vec<CachedCourse> = Vec::new();

    // Streaming state: accumulate schedules for the current course id.
    let mut current_id: Option<i32> = None;
    let mut current_subject: Arc<str> = Arc::from("");
    let mut current_enrollment: i32 = 0;
    let mut current_schedules: Vec<ParsedSchedule> = Vec::new();

    let mut stream = sqlx::query_as::<_, MeetingRow>(SCHEDULE_QUERY).fetch(pool);

    while let Some(row) = stream.try_next().await? {
        if current_id != Some(row.id) {
            // Emit the previous course (if any).
            if current_id.is_some() {
                courses.push(CachedCourse {
                    subject: Arc::clone(&current_subject),
                    enrollment: current_enrollment,
                    schedules: std::mem::take(&mut current_schedules),
                });
            }

            current_id = Some(row.id);
            current_subject = intern_subject(&mut subject_intern, &row.subject);
            current_enrollment = row.enrollment;
        }

        // Parse meeting from the flat columns (skips NULL rows from LEFT JOIN).
        if let Some(sched) = parse_meeting_row(&row) {
            current_schedules.push(sched);
        }
    }

    // Emit the last course.
    if current_id.is_some() {
        courses.push(CachedCourse {
            subject: current_subject,
            enrollment: current_enrollment,
            schedules: current_schedules,
        });
    }

    debug!(
        courses = courses.len(),
        subjects = subject_intern.len(),
        elapsed = fmt_duration(start.elapsed()),
        "Schedule snapshot built"
    );

    Ok(ScheduleSnapshot {
        courses,
        refreshed_at: std::time::Instant::now(),
    })
}

/// Look up or insert a subject in the intern set, returning a cheap `Arc<str>`.
fn intern_subject(set: &mut HashSet<Arc<str>>, subject: &str) -> Arc<str> {
    if let Some(existing) = set.get(subject) {
        return Arc::clone(existing);
    }
    let arc: Arc<str> = Arc::from(subject);
    set.insert(Arc::clone(&arc));
    arc
}

/// Parse a single meeting row's flat columns into a `ParsedSchedule`.
/// Returns `None` for NULL meeting columns (courses with empty meeting_times)
/// or unparseable / zero-day meetings.
fn parse_meeting_row(row: &MeetingRow) -> Option<ParsedSchedule> {
    let begin_str = row.begin_time.as_deref()?;
    let end_str = row.end_time.as_deref()?;
    let start_date_str = row.start_date.as_deref()?;
    let end_date_str = row.end_date.as_deref()?;

    let begin_minutes = parse_time(begin_str)?;
    let end_minutes = parse_time(end_str)?;

    if end_minutes <= begin_minutes {
        return None;
    }

    let start_date = parse_date(start_date_str)?;
    let end_date = parse_date(end_date_str)?;

    let days = row.day_bits.unwrap_or(0) as u8;
    if days == 0 {
        return None;
    }

    Some(ParsedSchedule {
        days,
        begin_minutes,
        end_minutes,
        start_date,
        end_date,
    })
}

/// Parse a time string to minutes since midnight.
///
/// Accepts two formats:
/// - Legacy "HHMM" (e.g. "1000" → 600)
/// - Current "HH:MM:SS" or "HH:MM" (e.g. "10:00:00" → 600)
fn parse_time(s: &str) -> Option<u16> {
    if let Some((h, rest)) = s.split_once(':') {
        // "HH:MM:SS" or "HH:MM"
        let m = rest.split(':').next()?;
        let hours: u16 = h.parse().ok()?;
        let mins: u16 = m.parse().ok()?;
        if hours >= 24 || mins >= 60 {
            return None;
        }
        Some(hours * 60 + mins)
    } else {
        // Legacy "HHMM"
        parse_hhmm(s)
    }
}

/// Parse "HHMM" → minutes since midnight.
fn parse_hhmm(s: &str) -> Option<u16> {
    if s.len() != 4 {
        return None;
    }
    let hours: u16 = s[..2].parse().ok()?;
    let mins: u16 = s[2..].parse().ok()?;
    if hours >= 24 || mins >= 60 {
        return None;
    }
    Some(hours * 60 + mins)
}

/// Parse a date string in either MM/DD/YYYY or YYYY-MM-DD format.
fn parse_date(s: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(s, "%m/%d/%Y")
        .or_else(|_| NaiveDate::parse_from_str(s, "%Y-%m-%d"))
        .ok()
}

/// Day-of-week as our bitmask index (Monday = 0 .. Sunday = 6).
/// Chrono's `weekday().num_days_from_monday()` already gives 0=Mon..6=Sun.
pub(crate) fn weekday_bit(day: chrono::Weekday) -> u8 {
    1 << day.num_days_from_monday()
}

impl ParsedSchedule {
    /// Check if this schedule is active during a given slot.
    ///
    /// `slot_date` is the calendar date of the slot.
    /// `slot_start` / `slot_end` are minutes since midnight for the 15-min window.
    #[inline]
    pub(crate) fn active_during(
        &self,
        slot_date: NaiveDate,
        slot_weekday_bit: u8,
        slot_start_minutes: u16,
        slot_end_minutes: u16,
    ) -> bool {
        // Day-of-week check
        if self.days & slot_weekday_bit == 0 {
            return false;
        }
        // Date range check
        if slot_date < self.start_date || slot_date > self.end_date {
            return false;
        }
        // Time overlap: meeting [begin, end) overlaps slot [start, end)
        self.begin_minutes < slot_end_minutes && self.end_minutes > slot_start_minutes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    // -- parse_time tests (covers both formats) --

    #[test]
    fn parse_time_hhmm() {
        assert_eq!(parse_time("0000"), Some(0));
        assert_eq!(parse_time("0930"), Some(570));
        assert_eq!(parse_time("1350"), Some(830));
        assert_eq!(parse_time("2359"), Some(1439));
    }

    #[test]
    fn parse_time_colon_format() {
        assert_eq!(parse_time("00:00:00"), Some(0));
        assert_eq!(parse_time("09:30:00"), Some(570));
        assert_eq!(parse_time("13:50:00"), Some(830));
        assert_eq!(parse_time("23:59:00"), Some(1439));
        // HH:MM without seconds
        assert_eq!(parse_time("10:00"), Some(600));
        assert_eq!(parse_time("14:30"), Some(870));
    }

    #[test]
    fn parse_time_invalid() {
        assert_eq!(parse_time(""), None);
        assert_eq!(parse_time("abc"), None);
        assert_eq!(parse_time("2500"), None);
        assert_eq!(parse_time("0060"), None);
        assert_eq!(parse_time("25:00:00"), None);
        assert_eq!(parse_time("00:60:00"), None);
    }

    #[test]
    fn parse_hhmm_valid() {
        assert_eq!(parse_hhmm("0000"), Some(0));
        assert_eq!(parse_hhmm("0930"), Some(570));
        assert_eq!(parse_hhmm("1350"), Some(830));
        assert_eq!(parse_hhmm("2359"), Some(1439));
    }

    #[test]
    fn parse_hhmm_invalid() {
        assert_eq!(parse_hhmm(""), None);
        assert_eq!(parse_hhmm("abc"), None);
        assert_eq!(parse_hhmm("2500"), None);
        assert_eq!(parse_hhmm("0060"), None);
    }

    #[test]
    fn parse_date_valid() {
        assert_eq!(
            parse_date("08/26/2025"),
            Some(NaiveDate::from_ymd_opt(2025, 8, 26).unwrap())
        );
        assert_eq!(
            parse_date("2025-08-26"),
            Some(NaiveDate::from_ymd_opt(2025, 8, 26).unwrap())
        );
    }

    // -- parse_meeting_row tests --

    #[test]
    fn parse_meeting_row_old_format() {
        let row = MeetingRow {
            id: 1,
            subject: "CS".into(),
            enrollment: 30,
            begin_time: Some("1000".into()),
            end_time: Some("1050".into()),
            start_date: Some("08/26/2025".into()),
            end_date: Some("12/13/2025".into()),
            day_bits: Some(0b0010101), // Mon, Wed, Fri
        };
        let sched = parse_meeting_row(&row).unwrap();
        assert_eq!(sched.begin_minutes, 600);
        assert_eq!(sched.end_minutes, 650);
        assert_eq!(sched.days, 0b0010101);
        assert_eq!(
            sched.start_date,
            NaiveDate::from_ymd_opt(2025, 8, 26).unwrap()
        );
    }

    #[test]
    fn parse_meeting_row_new_format() {
        let row = MeetingRow {
            id: 2,
            subject: "MAT".into(),
            enrollment: 25,
            begin_time: Some("10:00:00".into()),
            end_time: Some("10:50:00".into()),
            start_date: Some("2025-08-26".into()),
            end_date: Some("2025-12-13".into()),
            day_bits: Some(0b0001010), // Tue, Thu
        };
        let sched = parse_meeting_row(&row).unwrap();
        assert_eq!(sched.begin_minutes, 600);
        assert_eq!(sched.end_minutes, 650);
        assert_eq!(sched.days, 0b0001010);
    }

    #[test]
    fn parse_meeting_row_null_columns() {
        let row = MeetingRow {
            id: 3,
            subject: "ENG".into(),
            enrollment: 20,
            begin_time: None,
            end_time: None,
            start_date: None,
            end_date: None,
            day_bits: None,
        };
        assert!(parse_meeting_row(&row).is_none());
    }

    #[test]
    fn parse_meeting_row_zero_days() {
        let row = MeetingRow {
            id: 4,
            subject: "PHY".into(),
            enrollment: 15,
            begin_time: Some("1000".into()),
            end_time: Some("1050".into()),
            start_date: Some("08/26/2025".into()),
            end_date: Some("12/13/2025".into()),
            day_bits: Some(0),
        };
        assert!(parse_meeting_row(&row).is_none());
    }

    #[test]
    fn parse_meeting_row_invalid_time_order() {
        let row = MeetingRow {
            id: 5,
            subject: "BIO".into(),
            enrollment: 10,
            begin_time: Some("1100".into()),
            end_time: Some("1000".into()), // end before begin
            start_date: Some("08/26/2025".into()),
            end_date: Some("12/13/2025".into()),
            day_bits: Some(1),
        };
        assert!(parse_meeting_row(&row).is_none());
    }

    // -- intern_subject tests --

    #[test]
    fn intern_subject_deduplicates() {
        let mut set = HashSet::new();
        let a = intern_subject(&mut set, "CS");
        let b = intern_subject(&mut set, "CS");
        assert!(Arc::ptr_eq(&a, &b));
        assert_eq!(set.len(), 1);

        let c = intern_subject(&mut set, "MAT");
        assert!(!Arc::ptr_eq(&a, &c));
        assert_eq!(set.len(), 2);
    }

    // -- active_during tests --

    #[test]
    fn active_during_matching_slot() {
        let sched = ParsedSchedule {
            days: 0b0000001, // Monday
            begin_minutes: 600,
            end_minutes: 650,
            start_date: NaiveDate::from_ymd_opt(2025, 8, 26).unwrap(),
            end_date: NaiveDate::from_ymd_opt(2025, 12, 13).unwrap(),
        };

        // Monday Sept 1 2025, 10:00-10:15 slot
        let date = NaiveDate::from_ymd_opt(2025, 9, 1).unwrap();
        assert!(sched.active_during(date, weekday_bit(chrono::Weekday::Mon), 600, 615));
    }

    #[test]
    fn active_during_wrong_day() {
        let sched = ParsedSchedule {
            days: 0b0000001, // Monday only
            begin_minutes: 600,
            end_minutes: 650,
            start_date: NaiveDate::from_ymd_opt(2025, 8, 26).unwrap(),
            end_date: NaiveDate::from_ymd_opt(2025, 12, 13).unwrap(),
        };

        // Tuesday Sept 2 2025
        let date = NaiveDate::from_ymd_opt(2025, 9, 2).unwrap();
        assert!(!sched.active_during(date, weekday_bit(chrono::Weekday::Tue), 600, 615));
    }

    #[test]
    fn active_during_no_time_overlap() {
        let sched = ParsedSchedule {
            days: 0b0000001,
            begin_minutes: 600, // 10:00
            end_minutes: 650,   // 10:50
            start_date: NaiveDate::from_ymd_opt(2025, 8, 26).unwrap(),
            end_date: NaiveDate::from_ymd_opt(2025, 12, 13).unwrap(),
        };

        let date = NaiveDate::from_ymd_opt(2025, 9, 1).unwrap(); // Monday
        // Slot 11:00-11:15 — after the meeting ends
        assert!(!sched.active_during(date, weekday_bit(chrono::Weekday::Mon), 660, 675));
        // Slot 9:45-10:00 — just before meeting starts (end=600, begin=600 → no overlap)
        assert!(!sched.active_during(date, weekday_bit(chrono::Weekday::Mon), 585, 600));
    }

    #[test]
    fn active_during_outside_date_range() {
        let sched = ParsedSchedule {
            days: 0b0000001,
            begin_minutes: 600,
            end_minutes: 650,
            start_date: NaiveDate::from_ymd_opt(2025, 8, 26).unwrap(),
            end_date: NaiveDate::from_ymd_opt(2025, 12, 13).unwrap(),
        };

        // Monday Jan 6 2025 — before semester
        let date = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        assert!(!sched.active_during(date, weekday_bit(chrono::Weekday::Mon), 600, 615));
    }

    #[test]
    fn active_during_edge_overlap() {
        let sched = ParsedSchedule {
            days: 0b0000001,
            begin_minutes: 600,
            end_minutes: 650,
            start_date: NaiveDate::from_ymd_opt(2025, 8, 26).unwrap(),
            end_date: NaiveDate::from_ymd_opt(2025, 12, 13).unwrap(),
        };

        let date = NaiveDate::from_ymd_opt(2025, 9, 1).unwrap();
        // Slot 10:45-11:00 — overlaps last 5 minutes of meeting
        assert!(sched.active_during(date, weekday_bit(chrono::Weekday::Mon), 645, 660));
        // Slot 9:45-10:00 — ends exactly when meeting starts, no overlap
        assert!(!sched.active_during(date, weekday_bit(chrono::Weekday::Mon), 585, 600));
        // Slot 10:50-11:05 — starts exactly when meeting ends, no overlap
        assert!(!sched.active_during(date, weekday_bit(chrono::Weekday::Mon), 650, 665));
    }
}
