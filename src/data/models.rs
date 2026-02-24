//! `sqlx` models for the database schema.

use std::collections::BTreeSet;

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use ts_rs::TS;

use crate::banner::models::meetings::TimeRange;
use crate::data::course_types::{DateRange, MeetingLocation};

/// Serialize an `i64` as a string to avoid JavaScript precision loss for values exceeding 2^53.
fn serialize_i64_as_string<S: Serializer>(value: &i64, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&value.to_string())
}

/// Deserialize an `i64` from either a number or a string.
fn deserialize_i64_from_string<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<i64, D::Error> {
    use serde::de;

    struct I64OrStringVisitor;

    impl<'de> de::Visitor<'de> for I64OrStringVisitor {
        type Value = i64;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("an integer or a string containing an integer")
        }

        fn visit_i64<E: de::Error>(self, value: i64) -> Result<i64, E> {
            Ok(value)
        }

        fn visit_u64<E: de::Error>(self, value: u64) -> Result<i64, E> {
            i64::try_from(value).map_err(|_| E::custom(format!("u64 {value} out of i64 range")))
        }

        fn visit_str<E: de::Error>(self, value: &str) -> Result<i64, E> {
            value.parse().map_err(de::Error::custom)
        }
    }

    deserializer.deserialize_any(I64OrStringVisitor)
}

/// Day of the week.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum DayOfWeek {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

/// Represents a meeting time stored as JSONB in the courses table.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DbMeetingTime {
    /// Time range for the meeting; `None` means TBA.
    pub time_range: Option<TimeRange>,
    /// Date range over which the meeting recurs.
    pub date_range: DateRange,
    /// Active days of the week. Empty means days are TBA.
    pub days: BTreeSet<DayOfWeek>,
    /// Physical location; `None` when all location fields are absent.
    pub location: Option<MeetingLocation>,
    pub meeting_type: String,
    pub meeting_schedule_type: String,
}

impl DbMeetingTime {
    /// Whether no days of the week are set (i.e. days are TBA).
    #[allow(dead_code)]
    pub fn is_days_tba(&self) -> bool {
        self.days.is_empty()
    }

    /// Whether no time range is set (i.e. time is TBA).
    #[allow(dead_code)]
    pub fn is_time_tba(&self) -> bool {
        self.time_range.is_none()
    }
}

/// Normalize a date string to ISO-8601 (YYYY-MM-DD).
///
/// Accepts MM/DD/YYYY (from Banner API) and returns YYYY-MM-DD.
/// Already-normalized dates are returned as-is.
#[allow(dead_code)]
fn normalize_date(s: &str) -> String {
    if let Some((month_day, year)) = s.rsplit_once('/')
        && let Some((month, day)) = month_day.split_once('/')
    {
        return format!("{year}-{month:0>2}-{day:0>2}");
    }
    s.to_string()
}

/// Parse a date string that may be in MM/DD/YYYY or YYYY-MM-DD format.
fn parse_flexible_date(s: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(s, "%m/%d/%Y")
        .or_else(|_| NaiveDate::parse_from_str(s, "%Y-%m-%d"))
        .ok()
}

impl<'de> Deserialize<'de> for DbMeetingTime {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        /// Intermediate representation that accepts both old and new JSON formats.
        #[derive(Deserialize)]
        struct Raw {
            // New-format fields (camelCase in JSON)
            #[serde(rename = "timeRange")]
            time_range: Option<TimeRange>,
            #[serde(rename = "dateRange")]
            date_range: Option<DateRange>,
            days: Option<BTreeSet<DayOfWeek>>,
            location: Option<MeetingLocation>,

            // Old-format fields (snake_case in JSON)
            begin_time: Option<String>,
            end_time: Option<String>,
            start_date: Option<String>,
            end_date: Option<String>,
            #[serde(default)]
            monday: bool,
            #[serde(default)]
            tuesday: bool,
            #[serde(default)]
            wednesday: bool,
            #[serde(default)]
            thursday: bool,
            #[serde(default)]
            friday: bool,
            #[serde(default)]
            saturday: bool,
            #[serde(default)]
            sunday: bool,
            building: Option<String>,
            building_description: Option<String>,
            room: Option<String>,
            campus: Option<String>,

            // Always present (camelCase in new format, snake_case in old format)
            #[serde(rename = "meetingType", alias = "meeting_type")]
            meeting_type: String,
            #[serde(rename = "meetingScheduleType", alias = "meeting_schedule_type")]
            meeting_schedule_type: String,

            // Legacy computed fields (ignored on read)
            #[serde(default)]
            #[allow(dead_code)]
            is_days_tba: bool,
            #[serde(default)]
            #[allow(dead_code)]
            is_time_tba: bool,
            #[serde(default)]
            #[allow(dead_code)]
            active_days: Vec<DayOfWeek>,
        }

        let raw = Raw::deserialize(deserializer)?;

        // Resolve time_range: prefer new field, fall back to old begin_time/end_time
        let time_range =
            raw.time_range.or_else(
                || match (raw.begin_time.as_deref(), raw.end_time.as_deref()) {
                    (Some(begin), Some(end)) => {
                        let result = TimeRange::from_hhmm(begin, end);
                        if result.is_none() {
                            tracing::warn!(begin, end, "failed to parse old-format time range");
                        }
                        result
                    }
                    _ => None,
                },
            );

        // Resolve date_range: prefer new field, fall back to old start_date/end_date
        let date_range = if let Some(dr) = raw.date_range {
            dr
        } else {
            let start_str = raw.start_date.as_deref().unwrap_or("");
            let end_str = raw.end_date.as_deref().unwrap_or("");
            let start = parse_flexible_date(start_str);
            let end = parse_flexible_date(end_str);
            match (start, end) {
                (Some(s), Some(e)) => DateRange { start: s, end: e },
                _ => {
                    tracing::warn!(
                        start_date = start_str,
                        end_date = end_str,
                        "failed to parse old-format date range, using epoch fallback"
                    );
                    let epoch = NaiveDate::from_ymd_opt(1970, 1, 1).unwrap();
                    DateRange {
                        start: epoch,
                        end: epoch,
                    }
                }
            }
        };

        // Resolve days: prefer new field, fall back to old boolean flags
        let days = raw.days.unwrap_or_else(|| {
            let mut set = BTreeSet::new();
            if raw.monday {
                set.insert(DayOfWeek::Monday);
            }
            if raw.tuesday {
                set.insert(DayOfWeek::Tuesday);
            }
            if raw.wednesday {
                set.insert(DayOfWeek::Wednesday);
            }
            if raw.thursday {
                set.insert(DayOfWeek::Thursday);
            }
            if raw.friday {
                set.insert(DayOfWeek::Friday);
            }
            if raw.saturday {
                set.insert(DayOfWeek::Saturday);
            }
            if raw.sunday {
                set.insert(DayOfWeek::Sunday);
            }
            set
        });

        // Resolve location: prefer new field, fall back to old building/room/campus fields
        let location = raw.location.or_else(|| {
            let loc = MeetingLocation {
                building: raw.building,
                building_description: raw.building_description,
                room: raw.room,
                campus: raw.campus,
            };
            // Only produce Some if at least one field is present
            if loc.building.is_some()
                || loc.building_description.is_some()
                || loc.room.is_some()
                || loc.campus.is_some()
            {
                Some(loc)
            } else {
                None
            }
        });

        Ok(DbMeetingTime {
            time_range,
            date_range,
            days,
            location,
            meeting_type: raw.meeting_type,
            meeting_schedule_type: raw.meeting_schedule_type,
        })
    }
}

#[allow(dead_code)]
#[derive(sqlx::FromRow, Debug, Clone)]
pub struct Course {
    pub id: i32,
    pub crn: String,
    pub subject: String,
    pub course_number: String,
    pub title: String,
    pub term_code: String,
    pub enrollment: i32,
    pub max_enrollment: i32,
    pub wait_count: i32,
    pub wait_capacity: i32,
    pub last_scraped_at: DateTime<Utc>,
    // New scalar fields
    pub sequence_number: Option<String>,
    pub part_of_term: Option<String>,
    pub instructional_method: Option<String>,
    pub campus: Option<String>,
    pub credit_hours: Option<f64>,
    pub credit_hour_low: Option<f64>,
    pub credit_hour_high: Option<f64>,
    pub cross_list: Option<String>,
    pub cross_list_capacity: Option<i32>,
    pub cross_list_count: Option<i32>,
    pub link_identifier: Option<String>,
    pub is_section_linked: Option<bool>,
    // JSONB fields
    pub meeting_times: Value,
    pub attributes: Value,
}

#[allow(dead_code)]
#[derive(sqlx::FromRow, Debug, Clone)]
pub struct Instructor {
    pub id: i32,
    pub display_name: String,
    pub email: Option<String>,
    pub rmp_match_status: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
}

#[allow(dead_code)]
#[derive(sqlx::FromRow, Debug, Clone)]
pub struct CourseInstructor {
    pub course_id: i32,
    pub instructor_id: i32,
    pub banner_id: String,
    pub is_primary: bool,
}

/// Joined instructor data for a course (from course_instructors + instructors + rmp_professors).
#[derive(sqlx::FromRow, Debug, Clone)]
pub struct CourseInstructorDetail {
    pub instructor_id: i32,
    pub banner_id: String,
    pub display_name: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub email: Option<String>,
    pub is_primary: bool,
    pub avg_rating: Option<f32>,
    pub num_ratings: Option<i32>,
    pub rmp_legacy_id: Option<i32>,
    pub slug: Option<String>,
    /// Present when fetched via batch query; `None` for single-course queries.
    pub course_id: Option<i32>,
}

#[allow(dead_code)]
#[derive(sqlx::FromRow, Debug, Clone)]
pub struct ReferenceData {
    pub category: String,
    pub code: String,
    pub description: String,
}

#[allow(dead_code)]
#[derive(sqlx::FromRow, Debug, Clone)]
pub struct CourseMetric {
    pub id: i32,
    pub course_id: i32,
    pub timestamp: DateTime<Utc>,
    pub enrollment: i32,
    pub wait_count: i32,
    pub seats_available: i32,
}

#[allow(dead_code)]
#[derive(sqlx::FromRow, Debug, Clone)]
pub struct CourseAudit {
    pub id: i32,
    pub course_id: i32,
    pub timestamp: DateTime<Utc>,
    pub field_changed: String,
    pub old_value: Option<serde_json::Value>,
    pub new_value: serde_json::Value,
}

/// Aggregate counts returned by batch upsert, used for scrape job result logging.
#[derive(Debug, Clone, Default)]
pub struct UpsertCounts {
    pub courses_fetched: i32,
    pub courses_changed: i32,
    pub courses_unchanged: i32,
    pub audits_generated: i32,
    pub metrics_generated: i32,
}

/// The priority level of a scrape job.
#[derive(sqlx::Type, Copy, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[sqlx(type_name = "scrape_priority", rename_all = "PascalCase")]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ScrapePriority {
    Low,
    Medium,
    High,
    Critical,
}

/// The type of target for a scrape job, determining how the payload is interpreted.
#[derive(sqlx::Type, Copy, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[sqlx(type_name = "target_type", rename_all = "PascalCase")]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum TargetType {
    Subject,
    CourseRange,
    CrnList,
    SingleCrn,
}

/// Computed status for a scrape job, derived from existing fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ScrapeJobStatus {
    Processing,
    StaleLock,
    Exhausted,
    Scheduled,
    Pending,
}

/// How long a lock can be held before it is considered stale (mirrors `scrape_jobs::LOCK_EXPIRY`).
const LOCK_EXPIRY_SECS: i64 = 10 * 60;

/// Represents a queryable job from the database.
#[allow(dead_code)]
#[derive(sqlx::FromRow, Debug, Clone)]
pub struct ScrapeJob {
    pub id: i32,
    pub target_type: TargetType,
    pub target_payload: Value,
    pub priority: ScrapePriority,
    pub execute_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub locked_at: Option<DateTime<Utc>>,
    /// Number of retry attempts for this job (non-negative, enforced by CHECK constraint)
    pub retry_count: i32,
    /// Maximum number of retry attempts allowed (non-negative, enforced by CHECK constraint)
    pub max_retries: i32,
    /// When the job last entered the "ready to pick up" state.
    /// Set to NOW() on creation; updated to NOW() on retry.
    pub queued_at: DateTime<Utc>,
}

impl ScrapeJob {
    /// Compute the current status of this job from its fields.
    pub fn status(&self) -> ScrapeJobStatus {
        let now = Utc::now();
        match self.locked_at {
            Some(locked) if (now - locked).num_seconds() < LOCK_EXPIRY_SECS => {
                ScrapeJobStatus::Processing
            }
            Some(_) => ScrapeJobStatus::StaleLock,
            None if self.retry_count >= self.max_retries && self.max_retries > 0 => {
                ScrapeJobStatus::Exhausted
            }
            None if self.execute_at > now => ScrapeJobStatus::Scheduled,
            None => ScrapeJobStatus::Pending,
        }
    }
}

/// A user authenticated via Discord OAuth.
#[derive(sqlx::FromRow, Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct User {
    #[serde(
        serialize_with = "serialize_i64_as_string",
        deserialize_with = "deserialize_i64_from_string"
    )]
    #[ts(type = "string")]
    pub discord_id: i64,
    pub discord_username: String,
    pub discord_avatar_hash: Option<String>,
    pub is_admin: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A server-side session for an authenticated user.
#[allow(dead_code)] // Fields read via sqlx::FromRow; some only used in DB queries
#[derive(sqlx::FromRow, Debug, Clone)]
pub struct UserSession {
    pub id: String,
    pub user_id: i64,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub last_active_at: DateTime<Utc>,
}

/// Per-subject-term aggregated stats from recent scrape results.
///
/// Populated by `ScrapeJobOps::fetch_subject_stats` and converted into
/// `crate::scraper::adaptive::SubjectStats` for interval computation.
#[derive(sqlx::FromRow, Debug, Clone)]
pub struct SubjectResultStats {
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
