//! Rate limiting for Banner API requests.
//!
//! Combines rate limiter logic with HTTP middleware enforcement,
//! classifying requests by URL pattern and throttling each type independently.

use crate::config::RateLimitingConfig;
use crate::telemetry::{
    BANNER_DECODE_FAILURES, BANNER_DURATION, BANNER_RATE_LIMIT_WAIT, BANNER_REQUESTS,
};
use crate::utils::fmt_duration;
use governor::{
    Quota, RateLimiter,
    clock::DefaultClock,
    state::{InMemoryState, NotKeyed},
};
use http::Extensions;
use reqwest::{Request, Response};
use reqwest_middleware::{Middleware, Next};
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;
use tracing::debug;

/// Different types of Banner API requests, each with its own rate limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RequestType {
    /// Metadata lookups: `/getTerms`, `/get_subject`, `/get_campus`,
    /// `/get_instructionalMethod`, `/get_partOfTerm`, `/get_attribute`
    Metadata,
    /// Session creation and management: `/registration`, `/selfServiceMenu`,
    /// `/term/termSelection`, `/term/search`
    Session,
    /// Data form resets: `/resetDataForm`
    Reset,
    /// Course search requests: `/searchResults`, `/classSearch`
    Search,
}

/// One row per known endpoint: URL pattern, bounded metrics label, and rate limit category.
struct EndpointRule {
    pattern: &'static str,
    label: &'static str,
    request_type: RequestType,
}

/// Ordered most-specific first so `/classSearch/getTerms` matches `terms`, not the
/// generic `/classSearch` fallback.
const ENDPOINT_RULES: &[EndpointRule] = &[
    EndpointRule {
        pattern: "/getTerms",
        label: "terms",
        request_type: RequestType::Metadata,
    },
    EndpointRule {
        pattern: "/get_subject",
        label: "subjects",
        request_type: RequestType::Metadata,
    },
    EndpointRule {
        pattern: "/get_campus",
        label: "campuses",
        request_type: RequestType::Metadata,
    },
    EndpointRule {
        pattern: "/get_instructionalMethod",
        label: "instructional_methods",
        request_type: RequestType::Metadata,
    },
    EndpointRule {
        pattern: "/get_partOfTerm",
        label: "parts_of_term",
        request_type: RequestType::Metadata,
    },
    EndpointRule {
        pattern: "/get_attribute",
        label: "attributes",
        request_type: RequestType::Metadata,
    },
    EndpointRule {
        pattern: "/registration",
        label: "registration",
        request_type: RequestType::Session,
    },
    EndpointRule {
        pattern: "/selfServiceMenu",
        label: "self_service_menu",
        request_type: RequestType::Session,
    },
    EndpointRule {
        pattern: "/term/termSelection",
        label: "term_selection",
        request_type: RequestType::Session,
    },
    EndpointRule {
        pattern: "/term/search",
        label: "term_search",
        request_type: RequestType::Session,
    },
    EndpointRule {
        pattern: "/resetDataForm",
        label: "reset_data_form",
        request_type: RequestType::Reset,
    },
    EndpointRule {
        pattern: "/getFacultyMeetingTimes",
        label: "meeting_times",
        request_type: RequestType::Search,
    },
    EndpointRule {
        pattern: "/searchResults",
        label: "search",
        request_type: RequestType::Search,
    },
    EndpointRule {
        pattern: "/classSearch",
        label: "search",
        request_type: RequestType::Search,
    },
];

fn lookup_endpoint(path: &str) -> Option<&'static EndpointRule> {
    ENDPOINT_RULES
        .iter()
        .find(|rule| path.contains(rule.pattern))
}

/// Classifies a URL path into a request type using `ENDPOINT_RULES`.
fn classify(path: &str) -> RequestType {
    lookup_endpoint(path).map_or(RequestType::Search, |rule| rule.request_type)
}

/// Counts a response that arrived intact but could not be deserialized.
///
/// Separate from `banner_api_requests_total`, which already counted this request a success: the
/// transport did succeed, and re-incrementing it here would double-count the same request.
pub(crate) fn record_decode_failure(url: &str) {
    metrics::counter!(BANNER_DECODE_FAILURES, "endpoint" => endpoint_label(url)).increment(1);
}

/// Bounded metrics label for a URL path; never derived from the path itself.
fn endpoint_label(path: &str) -> &'static str {
    lookup_endpoint(path).map_or("other", |rule| rule.label)
}

/// Bounded outcome label for an upstream HTTP status code.
fn outcome_for_status(status: u16) -> &'static str {
    match status {
        429 => "rate_limited",
        200..=299 => "success",
        _ => "http_error",
    }
}

/// A rate limiter that manages different request types with different limits.
pub struct BannerRateLimiter {
    config: RateLimitingConfig,
    session_limiter: RateLimiter<NotKeyed, InMemoryState, DefaultClock>,
    search_limiter: RateLimiter<NotKeyed, InMemoryState, DefaultClock>,
    metadata_limiter: RateLimiter<NotKeyed, InMemoryState, DefaultClock>,
    reset_limiter: RateLimiter<NotKeyed, InMemoryState, DefaultClock>,
}

impl BannerRateLimiter {
    /// Creates a new rate limiter with the given configuration.
    pub fn new(config: RateLimitingConfig) -> Self {
        let session_quota = Quota::with_period(Duration::from_secs(60) / config.session_rpm)
            .unwrap()
            .allow_burst(NonZeroU32::new(config.burst_allowance).unwrap());

        let search_quota = Quota::with_period(Duration::from_secs(60) / config.search_rpm)
            .unwrap()
            .allow_burst(NonZeroU32::new(config.burst_allowance).unwrap());

        let metadata_quota = Quota::with_period(Duration::from_secs(60) / config.metadata_rpm)
            .unwrap()
            .allow_burst(NonZeroU32::new(config.burst_allowance).unwrap());

        let reset_quota = Quota::with_period(Duration::from_secs(60) / config.reset_rpm)
            .unwrap()
            .allow_burst(NonZeroU32::new(config.burst_allowance).unwrap());

        Self {
            config,
            session_limiter: RateLimiter::direct(session_quota),
            search_limiter: RateLimiter::direct(search_quota),
            metadata_limiter: RateLimiter::direct(metadata_quota),
            reset_limiter: RateLimiter::direct(reset_quota),
        }
    }

    /// Waits for permission to make a request of the given type.
    pub async fn wait_for_permission(&self, request_type: RequestType) {
        let limiter = match request_type {
            RequestType::Session => &self.session_limiter,
            RequestType::Search => &self.search_limiter,
            RequestType::Metadata => &self.metadata_limiter,
            RequestType::Reset => &self.reset_limiter,
        };

        limiter.until_ready().await;
    }

    /// Returns the configured requests-per-minute for the given type.
    pub fn rpm(&self, request_type: RequestType) -> u32 {
        match request_type {
            RequestType::Session => self.config.session_rpm,
            RequestType::Search => self.config.search_rpm,
            RequestType::Metadata => self.config.metadata_rpm,
            RequestType::Reset => self.config.reset_rpm,
        }
    }
}

impl Default for BannerRateLimiter {
    fn default() -> Self {
        Self::new(RateLimitingConfig::default())
    }
}

/// A shared rate limiter instance.
pub type SharedRateLimiter = Arc<BannerRateLimiter>;

/// Middleware that enforces rate limiting based on request URL patterns.
pub struct RateLimitMiddleware {
    rate_limiter: SharedRateLimiter,
}

impl RateLimitMiddleware {
    /// Creates a new rate limiting middleware.
    pub fn new(rate_limiter: SharedRateLimiter) -> Self {
        Self { rate_limiter }
    }
}

#[async_trait::async_trait]
impl Middleware for RateLimitMiddleware {
    async fn handle(
        &self,
        req: Request,
        extensions: &mut Extensions,
        next: Next<'_>,
    ) -> std::result::Result<Response, reqwest_middleware::Error> {
        let request_type = classify(req.url().path());
        let endpoint = endpoint_label(req.url().path());

        let wait_start = std::time::Instant::now();
        self.rate_limiter.wait_for_permission(request_type).await;
        let wait_duration = wait_start.elapsed();

        metrics::histogram!(BANNER_RATE_LIMIT_WAIT, "endpoint" => endpoint)
            .record(wait_duration.as_secs_f64());

        if wait_duration >= Duration::from_secs(5) {
            debug!(
                request_type = ?request_type,
                wait = fmt_duration(wait_duration),
                rpm = self.rate_limiter.rpm(request_type),
                "Rate limit caused significant delay"
            );
        }

        let request_start = std::time::Instant::now();
        let result = next.run(req, extensions).await;
        let duration = request_start.elapsed();

        let outcome = match &result {
            Ok(response) => outcome_for_status(response.status().as_u16()),
            Err(reqwest_middleware::Error::Reqwest(e)) if e.is_timeout() => "timeout",
            Err(_) => "transport_error",
        };

        metrics::counter!(BANNER_REQUESTS, "endpoint" => endpoint, "outcome" => outcome)
            .increment(1);
        metrics::histogram!(BANNER_DURATION, "endpoint" => endpoint).record(duration.as_secs_f64());

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert2::check;

    #[test]
    fn test_new_with_default_config() {
        let _limiter = BannerRateLimiter::new(RateLimitingConfig::default());
    }

    #[test]
    fn test_new_with_custom_config() {
        let config = RateLimitingConfig {
            session_rpm: 10,
            search_rpm: 30,
            metadata_rpm: 20,
            reset_rpm: 15,
            burst_allowance: 5,
        };
        let _limiter = BannerRateLimiter::new(config);
    }

    #[test]
    fn test_new_with_minimum_valid_values() {
        let config = RateLimitingConfig {
            session_rpm: 1,
            search_rpm: 1,
            metadata_rpm: 1,
            reset_rpm: 1,
            burst_allowance: 1,
        };
        let _limiter = BannerRateLimiter::new(config);
    }

    #[test]
    fn test_new_with_high_rpm_values() {
        let config = RateLimitingConfig {
            session_rpm: 10000,
            search_rpm: 10000,
            metadata_rpm: 10000,
            reset_rpm: 10000,
            burst_allowance: 1,
        };
        let _limiter = BannerRateLimiter::new(config);
    }

    #[test]
    fn test_default_impl() {
        let _limiter = BannerRateLimiter::default();
    }

    #[test]
    #[should_panic]
    fn test_new_panics_on_zero_session_rpm() {
        let config = RateLimitingConfig {
            session_rpm: 0,
            ..RateLimitingConfig::default()
        };
        let _limiter = BannerRateLimiter::new(config);
    }

    #[test]
    #[should_panic]
    fn test_new_panics_on_zero_search_rpm() {
        let config = RateLimitingConfig {
            search_rpm: 0,
            ..RateLimitingConfig::default()
        };
        let _limiter = BannerRateLimiter::new(config);
    }

    #[test]
    #[should_panic]
    fn test_new_panics_on_zero_metadata_rpm() {
        let config = RateLimitingConfig {
            metadata_rpm: 0,
            ..RateLimitingConfig::default()
        };
        let _limiter = BannerRateLimiter::new(config);
    }

    #[test]
    #[should_panic]
    fn test_new_panics_on_zero_reset_rpm() {
        let config = RateLimitingConfig {
            reset_rpm: 0,
            ..RateLimitingConfig::default()
        };
        let _limiter = BannerRateLimiter::new(config);
    }

    #[test]
    #[should_panic]
    fn test_new_panics_on_zero_burst_allowance() {
        let config = RateLimitingConfig {
            burst_allowance: 0,
            ..RateLimitingConfig::default()
        };
        let _limiter = BannerRateLimiter::new(config);
    }

    #[tokio::test]
    async fn test_wait_for_permission_completes() {
        let limiter = BannerRateLimiter::default();
        let timeout_duration = std::time::Duration::from_secs(1);

        for request_type in [
            RequestType::Session,
            RequestType::Search,
            RequestType::Metadata,
            RequestType::Reset,
        ] {
            let result =
                tokio::time::timeout(timeout_duration, limiter.wait_for_permission(request_type))
                    .await;
            assert!(
                result.is_ok(),
                "wait_for_permission timed out for {:?}",
                request_type
            );
        }
    }

    #[test]
    fn test_rpm_returns_config_values() {
        let config = RateLimitingConfig {
            session_rpm: 20,
            search_rpm: 60,
            metadata_rpm: 40,
            reset_rpm: 30,
            burst_allowance: 5,
        };
        let limiter = BannerRateLimiter::new(config);

        assert_eq!(limiter.rpm(RequestType::Session), 20);
        assert_eq!(limiter.rpm(RequestType::Search), 60);
        assert_eq!(limiter.rpm(RequestType::Metadata), 40);
        assert_eq!(limiter.rpm(RequestType::Reset), 30);
    }

    #[test]
    fn test_classify_metadata_endpoints() {
        assert_eq!(classify("/classSearch/getTerms"), RequestType::Metadata);
        assert_eq!(classify("/classSearch/get_subject"), RequestType::Metadata);
        assert_eq!(classify("/classSearch/get_campus"), RequestType::Metadata);
        assert_eq!(
            classify("/classSearch/get_instructionalMethod"),
            RequestType::Metadata
        );
        assert_eq!(
            classify("/classSearch/get_partOfTerm"),
            RequestType::Metadata
        );
        assert_eq!(
            classify("/classSearch/get_attribute"),
            RequestType::Metadata
        );
    }

    #[test]
    fn test_classify_session_endpoints() {
        assert_eq!(classify("/registration"), RequestType::Session);
        assert_eq!(classify("/selfServiceMenu/data"), RequestType::Session);
        assert_eq!(classify("/term/termSelection"), RequestType::Session);
        assert_eq!(classify("/term/search"), RequestType::Session);
    }

    #[test]
    fn test_classify_reset_endpoints() {
        assert_eq!(classify("/classSearch/resetDataForm"), RequestType::Reset);
    }

    #[test]
    fn test_classify_search_endpoints() {
        assert_eq!(
            classify("/searchResults/searchResults"),
            RequestType::Search
        );
        assert_eq!(
            classify("/searchResults/getFacultyMeetingTimes"),
            RequestType::Search
        );
    }

    #[test]
    fn test_classify_ambiguous_class_search_path() {
        // /classSearch/getTerms should match Metadata (getTerms), not Search (classSearch)
        assert_eq!(classify("/classSearch/getTerms"), RequestType::Metadata);
    }

    #[test]
    fn test_classify_unknown_defaults_to_search() {
        assert_eq!(classify("/some/unknown/endpoint"), RequestType::Search);
    }

    #[test]
    fn test_endpoint_label_terms_is_distinct_from_search() {
        check!(endpoint_label("/classSearch/getTerms") == "terms");
    }

    #[test]
    fn test_endpoint_label_meeting_times_is_distinct_from_search() {
        check!(endpoint_label("/searchResults/getFacultyMeetingTimes") == "meeting_times");
    }

    #[test]
    fn test_endpoint_label_search_results_is_search() {
        check!(endpoint_label("/searchResults/searchResults") == "search");
    }

    #[test]
    fn test_endpoint_label_unknown_defaults_to_other() {
        check!(endpoint_label("/some/unknown/endpoint") == "other");
    }

    #[test]
    fn test_endpoint_label_all_rules_are_bounded_lowercase_snake_case() {
        for rule in ENDPOINT_RULES {
            check!(
                rule.label
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c == '_')
            );
        }
    }

    #[test]
    fn test_outcome_for_status_2xx_is_success() {
        check!(outcome_for_status(200) == "success");
        check!(outcome_for_status(204) == "success");
    }

    #[test]
    fn test_outcome_for_status_429_is_rate_limited() {
        check!(outcome_for_status(429) == "rate_limited");
    }

    #[test]
    fn test_outcome_for_status_5xx_is_http_error() {
        check!(outcome_for_status(500) == "http_error");
    }

    #[test]
    fn test_outcome_for_status_4xx_non_429_is_http_error() {
        check!(outcome_for_status(404) == "http_error");
    }
}
