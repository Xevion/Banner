//! Public instructor directory and profile HTTP handlers.

use axum::extract::{Path, Query, State};
use axum::response::Json;

use crate::data;
use crate::data::instructors::{IdentifierKind, PublicInstructorListParams, classify_identifier};
use crate::state::AppState;
use crate::web::error::{ApiError, db_error};
use crate::web::routes::{CourseResponse, build_course_response};

/// `GET /api/instructors`
pub async fn list_instructors(
    State(state): State<AppState>,
    Query(params): Query<PublicInstructorListParams>,
) -> Result<Json<data::instructors::PublicInstructorListResponse>, ApiError> {
    let result = data::instructors::list_public_instructors(&state.db_pool, &params)
        .await
        .map_err(|e| db_error("List instructors", e))?;
    Ok(Json(result))
}

/// `GET /api/instructors/{slug}`
pub async fn get_instructor(
    State(state): State<AppState>,
    Path(raw): Path<String>,
) -> Result<axum::response::Response, ApiError> {
    use axum::response::{IntoResponse, Redirect};

    let (_, slug) = data::instructors::resolve_instructor_identifier(&state.db_pool, &raw)
        .await
        .map_err(|e| db_error("Resolve instructor", e))?
        .ok_or_else(|| ApiError::not_found("Instructor not found"))?;

    // Non-canonical identifier: redirect to the canonical slug URL
    if !matches!(classify_identifier(&raw), IdentifierKind::Slug) {
        return Ok(Redirect::permanent(&format!("/api/instructors/{slug}")).into_response());
    }

    let profile = data::instructors::get_public_instructor_by_slug(&state.db_pool, &slug)
        .await
        .map_err(|e| db_error("Get instructor", e))?
        .ok_or_else(|| ApiError::not_found("Instructor not found"))?;

    Ok(Json(profile).into_response())
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
            .ok_or_else(|| ApiError::not_found("Instructor not found"))?;

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

    Ok(Json(responses).into_response())
}
