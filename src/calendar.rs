//! Shared calendar generation logic for ICS files and Google Calendar URLs.
//!
//! Used by both the Discord bot commands and the web API endpoints.

use crate::data::models::{DayOfWeek, DbMeetingTime};
use chrono::{Datelike, Duration, NaiveDate, Weekday};

/// Course metadata needed for calendar generation (shared interface between bot and web).
pub struct CalendarCourse {
    pub crn: String,
    pub subject: String,
    pub course_number: String,
    pub title: String,
    pub sequence_number: Option<String>,
    pub primary_instructor: Option<String>,
}

impl CalendarCourse {
    /// Display title like "CS 1083 - Introduction to Computer Science"
    pub fn display_title(&self) -> String {
        format!("{} {} - {}", self.subject, self.course_number, self.title)
    }

    /// Filename-safe identifier: "CS_1083_001"
    pub fn filename_stem(&self) -> String {
        format!(
            "{}_{}{}",
            self.subject.replace(' ', "_"),
            self.course_number,
            self.sequence_number
                .as_deref()
                .map(|s| format!("_{s}"))
                .unwrap_or_default()
        )
    }
}

/// Convert a `DayOfWeek` to a chrono `Weekday`.
fn to_weekday(day: &DayOfWeek) -> Weekday {
    match day {
        DayOfWeek::Monday => Weekday::Mon,
        DayOfWeek::Tuesday => Weekday::Tue,
        DayOfWeek::Wednesday => Weekday::Wed,
        DayOfWeek::Thursday => Weekday::Thu,
        DayOfWeek::Friday => Weekday::Fri,
        DayOfWeek::Saturday => Weekday::Sat,
        DayOfWeek::Sunday => Weekday::Sun,
    }
}

/// Active weekdays for a meeting time.
fn active_weekdays(mt: &DbMeetingTime) -> Vec<Weekday> {
    mt.days.iter().map(to_weekday).collect()
}

/// ICS two-letter day code for RRULE BYDAY.
fn ics_day_code(day: Weekday) -> &'static str {
    match day {
        Weekday::Mon => "MO",
        Weekday::Tue => "TU",
        Weekday::Wed => "WE",
        Weekday::Thu => "TH",
        Weekday::Fri => "FR",
        Weekday::Sat => "SA",
        Weekday::Sun => "SU",
    }
}

/// Location string from a `DbMeetingTime`.
fn location_string(mt: &DbMeetingTime) -> String {
    let building = mt
        .location
        .as_ref()
        .and_then(|loc| loc.building_description.as_deref())
        .or_else(|| mt.location.as_ref().and_then(|loc| loc.building.as_deref()))
        .unwrap_or("");
    let room = mt
        .location
        .as_ref()
        .and_then(|loc| loc.room.as_deref())
        .unwrap_or("");
    let combined = format!("{building} {room}").trim().to_string();
    if combined.is_empty() {
        "Online".to_string()
    } else {
        combined
    }
}

/// Days display string (e.g. "MWF", "TTh").
fn days_display(mt: &DbMeetingTime) -> String {
    let weekdays = active_weekdays(mt);
    if weekdays.is_empty() {
        return "TBA".to_string();
    }
    weekdays
        .iter()
        .map(|d| ics_day_code(*d))
        .collect::<Vec<_>>()
        .join("")
}

/// Escape text for ICS property values.
fn escape_ics(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace(';', "\\;")
        .replace(',', "\\,")
        .replace('\n', "\\n")
        .replace('\r', "")
}

/// Find the nth occurrence of a weekday in a given month/year (1-based).
fn nth_weekday_of_month(year: i32, month: u32, weekday: Weekday, n: u32) -> Option<NaiveDate> {
    let first = NaiveDate::from_ymd_opt(year, month, 1)?;
    let days_ahead = (weekday.num_days_from_monday() as i64
        - first.weekday().num_days_from_monday() as i64)
        .rem_euclid(7) as u32;
    let day = 1 + days_ahead + 7 * (n - 1);
    NaiveDate::from_ymd_opt(year, month, day)
}

/// Compute a consecutive range of dates starting from `start` for `count` days.
fn date_range(start: NaiveDate, count: i64) -> Vec<NaiveDate> {
    (0..count)
        .filter_map(|i| start.checked_add_signed(Duration::days(i)))
        .collect()
}

/// Compute university holidays for a given year.
fn compute_holidays_for_year(year: i32) -> Vec<(&'static str, Vec<NaiveDate>)> {
    let mut holidays = Vec::new();

    // Labor Day: 1st Monday of September
    if let Some(d) = nth_weekday_of_month(year, 9, Weekday::Mon, 1) {
        holidays.push(("Labor Day", vec![d]));
    }

    // Fall Break: Mon-Tue of Columbus Day week
    if let Some(mon) = nth_weekday_of_month(year, 10, Weekday::Mon, 2) {
        holidays.push(("Fall Break", date_range(mon, 2)));
    }

    // Day before Thanksgiving
    if let Some(thu) = nth_weekday_of_month(year, 11, Weekday::Thu, 4)
        && let Some(wed) = thu.checked_sub_signed(Duration::days(1))
    {
        holidays.push(("Day Before Thanksgiving", vec![wed]));
    }

    // Thanksgiving: 4th Thursday + Friday
    if let Some(thu) = nth_weekday_of_month(year, 11, Weekday::Thu, 4) {
        holidays.push(("Thanksgiving", date_range(thu, 2)));
    }

    // Winter Holiday: Dec 23-31
    if let Some(start) = NaiveDate::from_ymd_opt(year, 12, 23) {
        holidays.push(("Winter Holiday", date_range(start, 9)));
    }

    // New Year's Day
    if let Some(d) = NaiveDate::from_ymd_opt(year, 1, 1) {
        holidays.push(("New Year's Day", vec![d]));
    }

    // MLK Day: 3rd Monday of January
    if let Some(d) = nth_weekday_of_month(year, 1, Weekday::Mon, 3) {
        holidays.push(("MLK Day", vec![d]));
    }

    // Spring Break: full week starting 2nd Monday of March
    if let Some(mon) = nth_weekday_of_month(year, 3, Weekday::Mon, 2) {
        holidays.push(("Spring Break", date_range(mon, 6)));
    }

    holidays
}

/// Get holiday dates within a date range that fall on specific weekdays.
fn holiday_exceptions(start: NaiveDate, end: NaiveDate, weekdays: &[Weekday]) -> Vec<NaiveDate> {
    let start_year = start.year();
    let end_year = end.year();

    (start_year..=end_year)
        .flat_map(compute_holidays_for_year)
        .flat_map(|(_, dates)| dates)
        .filter(|&date| date >= start && date <= end && weekdays.contains(&date.weekday()))
        .collect()
}

/// Names of excluded holidays (for user-facing messages).
fn excluded_holiday_names(
    start: NaiveDate,
    end: NaiveDate,
    exceptions: &[NaiveDate],
) -> Vec<String> {
    let start_year = start.year();
    let end_year = end.year();
    let all_holidays: Vec<_> = (start_year..=end_year)
        .flat_map(compute_holidays_for_year)
        .collect();

    let mut names = Vec::new();
    for (holiday_name, holiday_dates) in &all_holidays {
        for &exc in exceptions {
            if holiday_dates.contains(&exc) {
                names.push(format!("{} ({})", holiday_name, exc.format("%a, %b %d")));
            }
        }
    }
    names.sort();
    names.dedup();
    names
}

/// Result from ICS generation, including the file content and excluded holiday names.
pub struct IcsResult {
    pub content: String,
    pub filename: String,
    /// Holiday dates excluded via EXDATE rules, for user-facing messages.
    #[allow(dead_code)]
    pub excluded_holidays: Vec<String>,
}

/// Generate an ICS calendar file for a course.
pub fn generate_ics(
    course: &CalendarCourse,
    meeting_times: &[DbMeetingTime],
) -> Result<IcsResult, anyhow::Error> {
    let mut ics = String::new();
    let mut all_excluded = Vec::new();

    // Header
    ics.push_str("BEGIN:VCALENDAR\r\n");
    ics.push_str("VERSION:2.0\r\n");
    ics.push_str("PRODID:-//Banner Bot//Course Calendar//EN\r\n");
    ics.push_str("CALSCALE:GREGORIAN\r\n");
    ics.push_str("METHOD:PUBLISH\r\n");
    ics.push_str(&format!(
        "X-WR-CALNAME:{}\r\n",
        escape_ics(&course.display_title())
    ));

    for (index, mt) in meeting_times.iter().enumerate() {
        let (event, holidays) = generate_ics_event(course, mt, index)?;
        ics.push_str(&event);
        all_excluded.extend(holidays);
    }

    ics.push_str("END:VCALENDAR\r\n");

    Ok(IcsResult {
        content: ics,
        filename: format!("{}.ics", course.filename_stem()),
        excluded_holidays: all_excluded,
    })
}

/// Generate a single VEVENT for one meeting time.
fn generate_ics_event(
    course: &CalendarCourse,
    mt: &DbMeetingTime,
    index: usize,
) -> Result<(String, Vec<String>), anyhow::Error> {
    let start_date = mt.date_range.start;
    let end_date = mt.date_range.end;

    let start_time = mt.time_range.as_ref().map(|tr| tr.start);
    let end_time = mt.time_range.as_ref().map(|tr| tr.end);

    // DTSTART/DTEND: first occurrence with time, or all-day on start_date
    let (dtstart, dtend) = match (start_time, end_time) {
        (Some(st), Some(et)) => {
            let s = start_date.and_time(st).and_utc();
            let e = start_date.and_time(et).and_utc();
            (
                s.format("%Y%m%dT%H%M%SZ").to_string(),
                e.format("%Y%m%dT%H%M%SZ").to_string(),
            )
        }
        _ => {
            let s = start_date.and_hms_opt(0, 0, 0).unwrap().and_utc();
            let e = start_date.and_hms_opt(0, 0, 0).unwrap().and_utc();
            (
                s.format("%Y%m%dT%H%M%SZ").to_string(),
                e.format("%Y%m%dT%H%M%SZ").to_string(),
            )
        }
    };

    let event_title = if index > 0 {
        format!("{} (Meeting {})", course.display_title(), index + 1)
    } else {
        course.display_title()
    };

    let instructor = course.primary_instructor.as_deref().unwrap_or("Staff");

    let description = format!(
        "CRN: {}\\nInstructor: {}\\nDays: {}\\nMeeting Type: {}",
        course.crn,
        instructor,
        days_display(mt),
        mt.meeting_type,
    );

    let location = location_string(mt);

    let uid = format!(
        "{}-{}-{}@banner-bot.local",
        course.crn,
        index,
        start_date
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp()
    );

    let mut event = String::new();
    event.push_str("BEGIN:VEVENT\r\n");
    event.push_str(&format!("UID:{uid}\r\n"));
    event.push_str(&format!("DTSTART:{dtstart}\r\n"));
    event.push_str(&format!("DTEND:{dtend}\r\n"));
    event.push_str(&format!("SUMMARY:{}\r\n", escape_ics(&event_title)));
    event.push_str(&format!("DESCRIPTION:{}\r\n", escape_ics(&description)));
    event.push_str(&format!("LOCATION:{}\r\n", escape_ics(&location)));

    let weekdays = active_weekdays(mt);
    let mut holiday_names = Vec::new();

    if let (false, Some(st)) = (weekdays.is_empty(), start_time) {
        let by_day: Vec<&str> = weekdays.iter().map(|d| ics_day_code(*d)).collect();
        let until = end_date.format("%Y%m%dT000000Z").to_string();

        event.push_str(&format!(
            "RRULE:FREQ=WEEKLY;BYDAY={};UNTIL={}\r\n",
            by_day.join(","),
            until,
        ));

        // Holiday exceptions
        let exceptions = holiday_exceptions(start_date, end_date, &weekdays);
        if !exceptions.is_empty() {
            let start_utc = start_date.and_time(st).and_utc();
            let exdates: Vec<String> = exceptions
                .iter()
                .map(|&d| {
                    d.and_time(start_utc.time())
                        .and_utc()
                        .format("%Y%m%dT%H%M%SZ")
                        .to_string()
                })
                .collect();
            event.push_str(&format!("EXDATE:{}\r\n", exdates.join(",")));
        }

        holiday_names = excluded_holiday_names(start_date, end_date, &exceptions);
    }

    event.push_str("END:VEVENT\r\n");
    Ok((event, holiday_names))
}

/// Generate a Google Calendar "add event" URL for a single meeting time.
pub fn generate_gcal_url(
    course: &CalendarCourse,
    mt: &DbMeetingTime,
) -> Result<String, anyhow::Error> {
    let start_date = mt.date_range.start;
    let end_date = mt.date_range.end;

    let start_time = mt.time_range.as_ref().map(|tr| tr.start);
    let end_time = mt.time_range.as_ref().map(|tr| tr.end);

    let dates_text = match (start_time, end_time) {
        (Some(st), Some(et)) => {
            let s = start_date.and_time(st);
            let e = start_date.and_time(et);
            format!(
                "{}/{}",
                s.format("%Y%m%dT%H%M%S"),
                e.format("%Y%m%dT%H%M%S")
            )
        }
        _ => {
            let s = start_date.format("%Y%m%d").to_string();
            format!("{s}/{s}")
        }
    };

    let instructor = course.primary_instructor.as_deref().unwrap_or("Staff");

    let details = format!(
        "CRN: {}\nInstructor: {}\nDays: {}",
        course.crn,
        instructor,
        days_display(mt),
    );

    let location = location_string(mt);

    let weekdays = active_weekdays(mt);
    let recur = if !weekdays.is_empty() && start_time.is_some() {
        let by_day: Vec<&str> = weekdays.iter().map(|d| ics_day_code(*d)).collect();
        let until = end_date.format("%Y%m%dT000000Z").to_string();
        format!(
            "RRULE:FREQ=WEEKLY;BYDAY={};UNTIL={}",
            by_day.join(","),
            until
        )
    } else {
        String::new()
    };

    let course_text = course.display_title();

    let params: Vec<(&str, &str)> = vec![
        ("action", "TEMPLATE"),
        ("text", &course_text),
        ("dates", &dates_text),
        ("details", &details),
        ("location", &location),
        ("trp", "true"),
        ("ctz", "America/Chicago"),
        ("recur", &recur),
    ];

    let url = url::Url::parse_with_params("https://calendar.google.com/calendar/render", &params)?;
    Ok(url.to_string())
}
