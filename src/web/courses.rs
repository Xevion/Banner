//! Course search and detail handlers.

use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{
    extract::{Path, State},
    http::HeaderValue,
    response::{Json, Response},
};
use serde::{Deserialize, Serialize};
use tracing::{error, warn};
use ts_rs::TS;

use crate::data::course_types::{CreditHours, CrossList, Enrollment, RmpBrief, SectionLink};
use crate::data::courses::{SortColumn, SortDirection};
use crate::data::reference_types::{
    Attribute, Campus, FilterValue, InstructionalMethod, PartOfTerm,
};
use crate::data::unsigned::Count;
use crate::data::{self, models};
use crate::state::AppState;
use crate::web::error::{ApiError, OptionNotFoundExt, db_error};
use crate::web::routes::{cache, with_cache_control};

fn default_limit() -> i32 {
    25
}

/// Convert a raw Banner code to its typed filter string for a given reference category.
pub fn code_to_filter_value(category: &str, code: &str, description: Option<&str>) -> String {
    match category {
        "instructional_method" => InstructionalMethod::from_code(code)
            .map(|m| m.to_filter_str().to_owned())
            .unwrap_or_else(|_| format!("raw:{code}")),
        "campus" => Campus::from_code(code, description)
            .to_filter_str()
            .into_owned(),
        "attribute" => Attribute::from_code(code, description)
            .to_filter_str()
            .into_owned(),
        "part_of_term" => PartOfTerm::from_code(code, description)
            .to_filter_str()
            .into_owned(),
        _ => format!("raw:{code}"),
    }
}

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CourseResponse {
    crn: String,
    subject: String,
    course_number: String,
    title: String,
    term_slug: String,
    sequence_number: Option<String>,
    instructional_method: Option<InstructionalMethod>,
    /// Raw instructional method code, included when parsing fails (Tier 1 fallback).
    #[serde(skip_serializing_if = "Option::is_none")]
    instructional_method_code: Option<String>,
    campus: Option<Campus>,
    enrollment: Enrollment,
    credit_hours: Option<CreditHours>,
    cross_list: Option<CrossList>,
    section_link: Option<SectionLink>,
    part_of_term: Option<PartOfTerm>,
    meeting_times: Vec<models::DbMeetingTime>,
    attributes: Vec<Attribute>,
    is_async_online: bool,
    /// Best display-ready location: physical room ("MH 2.206"), "Online", or campus fallback.
    primary_location: Option<String>,
    /// Whether a physical (non-INT) building was found in meeting times.
    has_physical_location: bool,
    primary_instructor_id: Option<i32>,
    instructors: Vec<InstructorResponse>,
}

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct InstructorResponse {
    instructor_id: i32,
    banner_id: String,
    display_name: String,
    first_name: Option<String>,
    last_name: Option<String>,
    email: Option<String>,
    slug: Option<String>,
    is_primary: bool,
    rmp: Option<RmpBrief>,
    bluebook: Option<crate::data::course_types::BlueBookBrief>,
    rating: Option<crate::data::course_types::InstructorRating>,
}

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SearchResponse {
    courses: Vec<CourseResponse>,
    total_count: Count,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CodeDescription {
    pub code: String,
    pub description: String,
    /// Typed filter string for query params (e.g. "Online.Async", "Main", "raw:XYZ").
    pub filter_value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TermResponse {
    pub code: String,
    pub slug: String,
    pub description: String,
}

#[derive(Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SearchParams {
    pub term: String,
    #[serde(default)]
    pub subject: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none", alias = "q")]
    pub query: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", alias = "course_number_low")]
    pub course_number_low: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none", alias = "course_number_high")]
    pub course_number_high: Option<i32>,
    #[serde(default, alias = "open_only")]
    pub open_only: bool,
    #[serde(default, alias = "instructional_method")]
    #[ts(type = "Array<string>")]
    pub instructional_method: Vec<FilterValue<InstructionalMethod>>,
    #[serde(default)]
    #[ts(type = "Array<string>")]
    pub campus: Vec<FilterValue<Campus>>,
    #[serde(default = "default_limit")]
    pub limit: i32,
    #[serde(default)]
    pub offset: i32,
    #[serde(skip_serializing_if = "Option::is_none", alias = "sort_by")]
    pub sort_by: Option<SortColumn>,
    #[serde(skip_serializing_if = "Option::is_none", alias = "sort_dir")]
    pub sort_dir: Option<SortDirection>,
    #[serde(skip_serializing_if = "Option::is_none", alias = "wait_count_max")]
    pub wait_count_max: Option<i32>,
    #[serde(default)]
    pub days: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none", alias = "time_start")]
    pub time_start: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", alias = "time_end")]
    pub time_end: Option<String>,
    #[serde(default, alias = "part_of_term")]
    #[ts(type = "Array<string>")]
    pub part_of_term: Vec<FilterValue<PartOfTerm>>,
    #[serde(default)]
    #[ts(type = "Array<string>")]
    pub attributes: Vec<FilterValue<Attribute>>,
    #[serde(skip_serializing_if = "Option::is_none", alias = "credit_hour_min")]
    pub credit_hour_min: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none", alias = "credit_hour_max")]
    pub credit_hour_max: Option<f64>,
    #[serde(default)]
    pub instructor: Vec<String>,
}

/// Build a `CourseResponse` from a DB course with pre-fetched instructor details.
pub fn build_course_response(
    course: &models::Course,
    instructors: Vec<models::CourseInstructorDetail>,
) -> CourseResponse {
    let instructors: Vec<InstructorResponse> = instructors
        .into_iter()
        .map(|i| {
            let rmp = i.rmp_legacy_id.map(|legacy_id| {
                let (avg_rating, num_ratings) = crate::data::course_types::sanitize_rmp_ratings(
                    i.avg_rating.map(|v| v as f32),
                    i.num_ratings,
                );
                RmpBrief {
                    avg_rating,
                    num_ratings,
                    legacy_id,
                }
            });
            let bluebook = crate::data::course_types::build_bluebook_brief(
                i.bb_avg_instructor_rating,
                i.bb_total_responses,
            );
            let rating = match (
                i.sc_display_score,
                i.sc_sort_score,
                i.sc_ci_lower,
                i.sc_ci_upper,
                i.sc_confidence,
                i.sc_source,
            ) {
                (Some(ds), Some(ss), Some(cl), Some(cu), Some(conf), Some(src)) => {
                    Some(crate::data::scoring::build_rating_from_score_row(
                        &crate::data::scoring::ScoreRow {
                            display_score: ds,
                            sort_score: ss,
                            ci_lower: cl,
                            ci_upper: cu,
                            confidence: conf,
                            source: src,
                            rmp_count: i.sc_rmp_count.unwrap_or(0),
                            bb_count: i.sc_bb_count.unwrap_or(0),
                        },
                    ))
                }
                _ => None,
            };
            InstructorResponse {
                instructor_id: i.instructor_id,
                banner_id: i.banner_id,
                display_name: i.display_name,
                first_name: i.first_name,
                last_name: i.last_name,
                email: i.email,
                slug: i.slug,
                is_primary: i.is_primary,
                rmp,
                bluebook,
                rating,
            }
        })
        .collect();

    let primary_instructor_id = instructors
        .iter()
        .find(|i| i.is_primary)
        .or(instructors.first())
        .map(|i| i.instructor_id);

    let meeting_times: Vec<models::DbMeetingTime> =
        serde_json::from_value(course.meeting_times.clone())
            .map_err(|e| {
                error!(
                    course_id = course.id,
                    crn = %course.crn,
                    term = %course.term_code,
                    %e,
                    "Failed to deserialize meeting_times JSONB"
                );
                e
            })
            .unwrap_or_default();

    let attributes: Vec<Attribute> =
        serde_json::from_value::<Vec<String>>(course.attributes.clone())
            .map_err(|e| {
                error!(
                    course_id = course.id,
                    crn = %course.crn,
                    term = %course.term_code,
                    %e,
                    "Failed to deserialize attributes JSONB"
                );
                e
            })
            .unwrap_or_default()
            .into_iter()
            .map(|code| Attribute::from_code(&code, None))
            .collect();

    let (instructional_method, instructional_method_code) = match &course.instructional_method {
        Some(code) => match InstructionalMethod::from_code(code) {
            Ok(method) => (Some(method), None),
            Err(_) => {
                warn!(
                    crn = %course.crn,
                    term = %course.term_code,
                    %code,
                    "Unknown instructional method code"
                );
                (None, Some(code.clone()))
            }
        },
        None => (None, None),
    };

    let campus = course
        .campus
        .as_ref()
        .map(|code| Campus::from_code(code, None));
    let part_of_term = course
        .part_of_term
        .as_ref()
        .map(|code| PartOfTerm::from_code(code, None));

    let is_async_online = meeting_times.first().is_some_and(|mt| {
        mt.location.as_ref().and_then(|loc| loc.building.as_deref()) == Some("INT")
            && mt.is_time_tba()
    });

    let physical_location = meeting_times
        .iter()
        .filter(|mt| mt.location.as_ref().and_then(|loc| loc.building.as_deref()) != Some("INT"))
        .find_map(|mt| {
            mt.location.as_ref().and_then(|loc| {
                loc.building.as_ref().map(|b| match &loc.room {
                    Some(r) => format!("{b} {r}"),
                    None => b.clone(),
                })
            })
        });
    let has_physical_location = physical_location.is_some();

    let primary_location = physical_location.or_else(|| {
        let is_hybrid = instructional_method
            .as_ref()
            .is_some_and(|m| matches!(m, InstructionalMethod::Hybrid(_)));
        let is_online_method = instructional_method
            .as_ref()
            .is_some_and(|m| matches!(m, InstructionalMethod::Online(_)));
        let is_virtual_campus = campus
            .as_ref()
            .is_some_and(|c| matches!(c, Campus::Internet | Campus::OnlinePrograms));
        if is_hybrid {
            Some("Hybrid".to_string())
        } else if is_online_method || is_virtual_campus {
            Some("Online".to_string())
        } else {
            None
        }
    });

    let enrollment = Enrollment {
        current: Count::new(course.enrollment.max(0) as u32),
        max: Count::new(course.max_enrollment.max(0) as u32),
        wait_count: Count::new(course.wait_count.max(0) as u32),
        wait_capacity: Count::new(course.wait_capacity.max(0) as u32),
    };

    let credit_hours = match (
        course.credit_hours,
        course.credit_hour_low,
        course.credit_hour_high,
    ) {
        (Some(fixed), _, _) => Some(CreditHours::Fixed { hours: fixed }),
        (None, Some(low), Some(high)) if low != high => Some(CreditHours::Range { low, high }),
        (None, Some(hours), None) | (None, None, Some(hours)) => Some(CreditHours::Fixed { hours }),
        _ => None,
    };

    let cross_list = course.cross_list.as_ref().and_then(|identifier| {
        course.cross_list_capacity.and_then(|capacity| {
            course.cross_list_count.map(|count| CrossList {
                identifier: identifier.clone(),
                capacity,
                count,
            })
        })
    });

    let section_link = course
        .link_identifier
        .clone()
        .map(|identifier| SectionLink { identifier });

    use crate::banner::models::terms::Term;
    let term_slug = course
        .term_code
        .parse::<Term>()
        .map(|t| t.slug())
        .unwrap_or_else(|_| course.term_code.clone());

    CourseResponse {
        crn: course.crn.clone(),
        subject: course.subject.clone(),
        course_number: course.course_number.clone(),
        title: course.title.clone(),
        term_slug,
        sequence_number: course.sequence_number.clone(),
        instructional_method,
        instructional_method_code,
        campus,
        enrollment,
        credit_hours,
        cross_list,
        section_link,
        part_of_term,
        is_async_online,
        primary_location,
        has_physical_location,
        primary_instructor_id,
        meeting_times,
        attributes,
        instructors,
    }
}

/// `GET /api/courses/search`
pub(super) async fn search_courses(
    State(state): State<AppState>,
    axum_extra::extract::Query(params): axum_extra::extract::Query<SearchParams>,
) -> Result<Response, ApiError> {
    use crate::banner::models::terms::Term;

    let term_code =
        Term::resolve_to_code(&params.term).ok_or_else(|| ApiError::invalid_term(&params.term))?;
    let limit = params.limit.clamp(1, 100);
    let offset = params.offset.max(0);

    // Convert typed filter values to raw Banner codes for SQL
    let method_codes: Vec<String> = params
        .instructional_method
        .iter()
        .map(|fv| fv.to_code().into_owned())
        .collect();
    let campus_codes: Vec<String> = params
        .campus
        .iter()
        .map(|fv| fv.to_code().into_owned())
        .collect();
    let pot_codes: Vec<String> = params
        .part_of_term
        .iter()
        .map(|fv| fv.to_code().into_owned())
        .collect();
    let attr_codes: Vec<String> = params
        .attributes
        .iter()
        .map(|fv| fv.to_code().into_owned())
        .collect();

    let (courses, total_count) = data::courses::search_courses(
        &state.db_pool,
        &term_code,
        if params.subject.is_empty() {
            None
        } else {
            Some(&params.subject)
        },
        params.query.as_deref(),
        params.course_number_low,
        params.course_number_high,
        params.open_only,
        if method_codes.is_empty() {
            None
        } else {
            Some(&method_codes)
        },
        if campus_codes.is_empty() {
            None
        } else {
            Some(&campus_codes)
        },
        params.wait_count_max,
        if params.days.is_empty() {
            None
        } else {
            Some(&params.days)
        },
        params.time_start.as_deref(),
        params.time_end.as_deref(),
        if pot_codes.is_empty() {
            None
        } else {
            Some(&pot_codes)
        },
        if attr_codes.is_empty() {
            None
        } else {
            Some(&attr_codes)
        },
        params.credit_hour_min,
        params.credit_hour_max,
        if params.instructor.is_empty() {
            None
        } else {
            Some(&params.instructor[..])
        },
        limit,
        offset,
        params.sort_by,
        params.sort_dir,
    )
    .await
    .map_err(|e| db_error("Course search", e))?;

    let course_ids: Vec<i32> = courses.iter().map(|c| c.id).collect();
    let mut instructor_map =
        data::courses::get_instructors_for_courses(&state.db_pool, &course_ids)
            .await
            .unwrap_or_else(|e| {
                error!(error = %e, "Failed to fetch instructors for course search");
                Default::default()
            });

    let course_responses: Vec<CourseResponse> = courses
        .iter()
        .map(|course| {
            let instructors = instructor_map.remove(&course.id).unwrap_or_default();
            build_course_response(course, instructors)
        })
        .collect();

    let total_count = Count::try_from(total_count)
        .map_err(|_| ApiError::internal_error("total count overflow"))?;

    Ok(with_cache_control(
        SearchResponse {
            courses: course_responses,
            total_count,
        },
        cache::SEARCH,
    ))
}

/// `GET /api/courses/:term/:crn`
pub(super) async fn get_course(
    State(state): State<AppState>,
    Path((term, crn)): Path<(String, String)>,
    headers: axum::http::HeaderMap,
) -> Result<Response, ApiError> {
    use crate::banner::models::terms::Term;
    let term_code = Term::resolve_to_code(&term).ok_or_else(|| ApiError::invalid_term(&term))?;
    let course = data::courses::get_course_by_crn(&state.db_pool, &crn, &term_code)
        .await
        .map_err(|e| db_error("Course lookup", e))?
        .or_not_found("Course", &crn)?;

    // ETag based on term, CRN, and last scrape timestamp
    let etag = format!(
        "\"c:{}:{}:{}\"",
        term_code,
        crn,
        course.last_scraped_at.timestamp()
    );

    // 304 Not Modified if client ETag matches
    if let Some(if_none_match) = headers.get(axum::http::header::IF_NONE_MATCH)
        && if_none_match.as_bytes() == etag.as_bytes()
    {
        let mut resp = StatusCode::NOT_MODIFIED.into_response();
        resp.headers_mut().insert(
            axum::http::header::ETAG,
            HeaderValue::from_str(&etag).unwrap(),
        );
        resp.headers_mut().insert(
            axum::http::header::CACHE_CONTROL,
            HeaderValue::from_static(cache::DETAIL),
        );
        return Ok(resp);
    }

    let instructors = data::courses::get_course_instructors(&state.db_pool, course.id)
        .await
        .unwrap_or_else(|e| {
            error!(error = %e, course_id = course.id, "Failed to fetch instructors for course");
            Vec::new()
        });

    let mut resp = Json(build_course_response(&course, instructors)).into_response();
    resp.headers_mut().insert(
        axum::http::header::ETAG,
        HeaderValue::from_str(&etag).unwrap(),
    );
    resp.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        HeaderValue::from_static(cache::DETAIL),
    );
    Ok(resp)
}

/// `GET /api/courses/:term/:subject/:course_number/sections`
///
/// Returns all sections of the same course (same term, subject, course number).
pub(super) async fn get_related_sections(
    State(state): State<AppState>,
    Path((term, subject, course_number)): Path<(String, String, String)>,
) -> Result<Response, ApiError> {
    use crate::banner::models::terms::Term;
    let term_code = Term::resolve_to_code(&term).ok_or_else(|| ApiError::invalid_term(&term))?;
    let courses =
        data::courses::get_related_sections(&state.db_pool, &term_code, &subject, &course_number)
            .await
            .map_err(|e| db_error("Related sections lookup", e))?;

    let course_ids: Vec<i32> = courses.iter().map(|c| c.id).collect();
    let mut instructor_map =
        data::courses::get_instructors_for_courses(&state.db_pool, &course_ids)
            .await
            .unwrap_or_else(|e| {
                error!(error = %e, "Failed to fetch instructors for related sections");
                Default::default()
            });

    let responses: Vec<CourseResponse> = courses
        .iter()
        .map(|course| {
            let instructors = instructor_map.remove(&course.id).unwrap_or_default();
            build_course_response(course, instructors)
        })
        .collect();

    Ok(with_cache_control(responses, cache::DETAIL))
}
