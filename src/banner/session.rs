//! Session management for Banner API.

use crate::banner::BannerTerm;
use crate::banner::models::Term;
use anyhow::{Context, Result};
use cookie::Cookie;
use dashmap::DashMap;
use rand::distr::{Alphanumeric, SampleString};
use reqwest_middleware::ClientWithMiddleware;
use std::collections::{HashMap, VecDeque};

use crate::utils::fmt_duration;
use std::mem::ManuallyDrop;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, Notify};
use tracing::trace;
use url::Url;

const SESSION_EXPIRY: Duration = Duration::from_secs(25 * 60); // 25 minutes

/// Represents an active anonymous session within the Banner API.
/// Identified by multiple persistent cookies, as well as a client-generated "unique session ID".
#[derive(Debug, Clone)]
pub struct BannerSession {
    // Randomly generated
    pub unique_session_id: String,
    // Timestamp of creation
    created_at: Instant,
    // Timestamp of last activity
    last_activity: Option<Instant>,
    // Cookie values from initial registration page
    jsessionid: String,
    ssb_cookie: String,
}

/// Generates a new session ID mimicking Banner's format
fn generate_session_id() -> String {
    let random_part = Alphanumeric.sample_string(&mut rand::rng(), 5);
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock before UNIX epoch")
        .as_millis();
    format!("{}{}", random_part, timestamp)
}

/// Generates a timestamp-based nonce
pub fn nonce() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock before UNIX epoch")
        .as_millis()
        .to_string()
}

impl BannerSession {
    /// Creates a new session
    pub fn new(unique_session_id: &str, jsessionid: &str, ssb_cookie: &str) -> Self {
        let now = Instant::now();

        Self {
            created_at: now,
            last_activity: None,
            unique_session_id: unique_session_id.to_string(),
            jsessionid: jsessionid.to_string(),
            ssb_cookie: ssb_cookie.to_string(),
        }
    }

    /// Returns the unique session ID
    pub fn id(&self) -> &str {
        &self.unique_session_id
    }

    /// Updates the last activity timestamp
    pub fn touch(&mut self) {
        self.last_activity = Some(Instant::now());
    }

    /// Returns true if the session is expired
    pub fn is_expired(&self) -> bool {
        self.last_activity.unwrap_or(self.created_at).elapsed() > SESSION_EXPIRY
    }

    /// Returns a string used to for the "Cookie" header
    pub fn cookie(&self) -> String {
        format!(
            "JSESSIONID={}; SSB_COOKIE={}",
            self.jsessionid, self.ssb_cookie
        )
    }

    pub fn been_used(&self) -> bool {
        self.last_activity.is_some()
    }

    #[cfg(test)]
    pub(crate) fn new_with_created_at(
        unique_session_id: &str,
        jsessionid: &str,
        ssb_cookie: &str,
        created_at: Instant,
    ) -> Self {
        Self {
            unique_session_id: unique_session_id.to_string(),
            created_at,
            last_activity: None,
            jsessionid: jsessionid.to_string(),
            ssb_cookie: ssb_cookie.to_string(),
        }
    }
}

/// A smart pointer that returns a `BannerSession` to the pool when dropped.
pub struct PooledSession {
    session: ManuallyDrop<BannerSession>,
    pool: Arc<TermPool>,
}

impl Deref for PooledSession {
    type Target = BannerSession;
    fn deref(&self) -> &Self::Target {
        &self.session
    }
}

impl DerefMut for PooledSession {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.session
    }
}

impl Drop for PooledSession {
    fn drop(&mut self) {
        // SAFETY: `drop` is called exactly once by Rust's drop semantics,
        // so `ManuallyDrop::take` is guaranteed to see a valid value.
        let session = unsafe { ManuallyDrop::take(&mut self.session) };
        let pool = self.pool.clone();
        tokio::spawn(async move {
            pool.release(session).await;
        });
    }
}

pub struct TermPool {
    sessions: Mutex<VecDeque<BannerSession>>,
    notifier: Notify,
    is_creating: AtomicBool,
}

/// RAII guard ensuring `is_creating` is reset on drop for cancellation safety.
/// Without this, a cancelled `acquire()` future would leave the flag set permanently,
/// deadlocking all subsequent callers.
struct CreatingGuard(Arc<TermPool>);

impl Drop for CreatingGuard {
    fn drop(&mut self) {
        self.0.is_creating.store(false, Ordering::Release);
        self.0.notifier.notify_waiters();
    }
}

impl TermPool {
    fn new() -> Self {
        Self {
            sessions: Mutex::new(VecDeque::new()),
            notifier: Notify::new(),
            is_creating: AtomicBool::new(false),
        }
    }

    async fn release(&self, session: BannerSession) {
        let id = session.unique_session_id.clone();
        if session.is_expired() {
            trace!(id = id, "Session expired, dropping");
            // Wake up a waiter, as it might need to create a new session
            // if this was the last one.
            self.notifier.notify_one();
            return;
        }

        let mut queue = self.sessions.lock().await;
        queue.push_back(session);
        drop(queue); // Release lock before notifying

        self.notifier.notify_one();
    }
}

pub struct SessionPool {
    sessions: DashMap<Term, Arc<TermPool>>,
    http: ClientWithMiddleware,
    base_url: String,
}

impl SessionPool {
    pub fn new(http: ClientWithMiddleware, base_url: String) -> Self {
        Self {
            sessions: DashMap::new(),
            http,
            base_url,
        }
    }

    /// Acquires a session from the pool.
    /// If no sessions are available, a new one is created on demand,
    /// respecting the global rate limit.
    pub async fn acquire(&self, term: Term) -> Result<PooledSession> {
        let term_pool = self
            .sessions
            .entry(term)
            .or_insert_with(|| Arc::new(TermPool::new()))
            .clone();

        let start = Instant::now();
        let mut waited_for_creation = false;

        loop {
            // Fast path: Try to get an existing, non-expired session.
            {
                let mut queue = term_pool.sessions.lock().await;
                if let Some(session) = queue.pop_front() {
                    if !session.is_expired() {
                        let age = session
                            .last_activity
                            .unwrap_or(session.created_at)
                            .elapsed();
                        trace!(
                            id = session.unique_session_id,
                            age_secs = age.as_secs(),
                            "Reused existing session from pool"
                        );
                        return Ok(PooledSession {
                            session: ManuallyDrop::new(session),
                            pool: Arc::clone(&term_pool),
                        });
                    } else {
                        trace!(id = session.unique_session_id, "Discarded expired session");
                    }
                }
            } // MutexGuard is dropped, lock is released.

            // Slow path: wait for an in-progress creation, or become the creator.
            if term_pool.is_creating.load(Ordering::Acquire) {
                if !waited_for_creation {
                    trace!("Waiting for another task to create session");
                    waited_for_creation = true;
                }
                term_pool.notifier.notified().await;
                continue;
            }

            // CAS to become the designated creator.
            if term_pool
                .is_creating
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
            {
                continue; // Lost the race — loop back and wait.
            }

            // Guard resets is_creating on drop (including cancellation).
            let creating_guard = CreatingGuard(Arc::clone(&term_pool));

            trace!(term = %term, "Creating new session (pool empty)");
            let new_session_result = self.create_session(&term).await;
            drop(creating_guard);

            match new_session_result {
                Ok(new_session) => {
                    let elapsed = start.elapsed();
                    trace!(
                        id = new_session.unique_session_id,
                        elapsed = fmt_duration(elapsed),
                        "Created new session"
                    );
                    return Ok(PooledSession {
                        session: ManuallyDrop::new(new_session),
                        pool: term_pool,
                    });
                }
                Err(e) => {
                    return Err(e.context("Failed to create new session in pool"));
                }
            }
        }
    }

    /// Establishes session cookies by hitting the Banner registration page.
    ///
    /// Returns `(jsessionid, ssb_cookie, cookie_header)` on success.
    /// Validates the HTTP response status and required cookies.
    async fn establish_cookies(&self) -> Result<(String, String, String)> {
        let response = self
            .http
            .get(format!("{}/registration", self.base_url))
            .send()
            .await
            .context("Failed to reach registration page")?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Registration page returned {}",
                response.status()
            ));
        }

        let cookies: HashMap<String, String> = response
            .headers()
            .get_all("Set-Cookie")
            .iter()
            .filter_map(|v| {
                let c = Cookie::parse(v.to_str().ok()?).ok()?;
                Some((c.name().to_string(), c.value().to_string()))
            })
            .collect();

        let jsessionid = cookies
            .get("JSESSIONID")
            .ok_or_else(|| anyhow::anyhow!("JSESSIONID cookie missing"))?
            .clone();
        let ssb_cookie = cookies
            .get("SSB_COOKIE")
            .ok_or_else(|| anyhow::anyhow!("SSB_COOKIE cookie missing"))?
            .clone();
        let cookie_header = format!("JSESSIONID={}; SSB_COOKIE={}", jsessionid, ssb_cookie);

        // Navigate through the required pages to initialize the session
        self.http
            .get(format!("{}/selfServiceMenu/data", self.base_url))
            .header("Cookie", &cookie_header)
            .send()
            .await?
            .error_for_status()
            .context("Failed to get data page")?;

        self.http
            .get(format!("{}/term/termSelection", self.base_url))
            .header("Cookie", &cookie_header)
            .query(&[("mode", "search")])
            .send()
            .await?
            .error_for_status()
            .context("Failed to get term selection page")?;

        Ok((jsessionid, ssb_cookie, cookie_header))
    }

    /// Sets up a complete Banner session for the given term.
    ///
    /// Establishes cookies, verifies the term is available, and selects it.
    async fn create_session(&self, term: &Term) -> Result<BannerSession> {
        trace!(term = %term, "Setting up new Banner session with cookies");

        let (jsessionid, ssb_cookie, cookie_header) = self
            .establish_cookies()
            .await
            .context("Failed to establish session cookies")?;

        let term_code = term.to_string();
        let unique_session_id = generate_session_id();
        self.select_term(&term_code, &unique_session_id, &cookie_header)
            .await?;

        Ok(BannerSession::new(
            &unique_session_id,
            &jsessionid,
            &ssb_cookie,
        ))
    }

    /// Retrieves a list of terms from the Banner API.
    pub async fn get_terms(
        &self,
        search: &str,
        page: i32,
        max_results: i32,
    ) -> Result<Vec<BannerTerm>> {
        if page <= 0 {
            return Err(anyhow::anyhow!("Page must be greater than 0"));
        }

        let url = format!("{}/classSearch/getTerms", self.base_url);
        let params = [
            ("searchTerm", search),
            ("offset", &page.to_string()),
            ("max", &max_results.to_string()),
            ("_", &nonce()),
        ];

        let response = self
            .http
            .get(&url)
            .query(&params)
            .send()
            .await
            .with_context(|| "Failed to get terms".to_string())?;

        let terms: Vec<BannerTerm> = response
            .json()
            .await
            .context("Failed to parse terms response")?;

        Ok(terms)
    }

    /// Selects a term for the current session.
    async fn select_term(
        &self,
        term: &str,
        unique_session_id: &str,
        cookie_header: &str,
    ) -> Result<()> {
        let form_data = [
            ("term", term),
            ("studyPath", ""),
            ("studyPathText", ""),
            ("startDatepicker", ""),
            ("endDatepicker", ""),
            ("uniqueSessionId", unique_session_id),
        ];

        let url = format!("{}/term/search", self.base_url);
        let response = self
            .http
            .post(&url)
            .header("Cookie", cookie_header)
            .query(&[("mode", "search")])
            .form(&form_data)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Failed to select term {}: {}",
                term,
                response.status()
            ));
        }

        #[derive(serde::Deserialize)]
        struct RedirectResponse {
            #[serde(rename = "fwdURL")]
            fwd_url: String,
        }

        let redirect: RedirectResponse = response.json().await?;

        let base_url_path = self
            .base_url
            .parse::<Url>()
            .context("Failed to parse base URL")?
            .path()
            .to_string();
        let non_overlap_redirect =
            redirect
                .fwd_url
                .strip_prefix(&base_url_path)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "Redirect URL '{}' does not start with expected prefix '{}'",
                        redirect.fwd_url,
                        base_url_path
                    )
                })?;

        // Follow the redirect
        let redirect_url = format!("{}{}", self.base_url, non_overlap_redirect);
        let redirect_response = self
            .http
            .get(&redirect_url)
            .header("Cookie", cookie_header)
            .send()
            .await?;

        if !redirect_response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Failed to follow redirect: {}",
                redirect_response.status()
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// Verifies that cancelling `acquire()` mid-session-creation resets `is_creating`,
    /// allowing subsequent callers to proceed rather than deadlocking.
    #[tokio::test]
    async fn test_acquire_not_deadlocked_after_cancellation() {
        use tokio::sync::mpsc;

        let (tx, mut rx) = mpsc::channel::<()>(10);

        // Local server: /registration signals arrival via `tx`, then hangs forever.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let app = axum::Router::new().route(
            "/StudentRegistrationSsb/registration",
            axum::routing::get(move || {
                let tx = tx.clone();
                async move {
                    let _ = tx.send(()).await;
                    std::future::pending::<&str>().await
                }
            }),
        );
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let base_url = format!("http://{}/StudentRegistrationSsb", addr);
        let client = reqwest_middleware::ClientBuilder::new(
            reqwest::Client::builder()
                .timeout(Duration::from_secs(300))
                .build()
                .unwrap(),
        )
        .build();

        let pool = SessionPool::new(client, base_url);
        let term: Term = "202620".parse().unwrap();

        // First acquire: cancel once the request reaches the server.
        tokio::select! {
            _ = pool.acquire(term) => panic!("server hangs — acquire should never complete"),
            _ = rx.recv() => {} // Request arrived; dropping the future simulates timeout cancellation.
        }

        // Second acquire: verify it reaches the server (i.e., is_creating was reset).
        tokio::select! {
            _ = pool.acquire(term) => {}
            result = tokio::time::timeout(Duration::from_secs(5), rx.recv()) => {
                assert!(
                    result.is_ok(),
                    "acquire() deadlocked — is_creating was not reset after cancellation"
                );
            }
        }
    }

    #[test]
    fn test_new_session_creates_session() {
        let session = BannerSession::new("sess-1", "JSID123", "SSB456");
        assert_eq!(session.id(), "sess-1");
    }

    #[test]
    fn test_fresh_session_not_expired() {
        let session = BannerSession::new("sess-1", "JSID123", "SSB456");
        assert!(!session.is_expired());
    }

    #[test]
    fn test_fresh_session_not_been_used() {
        let session = BannerSession::new("sess-1", "JSID123", "SSB456");
        assert!(!session.been_used());
    }

    #[test]
    fn test_touch_marks_used() {
        let mut session = BannerSession::new("sess-1", "JSID123", "SSB456");
        session.touch();
        assert!(session.been_used());
    }

    #[test]
    fn test_touched_session_not_expired() {
        let mut session = BannerSession::new("sess-1", "JSID123", "SSB456");
        session.touch();
        assert!(!session.is_expired());
    }

    #[test]
    fn test_cookie_format() {
        let session = BannerSession::new("sess-1", "JSID123", "SSB456");
        assert_eq!(session.cookie(), "JSESSIONID=JSID123; SSB_COOKIE=SSB456");
    }

    #[test]
    fn test_id_returns_unique_session_id() {
        let session = BannerSession::new("my-unique-id", "JSID123", "SSB456");
        assert_eq!(session.id(), "my-unique-id");
    }

    #[test]
    fn test_expired_session() {
        let session = BannerSession::new_with_created_at(
            "sess-old",
            "JSID123",
            "SSB456",
            Instant::now() - Duration::from_secs(26 * 60),
        );
        assert!(session.is_expired());
    }

    #[test]
    fn test_not_quite_expired_session() {
        let session = BannerSession::new_with_created_at(
            "sess-recent",
            "JSID123",
            "SSB456",
            Instant::now() - Duration::from_secs(24 * 60),
        );
        assert!(!session.is_expired());
    }

    #[test]
    fn test_session_at_expiry_boundary() {
        let session = BannerSession::new_with_created_at(
            "sess-boundary",
            "JSID123",
            "SSB456",
            Instant::now() - Duration::from_secs(25 * 60 + 1),
        );
        assert!(session.is_expired());
    }
}
