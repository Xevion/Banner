//! Request counters and latency histograms, labelled by matched route.

use axum::extract::{MatchedPath, Request};
use axum::middleware::Next;
use axum::response::Response;
use std::time::Instant;

use crate::telemetry::{HTTP_DURATION, HTTP_REQUESTS, method_label};

/// Raw URIs are attacker-controlled, so unmatched requests must not contribute path labels.
const UNMATCHED: &str = "unmatched";

/// Applied via `Router::layer`, which runs after routing, so `MatchedPath` is the route template.
pub async fn track_metrics(req: Request, next: Next) -> Response {
    let path = req
        .extensions()
        .get::<MatchedPath>()
        .map_or(UNMATCHED, MatchedPath::as_str)
        .to_owned();
    let method = method_label(req.method());

    let started = Instant::now();
    let response = next.run(req).await;
    let elapsed = started.elapsed();

    let status = response.status().as_u16().to_string();

    metrics::counter!(
        HTTP_REQUESTS,
        "method" => method,
        "path" => path.clone(),
        "status" => status,
    )
    .increment(1);

    // Path only. Status multiplies every bucket, and one route is near-always one method, so
    // neither label earns the series count it costs on a histogram.
    metrics::histogram!(HTTP_DURATION, "path" => path).record(elapsed.as_secs_f64());

    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::Router;
    use axum::body::Body;
    use axum::http::{Request as HttpRequest, StatusCode};
    use axum::response::IntoResponse;
    use axum::routing::get;
    use metrics_util::debugging::{DebugValue, DebuggingRecorder, Snapshot};
    use tower::ServiceExt;

    fn counter_label(snapshot: Snapshot, label: &str) -> Option<String> {
        snapshot
            .into_vec()
            .into_iter()
            .find_map(|(ck, _, _, value)| {
                let key = ck.key();
                match (key.name() == HTTP_REQUESTS, value) {
                    (true, DebugValue::Counter(_)) => key
                        .labels()
                        .find(|l| l.key() == label)
                        .map(|l| l.value().to_owned()),
                    _ => None,
                }
            })
    }

    /// Drives one request through `router` on this thread, so the thread-local recorder applies.
    fn request_labels(router: Router, uri: &str) -> (StatusCode, Snapshot) {
        let recorder = DebuggingRecorder::new();
        let snapshotter = recorder.snapshotter();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("current-thread runtime builds");

        let status = metrics::with_local_recorder(&recorder, || {
            let request = HttpRequest::builder()
                .uri(uri)
                .body(Body::empty())
                .expect("request builds");
            runtime
                .block_on(router.oneshot(request))
                .expect("router responds")
                .status()
        });

        (status, snapshotter.snapshot())
    }

    #[test]
    fn test_records_matched_path_template_not_raw_uri() {
        let router = Router::new()
            .route("/courses/{term}/{crn}", get(|| async { "ok" }))
            .layer(axum::middleware::from_fn(track_metrics));

        let (status, snapshot) = request_labels(router, "/courses/202620/12345");

        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            counter_label(snapshot, "path").as_deref(),
            Some("/courses/{term}/{crn}"),
            "the concrete term and CRN must never reach a label"
        );
    }

    /// Layer order is load-bearing: below the rate limiter a 429 never reaches this middleware.
    #[test]
    fn test_counts_responses_short_circuited_below_it() {
        let reject = axum::middleware::from_fn(|_req: Request, _next: Next| async {
            StatusCode::TOO_MANY_REQUESTS.into_response()
        });
        let router = Router::new()
            .route("/courses/{term}/{crn}", get(|| async { "ok" }))
            .layer(reject)
            .layer(axum::middleware::from_fn(track_metrics));

        let (status, snapshot) = request_labels(router, "/courses/202620/12345");

        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(
            counter_label(snapshot, "status").as_deref(),
            Some("429"),
            "a rate-limited response must still be counted"
        );
    }
}
