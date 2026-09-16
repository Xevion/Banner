//! ICS command implementation for generating calendar files.

use crate::banner::{Course, MeetingDays, MeetingScheduleInfo, WeekdayExt};
use crate::bot::{Context, Error, utils};
use chrono::{Datelike, Duration, NaiveDate, Utc, Weekday};
use serenity::all::CreateAttachment;
use tracing::info;

/// Find the nth occurrence of a weekday in a given month/year (1-based).
fn nth_weekday_of_month(year: i32, month: u32, weekday: Weekday, n: u32) -> Option<NaiveDate> {
    let first = NaiveDate::from_ymd_opt(year, month, 1)?;
    let days_ahead =
        (weekday.num_days_from_monday() as i64 - first.weekday().num_days_from_monday() as i64).rem_euclid(7) as u32;
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
///
/// Federal holidays use weekday-of-month rules so they're correct for any year.
/// University-specific breaks (Fall Break, Spring Break, Winter Holiday) are derived
/// from anchoring federal holidays or using UTSA's typical scheduling patterns.
fn compute_holidays_for_year(year: i32) -> Vec<(&'static str, Vec<NaiveDate>)> {
    let mut holidays = Vec::new();

    // Labor Day: 1st Monday of September
    if let Some(d) = nth_weekday_of_month(year, 9, Weekday::Mon, 1) {
        holidays.push(("Labor Day", vec![d]));
    }

    // Fall Break: Mon-Tue of Columbus Day week (2nd Monday of October + Tuesday)
    if let Some(mon) = nth_weekday_of_month(year, 10, Weekday::Mon, 2) {
        holidays.push(("Fall Break", date_range(mon, 2)));
    }

    // Day before Thanksgiving: Wednesday before 4th Thursday of November
    if let Some(thu) = nth_weekday_of_month(year, 11, Weekday::Thu, 4)
        && let Some(wed) = thu.checked_sub_signed(Duration::days(1))
    {
        holidays.push(("Day Before Thanksgiving", vec![wed]));
    }

    // Thanksgiving: 4th Thursday of November + Friday
    if let Some(thu) = nth_weekday_of_month(year, 11, Weekday::Thu, 4) {
        holidays.push(("Thanksgiving", date_range(thu, 2)));
    }

    // Winter Holiday: Dec 23-31
    if let Some(start) = NaiveDate::from_ymd_opt(year, 12, 23) {
        holidays.push(("Winter Holiday", date_range(start, 9)));
    }

    // New Year's Day: January 1
    if let Some(d) = NaiveDate::from_ymd_opt(year, 1, 1) {
        holidays.push(("New Year's Day", vec![d]));
    }

    // MLK Day: 3rd Monday of January
    if let Some(d) = nth_weekday_of_month(year, 1, Weekday::Mon, 3) {
        holidays.push(("MLK Day", vec![d]));
    }

    // Spring Break: full week (Mon-Sat) starting the 2nd or 3rd Monday of March
    // UTSA typically uses the 2nd full week of March
    if let Some(mon) = nth_weekday_of_month(year, 3, Weekday::Mon, 2) {
        holidays.push(("Spring Break", date_range(mon, 6)));
    }

    holidays
}

/// Generate an ICS file for a course
#[poise::command(slash_command, prefix_command)]
pub async fn ics(ctx: Context<'_>, #[description = "Course Reference Number (CRN)"] crn: i32) -> Result<(), Error> {
    ctx.defer().await?;

    let course = utils::get_course_by_crn(&ctx, crn).await?;
    let term = course.term.clone();

    // Get meeting times
    let meeting_times = ctx
        .data()
        .app_state
        .banner_api
        .get_course_meeting_time(&term, &crn.to_string())
        .await?;

    if meeting_times.is_empty() {
        ctx.say("No meeting times found for this course.").await?;
        return Ok(());
    }

    // Sort meeting times by start time
    let mut sorted_meeting_times = meeting_times.to_vec();
    MeetingScheduleInfo::sort_by_start_time(&mut sorted_meeting_times);

    // Generate ICS content
    let (ics_content, excluded_holidays) = generate_ics_content(&course, &term, &sorted_meeting_times)?;

    // Create file attachment
    let filename = format!(
        "{subject}_{number}_{section}.ics",
        subject = course.subject.replace(" ", "_"),
        number = course.course_number,
        section = course.sequence_number,
    );

    let file = CreateAttachment::bytes(ics_content.into_bytes(), filename.clone());

    // Build response content
    let mut response_content = format!(
        "📅 Generated ICS calendar for **{}**\n\n**Meeting Times:**\n{}",
        course.display_title(),
        sorted_meeting_times
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let time_info = match &m.time_range {
                    Some(range) => format!(
                        "{} {}",
                        m.days_string().unwrap_or("TBA".to_string()),
                        range.format_12hr()
                    ),
                    None => m.days_string().unwrap_or("TBA".to_string()),
                };
                format!("{}. {}", i + 1, time_info)
            })
            .collect::<Vec<_>>()
            .join("\n")
    );

    // Add holiday exclusion information
    if !excluded_holidays.is_empty() {
        let count = excluded_holidays.len();
        let count_text = if count == 1 {
            "1 date was".to_string()
        } else {
            format!("{} dates were", count)
        };
        response_content.push_str(&format!("\n\n{} excluded from the ICS file:\n", count_text));
        response_content.push_str(
            &excluded_holidays
                .iter()
                .map(|s| format!("- {}", s))
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }

    ctx.send(poise::CreateReply::default().content(response_content).attachment(file))
        .await?;

    info!(crn = %crn, "ics command completed");
    Ok(())
}

/// Generate ICS content for a course and its meeting times
fn generate_ics_content(
    course: &Course,
    term: &str,
    meeting_times: &[MeetingScheduleInfo],
) -> Result<(String, Vec<String>), anyhow::Error> {
    let mut ics_content = String::new();
    let mut excluded_holidays = Vec::new();

    // ICS header
    ics_content.push_str("BEGIN:VCALENDAR\r\n");
    ics_content.push_str("VERSION:2.0\r\n");
    ics_content.push_str("PRODID:-//Banner Bot//Course Calendar//EN\r\n");
    ics_content.push_str("CALSCALE:GREGORIAN\r\n");
    ics_content.push_str("METHOD:PUBLISH\r\n");

    // Calendar name
    ics_content.push_str(&format!("X-WR-CALNAME:{} - {}\r\n", course.display_title(), term));

    // Generate events for each meeting time
    for (index, meeting_time) in meeting_times.iter().enumerate() {
        let (event_content, holidays) = generate_event_content(course, meeting_time, index)?;
        ics_content.push_str(&event_content);
        excluded_holidays.extend(holidays);
    }

    // ICS footer
    ics_content.push_str("END:VCALENDAR\r\n");

    Ok((ics_content, excluded_holidays))
}

/// Generate ICS event content for a single meeting time
fn generate_event_content(
    course: &Course,
    meeting_time: &MeetingScheduleInfo,
    index: usize,
) -> Result<(String, Vec<String>), anyhow::Error> {
    let course_title = course.display_title();
    let instructor_name = course.primary_instructor_name();
    let location = meeting_time.place_string();

    // Create event title with meeting index if multiple meetings
    let event_title = if index > 0 {
        format!("{} (Meeting {})", course_title, index + 1)
    } else {
        course_title
    };

    // Create event description
    let description = format!(
        "CRN: {}\\nInstructor: {}\\nDays: {}\\nMeeting Type: {}",
        course.course_reference_number,
        instructor_name,
        meeting_time.days_string().unwrap_or("TBA".to_string()),
        meeting_time.meeting_type.description()
    );

    // Get start and end times
    let (start_dt, end_dt) = meeting_time.datetime_range();

    // Format datetimes for ICS (UTC format)
    let start_utc = start_dt.with_timezone(&Utc);
    let end_utc = end_dt.with_timezone(&Utc);

    let start_str = start_utc.format("%Y%m%dT%H%M%SZ").to_string();
    let end_str = end_utc.format("%Y%m%dT%H%M%SZ").to_string();

    // Generate unique ID for the event
    let uid = format!(
        "{}-{}-{}@banner-bot.local",
        course.course_reference_number,
        index,
        start_utc.timestamp()
    );

    let mut event_content = String::new();

    // Event header
    event_content.push_str("BEGIN:VEVENT\r\n");
    event_content.push_str(&format!("UID:{}\r\n", uid));
    event_content.push_str(&format!("DTSTART:{}\r\n", start_str));
    event_content.push_str(&format!("DTEND:{}\r\n", end_str));
    event_content.push_str(&format!("SUMMARY:{}\r\n", escape_ics_text(&event_title)));
    event_content.push_str(&format!("DESCRIPTION:{}\r\n", escape_ics_text(&description)));
    event_content.push_str(&format!("LOCATION:{}\r\n", escape_ics_text(&location)));

    // Add recurrence rule if there are specific days and times
    if !meeting_time.days.is_empty() && meeting_time.time_range.is_some() {
        let days_of_week = meeting_time.days_of_week();
        let by_day: Vec<String> = days_of_week
            .iter()
            .map(|day| day.to_short_string().to_uppercase())
            .collect();

        if !by_day.is_empty() {
            let until_date = meeting_time.date_range.end.format("%Y%m%dT000000Z").to_string();

            event_content.push_str(&format!(
                "RRULE:FREQ=WEEKLY;BYDAY={};UNTIL={}\r\n",
                by_day.join(","),
                until_date
            ));

            // Add holiday exceptions (EXDATE) if the class would meet on holiday dates
            let holiday_exceptions = get_holiday_exceptions(meeting_time);
            if let Some(exdate_property) = generate_exdate_property(&holiday_exceptions, start_utc) {
                event_content.push_str(&format!("{}\r\n", exdate_property));
            }

            // Collect holiday names for reporting
            let start_year = meeting_time.date_range.start.year();
            let end_year = meeting_time.date_range.end.year();
            let all_holidays: Vec<_> = (start_year..=end_year).flat_map(compute_holidays_for_year).collect();

            let mut holiday_names = Vec::new();
            for (holiday_name, holiday_dates) in &all_holidays {
                for &exception_date in &holiday_exceptions {
                    if holiday_dates.contains(&exception_date) {
                        holiday_names.push(format!("{} ({})", holiday_name, exception_date.format("%a, %b %d")));
                    }
                }
            }
            holiday_names.sort();
            holiday_names.dedup();

            event_content.push_str("END:VEVENT\r\n");
            return Ok((event_content, holiday_names));
        }
    }

    // Event footer
    event_content.push_str("END:VEVENT\r\n");

    Ok((event_content, Vec::new()))
}

/// Check if a class meets on a specific date based on its meeting days
fn class_meets_on_date(meeting_time: &MeetingScheduleInfo, date: NaiveDate) -> bool {
    let day: MeetingDays = date.weekday().into();
    meeting_time.days.contains(day)
}

/// Get holiday dates that fall within the course date range and would conflict with class meetings
fn get_holiday_exceptions(meeting_time: &MeetingScheduleInfo) -> Vec<NaiveDate> {
    let start_year = meeting_time.date_range.start.year();
    let end_year = meeting_time.date_range.end.year();

    (start_year..=end_year)
        .flat_map(compute_holidays_for_year)
        .flat_map(|(_, dates)| dates)
        .filter(|&date| {
            date >= meeting_time.date_range.start
                && date <= meeting_time.date_range.end
                && class_meets_on_date(meeting_time, date)
        })
        .collect()
}

/// Generate EXDATE property for holiday exceptions
fn generate_exdate_property(exceptions: &[NaiveDate], start_time: chrono::DateTime<Utc>) -> Option<String> {
    if exceptions.is_empty() {
        return None;
    }

    let mut exdate_values = Vec::new();

    for &exception_date in exceptions {
        // Create a datetime for the exception using the same time as the start time
        let exception_datetime = exception_date.and_time(start_time.time()).and_utc();

        let exdate_str = exception_datetime.format("%Y%m%dT%H%M%SZ").to_string();
        exdate_values.push(exdate_str);
    }

    Some(format!("EXDATE:{}", exdate_values.join(",")))
}

/// Escape text for ICS format
fn escape_ics_text(text: &str) -> String {
    text.replace("\\", "\\\\")
        .replace(";", "\\;")
        .replace(",", "\\,")
        .replace("\n", "\\n")
        .replace("\r", "")
}
