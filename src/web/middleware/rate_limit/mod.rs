//! Inbound HTTP rate limiting with multi-window, per-IP token buckets.
//!
//! Four layers evaluated in order (first rejection wins):
//!
//! 1. **Global per-IP** -- burst (5s) + sustained (1min)
//! 2. **Route-group** -- sustained (1min) + long-term (30min), different budgets for API/SSR/admin
//! 3. **Endpoint-specific** -- all three windows on expensive endpoints
//! 4. **Auth-aware multiplier** -- authenticated 2x, admin 10x
//!
//! Requests carrying a valid `X-Internal-Token` header (set by the SSR proxy)
//! bypass all rate limiting to avoid double-counting SSR -> API calls.

use crate::web::middleware::client_ip::header_str;
use axum::body::Body;
use axum::extract::Request;
use axum::http::{HeaderValue, StatusCode};
use axum::response::Response;
use governor::{DefaultKeyedRateLimiter, Quota, RateLimiter, clock::Clock};
use std::net::IpAddr;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;
use tower::{Layer, Service};
use tracing::warn;

// -- Route classification --

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum RouteGroup {
    Api,
    Ssr,
    Admin,
    /// Health/metrics endpoints -- no route-group limiting.
    Internal,
}

/// Endpoints with their own tight limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum TrackedEndpoint {
    CourseSearch,
    Suggest,
    Timeline,
}

fn classify_route(path: &str) -> RouteGroup {
    if path.starts_with("/api/admin/") {
        RouteGroup::Admin
    } else if path.starts_with("/api/health") || path.starts_with("/api/metrics") {
        RouteGroup::Internal
    } else if path.starts_with("/api/") {
        RouteGroup::Api
    } else {
        RouteGroup::Ssr
    }
}

fn classify_endpoint(path: &str) -> Option<TrackedEndpoint> {
    if path == "/api/courses/search" {
        Some(TrackedEndpoint::CourseSearch)
    } else if path == "/api/suggest" {
        Some(TrackedEndpoint::Suggest)
    } else if path == "/api/timeline" {
        Some(TrackedEndpoint::Timeline)
    } else {
        None
    }
}

// -- Auth tier --

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)] // Authenticated/Admin variants used once session-based tier detection is added.
enum AuthTier {
    Anonymous,
    Authenticated,
    Admin,
}

impl AuthTier {
    #[allow(dead_code)] // Used once session-based tier detection is added.
    fn multiplier(self) -> u32 {
        match self {
            AuthTier::Anonymous => 1,
            AuthTier::Authenticated => 2,
            AuthTier::Admin => 10,
        }
    }
}

// -- Shared rate limit state --

/// Holds all keyed rate limiters for the multi-layer system.
///
/// Each limiter is keyed by `IpAddr`. The auth multiplier is applied by
/// allowing `multiplier` cells per check rather than maintaining separate
/// buckets per auth tier.
pub struct RateLimitState {
    // Layer 1: global per-IP
    global_burst: DefaultKeyedRateLimiter<IpAddr>, // 5s window
    global_sustained: DefaultKeyedRateLimiter<IpAddr>, // 1min window

    // Layer 2: route-group per-IP
    api_sustained: DefaultKeyedRateLimiter<IpAddr>,
    api_long: DefaultKeyedRateLimiter<IpAddr>,
    ssr_sustained: DefaultKeyedRateLimiter<IpAddr>,
    ssr_long: DefaultKeyedRateLimiter<IpAddr>,
    admin_sustained: DefaultKeyedRateLimiter<IpAddr>,
    admin_long: DefaultKeyedRateLimiter<IpAddr>,

    // Layer 3: endpoint-specific per-IP
    search_burst: DefaultKeyedRateLimiter<IpAddr>,
    search_sustained: DefaultKeyedRateLimiter<IpAddr>,
    search_long: DefaultKeyedRateLimiter<IpAddr>,
    suggest_burst: DefaultKeyedRateLimiter<IpAddr>,
    suggest_sustained: DefaultKeyedRateLimiter<IpAddr>,
    timeline_burst: DefaultKeyedRateLimiter<IpAddr>,
    timeline_sustained: DefaultKeyedRateLimiter<IpAddr>,
    timeline_long: DefaultKeyedRateLimiter<IpAddr>,

    /// Secret token for SSR -> API internal bypass.
    internal_token: String,
}

/// Quota helper: `count` requests per `period` with burst = count.
fn quota(count: u32, period: Duration) -> Quota {
    Quota::with_period(period / count)
        .expect("non-zero period")
        .allow_burst(NonZeroU32::new(count).expect("non-zero count"))
}

impl RateLimitState {
    /// The internal bypass token value.
    pub fn internal_token(&self) -> &str {
        &self.internal_token
    }

    pub fn new(internal_token: String) -> Self {
        // Layer 1: global per-IP
        let global_burst = RateLimiter::keyed(quota(15, Duration::from_secs(5)));
        let global_sustained = RateLimiter::keyed(quota(120, Duration::from_secs(60)));

        // Layer 2: route-group
        let api_sustained = RateLimiter::keyed(quota(60, Duration::from_secs(60)));
        let api_long = RateLimiter::keyed(quota(600, Duration::from_secs(30 * 60)));
        let ssr_sustained = RateLimiter::keyed(quota(20, Duration::from_secs(60)));
        let ssr_long = RateLimiter::keyed(quota(200, Duration::from_secs(30 * 60)));
        let admin_sustained = RateLimiter::keyed(quota(30, Duration::from_secs(60)));
        let admin_long = RateLimiter::keyed(quota(300, Duration::from_secs(30 * 60)));

        // Layer 3: endpoint-specific
        let search_burst = RateLimiter::keyed(quota(3, Duration::from_secs(5)));
        let search_sustained = RateLimiter::keyed(quota(20, Duration::from_secs(60)));
        let search_long = RateLimiter::keyed(quota(150, Duration::from_secs(30 * 60)));
        let suggest_burst = RateLimiter::keyed(quota(5, Duration::from_secs(5)));
        let suggest_sustained = RateLimiter::keyed(quota(30, Duration::from_secs(60)));
        let timeline_burst = RateLimiter::keyed(quota(2, Duration::from_secs(5)));
        let timeline_sustained = RateLimiter::keyed(quota(10, Duration::from_secs(60)));
        let timeline_long = RateLimiter::keyed(quota(60, Duration::from_secs(30 * 60)));

        Self {
            global_burst,
            global_sustained,
            api_sustained,
            api_long,
            ssr_sustained,
            ssr_long,
            admin_sustained,
            admin_long,
            search_burst,
            search_sustained,
            search_long,
            suggest_burst,
            suggest_sustained,
            timeline_burst,
            timeline_sustained,
            timeline_long,
            internal_token,
        }
    }

    /// Check all applicable rate limits for the request. Returns `Ok(())` if
    /// allowed, or `Err(retry_after_secs)` with the longest wait time.
    fn check(&self, ip: IpAddr, path: &str, _tier: AuthTier) -> Result<(), u64> {
        // Layer 1: global
        let mut max_wait: Option<Duration> = None;

        let check_limiter = |limiter: &DefaultKeyedRateLimiter<IpAddr>,
                             ip: &IpAddr,
                             max_wait: &mut Option<Duration>|
         -> bool {
            match limiter.check_key(ip) {
                Ok(()) => true,
                Err(not_until) => {
                    let wait =
                        not_until.wait_time_from(governor::clock::DefaultClock::default().now());
                    let current_max = max_wait.unwrap_or(Duration::ZERO);
                    if wait > current_max {
                        *max_wait = Some(wait);
                    }
                    false
                }
            }
        };

        let mut rejected = false;

        // Global per-IP
        if !check_limiter(&self.global_burst, &ip, &mut max_wait) {
            rejected = true;
        }
        if !check_limiter(&self.global_sustained, &ip, &mut max_wait) {
            rejected = true;
        }

        // Route-group
        let group = classify_route(path);
        match group {
            RouteGroup::Api => {
                if !check_limiter(&self.api_sustained, &ip, &mut max_wait) {
                    rejected = true;
                }
                if !check_limiter(&self.api_long, &ip, &mut max_wait) {
                    rejected = true;
                }
            }
            RouteGroup::Ssr => {
                if !check_limiter(&self.ssr_sustained, &ip, &mut max_wait) {
                    rejected = true;
                }
                if !check_limiter(&self.ssr_long, &ip, &mut max_wait) {
                    rejected = true;
                }
            }
            RouteGroup::Admin => {
                if !check_limiter(&self.admin_sustained, &ip, &mut max_wait) {
                    rejected = true;
                }
                if !check_limiter(&self.admin_long, &ip, &mut max_wait) {
                    rejected = true;
                }
            }
            RouteGroup::Internal => {}
        }

        // Endpoint-specific
        if let Some(endpoint) = classify_endpoint(path) {
            match endpoint {
                TrackedEndpoint::CourseSearch => {
                    if !check_limiter(&self.search_burst, &ip, &mut max_wait) {
                        rejected = true;
                    }
                    if !check_limiter(&self.search_sustained, &ip, &mut max_wait) {
                        rejected = true;
                    }
                    if !check_limiter(&self.search_long, &ip, &mut max_wait) {
                        rejected = true;
                    }
                }
                TrackedEndpoint::Suggest => {
                    if !check_limiter(&self.suggest_burst, &ip, &mut max_wait) {
                        rejected = true;
                    }
                    if !check_limiter(&self.suggest_sustained, &ip, &mut max_wait) {
                        rejected = true;
                    }
                }
                TrackedEndpoint::Timeline => {
                    if !check_limiter(&self.timeline_burst, &ip, &mut max_wait) {
                        rejected = true;
                    }
                    if !check_limiter(&self.timeline_sustained, &ip, &mut max_wait) {
                        rejected = true;
                    }
                    if !check_limiter(&self.timeline_long, &ip, &mut max_wait) {
                        rejected = true;
                    }
                }
            }
        }

        if rejected {
            let secs = max_wait.map(|d| d.as_secs().max(1)).unwrap_or(1);
            Err(secs)
        } else {
            Ok(())
        }
    }

    /// Returns true if the request carries a valid internal bypass token.
    fn is_internal(&self, headers: &http::HeaderMap) -> bool {
        header_str(headers, "x-internal-token").is_some_and(|v| v == self.internal_token)
    }
}

pub type SharedRateLimitState = Arc<RateLimitState>;

// -- Tower Layer + Service --

#[derive(Clone)]
pub struct RateLimitLayer {
    state: SharedRateLimitState,
}

impl RateLimitLayer {
    pub fn new(state: SharedRateLimitState) -> Self {
        Self { state }
    }
}

impl<S> Layer<S> for RateLimitLayer {
    type Service = RateLimitService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        RateLimitService {
            inner,
            state: self.state.clone(),
        }
    }
}

#[derive(Clone)]
pub struct RateLimitService<S> {
    inner: S,
    state: SharedRateLimitState,
}

impl<S, ResBody> Service<Request> for RateLimitService<S>
where
    S: Service<Request, Response = Response<ResBody>> + Send + Clone + 'static,
    S::Future: Send + 'static,
    S::Error: std::fmt::Debug + Send,
    ResBody: Send + 'static,
    Body: Into<ResBody>,
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
        // Internal SSR -> API calls bypass rate limiting entirely.
        if self.state.is_internal(req.headers()) {
            let future = self.inner.call(req);
            return Box::pin(future);
        }

        // Extract client IP from headers (same logic as ClientIp extractor).
        let client_ip = extract_ip_from_headers(req.headers());

        let path = req.uri().path().to_string();

        // TODO: auth tier detection from session cookie -- for now, anonymous.
        let tier = AuthTier::Anonymous;

        match client_ip {
            Some(ip) => match self.state.check(ip, &path, tier) {
                Ok(()) => {
                    let future = self.inner.call(req);
                    Box::pin(future)
                }
                Err(retry_after) => {
                    warn!(
                        client_ip = %ip,
                        path = %path,
                        retry_after_secs = retry_after,
                        "Rate limit exceeded"
                    );
                    let resp = rate_limit_response(retry_after).map(Into::into);
                    Box::pin(async move { Ok(resp) })
                }
            },
            None => {
                // Cannot determine IP -- allow but log.
                let future = self.inner.call(req);
                Box::pin(future)
            }
        }
    }
}

/// Extract client IP from request headers without going through the full
/// Axum extractor system (we're in a Tower middleware, not an Axum handler).
fn extract_ip_from_headers(headers: &http::HeaderMap) -> Option<IpAddr> {
    // CF-Connecting-IP (Cloudflare)
    if let Some(ip) = header_str(headers, "cf-connecting-ip").and_then(|s| s.parse().ok()) {
        return Some(ip);
    }
    // Rightmost X-Forwarded-For (Railway)
    if let Some(xff) = header_str(headers, "x-forwarded-for")
        && let Some(ip) = xff
            .rsplit(',')
            .next()
            .map(str::trim)
            .and_then(|s| s.parse().ok())
    {
        return Some(ip);
    }
    None
}

fn rate_limit_response(retry_after: u64) -> Response<Body> {
    let body = format!(
        r#"{{"code":"RATE_LIMITED","message":"Too many requests. Retry after {} seconds.","details":null}}"#,
        retry_after
    );
    let mut response = Response::new(Body::from(body));
    *response.status_mut() = StatusCode::TOO_MANY_REQUESTS;
    response
        .headers_mut()
        .insert("content-type", HeaderValue::from_static("application/json"));
    response.headers_mut().insert(
        "retry-after",
        HeaderValue::from_str(&retry_after.to_string()).unwrap(),
    );
    response
}
