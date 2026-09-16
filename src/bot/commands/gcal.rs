//! Google Calendar command implementation.

use crate::banner::{Course, MeetingScheduleInfo};
use crate::bot::{Context, Error, utils};
use chrono::{NaiveDate, Weekday};
use std::collections::HashMap;
use tracing::info;
use url::Url;

/// Generate a link to create a Google Calendar event for a course
#[poise::command(slash_command)]
pub async fn gcal(ctx: Context<'_>, #[description = "Course Reference Number (CRN)"] crn: i32) -> Result<(), Error> {
    let user = ctx.author();
    info!(source = user.name, target = crn, "gcal command invoked");

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

    struct LinkDetail {
        link: String,
        detail: String,
    }

    let response: Vec<LinkDetail> = match meeting_times.len() {
        0 => Err(anyhow::anyhow!("No meeting times found for this course.")),
        1.. => {
            // Sort meeting times by start time of their TimeRange
            let mut sorted_meeting_times = meeting_times.to_vec();
            MeetingScheduleInfo::sort_by_start_time(&mut sorted_meeting_times);

            let links = sorted_meeting_times
                .iter()
                .map(|m| {
                    let link = generate_gcal_url(&course, m)?;
                    let days = m.days_string().unwrap_or_else(|| "TBA".to_string());
                    let detail = match &m.time_range {
                        Some(range) => {
                            format!("{days} {}", range.format_12hr())
                        }
                        None => days,
                    };
                    Ok(LinkDetail { link, detail })
                })
                .collect::<Result<Vec<LinkDetail>, anyhow::Error>>()?;
            Ok(links)
        }
    }?;

    ctx.say(
        response
            .iter()
            .map(|LinkDetail { link, detail }| format!("[Add to Google Calendar](<{link}>) ({detail})"))
            .collect::<Vec<String>>()
            .join("\n"),
    )
    .await?;

    info!(crn = %crn, "gcal command completed");
    Ok(())
}

/// Generate Google Calendar URL for a course
fn generate_gcal_url(course: &Course, meeting_time: &MeetingScheduleInfo) -> Result<String, anyhow::Error> {
    let course_text = course.display_title();

    let dates_text = {
        let (start, end) = meeting_time.datetime_range();
        format!("{}/{}", start.format("%Y%m%dT%H%M%S"), end.format("%Y%m%dT%H%M%S"))
    };

    // Get instructor name
    let instructor_name = course.primary_instructor_name();

    // The event description
    let details_text = format!(
        "CRN: {}\nInstructor: {}\nDays: {}",
        course.course_reference_number,
        instructor_name,
        meeting_time.days_string().unwrap_or_else(|| "TBA".to_string())
    );

    // The event location
    let location_text = meeting_time.place_string();

    // The event recurrence rule
    let recur_text = generate_rrule(meeting_time, meeting_time.date_range.end);

    let mut params = HashMap::new();
    params.insert("action", "TEMPLATE");
    params.insert("text", &course_text);
    params.insert("dates", &dates_text);
    params.insert("details", &details_text);
    params.insert("location", &location_text);
    params.insert("trp", "true");
    params.insert("ctz", "America/Chicago");
    params.insert("recur", &recur_text);

    Ok(Url::parse_with_params("https://calendar.google.com/calendar/render", &params)?.to_string())
}

/// Generate RRULE for recurrence
fn generate_rrule(meeting_time: &MeetingScheduleInfo, end_date: NaiveDate) -> String {
    let days_of_week = meeting_time.days_of_week();
    let by_day = days_of_week
        .iter()
        .map(|day| match day {
            Weekday::Mon => "MO",
            Weekday::Tue => "TU",
            Weekday::Wed => "WE",
            Weekday::Thu => "TH",
            Weekday::Fri => "FR",
            Weekday::Sat => "SA",
            Weekday::Sun => "SU",
        })
        .collect::<Vec<&str>>()
        .join(",");

    // Format end date for RRULE (YYYYMMDD format)
    let until = end_date.format("%Y%m%dT000000Z").to_string();

    format!("RRULE:FREQ=WEEKLY;BYDAY={by_day};UNTIL={until}")
}
