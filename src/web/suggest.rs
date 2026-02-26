//! Suggest and instructor resolution handlers.

use axum::extract::{Query, State};
use axum::response::Response;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use ts_rs::TS;

use crate::data;
use crate::data::courses::{CourseSuggestion, InstructorSuggestion};
use crate::state::AppState;
use crate::web::error::{ApiError, db_error};
use crate::web::routes::{cache, with_cache_control};

fn default_suggest_limit() -> i32 {
    10
}

#[derive(Deserialize, Serialize, TS)]
#[ts(export)]
pub struct SuggestParams {
    pub term: String,
    pub q: String,
    #[serde(default = "default_suggest_limit")]
    pub limit: i32,
}

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SuggestResponse {
    pub courses: Vec<CourseSuggestion>,
    pub instructors: Vec<InstructorSuggestion>,
}

#[derive(Deserialize)]
pub struct SuggestInstructorsParams {
    pub q: String,
    pub term: Option<String>,
    #[serde(default = "default_suggest_limit")]
    pub limit: i32,
}

#[derive(Deserialize)]
pub struct ResolveInstructorsParams {
    #[serde(default)]
    pub slug: Vec<String>,
}

/// `GET /api/suggest?term={slug}&q={query}&limit=10`
pub(super) async fn suggest(
    State(state): State<AppState>,
    Query(params): Query<SuggestParams>,
) -> Result<Response, ApiError> {
    use crate::banner::models::terms::Term;

    let term_code =
        Term::resolve_to_code(&params.term).ok_or_else(|| ApiError::invalid_term(&params.term))?;
    let limit = params.limit.clamp(1, 25);
    let q = params.q.trim();

    if q.chars().count() < 2 {
        return Ok(with_cache_control(
            SuggestResponse {
                courses: vec![],
                instructors: vec![],
            },
            cache::REFERENCE,
        ));
    }

    let (courses, instructors) = tokio::try_join!(
        data::courses::suggest_courses(&state.db_pool, &term_code, q, limit),
        data::courses::suggest_instructors(&state.db_pool, &term_code, q, limit),
    )
    .map_err(|e| db_error("Suggest query", e))?;

    Ok(with_cache_control(
        SuggestResponse {
            courses,
            instructors,
        },
        cache::REFERENCE,
    ))
}

/// `GET /api/instructors/suggest?q={query}&term={slug}&limit=10`
pub(super) async fn suggest_instructors(
    State(state): State<AppState>,
    Query(params): Query<SuggestInstructorsParams>,
) -> Result<Response, ApiError> {
    use crate::banner::models::terms::Term;

    let limit = params.limit.clamp(1, 25);
    let q = params.q.trim();

    if q.chars().count() < 2 {
        return Ok(with_cache_control(
            Vec::<InstructorSuggestion>::new(),
            cache::REFERENCE,
        ));
    }

    let term_code = params
        .term
        .as_deref()
        .map(|t| Term::resolve_to_code(t).ok_or_else(|| ApiError::invalid_term(t)))
        .transpose()?;

    let instructors =
        data::courses::suggest_instructors_global(&state.db_pool, term_code.as_deref(), q, limit)
            .await
            .map_err(|e| db_error("Suggest instructors", e))?;

    Ok(with_cache_control(instructors, cache::REFERENCE))
}

/// `GET /api/instructors/resolve?slug=a&slug=b`
pub(super) async fn resolve_instructors(
    State(state): State<AppState>,
    axum_extra::extract::Query(params): axum_extra::extract::Query<ResolveInstructorsParams>,
) -> Result<Response, ApiError> {
    if params.slug.is_empty() {
        return Ok(with_cache_control(
            HashMap::<String, String>::new(),
            cache::REFERENCE,
        ));
    }

    if params.slug.len() > 50 {
        return Err(ApiError::bad_request("Too many slugs (max 50)"));
    }

    let rows = data::instructors::resolve_instructor_slugs(&state.db_pool, &params.slug)
        .await
        .map_err(|e| db_error("Resolve instructor slugs", e))?;

    let map: HashMap<String, String> = rows.into_iter().collect();
    Ok(with_cache_control(map, cache::REFERENCE))
}
