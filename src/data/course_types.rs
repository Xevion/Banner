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

/// Credit hours for a course section — either a fixed value or a range.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", tag = "type")]
#[ts(export)]
pub enum CreditHours {
    /// A single fixed credit hour value.
    Fixed { hours: i32 },
    /// A range of credit hours with the invariant that `low <= high`.
    Range { low: i32, high: i32 },
}

impl CreditHours {
    /// Creates a `CreditHours::Range`, returning an error if `low > high`.
    #[allow(dead_code)]
    pub fn range(low: i32, high: i32) -> Result<Self, String> {
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
