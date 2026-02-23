//! SSR proxy: forwards non-API, non-static requests to the SvelteKit SSR server.

use axum::http::{HeaderMap, HeaderName, StatusCode, header};
use axum::response::{IntoResponse, Response};
use tracing::{debug, warn};

use crate::state::AppState;

/// Headers to strip from the downstream SSR response before returning to the client.
const STRIPPED_HEADERS: &[HeaderName] = &[header::TRANSFER_ENCODING, header::CONNECTION];

/// Proxy a request to the downstream SSR server.
pub async fn proxy_to_ssr(
    state: &AppState,
    method: &axum::http::Method,
    path: &str,
    query: Option<&str>,
    forward_headers: HeaderMap,
) -> Response {
    // Only proxy GET/HEAD for page requests
    if *method != axum::http::Method::GET && *method != axum::http::Method::HEAD {
        return StatusCode::METHOD_NOT_ALLOWED.into_response();
    }

    let url = match query {
        Some(q) => format!("{}{path}?{q}", state.ssr_downstream),
        None => format!("{}{path}", state.ssr_downstream),
    };

    debug!(url = %url, "proxying to SSR");

    let mut req = state.ssr_client.get(&url);
    for (name, value) in forward_headers.iter() {
        // Don't forward hop-by-hop headers
        if *name == header::HOST || *name == header::CONNECTION {
            continue;
        }
        req = req.header(name, value);
    }

    let resp = match req.send().await {
        Ok(r) => r,
        Err(e) => {
            warn!(error = %e, "SSR proxy request failed");
            return (StatusCode::BAD_GATEWAY, "SSR server unavailable").into_response();
        }
    };

    let status = resp.status();
    let resp_headers = resp.headers().clone();
    let body = match resp.bytes().await {
        Ok(b) => b,
        Err(e) => {
            warn!(error = %e, "Failed to read SSR response body");
            return StatusCode::BAD_GATEWAY.into_response();
        }
    };

    let mut headers = HeaderMap::new();
    for (name, value) in resp_headers.iter() {
        if STRIPPED_HEADERS.contains(name) {
            continue;
        }
        // Skip content-length since we're buffering the full body and axum will set it
        if *name == header::CONTENT_LENGTH {
            continue;
        }
        headers.insert(name.clone(), value.clone());
    }

    (
        StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY),
        headers,
        body,
    )
        .into_response()
}
