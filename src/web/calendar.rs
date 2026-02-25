//! Web API endpoints for calendar export (ICS download + Google Calendar redirect).

use axum::{
    extract::{Path, State},
    http::{StatusCode, header},
    response::{IntoResponse, Redirect, Response},
};
use tracing::{error, instrument};

use crate::calendar::{CalendarCourse, generate_gcal_url, generate_ics};
use crate::data::models::DbMeetingTime;
use crate::state::AppState;

/// Fetch course + meeting times, build a `CalendarCourse`.
async fn load_calendar_course(
    state: &AppState,
    term: &str,
    crn: &str,
) -> Result<(CalendarCourse, Vec<DbMeetingTime>), (StatusCode, String)> {
    let course = crate::data::courses::get_course_by_crn(&state.db_pool, crn, term)
        .await
        .map_err(|e| {
            error!(%e, "Course lookup failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Lookup failed".to_string(),
            )
        })?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Course not found".to_string()))?;

    let instructors = crate::data::courses::get_course_instructors(&state.db_pool, course.id)
        .await
        .unwrap_or_else(|e| {
            error!(course_id = course.id, error = %e, "Failed to fetch instructors for calendar export");
            Vec::new()
        });

    let primary_instructor = instructors
        .iter()
        .find(|i| i.is_primary)
        .or(instructors.first())
        .map(|i| i.display_name.clone());

    let meeting_times: Vec<DbMeetingTime> =
        serde_json::from_value(course.meeting_times.clone()).unwrap_or_default();

    let cal_course = CalendarCourse {
        crn: course.crn.clone(),
        subject: course.subject.clone(),
        course_number: course.course_number.clone(),
        title: course.title.clone(),
        sequence_number: course.sequence_number.clone(),
        primary_instructor,
    };

    Ok((cal_course, meeting_times))
}

/// `GET /api/courses/{term}/{crn}/calendar.ics`
///
/// Returns an ICS file download for the course.
#[instrument(skip_all, fields(crn))]
pub async fn course_ics(
    State(state): State<AppState>,
    Path((term, crn)): Path<(String, String)>,
) -> Result<Response, (StatusCode, String)> {
    let (cal_course, meeting_times) = load_calendar_course(&state, &term, &crn).await?;

    if meeting_times.is_empty() {
        return Err((
            StatusCode::NOT_FOUND,
            "No meeting times found for this course".to_string(),
        ));
    }

    let result = generate_ics(&cal_course, &meeting_times).map_err(|e| {
        error!(%e, "ICS generation failed");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to generate ICS file".to_string(),
        )
    })?;

    let response = (
        [
            (header::CONTENT_TYPE, "text/calendar; charset=utf-8"),
            (
                header::CONTENT_DISPOSITION,
                &format!("attachment; filename=\"{}\"", result.filename),
            ),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        result.content,
    )
        .into_response();

    Ok(response)
}

/// `GET /api/courses/{term}/{crn}/gcal`
///
/// Redirects to Google Calendar with a pre-filled event for the first meeting time.
/// If multiple meeting times exist, uses the first one with scheduled days/times.
#[instrument(skip_all, fields(crn))]
pub async fn course_gcal(
    State(state): State<AppState>,
    Path((term, crn)): Path<(String, String)>,
) -> Result<Response, (StatusCode, String)> {
    let (cal_course, meeting_times) = load_calendar_course(&state, &term, &crn).await?;

    if meeting_times.is_empty() {
        return Err((
            StatusCode::NOT_FOUND,
            "No meeting times found for this course".to_string(),
        ));
    }

    // Prefer the first meeting time that has actual days/times scheduled
    let mt = meeting_times
        .iter()
        .find(|mt| mt.time_range.is_some() && !mt.days.is_empty())
        .unwrap_or(&meeting_times[0]);

    let url = generate_gcal_url(&cal_course, mt).map_err(|e| {
        error!(%e, "Google Calendar URL generation failed");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to generate Google Calendar URL".to_string(),
        )
    })?;

    Ok(Redirect::temporary(&url).into_response())
}
