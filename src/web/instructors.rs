//! Public instructor directory and profile HTTP handlers.

use axum::extract::{Path, Query, State};
use axum::response::Json;

use crate::data;
use crate::data::instructors::{IdentifierKind, PublicInstructorListParams, classify_identifier};
use crate::state::AppState;
use crate::web::courses::{CourseResponse, build_course_response};
use crate::web::error::{ApiError, OptionNotFoundExt, db_error};

/// `GET /api/instructors`
pub async fn list_instructors(
    State(state): State<AppState>,
    Query(params): Query<PublicInstructorListParams>,
) -> Result<axum::response::Response, ApiError> {
    use crate::web::routes::{cache, with_cache_control};
    let result = data::instructors::list_public_instructors(&state.db_pool, &params)
        .await
        .map_err(|e| db_error("List instructors", e))?;
    Ok(with_cache_control(result, cache::REFERENCE))
}

/// `GET /api/instructors/{slug}`
pub async fn get_instructor(
    State(state): State<AppState>,
    Path(raw): Path<String>,
    headers: axum::http::HeaderMap,
) -> Result<axum::response::Response, ApiError> {
    use crate::web::routes::cache;
    use axum::http::{HeaderValue, StatusCode, header};
    use axum::response::{IntoResponse, Redirect};

    let (instructor_id, slug) =
        data::instructors::resolve_instructor_identifier(&state.db_pool, &raw)
            .await
            .map_err(|e| db_error("Resolve instructor", e))?
            .or_not_found("Instructor", &raw)?;

    // Non-canonical identifier: redirect to the canonical slug URL
    if !matches!(classify_identifier(&raw), IdentifierKind::Slug) {
        return Ok(Redirect::permanent(&format!("/api/instructors/{slug}")).into_response());
    }

    // Build ETag from instructor score timestamp (most frequently changing data)
    let score_ts: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("SELECT computed_at FROM instructor_scores WHERE instructor_id = $1")
            .bind(instructor_id)
            .fetch_optional(&state.db_pool)
            .await
            .unwrap_or(None);

    let etag = format!("\"i:{}:{}\"", slug, score_ts.map_or(0, |ts| ts.timestamp()));

    // 304 Not Modified if client ETag matches
    if let Some(if_none_match) = headers.get(header::IF_NONE_MATCH)
        && if_none_match.as_bytes() == etag.as_bytes()
    {
        let mut resp = StatusCode::NOT_MODIFIED.into_response();
        resp.headers_mut()
            .insert(header::ETAG, HeaderValue::from_str(&etag).unwrap());
        resp.headers_mut().insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static(cache::DETAIL),
        );
        return Ok(resp);
    }

    let profile = data::instructors::get_public_instructor_by_slug(&state.db_pool, &slug)
        .await
        .map_err(|e| db_error("Get instructor", e))?
        .or_not_found("Instructor", &slug)?;

    let mut resp = Json(profile).into_response();
    resp.headers_mut()
        .insert(header::ETAG, HeaderValue::from_str(&etag).unwrap());
    resp.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(cache::DETAIL),
    );
    Ok(resp)
}

#[derive(serde::Deserialize)]
pub struct InstructorSectionsParams {
    pub term: String,
}

/// `GET /api/instructors/{slug}/sections?term={code}`
pub async fn get_instructor_sections(
    State(state): State<AppState>,
    Path(raw): Path<String>,
    Query(params): Query<InstructorSectionsParams>,
) -> Result<axum::response::Response, ApiError> {
    use crate::banner::models::terms::Term;
    use axum::response::{IntoResponse, Redirect};

    let (instructor_id, slug) =
        data::instructors::resolve_instructor_identifier(&state.db_pool, &raw)
            .await
            .map_err(|e| db_error("Resolve instructor", e))?
            .or_not_found("Instructor", &raw)?;

    // Non-canonical: redirect, preserving the raw ?term= value so the redirect
    // target can still resolve "fall2025"-style aliases.
    if !matches!(classify_identifier(&raw), IdentifierKind::Slug) {
        let uri = format!("/api/instructors/{slug}/sections?term={}", params.term);
        return Ok(Redirect::permanent(&uri).into_response());
    }

    let term_code =
        Term::resolve_to_code(&params.term).ok_or_else(|| ApiError::invalid_term(&params.term))?;

    let courses =
        data::instructors::get_instructor_sections(&state.db_pool, instructor_id, &term_code)
            .await
            .map_err(|e| db_error("Instructor sections", e))?;

    let course_ids: Vec<i32> = courses.iter().map(|c| c.id).collect();
    let mut instructor_map =
        data::courses::get_instructors_for_courses(&state.db_pool, &course_ids)
            .await
            .unwrap_or_else(|e| {
                tracing::error!(error = %e, "Failed to fetch instructors for instructor sections");
                Default::default()
            });

    let responses: Vec<CourseResponse> = courses
        .iter()
        .map(|course| {
            let instructors = instructor_map.remove(&course.id).unwrap_or_default();
            build_course_response(course, instructors)
        })
        .collect();

    Ok(crate::web::routes::with_cache_control(
        responses,
        crate::web::routes::cache::DETAIL,
    ))
}
