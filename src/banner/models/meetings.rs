use bitflags::{Flags, bitflags};
use chrono::{DateTime, NaiveDate, NaiveTime, Timelike, Utc, Weekday};
use extension_traits::extension;
use serde::{Deserialize, Deserializer, Serialize};
use std::{cmp::Ordering, str::FromStr};
use ts_rs::TS;

use super::terms::Term;

#[extension(pub trait WeekdayExt)]
impl Weekday {
    /// Short two-letter representation (used for ICS generation)
    fn to_short_string(self) -> &'static str {
        match self {
            Weekday::Mon => "Mo",
            Weekday::Tue => "Tu",
            Weekday::Wed => "We",
            Weekday::Thu => "Th",
            Weekday::Fri => "Fr",
            Weekday::Sat => "Sa",
            Weekday::Sun => "Su",
        }
    }

    /// Full day name
    fn to_full_string(self) -> &'static str {
        match self {
            Weekday::Mon => "Monday",
            Weekday::Tue => "Tuesday",
            Weekday::Wed => "Wednesday",
            Weekday::Thu => "Thursday",
            Weekday::Fri => "Friday",
            Weekday::Sat => "Saturday",
            Weekday::Sun => "Sunday",
        }
    }
}

/// Deserialize a string field into a u32
fn deserialize_string_to_u32<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    s.parse::<u32>().map_err(serde::de::Error::custom)
}

/// Deserialize a string field into a Term
fn deserialize_string_to_term<'de, D>(deserializer: D) -> Result<Term, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    Term::from_str(&s).map_err(serde::de::Error::custom)
}

/// Represents a faculty member associated with a course
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FacultyItem {
    pub banner_id: String,        // e.g "@01647907" (can contain @ symbol)
    pub category: Option<String>, // zero-padded digits
    pub class: String,            // internal class name
    #[serde(deserialize_with = "deserialize_string_to_u32")]
    pub course_reference_number: u32, // CRN, e.g 27294
    pub display_name: String,     // "LastName, FirstName"
    pub email_address: Option<String>, // e.g. FirstName.LastName@utsaedu
    pub primary_indicator: bool,
    pub term: String, // e.g "202420"
}

/// Meeting time information for a course
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MeetingTime {
    pub start_date: String,         // MM/DD/YYYY, e.g 08/26/2025
    pub end_date: String,           // MM/DD/YYYY, e.g 08/26/2025
    pub begin_time: Option<String>, // HHMM, e.g 1000
    pub end_time: Option<String>,   // HHMM, e.g 1100
    pub category: String,           // unknown meaning, e.g. 01, 02, etc
    pub class: String, // internal class name, e.g. net.hedtech.banner.general.overallMeetingTimeDecorator
    pub monday: bool,  // true if the meeting time occurs on Monday
    pub tuesday: bool, // true if the meeting time occurs on Tuesday
    pub wednesday: bool, // true if the meeting time occurs on Wednesday
    pub thursday: bool, // true if the meeting time occurs on Thursday
    pub friday: bool,  // true if the meeting time occurs on Friday
    pub saturday: bool, // true if the meeting time occurs on Saturday
    pub sunday: bool,  // true if the meeting time occurs on Sunday
    pub room: Option<String>, // e.g. 1.238
    #[serde(deserialize_with = "deserialize_string_to_term")]
    pub term: Term, // e.g 202510
    pub building: Option<String>, // e.g NPB
    pub building_description: Option<String>, // e.g North Paseo Building
    pub campus: Option<String>, // campus code, e.g 11
    pub campus_description: Option<String>, // name of campus, e.g Main Campus
    pub course_reference_number: String, // CRN, e.g 27294
    pub credit_hour_session: Option<f64>, // e.g. 30
    pub hours_week: Option<f64>, // e.g. 30
    pub meeting_schedule_type: String, // e.g AFF
    pub meeting_type: Option<String>, // e.g HB, H2, H1, OS, OA, OH, ID, FF; null when not set by Banner
    pub meeting_type_description: Option<String>,
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub struct MeetingDays: u8 {
        const Monday = 1 << 0;
        const Tuesday = 1 << 1;
        const Wednesday = 1 << 2;
        const Thursday = 1 << 3;
        const Friday = 1 << 4;
        const Saturday = 1 << 5;
        const Sunday = 1 << 6;
    }
}

impl MeetingDays {
    /// Convert from the boolean flags in the raw API response
    pub fn from_meeting_time(meeting_time: &MeetingTime) -> MeetingDays {
        let mut days = MeetingDays::empty();

        if meeting_time.monday {
            days.insert(MeetingDays::Monday);
        }
        if meeting_time.tuesday {
            days.insert(MeetingDays::Tuesday);
        }
        if meeting_time.wednesday {
            days.insert(MeetingDays::Wednesday);
        }
        if meeting_time.thursday {
            days.insert(MeetingDays::Thursday);
        }
        if meeting_time.friday {
            days.insert(MeetingDays::Friday);
        }
        if meeting_time.saturday {
            days.insert(MeetingDays::Saturday);
        }
        if meeting_time.sunday {
            days.insert(MeetingDays::Sunday);
        }

        days
    }
}

impl Ord for MeetingDays {
    fn cmp(&self, other: &Self) -> Ordering {
        self.bits().cmp(&other.bits())
    }
}

impl PartialOrd for MeetingDays {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl From<Weekday> for MeetingDays {
    fn from(day: Weekday) -> Self {
        match day {
            Weekday::Mon => MeetingDays::Monday,
            Weekday::Tue => MeetingDays::Tuesday,
            Weekday::Wed => MeetingDays::Wednesday,
            Weekday::Thu => MeetingDays::Thursday,
            Weekday::Fri => MeetingDays::Friday,
            Weekday::Sat => MeetingDays::Saturday,
            Weekday::Sun => MeetingDays::Sunday,
        }
    }
}

impl TryFrom<MeetingDays> for Weekday {
    type Error = anyhow::Error;

    fn try_from(days: MeetingDays) -> Result<Self, Self::Error> {
        if days.contains_unknown_bits() {
            return Err(anyhow::anyhow!("Unknown days: {:?}", days));
        }

        let count = days.into_iter().count();
        if count == 1 {
            return Ok(match days {
                MeetingDays::Monday => Weekday::Mon,
                MeetingDays::Tuesday => Weekday::Tue,
                MeetingDays::Wednesday => Weekday::Wed,
                MeetingDays::Thursday => Weekday::Thu,
                MeetingDays::Friday => Weekday::Fri,
                MeetingDays::Saturday => Weekday::Sat,
                MeetingDays::Sunday => Weekday::Sun,
                _ => unreachable!(),
            });
        }

        Err(anyhow::anyhow!(
            "Cannot convert multiple days to a single day: {days:?}"
        ))
    }
}

/// Time range for meetings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[ts(export)]
pub struct TimeRange {
    pub start: NaiveTime,
    pub end: NaiveTime,
}

impl TimeRange {
    /// Parse time range from HHMM format strings
    pub fn from_hhmm(start: &str, end: &str) -> Option<Self> {
        let start_time = Self::parse_hhmm(start)?;
        let end_time = Self::parse_hhmm(end)?;

        Some(TimeRange {
            start: start_time,
            end: end_time,
        })
    }

    /// Parse HHMM format string to NaiveTime
    fn parse_hhmm(time_str: &str) -> Option<NaiveTime> {
        if time_str.len() != 4 {
            return None;
        }

        let hours = time_str[..2].parse::<u32>().ok()?;
        let minutes = time_str[2..].parse::<u32>().ok()?;

        if hours > 23 || minutes > 59 {
            return None;
        }

        NaiveTime::from_hms_opt(hours, minutes, 0)
    }

    /// Format time in 12-hour format
    pub fn format_12hr(&self) -> String {
        format!(
            "{}-{}",
            Self::format_time_12hr(self.start),
            Self::format_time_12hr(self.end)
        )
    }

    /// Format a single time in 12-hour format
    fn format_time_12hr(time: NaiveTime) -> String {
        let hour = time.hour();
        let minute = time.minute();

        let meridiem = if hour < 12 { "AM" } else { "PM" };
        let display_hour = match hour {
            0 => 12,
            13..=23 => hour - 12,
            _ => hour,
        };
        format!("{display_hour}:{minute:02}{meridiem}")
    }

    /// Get duration in minutes
    #[allow(dead_code)]
    pub fn duration_minutes(&self) -> i64 {
        let start_minutes = self.start.hour() as i64 * 60 + self.start.minute() as i64;
        let end_minutes = self.end.hour() as i64 * 60 + self.end.minute() as i64;
        end_minutes - start_minutes
    }
}

impl PartialOrd for TimeRange {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.start.cmp(&other.start))
    }
}

/// Date range for meetings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DateRange {
    pub start: NaiveDate,
    pub end: NaiveDate,
}

impl DateRange {
    /// Parse date range from MM/DD/YYYY format strings
    pub fn from_mm_dd_yyyy(start: &str, end: &str) -> Option<Self> {
        let start_date = Self::parse_mm_dd_yyyy(start)?;
        let end_date = Self::parse_mm_dd_yyyy(end)?;

        Some(DateRange {
            start: start_date,
            end: end_date,
        })
    }

    /// Parse MM/DD/YYYY format string to NaiveDate
    fn parse_mm_dd_yyyy(date_str: &str) -> Option<NaiveDate> {
        NaiveDate::parse_from_str(date_str, "%m/%d/%Y").ok()
    }

    /// Get the number of weeks between start and end dates
    pub fn weeks_duration(&self) -> u32 {
        let duration = self.end.signed_duration_since(self.start);
        duration.num_weeks().max(0) as u32
    }

    /// Check if a specific date falls within this range
    #[allow(dead_code)]
    pub fn contains_date(&self, date: NaiveDate) -> bool {
        date >= self.start && date <= self.end
    }
}

/// Meeting schedule type enum
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MeetingType {
    HybridBlended,      // HB, H2, H1
    OnlineSynchronous,  // OS
    OnlineAsynchronous, // OA
    OnlineHybrid,       // OH
    IndependentStudy,   // ID
    FaceToFace,         // FF
    Unknown(String),
}

impl std::str::FromStr for MeetingType {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(match s {
            "HB" | "H2" | "H1" => MeetingType::HybridBlended,
            "OS" => MeetingType::OnlineSynchronous,
            "OA" => MeetingType::OnlineAsynchronous,
            "OH" => MeetingType::OnlineHybrid,
            "ID" => MeetingType::IndependentStudy,
            "FF" => MeetingType::FaceToFace,
            other => MeetingType::Unknown(other.to_string()),
        })
    }
}

impl MeetingType {
    /// Get description for the meeting type
    pub fn description(&self) -> &'static str {
        match self {
            MeetingType::HybridBlended => "Hybrid",
            MeetingType::OnlineSynchronous => "Online Only",
            MeetingType::OnlineAsynchronous => "Online Asynchronous",
            MeetingType::OnlineHybrid => "Online Partial",
            MeetingType::IndependentStudy => "To Be Arranged",
            MeetingType::FaceToFace => "Face to Face",
            MeetingType::Unknown(_) => "Unknown",
        }
    }
}

/// Meeting location information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MeetingLocation {
    Online,
    InPerson {
        campus: String,
        campus_description: String,
        building: String,
        building_description: String,
        room: String,
    },
}

impl MeetingLocation {
    /// Create from raw MeetingTime data
    pub fn from_meeting_time(meeting_time: &MeetingTime) -> Self {
        if let (
            Some(campus),
            Some(campus_description),
            Some(building),
            Some(building_description),
            Some(room),
        ) = (
            &meeting_time.campus,
            &meeting_time.campus_description,
            &meeting_time.building,
            &meeting_time.building_description,
            &meeting_time.room,
        ) {
            if campus_description == "Internet" {
                return MeetingLocation::Online;
            }

            MeetingLocation::InPerson {
                campus: campus.clone(),
                campus_description: campus_description.clone(),
                building: building.clone(),
                building_description: building_description.clone(),
                room: room.clone(),
            }
        } else {
            MeetingLocation::Online
        }
    }
}

/// Clean, parsed meeting schedule information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeetingScheduleInfo {
    pub days: MeetingDays,
    pub time_range: Option<TimeRange>,
    pub date_range: DateRange,
    pub meeting_type: MeetingType,
    pub location: MeetingLocation,
    pub duration_weeks: u32,
}

impl MeetingScheduleInfo {
    /// Create from raw MeetingTime data
    pub fn from_meeting_time(meeting_time: &MeetingTime) -> Self {
        let days = MeetingDays::from_meeting_time(meeting_time);
        let time_range = match (&meeting_time.begin_time, &meeting_time.end_time) {
            (Some(begin), Some(end)) => TimeRange::from_hhmm(begin, end),
            _ => None,
        };

        let date_range =
            DateRange::from_mm_dd_yyyy(&meeting_time.start_date, &meeting_time.end_date)
                .unwrap_or_else(|| {
                    // Fallback to current date if parsing fails
                    let now = chrono::Utc::now().naive_utc().date();
                    DateRange {
                        start: now,
                        end: now,
                    }
                });
        let meeting_type: MeetingType = meeting_time
            .meeting_type
            .as_deref()
            .unwrap_or("")
            .parse()
            .unwrap_or(MeetingType::Unknown(String::new()));
        let location = MeetingLocation::from_meeting_time(meeting_time);
        let duration_weeks = date_range.weeks_duration();

        MeetingScheduleInfo {
            days,
            time_range,
            date_range,
            meeting_type,
            location,
            duration_weeks,
        }
    }

    /// Convert the meeting days bitset to a weekday vector
    pub fn days_of_week(&self) -> Vec<Weekday> {
        self.days
            .iter()
            .filter_map(|day| <MeetingDays as TryInto<Weekday>>::try_into(day).ok())
            .collect()
    }

    /// Get formatted days string
    pub fn days_string(&self) -> Option<String> {
        if self.days.is_empty() {
            return None;
        }
        if self.days.is_all() {
            return Some("Everyday".to_string());
        }

        let days_of_week = self.days_of_week();
        if days_of_week.len() == 1 {
            return Some(days_of_week[0].to_full_string().to_string());
        }

        // Mapper function to get the short string representation of the day of week
        let mapper = {
            let ambiguous = self.days.intersects(
                MeetingDays::Tuesday
                    | MeetingDays::Thursday
                    | MeetingDays::Saturday
                    | MeetingDays::Sunday,
            );

            if ambiguous {
                |day: &Weekday| day.to_short_string().to_string()
            } else {
                |day: &Weekday| day.to_short_string().chars().next().unwrap().to_string()
            }
        };

        Some(days_of_week.iter().map(mapper).collect::<String>())
    }

    /// Returns a formatted string representing the location of the meeting
    pub fn place_string(&self) -> String {
        match &self.location {
            MeetingLocation::Online => "Online".to_string(),
            MeetingLocation::InPerson {
                campus,
                building,
                building_description,
                room,
                ..
            } => format!(
                "{} | {} | {} {}",
                campus, building_description, building, room
            ),
        }
    }

    /// Sort a slice of meeting schedule infos by start time, with stable fallback to day bits.
    ///
    /// Meetings with a time range sort before those without one.
    /// Among meetings without a time range, ties break by day-of-week bits.
    pub fn sort_by_start_time(meetings: &mut [MeetingScheduleInfo]) {
        meetings.sort_unstable_by(|a, b| match (&a.time_range, &b.time_range) {
            (Some(a_time), Some(b_time)) => a_time.start.cmp(&b_time.start),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.days.bits().cmp(&b.days.bits()),
        });
    }

    /// Get the start and end date times for the meeting
    ///
    /// Uses the start and end times of the meeting if available, otherwise defaults to midnight (00:00:00.000).
    ///
    /// The returned times are in UTC.
    pub fn datetime_range(&self) -> (DateTime<Utc>, DateTime<Utc>) {
        let (start, end) = if let Some(time_range) = &self.time_range {
            let start = self.date_range.start.and_time(time_range.start);
            let end = self.date_range.end.and_time(time_range.end);
            (start, end)
        } else {
            (
                self.date_range.start.and_hms_opt(0, 0, 0).unwrap(),
                self.date_range.end.and_hms_opt(0, 0, 0).unwrap(),
            )
        };

        (start.and_utc(), end.and_utc())
    }
}

impl PartialEq for MeetingScheduleInfo {
    fn eq(&self, other: &Self) -> bool {
        self.days == other.days && self.time_range == other.time_range
    }
}

impl PartialOrd for MeetingScheduleInfo {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (&self.time_range, &other.time_range) {
            (Some(self_time), Some(other_time)) => self_time.partial_cmp(other_time),
            (None, None) => Some(self.days.partial_cmp(&other.days).unwrap()),
            (Some(_), None) => Some(Ordering::Less),
            (None, Some(_)) => Some(Ordering::Greater),
        }
    }
}

/// API response wrapper for meeting times
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeetingTimesApiResponse {
    pub fmt: Vec<MeetingTimeResponse>,
}

/// Meeting time response wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MeetingTimeResponse {
    pub category: Option<String>,
    pub class: String,
    pub course_reference_number: String,
    #[serde(default)]
    pub faculty: Vec<FacultyItem>,
    pub meeting_time: MeetingTime,
    pub term: String,
}

impl MeetingTimeResponse {
    /// Get parsed meeting schedule information
    pub fn schedule_info(&self) -> MeetingScheduleInfo {
        MeetingScheduleInfo::from_meeting_time(&self.meeting_time)
    }
}
