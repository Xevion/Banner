//! Structured types for course API responses.
//!
//! These types replace scattered Option fields and parallel booleans with
//! proper type-safe structures.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// An inclusive date range with the invariant that `start <= end`.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DateRange {
    pub start: NaiveDate,
    pub end: NaiveDate,
}

impl DateRange {
    /// Creates a new `DateRange`, returning an error if `start` is after `end`.
    pub fn new(start: NaiveDate, end: NaiveDate) -> Result<Self, String> {
        if start > end {
            return Err(format!(
                "invalid date range: start ({start}) is after end ({end})"
            ));
        }
        Ok(Self { start, end })
    }

    /// Number of days in the range (inclusive of both endpoints).
    #[allow(dead_code)]
    pub fn days(&self) -> i64 {
        (self.end - self.start).num_days() + 1
    }
}

/// Physical location where a course section meets.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MeetingLocation {
    pub building: Option<String>,
    pub building_description: Option<String>,
    pub room: Option<String>,
    pub campus: Option<String>,
}

/// Credit hours for a course section -- either a fixed value or a range.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", tag = "type")]
#[ts(export)]
pub enum CreditHours {
    /// A single fixed credit hour value.
    Fixed { hours: f64 },
    /// A range of credit hours with the invariant that `low <= high`.
    Range { low: f64, high: f64 },
}

impl CreditHours {
    /// Creates a `CreditHours::Range`, returning an error if `low > high`.
    #[allow(dead_code)]
    pub fn range(low: f64, high: f64) -> Result<Self, String> {
        if low > high {
            return Err(format!(
                "invalid credit hour range: low ({low}) is greater than high ({high})"
            ));
        }
        Ok(Self::Range { low, high })
    }
}

/// Cross-listed section information.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CrossList {
    pub identifier: String,
    pub capacity: i32,
    pub count: i32,
}

/// A linked section reference (e.g. lab linked to a lecture).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SectionLink {
    pub identifier: String,
}

/// Enrollment counts for a course section.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Enrollment {
    pub current: i32,
    pub max: i32,
    pub wait_count: i32,
    pub wait_capacity: i32,
}

impl Enrollment {
    /// Number of open seats remaining (never negative).
    #[allow(dead_code)]
    pub fn open_seats(&self) -> i32 {
        (self.max - self.current).max(0)
    }

    /// Whether the section is at or over capacity.
    #[allow(dead_code)]
    pub fn is_full(&self) -> bool {
        self.current >= self.max
    }

    /// Whether the section has at least one open seat.
    #[allow(dead_code)]
    pub fn is_open(&self) -> bool {
        !self.is_full()
    }

    /// Whether more students are enrolled than the section's stated capacity.
    #[allow(dead_code)]
    pub fn is_overenrolled(&self) -> bool {
        self.current > self.max
    }
}

/// Treat 0 ratings / 0.0 average as "no data", returning `None` for both fields.
/// Preserves meaningful values unchanged.
pub fn sanitize_rmp_ratings(
    avg_rating: Option<f32>,
    num_ratings: Option<i32>,
) -> (Option<f32>, Option<i32>) {
    match (avg_rating, num_ratings) {
        (Some(r), Some(n)) if r != 0.0 && n > 0 => (Some(r), Some(n)),
        _ => (None, None),
    }
}

/// RateMyProfessors rating summary for an instructor.
///
/// Present whenever an RMP profile link exists. Rating fields are `None` when the
/// profile has no reviews (0 ratings / 0.0 average).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RmpRating {
    pub avg_rating: Option<f32>,
    pub num_ratings: Option<i32>,
    pub legacy_id: i32,
    pub is_confident: bool,
}

pub const BLUEBOOK_CONFIDENCE_THRESHOLD: i32 = 10;

/// BlueBook evaluation summary for course search results.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BlueBookRating {
    pub avg_instructor_rating: f32,
    pub total_responses: i32,
    pub is_confident: bool,
}

/// BlueBook summary for instructor list cards.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BlueBookListSummary {
    pub avg_instructor_rating: f32,
    pub total_responses: i32,
}

/// Full BlueBook summary for instructor detail pages.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PublicBlueBookSummary {
    pub avg_instructor_rating: f32,
    pub avg_course_rating: Option<f32>,
    pub total_responses: i32,
    pub eval_count: i32,
}

/// Composite rating combining BlueBook and RMP via response-count-weighted average.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CompositeRating {
    pub score: f32,
    pub total_responses: i32,
}

pub fn build_bluebook_rating(
    avg_rating: Option<f32>,
    total_responses: Option<i64>,
) -> Option<BlueBookRating> {
    match (avg_rating, total_responses) {
        (Some(r), Some(n)) if r > 0.0 && n > 0 => {
            let n = n as i32;
            Some(BlueBookRating {
                avg_instructor_rating: r,
                total_responses: n,
                is_confident: n >= BLUEBOOK_CONFIDENCE_THRESHOLD,
            })
        }
        _ => None,
    }
}

/// Compute response-count-weighted average of available rating sources.
pub fn compute_composite(
    rmp_avg: Option<f32>,
    rmp_count: Option<i32>,
    bb_avg: Option<f32>,
    bb_count: i32,
) -> Option<CompositeRating> {
    let rmp = rmp_avg.zip(rmp_count).filter(|&(r, n)| r > 0.0 && n > 0);
    let bb = if bb_avg.is_some_and(|r| r > 0.0) && bb_count > 0 {
        Some((bb_avg.unwrap(), bb_count))
    } else {
        None
    };

    match (rmp, bb) {
        (Some((r_avg, r_n)), Some((b_avg, b_n))) => {
            let total = r_n + b_n;
            Some(CompositeRating {
                score: (r_avg * r_n as f32 + b_avg * b_n as f32) / total as f32,
                total_responses: total,
            })
        }
        (Some((r_avg, r_n)), None) => Some(CompositeRating {
            score: r_avg,
            total_responses: r_n,
        }),
        (None, Some((b_avg, b_n))) => Some(CompositeRating {
            score: b_avg,
            total_responses: b_n,
        }),
        (None, None) => None,
    }
}
