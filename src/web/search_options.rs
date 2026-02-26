//! Search options and reference data handlers.

use axum::extract::{Path, Query, State};
use axum::response::Response;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::banner::models::terms::Term;
use crate::data;

use crate::state::AppState;
use crate::web::courses::{CodeDescription, TermResponse, code_to_filter_value};
use crate::web::error::{ApiError, ApiErrorCode, db_error};
use crate::web::routes::{cache, with_cache_control};

/// Response for the consolidated search-options endpoint.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SearchOptionsResponse {
    pub terms: Vec<TermResponse>,
    pub subjects: Vec<CodeDescription>,
    pub reference: SearchOptionsReference,
    pub ranges: data::courses::FilterRanges,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SearchOptionsReference {
    pub instructional_methods: Vec<CodeDescription>,
    pub campuses: Vec<CodeDescription>,
    pub parts_of_term: Vec<CodeDescription>,
    pub attributes: Vec<CodeDescription>,
}

#[derive(Debug, Deserialize)]
pub struct SearchOptionsParams {
    pub term: Option<String>,
}

/// `GET /api/reference/:category`
pub(super) async fn get_reference(
    State(state): State<AppState>,
    Path(category): Path<String>,
) -> Result<Response, ApiError> {
    let cache_guard = state.reference_cache.read().await;
    let entries = cache_guard.entries_for_category(&category);

    if entries.is_empty() {
        drop(cache_guard);
        let rows = data::reference::get_by_category(&category, &state.db_pool)
            .await
            .map_err(|e| db_error(&format!("Reference lookup for {}", category), e))?;

        let rows_mapped: Vec<CodeDescription> = rows
            .into_iter()
            .map(|r| {
                let filter_value = code_to_filter_value(&category, &r.code, Some(&r.description));
                CodeDescription {
                    code: r.code,
                    description: r.description,
                    filter_value,
                }
            })
            .collect();
        return Ok(with_cache_control(rows_mapped, cache::REFERENCE));
    }

    let entries_mapped: Vec<CodeDescription> = entries
        .into_iter()
        .map(|(code, desc)| {
            let filter_value = code_to_filter_value(&category, code, Some(desc));
            CodeDescription {
                code: code.to_string(),
                description: desc.to_string(),
                filter_value,
            }
        })
        .collect();
    Ok(with_cache_control(entries_mapped, cache::REFERENCE))
}

/// `GET /api/search-options?term={slug}` (term optional, defaults to latest)
pub(super) async fn get_search_options(
    State(state): State<AppState>,
    Query(params): Query<SearchOptionsParams>,
) -> Result<Response, ApiError> {
    let term_slug = if let Some(ref t) = params.term {
        t.clone()
    } else {
        // Fetch available terms to get the default (latest)
        let term_codes = data::courses::get_available_terms(&state.db_pool)
            .await
            .map_err(|e| db_error("Get terms for default", e))?;

        let first_term: Term = term_codes
            .first()
            .and_then(|code| code.parse().ok())
            .ok_or_else(|| ApiError::new(ApiErrorCode::NoTerms, "No terms available"))?;

        first_term.slug()
    };

    let term_code =
        Term::resolve_to_code(&term_slug).ok_or_else(|| ApiError::invalid_term(&term_slug))?;

    if let Some(cached) = state.search_options_cache.get(&term_code) {
        return Ok(with_cache_control((*cached).clone(), cache::REFERENCE));
    }

    if !state.search_options_cache.try_claim(&term_code) {
        // Another request is building this term's options -- fall through and build it too.
        // (Acceptable: singleflight is best-effort, not strict.)
    }

    let (term_codes, subject_rows, ranges) = tokio::try_join!(
        data::courses::get_available_terms(&state.db_pool),
        data::courses::get_subjects_by_enrollment(&state.db_pool, &term_code),
        data::courses::get_filter_ranges(&state.db_pool, &term_code),
    )
    .map_err(|e| db_error("Search options", e))?;

    let terms: Vec<TermResponse> = term_codes
        .into_iter()
        .filter_map(|code| {
            let term: Term = code.parse().ok()?;
            Some(TermResponse {
                code,
                slug: term.slug(),
                description: term.description(),
            })
        })
        .collect();

    let subjects: Vec<CodeDescription> = subject_rows
        .into_iter()
        .map(|(code, description, _enrollment)| {
            let filter_value = code.clone();
            CodeDescription {
                code,
                description,
                filter_value,
            }
        })
        .collect();

    let ref_cache = state.reference_cache.read().await;
    let build_ref = |category: &str| -> Vec<CodeDescription> {
        ref_cache
            .entries_for_category(category)
            .into_iter()
            .map(|(code, desc)| {
                let filter_value = code_to_filter_value(category, code, Some(desc));
                CodeDescription {
                    code: code.to_string(),
                    description: desc.to_string(),
                    filter_value,
                }
            })
            .collect()
    };

    let reference = SearchOptionsReference {
        instructional_methods: build_ref("instructional_method"),
        campuses: build_ref("campus"),
        parts_of_term: build_ref("part_of_term"),
        attributes: build_ref("attribute"),
    };

    let response = SearchOptionsResponse {
        terms,
        subjects,
        reference,
        ranges,
    };

    state
        .search_options_cache
        .insert(term_code.clone(), response.clone());
    state.search_options_cache.release(&term_code);

    Ok(with_cache_control(response, cache::REFERENCE))
}
