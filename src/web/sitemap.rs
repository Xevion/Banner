//! XML sitemap endpoints for search engine discovery.
//!
//! Four endpoints: sitemap index, static pages, instructors, and per-term courses.
//! All responses are cached in-memory with a 15-minute TTL via `SitemapCache`.

use axum::{
    extract::{Path, State},
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use std::sync::Arc;

use crate::banner::models::terms::Term;
use crate::data;
use crate::state::AppState;

/// XML content type and cache control headers shared by all sitemap responses.
fn xml_response(body: Arc<String>) -> Response {
    let mut response = (*body).clone().into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/xml; charset=utf-8"),
    );
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(
            "public, max-age=3600, s-maxage=86400, stale-while-revalidate=3600",
        ),
    );
    response
}

/// Try to serve from cache, or claim the singleflight slot and build.
/// Returns `Ok(response)` on cache hit or contention fallback, `Err(())` if caller should build.
fn try_cache_or_claim(state: &AppState, key: &str) -> Result<Response, ()> {
    if let Some(cached) = state.sitemap_cache.get(key) {
        return Ok(xml_response(cached));
    }

    if !state.sitemap_cache.try_claim(key) {
        // Another request is building -- serve stale if available, else 503
        if let Some(stale) = state.sitemap_cache.get_stale(key) {
            return Ok(xml_response(stale));
        }
        let mut resp = StatusCode::SERVICE_UNAVAILABLE.into_response();
        resp.headers_mut()
            .insert(header::RETRY_AFTER, HeaderValue::from_static("5"));
        return Ok(resp);
    }

    Err(())
}

/// Insert into cache, release singleflight, return response.
fn finish(state: &AppState, key: &str, xml: String) -> Response {
    let key_owned = key.to_owned();
    state.sitemap_cache.insert(key_owned, xml);
    let cached = state.sitemap_cache.get(key).unwrap();
    state.sitemap_cache.release(key);
    xml_response(cached)
}

/// `GET /sitemap.xml` -- sitemap index pointing to sub-sitemaps.
pub async fn sitemap_index(State(state): State<AppState>) -> Response {
    let Some(ref origin) = state.public_origin else {
        return StatusCode::NOT_FOUND.into_response();
    };

    let key = "index";
    if let Ok(resp) = try_cache_or_claim(&state, key) {
        return resp;
    }

    let terms = match data::courses::get_available_terms(&state.db_pool).await {
        Ok(t) => t,
        Err(_) => {
            state.sitemap_cache.release(key);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <sitemapindex xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n",
    );

    xml.push_str(&format!(
        "  <sitemap><loc>{origin}/sitemap-static.xml</loc></sitemap>\n"
    ));
    xml.push_str(&format!(
        "  <sitemap><loc>{origin}/sitemap-instructors.xml</loc></sitemap>\n"
    ));
    for code in &terms {
        let slug = code
            .parse::<Term>()
            .map(|t| t.slug())
            .unwrap_or(code.clone());
        xml.push_str(&format!(
            "  <sitemap><loc>{origin}/sitemap-courses-{slug}.xml</loc></sitemap>\n"
        ));
    }

    xml.push_str("</sitemapindex>\n");

    finish(&state, key, xml)
}

/// `GET /sitemap-static.xml` -- homepage, instructors directory, timeline.
pub async fn sitemap_static(State(state): State<AppState>) -> Response {
    let Some(ref origin) = state.public_origin else {
        return StatusCode::NOT_FOUND.into_response();
    };

    let key = "static";
    if let Ok(resp) = try_cache_or_claim(&state, key) {
        return resp;
    }

    let pages = ["/", "/instructors", "/timeline"];

    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n",
    );

    for page in &pages {
        xml.push_str(&format!("  <url><loc>{origin}{page}</loc></url>\n"));
    }

    xml.push_str("</urlset>\n");

    finish(&state, key, xml)
}

/// `GET /sitemap-instructors.xml` -- all instructor profile URLs.
pub async fn sitemap_instructors(State(state): State<AppState>) -> Response {
    let Some(ref origin) = state.public_origin else {
        return StatusCode::NOT_FOUND.into_response();
    };

    let key = "instructors";
    if let Ok(resp) = try_cache_or_claim(&state, key) {
        return resp;
    }

    let entries = match data::instructors::list_all_instructor_sitemap_entries(&state.db_pool).await
    {
        Ok(e) => e,
        Err(_) => {
            state.sitemap_cache.release(key);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n",
    );

    for entry in &entries {
        xml.push_str("  <url>\n");
        xml.push_str(&format!(
            "    <loc>{origin}/instructors/{}</loc>\n",
            entry.slug
        ));
        if let Some(dt) = entry.last_modified {
            xml.push_str(&format!(
                "    <lastmod>{}</lastmod>\n",
                dt.format("%Y-%m-%d")
            ));
        }
        xml.push_str("  </url>\n");
    }

    xml.push_str("</urlset>\n");

    finish(&state, key, xml)
}

/// `GET /sitemap-courses-{rest}` -- all course URLs for a term.
///
/// The route captures everything after `sitemap-courses-` as `rest`.
/// E.g. `/sitemap-courses-spring-2026.xml` -> rest = `spring-2026.xml`.
/// We strip the `.xml` suffix and resolve the remainder as a term slug or code.
pub async fn sitemap_courses(State(state): State<AppState>, Path(rest): Path<String>) -> Response {
    let Some(ref origin) = state.public_origin else {
        return StatusCode::NOT_FOUND.into_response();
    };

    let term_input = rest.strip_suffix(".xml").unwrap_or(&rest);

    // Resolve slug or raw code to a term code for the DB query
    let Some(term_code) = Term::resolve_to_code(term_input) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let term_slug = term_code
        .parse::<Term>()
        .map(|t| t.slug())
        .unwrap_or(term_code.clone());

    let key_owned = format!("courses-{term_code}");
    let key = key_owned.as_str();
    if let Ok(resp) = try_cache_or_claim(&state, key) {
        return resp;
    }

    let crns = match data::courses::list_crns_for_term(&state.db_pool, &term_code).await {
        Ok(c) => c,
        Err(_) => {
            state.sitemap_cache.release(key);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    // Look up last_scraped_at for <lastmod>
    let lastmod = match data::terms::get_all_terms(&state.db_pool).await {
        Ok(terms) => terms
            .iter()
            .find(|t| t.code == term_code)
            .and_then(|t| t.last_scraped_at)
            .map(|dt| dt.format("%Y-%m-%d").to_string()),
        Err(_) => None,
    };

    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n",
    );

    for crn in &crns {
        xml.push_str("  <url>\n");
        xml.push_str(&format!(
            "    <loc>{origin}/courses/{term_slug}/{crn}</loc>\n"
        ));
        if let Some(ref lm) = lastmod {
            xml.push_str(&format!("    <lastmod>{lm}</lastmod>\n"));
        }
        xml.push_str("  </url>\n");
    }

    xml.push_str("</urlset>\n");

    finish(&state, key, xml)
}
