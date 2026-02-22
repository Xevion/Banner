//! Timeline API endpoint for enrollment aggregation by subject over time.
//!
//! Accepts multiple time ranges, merges overlaps, aligns to 15-minute
//! slot boundaries, and returns per-subject enrollment totals for each slot.
//! Only courses whose meeting times overlap a given slot contribute to that
//! slot's totals — so the chart reflects the actual class schedule rhythm.
//!
//! Course data is served from an ISR-style in-memory cache (see
//! [`ScheduleCache`]) that refreshes hourly in the background with
//! stale-while-revalidate semantics.

use axum::{extract::State, response::Json};
use chrono::{DateTime, Datelike, Duration, NaiveTime, Timelike, Utc};
use chrono_tz::US::Central;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use ts_rs::TS;

use crate::state::AppState;
use crate::web::error::ApiError;
use crate::web::schedule_cache::weekday_bit;

const SLOT_SECONDS: i64 = 15 * 60;
const SLOT_MINUTES: u16 = 15;

const MAX_RANGES: usize = 20;
const MAX_RANGE_SPAN: Duration = Duration::hours(72);
const MAX_TOTAL_SPAN: Duration = Duration::hours(168);

#[derive(Debug, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TimelineRequest {
    ranges: Vec<TimeRange>,
}

#[derive(Debug, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TimeRange {
    /// ISO-8601 UTC timestamp (e.g., "2024-01-15T10:30:00Z")
    #[ts(type = "string")]
    start: DateTime<Utc>,
    /// ISO-8601 UTC timestamp (e.g., "2024-01-15T12:30:00Z")
    #[ts(type = "string")]
    end: DateTime<Utc>,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TimelineResponse {
    /// 15-minute slots with per-subject enrollment totals, sorted by time.
    slots: Vec<TimelineSlot>,
    /// All subject codes present in the returned data.
    subjects: Vec<String>,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TimelineSlot {
    /// ISO-8601 UTC timestamp at the start of this 15-minute bucket (e.g., "2024-01-15T10:30:00Z")
    #[ts(type = "string")]
    time: DateTime<Utc>,
    /// Subject code → total enrollment in this slot.
    #[ts(type = "Record<string, number>")]
    subjects: BTreeMap<String, i64>,
}

/// Floor a timestamp to the nearest 15-minute boundary.
fn align_floor(ts: DateTime<Utc>) -> DateTime<Utc> {
    let secs = ts.timestamp();
    let aligned = (secs / SLOT_SECONDS) * SLOT_SECONDS;
    DateTime::from_timestamp(aligned, 0).unwrap_or(ts)
}

/// Ceil a timestamp to the nearest 15-minute boundary.
fn align_ceil(ts: DateTime<Utc>) -> DateTime<Utc> {
    let secs = ts.timestamp();
    let aligned = ((secs + SLOT_SECONDS - 1) / SLOT_SECONDS) * SLOT_SECONDS;
    DateTime::from_timestamp(aligned, 0).unwrap_or(ts)
}

#[derive(Debug, Clone, Copy)]
struct AlignedRange {
    start: DateTime<Utc>,
    end: DateTime<Utc>,
}

/// Merge overlapping/adjacent ranges into a minimal set.
fn merge_ranges(mut ranges: Vec<AlignedRange>) -> Vec<AlignedRange> {
    if ranges.is_empty() {
        return ranges;
    }
    ranges.sort_by_key(|r| r.start);
    let mut merged: Vec<AlignedRange> = vec![ranges[0]];
    for r in &ranges[1..] {
        let last = merged.last_mut().unwrap();
        if r.start <= last.end {
            last.end = last.end.max(r.end);
        } else {
            merged.push(*r);
        }
    }
    merged
}

/// Generate all aligned slot timestamps within the merged ranges.
fn generate_slots(merged: &[AlignedRange]) -> BTreeSet<DateTime<Utc>> {
    let mut slots = BTreeSet::new();
    for range in merged {
        let mut t = range.start;
        while t < range.end {
            slots.insert(t);
            t += Duration::seconds(SLOT_SECONDS);
        }
    }
    slots
}

/// `POST /api/timeline`
///
/// Accepts a JSON body with multiple time ranges. Returns per-subject
/// enrollment totals bucketed into 15-minute slots. Only courses whose
/// meeting schedule overlaps a slot contribute to that slot's count.
pub(crate) async fn timeline(
    State(state): State<AppState>,
    Json(body): Json<TimelineRequest>,
) -> Result<Json<TimelineResponse>, ApiError> {
    if body.ranges.is_empty() {
        return Err(ApiError::bad_request("At least one range is required"));
    }
    if body.ranges.len() > MAX_RANGES {
        return Err(ApiError::bad_request(format!(
            "Too many ranges (max {MAX_RANGES})"
        )));
    }

    let mut aligned: Vec<AlignedRange> = Vec::with_capacity(body.ranges.len());
    for r in &body.ranges {
        if r.end <= r.start {
            return Err(ApiError::bad_request(format!(
                "Range end ({}) must be after start ({})",
                r.end, r.start
            )));
        }
        let span = r.end - r.start;
        if span > MAX_RANGE_SPAN {
            return Err(ApiError::bad_request(format!(
                "Range span ({} hours) exceeds maximum ({} hours)",
                span.num_hours(),
                MAX_RANGE_SPAN.num_hours()
            )));
        }
        aligned.push(AlignedRange {
            start: align_floor(r.start),
            end: align_ceil(r.end),
        });
    }

    let merged = merge_ranges(aligned);

    // Validate total span
    let total_span: Duration = merged.iter().map(|r| r.end - r.start).sum();
    if total_span > MAX_TOTAL_SPAN {
        return Err(ApiError::bad_request(format!(
            "Total time span ({} hours) exceeds maximum ({} hours)",
            total_span.num_hours(),
            MAX_TOTAL_SPAN.num_hours()
        )));
    }

    state.schedule_cache.ensure_fresh();
    let snapshot = state.schedule_cache.snapshot();

    let slot_times = generate_slots(&merged);
    let mut all_subjects: BTreeSet<Arc<str>> = BTreeSet::new();

    let slots: Vec<TimelineSlot> = slot_times
        .into_iter()
        .map(|utc_time| {
            // Convert UTC slot to Central time for local day-of-week and time-of-day
            let local = utc_time.with_timezone(&Central);
            let local_date = local.date_naive();
            let local_time = local.time();
            let weekday = local.weekday();
            let wday_bit = weekday_bit(weekday);
            let slot_start_minutes = time_to_minutes(local_time);
            let slot_end_minutes = slot_start_minutes + SLOT_MINUTES;

            // Use Arc<str> keys internally — Arc::clone is an atomic refcount
            // bump vs String::clone which heap-allocates every time.
            let mut subject_totals: BTreeMap<Arc<str>, i64> = BTreeMap::new();

            for course in &snapshot.courses {
                let active = course.schedules.iter().any(|s| {
                    s.active_during(local_date, wday_bit, slot_start_minutes, slot_end_minutes)
                });
                if active {
                    *subject_totals
                        .entry(Arc::clone(&course.subject))
                        .or_default() += course.enrollment as i64;
                }
            }

            all_subjects.extend(subject_totals.keys().map(Arc::clone));

            // Convert to String keys at the response boundary.
            let subjects: BTreeMap<String, i64> = subject_totals
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect();

            TimelineSlot {
                time: utc_time,
                subjects,
            }
        })
        .collect();

    let subjects: Vec<String> = all_subjects.into_iter().map(|s| s.to_string()).collect();

    Ok(Json(TimelineResponse { slots, subjects }))
}

/// Convert a `NaiveTime` to minutes since midnight.
fn time_to_minutes(t: NaiveTime) -> u16 {
    (t.hour() * 60 + t.minute()) as u16
}
