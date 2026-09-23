//! On-demand client IP extraction from trusted proxy headers.
//!
//! Priority: `CF-Connecting-IP` (Cloudflare) -> rightmost `X-Forwarded-For`
//! (Railway-appended) -> socket peer address.
//!
//! Use as an Axum extractor in handlers that need the client's real IP:
//!
//! ```ignore
//! async fn handler(ClientIp(ip): ClientIp, ...) -> impl IntoResponse { ... }
//! ```

use axum::extract::{ConnectInfo, FromRequestParts};
use axum::http::StatusCode;
use http::request::Parts;
use http::{Extensions, HeaderMap};
use std::net::{IpAddr, SocketAddr};

/// The resolved client IP address.
pub struct ClientIp(pub IpAddr);

impl<S: Send + Sync> FromRequestParts<S> for ClientIp {
    type Rejection = (StatusCode, &'static str);

    // Resolution reads headers already in memory, so there is nothing to await.
    fn from_request_parts(parts: &mut Parts, _state: &S) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        std::future::ready(
            resolve_client_ip(&parts.headers, &parts.extensions)
                .map(Self)
                .ok_or((StatusCode::INTERNAL_SERVER_ERROR, "Unable to determine client IP")),
        )
    }
}

/// Resolves the client address in the priority order described in the module docs.
///
/// Returns `None` only when the server was not started with `ConnectInfo`.
#[must_use]
pub fn resolve_client_ip(headers: &HeaderMap, extensions: &Extensions) -> Option<IpAddr> {
    if let Some(ip) = header_str(headers, "cf-connecting-ip").and_then(|s| s.parse::<IpAddr>().ok()) {
        return Some(ip);
    }

    if let Some(xff) = header_str(headers, "x-forwarded-for")
        && let Some(ip) = xff
            .rsplit(',')
            .next()
            .map(str::trim)
            .and_then(|s| s.parse::<IpAddr>().ok())
    {
        return Some(ip);
    }

    extensions
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ConnectInfo(addr)| addr.ip())
}

#[must_use]
pub fn header_str<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name).and_then(|v| v.to_str().ok())
}
