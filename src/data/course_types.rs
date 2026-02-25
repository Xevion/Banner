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

/// Brief RateMyProfessors data for an instructor.
///
/// Present whenever an RMP profile link exists. Rating fields are `None` when the
/// profile has no reviews (0 ratings / 0.0 average).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RmpBrief {
    pub avg_rating: Option<f32>,
    pub num_ratings: Option<i32>,
    pub legacy_id: i32,
}

/// Brief BlueBook evaluation data for an instructor.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BlueBookBrief {
    pub avg_instructor_rating: f32,
    pub total_responses: i32,
}

/// Full BlueBook summary for instructor detail pages.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BlueBookFull {
    pub calibrated_rating: f32,
    pub avg_instructor_rating: f32,
    pub avg_course_rating: Option<f32>,
    pub total_responses: i32,
    pub eval_count: i32,
}

/// Full RateMyProfessors summary for instructor detail pages.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RmpFull {
    pub avg_rating: Option<f32>,
    pub avg_difficulty: Option<f32>,
    pub would_take_again_pct: Option<f32>,
    pub num_ratings: Option<i32>,
    pub legacy_id: i32,
}

/// Data source for an instructor rating.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum RatingSource {
    Both,
    Rmp,
    #[serde(rename = "bluebook")]
    BlueBook,
}

impl RatingSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Both => "both",
            Self::Rmp => "rmp",
            Self::BlueBook => "bluebook",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "both" => Some(Self::Both),
            "rmp" => Some(Self::Rmp),
            "bb" | "bluebook" => Some(Self::BlueBook),
            _ => None,
        }
    }
}

/// Bayesian composite rating combining RMP and BlueBook via regression calibration.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct InstructorRating {
    /// Posterior mean -- the headline number displayed to users.
    pub score: f32,
    /// CI lower bound -- used for ranking (penalizes low confidence).
    pub rank_score: f32,
    pub ci_lower: f32,
    pub ci_upper: f32,
    /// 0.0-1.0 scalar indicating how much data narrowed the posterior.
    pub confidence: f32,
    pub source: RatingSource,
    pub total_responses: i32,
}

pub fn build_bluebook_brief(
    avg_rating: Option<f32>,
    total_responses: Option<i64>,
) -> Option<BlueBookBrief> {
    match (avg_rating, total_responses) {
        (Some(r), Some(n)) if r > 0.0 && n > 0 => Some(BlueBookBrief {
            avg_instructor_rating: r,
            total_responses: n as i32,
        }),
        _ => None,
    }
}
