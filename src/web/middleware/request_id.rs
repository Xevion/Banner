//! Per-request tracing spans with upstream-aware request IDs.
//!
//! When running behind Railway, prefers the `X-Railway-Request-Id` header
//! so logs correlate directly with Railway's dashboard. Falls back to
//! generating a ULID (local development).
//!
//! Always sets an `X-Request-Id` response header with the resolved ID.

use axum::extract::Request;
use axum::http::HeaderValue;
use axum::response::Response;
use std::task::{Context, Poll};
use std::time::Instant;
use tower::{Layer, Service};
use tracing::Instrument;

static RAILWAY_REQUEST_ID: &str = "x-railway-request-id";

#[derive(Clone)]
pub struct RequestIdLayer;

impl<S> Layer<S> for RequestIdLayer {
    type Service = RequestIdService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        RequestIdService { inner }
    }
}

#[derive(Clone)]
pub struct RequestIdService<S> {
    inner: S,
}

impl<S, B> Service<Request> for RequestIdService<S>
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
        // Prefer Railway's edge request ID; fall back to a locally generated ULID.
        let req_id = req
            .headers()
            .get(RAILWAY_REQUEST_ID)
            .and_then(|v| v.to_str().ok())
            .map(String::from)
            .unwrap_or_else(|| ulid::Ulid::new().to_string());

        let method = req.method().clone();
        let path = req.uri().path().to_string();
        let span = tracing::info_span!("request", req_id = %req_id);
        let start = Instant::now();

        let future = self.inner.call(req);

        // Clone for the response header (the span closure moves `req_id`).
        let header_value = HeaderValue::from_str(&req_id).ok();

        Box::pin(
            async move {
                let mut result = future.await;

                let duration_ms = start.elapsed().as_millis() as u64;

                match &result {
                    Ok(response) => {
                        let status = response.status();
                        match status.as_u16() {
                            200..=399 => {
                                tracing::debug!(method = %method, path = %path, status = status.as_u16(), duration_ms, "Response");
                            }
                            400..=499 => {
                                tracing::info!(method = %method, path = %path, status = status.as_u16(), duration_ms, "Response");
                            }
                            _ => {
                                tracing::warn!(method = %method, path = %path, status = status.as_u16(), duration_ms, "Response");
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!(method = %method, path = %path, error = ?e, duration_ms, "Request failed");
                    }
                }

                // Attach the request ID to the response for client correlation.
                if let Ok(ref mut response) = result
                    && let Some(value) = header_value
                {
                    response.headers_mut().insert("x-request-id", value);
                }

                result
            }
            .instrument(span),
        )
    }
}
