//! Global security headers applied to every response.
//!
//! Injects standard security headers (XFO, XCTO, Referrer-Policy, etc.)
//! and conditionally adds HSTS when running behind Railway (detected via
//! the `X-Railway-Request-Id` header on the incoming request).

use axum::extract::Request;
use axum::http::HeaderValue;
use axum::response::Response;
use std::task::{Context, Poll};
use tower::{Layer, Service};

static XFO: HeaderValue = HeaderValue::from_static("DENY");
static XCTO: HeaderValue = HeaderValue::from_static("nosniff");
static REFERRER: HeaderValue = HeaderValue::from_static("strict-origin-when-cross-origin");
static PERMISSIONS: HeaderValue =
    HeaderValue::from_static("camera=(), microphone=(), geolocation=()");
static COOP: HeaderValue = HeaderValue::from_static("same-origin");
static HSTS: HeaderValue = HeaderValue::from_static("max-age=31536000; includeSubDomains");

#[derive(Clone)]
pub struct SecurityHeadersLayer;

impl<S> Layer<S> for SecurityHeadersLayer {
    type Service = SecurityHeadersService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        SecurityHeadersService { inner }
    }
}

#[derive(Clone)]
pub struct SecurityHeadersService<S> {
    inner: S,
}

impl<S, B> Service<Request> for SecurityHeadersService<S>
where
    S: Service<Request, Response = Response<B>> + Send + 'static,
    S::Future: Send + 'static,
    S::Error: std::fmt::Debug,
    B: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>,
    >;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let is_prod = req.headers().contains_key("x-railway-request-id");
        let future = self.inner.call(req);

        Box::pin(async move {
            let mut response = future.await?;
            let headers = response.headers_mut();

            headers.insert("x-frame-options", XFO.clone());
            headers.insert("x-content-type-options", XCTO.clone());
            headers.insert("referrer-policy", REFERRER.clone());
            headers.insert("permissions-policy", PERMISSIONS.clone());
            headers.insert("cross-origin-opener-policy", COOP.clone());

            if is_prod {
                headers.insert("strict-transport-security", HSTS.clone());
            }

            // Fallback CSP for non-SSR responses (API JSON, embedded static assets).
            // SvelteKit-proxied responses already carry a CSP from kit.csp — don't override.
            if !headers.contains_key("content-security-policy")
                && !headers.contains_key("content-security-policy-report-only")
            {
                headers.insert(
                    "content-security-policy",
                    HeaderValue::from_static(
                        "default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self'; font-src 'self'; frame-ancestors 'none'",
                    ),
                );
            }

            Ok(response)
        })
    }
}
