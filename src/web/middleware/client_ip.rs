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
use std::net::{IpAddr, SocketAddr};

/// The resolved client IP address.
#[allow(dead_code)]
pub struct ClientIp(pub IpAddr);

impl<S: Send + Sync> FromRequestParts<S> for ClientIp {
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        // 1. CF-Connecting-IP -- set by Cloudflare, most trustworthy.
        if let Some(ip) =
            header_str(&parts.headers, "cf-connecting-ip").and_then(|s| s.parse::<IpAddr>().ok())
        {
            return Ok(ClientIp(ip));
        }

        // 2. Rightmost X-Forwarded-For -- appended by Railway's edge proxy.
        if let Some(xff) = header_str(&parts.headers, "x-forwarded-for")
            && let Some(ip) = xff
                .rsplit(',')
                .next()
                .map(str::trim)
                .and_then(|s| s.parse::<IpAddr>().ok())
        {
            return Ok(ClientIp(ip));
        }

        // 3. Socket peer address (local dev fallback).
        if let Some(ConnectInfo(addr)) = parts.extensions.get::<ConnectInfo<SocketAddr>>() {
            return Ok(ClientIp(addr.ip()));
        }

        Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            "Unable to determine client IP",
        ))
    }
}

#[allow(dead_code)]
fn header_str<'a>(headers: &'a http::HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name).and_then(|v| v.to_str().ok())
}
