//! ISR-style schedule cache for timeline enrollment queries.
//!
//! Loads all courses with their pre-extracted meeting scalars from the
//! `course_meetings` table into a compact in-memory representation, and caches
//! the result. The cache is refreshed in the background every hour using a
//! stale-while-revalidate pattern with singleflight deduplication -- readers
//! always get the current cached value instantly, never blocking on a refresh.
//!
//! ## Optimizations
//!
//! - **Denormalized scalars**: Meeting times are pre-extracted into the
//!   `course_meetings` table as native SQL types (SMALLINT, DATE), eliminating
//!   runtime JSONB parsing entirely.
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
    /// Singleflight guard -- true while a refresh task is in flight.
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
    /// Always returns immediately -- the caller uses the current snapshot.
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

/// Reads pre-extracted meeting scalars from the `course_meetings` table.
/// Ordered by `c.id` so the streaming consumer can group rows by course.
const SCHEDULE_QUERY: &str = r#"
SELECT
    c.id,
    c.subject,
    c.enrollment,
    cm.day_bits,
    cm.begin_minutes,
    cm.end_minutes,
    cm.start_date,
    cm.end_date
FROM courses c
JOIN course_meetings cm ON cm.course_id = c.id
ORDER BY c.id
"#;

/// One row from the course_meetings join. Each course produces one row per
/// meeting time entry.
#[derive(sqlx::FromRow)]
struct MeetingRow {
    id: i32,
    subject: String,
    enrollment: i32,
    day_bits: i16,
    begin_minutes: i16,
    end_minutes: i16,
    start_date: NaiveDate,
    end_date: NaiveDate,
}

/// Load all courses from the `course_meetings` table and build a snapshot.
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

        current_schedules.push(ParsedSchedule {
            days: row.day_bits as u8,
            begin_minutes: row.begin_minutes as u16,
            end_minutes: row.end_minutes as u16,
            start_date: row.start_date,
            end_date: row.end_date,
        });
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
        // Slot 11:00-11:15 -- after the meeting ends
        assert!(!sched.active_during(date, weekday_bit(chrono::Weekday::Mon), 660, 675));
        // Slot 9:45-10:00 -- just before meeting starts (end=600, begin=600 -> no overlap)
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

        // Monday Jan 6 2025 -- before semester
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
        // Slot 10:45-11:00 -- overlaps last 5 minutes of meeting
        assert!(sched.active_during(date, weekday_bit(chrono::Weekday::Mon), 645, 660));
        // Slot 9:45-10:00 -- ends exactly when meeting starts, no overlap
        assert!(!sched.active_during(date, weekday_bit(chrono::Weekday::Mon), 585, 600));
        // Slot 10:50-11:05 -- starts exactly when meeting ends, no overlap
        assert!(!sched.active_during(date, weekday_bit(chrono::Weekday::Mon), 650, 665));
    }
}
