//! HTTP middleware for the Banner API client.

use crate::utils::fmt_duration;
use http::Extensions;
use reqwest::{Request, Response};
use reqwest_middleware::{Middleware, Next};
use std::time::Duration;
use tracing::{debug, trace, warn};

pub struct LoggingMiddleware;

/// Threshold for logging slow requests at DEBUG level
const SLOW_REQUEST_THRESHOLD: Duration = Duration::from_secs(5);

#[async_trait::async_trait]
impl Middleware for LoggingMiddleware {
    async fn handle(
        &self,
        req: Request,
        extensions: &mut Extensions,
        next: Next<'_>,
    ) -> std::result::Result<Response, reqwest_middleware::Error> {
        let method = req.method().to_string();
        // Use the full URL (including query parameters) for logging
        let url = req.url().to_string();

        let start = std::time::Instant::now();
        let response_result = next.run(req, extensions).await;
        let duration = start.elapsed();

        match response_result {
            Ok(response) => {
                let status = response.status().as_u16();

                if response.status().is_success() {
                    if duration >= SLOW_REQUEST_THRESHOLD {
                        debug!(
                            method,
                            url,
                            status,
                            duration = fmt_duration(duration),
                            "Request completed (slow)"
                        );
                    } else {
                        trace!(
                            method,
                            url,
                            status,
                            duration = fmt_duration(duration),
                            "Request completed"
                        );
                    }
                    Ok(response)
                } else {
                    warn!(
                        method,
                        url,
                        status,
                        duration = fmt_duration(duration),
                        "Request failed"
                    );
                    Ok(response)
                }
            }
            Err(error) => {
                warn!(
                    method,
                    url,
                    duration = fmt_duration(duration),
                    "Request failed"
                );
                Err(error)
            }
        }
    }
}
