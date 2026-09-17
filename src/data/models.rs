//! `sqlx` models for the database schema.

use std::collections::BTreeSet;

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sqlx::types::Json;
use strum::{AsRefStr, EnumString, IntoStaticStr, VariantArray};
use ts_rs::TS;

use crate::banner::models::meetings::TimeRange;
use crate::data::course_types::{DateRange, MeetingLocation, RatingSource};
use crate::data::unsigned::Count;

/// Serialize an `i64` as a string to avoid JavaScript precision loss for values exceeding 2^53.
#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde's serialize_with requires the &T signature regardless of T's size"
)]
fn serialize_i64_as_string<S: Serializer>(value: &i64, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&value.to_string())
}

/// Deserialize an `i64` from either a number or a string.
fn deserialize_i64_from_string<'de, D: Deserializer<'de>>(deserializer: D) -> Result<i64, D::Error> {
    use serde::de;

    struct I64OrStringVisitor;

    impl de::Visitor<'_> for I64OrStringVisitor {
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
    /// Whether no time range is set (i.e. time is TBA).
    #[must_use]
    pub const fn is_time_tba(&self) -> bool {
        self.time_range.is_none()
    }
}

/// Parse a date string that may be in MM/DD/YYYY or YYYY-MM-DD format.
fn parse_flexible_date(s: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(s, "%m/%d/%Y")
        .or_else(|_| NaiveDate::parse_from_str(s, "%Y-%m-%d"))
        .ok()
}

/// Intermediate representation that accepts both old and new JSON formats for `DbMeetingTime`.
#[derive(Deserialize)]
struct RawMeetingTime {
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
    #[expect(
        dead_code,
        reason = "accepted for backward compatibility with old-format JSON but never read"
    )]
    is_days_tba: bool,
    #[serde(default)]
    #[expect(
        dead_code,
        reason = "accepted for backward compatibility with old-format JSON but never read"
    )]
    is_time_tba: bool,
    #[serde(default)]
    #[expect(
        dead_code,
        reason = "accepted for backward compatibility with old-format JSON but never read"
    )]
    active_days: Vec<DayOfWeek>,
}

/// Resolve `time_range`, preferring the new field and falling back to old `begin_time`/`end_time`.
fn resolve_time_range(new: Option<TimeRange>, begin: Option<&str>, end: Option<&str>) -> Option<TimeRange> {
    new.or_else(|| match (begin, end) {
        (Some(begin), Some(end)) => {
            let result = TimeRange::from_hhmm(begin, end);
            if result.is_none() {
                tracing::warn!(begin, end, "failed to parse old-format time range");
            }
            result
        }
        _ => None,
    })
}

/// Resolve `date_range`, preferring the new field and falling back to old `start_date`/`end_date`.
fn resolve_date_range(new: Option<DateRange>, start: Option<&str>, end: Option<&str>) -> DateRange {
    if let Some(dr) = new {
        return dr;
    }
    let start_str = start.unwrap_or("");
    let end_str = end.unwrap_or("");
    let start = parse_flexible_date(start_str);
    let end = parse_flexible_date(end_str);
    if let (Some(s), Some(e)) = (start, end) {
        return DateRange { start: s, end: e };
    }
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

/// Resolve `days`, preferring the new field and falling back to old boolean flags
/// in Monday..Sunday order.
fn resolve_days(new: Option<BTreeSet<DayOfWeek>>, flags: [bool; 7]) -> BTreeSet<DayOfWeek> {
    new.unwrap_or_else(|| {
        let [monday, tuesday, wednesday, thursday, friday, saturday, sunday] = flags;
        let mut set = BTreeSet::new();
        if monday {
            set.insert(DayOfWeek::Monday);
        }
        if tuesday {
            set.insert(DayOfWeek::Tuesday);
        }
        if wednesday {
            set.insert(DayOfWeek::Wednesday);
        }
        if thursday {
            set.insert(DayOfWeek::Thursday);
        }
        if friday {
            set.insert(DayOfWeek::Friday);
        }
        if saturday {
            set.insert(DayOfWeek::Saturday);
        }
        if sunday {
            set.insert(DayOfWeek::Sunday);
        }
        set
    })
}

/// Resolve `location`, preferring the new field and falling back to old
/// building/room/campus fields. `None` unless at least one field is present.
fn resolve_location(
    new: Option<MeetingLocation>,
    building: Option<String>,
    building_description: Option<String>,
    room: Option<String>,
    campus: Option<String>,
) -> Option<MeetingLocation> {
    new.or_else(|| {
        let loc = MeetingLocation {
            building,
            building_description,
            room,
            campus,
        };
        if loc.building.is_some() || loc.building_description.is_some() || loc.room.is_some() || loc.campus.is_some() {
            Some(loc)
        } else {
            None
        }
    })
}

impl<'de> Deserialize<'de> for DbMeetingTime {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawMeetingTime::deserialize(deserializer)?;

        let time_range = resolve_time_range(raw.time_range, raw.begin_time.as_deref(), raw.end_time.as_deref());
        let date_range = resolve_date_range(raw.date_range, raw.start_date.as_deref(), raw.end_date.as_deref());
        let days = resolve_days(
            raw.days,
            [
                raw.monday,
                raw.tuesday,
                raw.wednesday,
                raw.thursday,
                raw.friday,
                raw.saturday,
                raw.sunday,
            ],
        );
        let location = resolve_location(
            raw.location,
            raw.building,
            raw.building_description,
            raw.room,
            raw.campus,
        );

        Ok(Self {
            time_range,
            date_range,
            days,
            location,
            meeting_type: raw.meeting_type,
            meeting_schedule_type: raw.meeting_schedule_type,
        })
    }
}

#[derive(sqlx::FromRow, Debug, Clone)]
pub struct Course {
    pub id: i32,
    pub crn: String,
    pub subject: String,
    pub course_number: String,
    pub title: String,
    pub term_code: String,
    pub enrollment: Count,
    pub max_enrollment: Count,
    pub wait_count: Count,
    pub wait_capacity: Count,
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
    pub meeting_times: Json<Vec<DbMeetingTime>>,
    /// Raw Banner attribute codes, mapped to typed `Attribute` values at the API edge.
    pub attributes: Json<Vec<String>>,
}

#[derive(sqlx::FromRow, Debug, Clone)]
pub struct Instructor {
    pub id: i32,
    pub display_name: String,
    pub email: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
}

/// A stored string that names no variant of a closed set.
///
/// Concrete rather than `anyhow` so a `SQLx` decode can carry it, and it names both
/// the column's type and the offending value, which `strum::ParseError` does not.
#[derive(Debug, Clone, thiserror::Error)]
#[error("unknown {kind} value: {value:?}")]
pub struct UnknownVariant {
    pub kind: &'static str,
    pub value: String,
}

impl UnknownVariant {
    #[must_use]
    pub fn new(kind: &'static str, value: &str) -> Self {
        Self {
            kind,
            value: value.to_owned(),
        }
    }
}

/// Give an enum the `SQLx` codec for a closed set stored in a text column.
///
/// The column is `TEXT`/`VARCHAR` rather than a Postgres enum type, so the codec
/// delegates to `&str`. The string mapping itself comes from strum's `AsRefStr`
/// and `EnumString`; only the encode/decode wiring lives here.
macro_rules! text_column_enum {
    ($name:ident) => {
        impl sqlx::Type<sqlx::Postgres> for $name {
            fn type_info() -> sqlx::postgres::PgTypeInfo {
                <str as sqlx::Type<sqlx::Postgres>>::type_info()
            }

            fn compatible(ty: &sqlx::postgres::PgTypeInfo) -> bool {
                <&str as sqlx::Type<sqlx::Postgres>>::compatible(ty)
            }
        }

        impl<'r> sqlx::Decode<'r, sqlx::Postgres> for $name {
            fn decode(value: sqlx::postgres::PgValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
                let text = <&str as sqlx::Decode<sqlx::Postgres>>::decode(value)?;
                text.parse()
                    .map_err(|_| UnknownVariant::new(stringify!($name), text).into())
            }
        }

        impl<'q> sqlx::Encode<'q, sqlx::Postgres> for $name {
            fn encode_by_ref(
                &self,
                buf: &mut sqlx::postgres::PgArgumentBuffer,
            ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
                <&str as sqlx::Encode<sqlx::Postgres>>::encode_by_ref(&self.as_ref(), buf)
            }
        }
    };
}

pub(crate) use text_column_enum;

/// One page of a list endpoint's results.
///
/// Every paginated endpoint returns this shape, so a client can page through any of
/// them with the same code. `page` is 1-based.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub total: Count,
    pub page: i32,
    pub per_page: i32,
}

/// Match status for RMP instructor matching.
///
/// Stored as VARCHAR, so `serde` and `strum` must spell every variant the same way:
/// one drives the API and TypeScript union, the other the column round-trip.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, AsRefStr, EnumString, IntoStaticStr, VariantArray,
)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
#[ts(export)]
pub enum RmpMatchStatus {
    Unmatched,
    Pending,
    Auto,
    Confirmed,
    Rejected,
}

text_column_enum!(RmpMatchStatus);

/// Review state of a single `rmp_match_candidates` row.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, AsRefStr, EnumString, IntoStaticStr, VariantArray,
)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
#[ts(export)]
pub enum RmpCandidateStatus {
    Pending,
    Accepted,
    Rejected,
}

text_column_enum!(RmpCandidateStatus);

/// Review state of an `instructor_bluebook_links` row.
///
/// `Auto` and `Pending` are algorithm-generated; the other two are human decisions.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, AsRefStr, EnumString, IntoStaticStr, VariantArray,
)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
#[ts(export)]
pub enum BluebookLinkStatus {
    Auto,
    Pending,
    Approved,
    Rejected,
}

text_column_enum!(BluebookLinkStatus);

#[derive(sqlx::FromRow, Debug, Clone)]
pub struct CourseInstructor {
    pub course_id: i32,
    pub instructor_id: i32,
    pub banner_id: String,
    pub is_primary: bool,
}

/// Joined instructor data for a course (from `course_instructors` + instructors + `rmp_professors` + `instructor_scores`).
#[derive(Debug, Clone)]
pub struct CourseInstructorDetail {
    pub instructor_id: i32,
    pub banner_id: String,
    pub display_name: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub email: Option<String>,
    pub is_primary: bool,
    pub avg_rating: Option<f64>,
    pub num_ratings: Option<i32>,
    pub rmp_legacy_id: Option<i32>,
    pub bb_avg_instructor_rating: Option<f32>,
    pub bb_total_responses: Option<i64>,
    pub slug: Option<String>,
    pub course_id: i32,
    // Precomputed Bayesian score fields (from instructor_scores)
    pub sc_display_score: Option<f32>,
    pub sc_sort_score: Option<f32>,
    pub sc_ci_lower: Option<f32>,
    pub sc_ci_upper: Option<f32>,
    pub sc_confidence: Option<f32>,
    pub sc_source: Option<RatingSource>,
    pub sc_rmp_count: Option<i32>,
    pub sc_bb_count: Option<i32>,
}

#[derive(Debug, Clone)]
pub struct ReferenceData {
    pub category: String,
    pub code: String,
    pub description: String,
}

#[derive(sqlx::FromRow, Debug, Clone)]
pub struct CourseMetric {
    pub id: i32,
    pub course_id: i32,
    pub timestamp: DateTime<Utc>,
    pub enrollment: Count,
    pub wait_count: Count,
    /// Legitimately negative for overenrolled courses, so stays as i32.
    pub seats_available: i32,
}

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
    pub courses_fetched: Count,
    pub courses_changed: Count,
    pub courses_unchanged: Count,
    pub audits_generated: Count,
    pub metrics_generated: Count,
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

/// Scrape target for [`TargetType::Subject`]: every section of one subject in one term.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SubjectTarget {
    pub subject: String,
    /// Term code (e.g. "202510"). Legacy jobs omit it and fall back to the current term.
    #[serde(default)]
    pub term: Option<String>,
}

/// Scrape target for [`TargetType::CourseRange`]: a numeric course-number span within a subject.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CourseRangeTarget {
    pub subject: String,
    pub low: i32,
    pub high: i32,
    #[serde(default)]
    pub term: Option<String>,
}

/// Scrape target for [`TargetType::CrnList`]: an explicit set of CRNs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CrnListTarget {
    pub crns: Vec<String>,
    #[serde(default)]
    pub term: Option<String>,
}

/// Scrape target for [`TargetType::SingleCrn`]: one section.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SingleCrnTarget {
    pub crn: String,
    #[serde(default)]
    pub term: Option<String>,
}

/// The payload of a scrape job, discriminated by the row's [`TargetType`].
///
/// The stored JSON carries no tag, so variants are ordered most-specific first:
/// deserialization picks the first shape whose required fields are all present.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(untagged)]
#[ts(export)]
pub enum TargetPayload {
    CourseRange(CourseRangeTarget),
    CrnList(CrnListTarget),
    SingleCrn(SingleCrnTarget),
    Subject(SubjectTarget),
}

impl TargetPayload {
    /// Term code the job targets, when the payload carries one.
    #[must_use]
    pub fn term(&self) -> Option<&str> {
        match self {
            Self::CourseRange(t) => t.term.as_deref(),
            Self::CrnList(t) => t.term.as_deref(),
            Self::SingleCrn(t) => t.term.as_deref(),
            Self::Subject(t) => t.term.as_deref(),
        }
    }

    /// Subject code the job targets, when the payload carries one.
    #[must_use]
    pub fn subject(&self) -> Option<&str> {
        match self {
            Self::CourseRange(t) => Some(&t.subject),
            Self::Subject(t) => Some(&t.subject),
            Self::CrnList(_) | Self::SingleCrn(_) => None,
        }
    }
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
#[derive(Debug, Clone)]
pub struct ScrapeJob {
    pub id: i32,
    pub target_type: TargetType,
    pub target_payload: Json<TargetPayload>,
    pub priority: ScrapePriority,
    pub execute_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub locked_at: Option<DateTime<Utc>>,
    /// Number of retry attempts for this job (non-negative, enforced by CHECK constraint)
    pub retry_count: Count,
    /// Maximum number of retry attempts allowed (non-negative, enforced by CHECK constraint)
    pub max_retries: Count,
    /// When the job last entered the "ready to pick up" state.
    /// Set to `NOW()` on creation; updated to `NOW()` on retry.
    pub queued_at: DateTime<Utc>,
}

impl ScrapeJob {
    /// Compute the current status of this job from its fields.
    #[must_use]
    pub fn status(&self) -> ScrapeJobStatus {
        let now = Utc::now();
        match self.locked_at {
            Some(locked) if (now - locked).num_seconds() < LOCK_EXPIRY_SECS => ScrapeJobStatus::Processing,
            Some(_) => ScrapeJobStatus::StaleLock,
            None if self.retry_count >= self.max_retries && self.max_retries.get() > 0 => ScrapeJobStatus::Exhausted,
            None if self.execute_at > now => ScrapeJobStatus::Scheduled,
            None => ScrapeJobStatus::Pending,
        }
    }
}

/// A user authenticated via Discord OAuth.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
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
#[derive(Debug, Clone)]
pub struct UserSession {
    pub id: String,
    pub user_id: i64,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub last_active_at: DateTime<Utc>,
}

/// Row returned by audit-log queries (audit + joined course fields).
#[derive(Debug)]
pub struct AuditRow {
    pub id: i32,
    pub course_id: i32,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub field_changed: String,
    pub old_value: Option<serde_json::Value>,
    pub new_value: serde_json::Value,
    pub subject: Option<String>,
    pub course_number: Option<String>,
    pub crn: Option<String>,
    pub title: Option<String>,
    pub term_code: Option<String>,
}

/// Per-subject-term aggregated stats from recent scrape results.
///
/// Populated by `ScrapeJobOps::fetch_subject_stats` and converted into
/// `crate::scraper::adaptive::SubjectStats` for interval computation.
#[derive(Debug, Clone)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::banner::models::terms::Season;
    use crate::data::course_types::RatingSource;
    use crate::data::instructor_merge::DuplicateTier;
    use crate::data::watches::WatchType;
    use assert2::check;
    use strum::VariantArray;

    /// Every variant's strum spelling, in declaration order.
    fn spellings<T>() -> Vec<&'static str>
    where
        T: VariantArray + Copy + Into<&'static str>,
    {
        T::VARIANTS.iter().map(|v| (*v).into()).collect()
    }

    /// These are the exact strings the columns hold, so a casing change is a data bug.
    #[test]
    fn test_strum_spellings_match_the_stored_column_values() {
        check!(spellings::<RmpMatchStatus>() == ["unmatched", "pending", "auto", "confirmed", "rejected"]);
        check!(spellings::<RmpCandidateStatus>() == ["pending", "accepted", "rejected"]);
        check!(spellings::<BluebookLinkStatus>() == ["auto", "pending", "approved", "rejected"]);
        check!(spellings::<RatingSource>() == ["both", "rmp", "bluebook"]);
        check!(spellings::<WatchType>() == ["seats_available", "waitlist_open", "any_change"]);
        check!(tier_spellings() == ["same_account", "missing_email", "different_account"]);
        check!(season_spellings() == ["Fall", "Spring", "Summer"]);
    }

    /// `Season` and `DuplicateTier` cannot derive `IntoStaticStr` cleanly, so they
    /// go through `AsRef` instead of the shared helper.
    fn tier_spellings() -> Vec<&'static str> {
        DuplicateTier::VARIANTS.iter().map(AsRef::as_ref).collect()
    }

    fn season_spellings() -> Vec<&'static str> {
        Season::VARIANTS.iter().map(AsRef::as_ref).collect()
    }

    /// serde drives the TypeScript union; strum drives the column. They must agree.
    #[test]
    fn test_serde_spelling_matches_strum_spelling() {
        for variant in RmpMatchStatus::VARIANTS {
            check!(serde_json::to_value(variant).unwrap() == variant.as_ref());
        }
        for variant in RmpCandidateStatus::VARIANTS {
            check!(serde_json::to_value(variant).unwrap() == variant.as_ref());
        }
        for variant in BluebookLinkStatus::VARIANTS {
            check!(serde_json::to_value(variant).unwrap() == variant.as_ref());
        }
        for variant in RatingSource::VARIANTS {
            check!(serde_json::to_value(variant).unwrap() == variant.as_ref());
        }
    }

    /// Every list endpoint shares these four keys, so a client can page any of them.
    #[test]
    fn test_page_serializes_to_the_shared_envelope_keys() {
        let page = Page {
            items: vec!["a", "b"],
            total: Count::new(7),
            page: 2,
            per_page: 2,
        };

        check!(
            serde_json::to_value(&page).unwrap()
                == serde_json::json!({
                    "items": ["a", "b"],
                    "total": 7,
                    "page": 2,
                    "perPage": 2,
                })
        );
    }

    #[test]
    fn test_parsing_rejects_a_value_no_variant_names() {
        check!("bogus".parse::<RmpMatchStatus>().is_err());
        check!("auto".parse::<RmpCandidateStatus>().is_err());
        check!("accepted".parse::<BluebookLinkStatus>().is_err());
        check!("any".parse::<WatchType>().is_err());
        // "bb" was dropped as a dead legacy spelling of "bluebook".
        check!("bb".parse::<RatingSource>().is_err());
    }
}
