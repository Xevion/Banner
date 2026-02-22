//! Rate limiting for Banner API requests.
//!
//! Combines rate limiter logic with HTTP middleware enforcement,
//! classifying requests by URL pattern and throttling each type independently.

use crate::config::RateLimitingConfig;
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

/// Static rule table for endpoint classification.
/// Ordered most-specific first so `/classSearch/getTerms` matches Metadata, not Search.
const ENDPOINT_RULES: &[(RequestType, &[&str])] = &[
    (
        RequestType::Metadata,
        &[
            "/getTerms",
            "/get_subject",
            "/get_campus",
            "/get_instructionalMethod",
            "/get_partOfTerm",
            "/get_attribute",
        ],
    ),
    (
        RequestType::Session,
        &[
            "/registration",
            "/selfServiceMenu",
            "/term/termSelection",
            "/term/search",
        ],
    ),
    (RequestType::Reset, &["/resetDataForm"]),
    (RequestType::Search, &["/searchResults", "/classSearch"]),
];

/// Classifies a URL path into a request type using `ENDPOINT_RULES`.
fn classify(path: &str) -> RequestType {
    for (request_type, patterns) in ENDPOINT_RULES {
        if patterns.iter().any(|p| path.contains(p)) {
            return *request_type;
        }
    }
    RequestType::Search // fallback for unknown endpoints
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

        let start = std::time::Instant::now();
        self.rate_limiter.wait_for_permission(request_type).await;
        let wait_duration = start.elapsed();

        if wait_duration >= Duration::from_secs(5) {
            debug!(
                request_type = ?request_type,
                wait = fmt_duration(wait_duration),
                rpm = self.rate_limiter.rpm(request_type),
                "Rate limit caused significant delay"
            );
        }

        next.run(req, extensions).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- BannerRateLimiter tests (from rate_limiter.rs) ---

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

    // --- rpm() tests ---

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

    // --- Classification tests ---

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
}
