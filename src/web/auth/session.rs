//! In-memory caches for session resolution and OAuth CSRF state.

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use rand::Rng;
use sqlx::PgPool;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::data::models::User;

/// Cached session entry with TTL.
#[derive(Debug, Clone)]
struct CachedSession {
    user: User,
    session_expires_at: DateTime<Utc>,
    cached_at: Instant,
}

/// In-memory session cache backed by PostgreSQL.
///
/// Provides fast session resolution without a DB round-trip on every request.
/// Cache entries expire after a configurable TTL (default 5 minutes).
#[derive(Clone)]
pub struct SessionCache {
    cache: Arc<DashMap<String, CachedSession>>,
    db_pool: PgPool,
    cache_ttl: Duration,
}

impl SessionCache {
    /// Create a new session cache with a 5-minute default TTL.
    pub fn new(db_pool: PgPool) -> Self {
        Self {
            cache: Arc::new(DashMap::new()),
            db_pool,
            cache_ttl: Duration::from_secs(5 * 60),
        }
    }

    /// Resolve a session token to a [`User`], using the cache when possible.
    ///
    /// On cache hit (entry present, not stale, session not expired), returns the
    /// cached user immediately. On miss or stale entry, queries the database for
    /// the session and user, populates the cache, and fire-and-forgets a
    /// `touch_session` call to update `last_active_at`.
    pub async fn get_user(&self, token: &str) -> Option<User> {
        // Check cache first
        if let Some(entry) = self.cache.get(token) {
            let now_instant = Instant::now();
            let now_utc = Utc::now();

            let cache_fresh = entry.cached_at + self.cache_ttl > now_instant;
            let session_valid = entry.session_expires_at > now_utc;

            if cache_fresh && session_valid {
                return Some(entry.user.clone());
            }

            // Stale or expired — drop the ref before removing
            drop(entry);
            self.cache.remove(token);
        }

        // Cache miss — query DB
        let session = crate::data::sessions::get_session(&self.db_pool, token)
            .await
            .ok()
            .flatten()?;

        let user = crate::data::users::get_user(&self.db_pool, session.user_id)
            .await
            .ok()
            .flatten()?;

        self.cache.insert(
            token.to_owned(),
            CachedSession {
                user: user.clone(),
                session_expires_at: session.expires_at,
                cached_at: Instant::now(),
            },
        );

        // Fire-and-forget touch to update last_active_at
        let pool = self.db_pool.clone();
        let token_owned = token.to_owned();
        tokio::spawn(async move {
            if let Err(e) = crate::data::sessions::touch_session(&pool, &token_owned).await {
                tracing::warn!(error = %e, "failed to touch session");
            }
        });

        Some(user)
    }

    /// Remove a single session from the cache (e.g. on logout).
    pub fn evict(&self, token: &str) {
        self.cache.remove(token);
    }

    /// Remove all cached sessions belonging to a user.
    pub fn evict_user(&self, discord_id: i64) {
        self.cache
            .retain(|_, entry| entry.user.discord_id != discord_id);
    }

    /// Delete expired sessions from the database and sweep the in-memory cache.
    ///
    /// Returns the number of sessions deleted from the database.
    pub async fn cleanup_expired(&self) -> anyhow::Result<u64> {
        let deleted = crate::data::sessions::cleanup_expired(&self.db_pool).await?;

        let now = Utc::now();
        self.cache.retain(|_, entry| entry.session_expires_at > now);

        Ok(deleted)
    }
}

/// Data stored alongside each OAuth CSRF state token.
struct OAuthStateEntry {
    created_at: Instant,
    /// The browser origin that initiated the login flow, so the callback
    /// can reconstruct the exact redirect_uri Discord expects.
    origin: String,
}

/// Ephemeral store for OAuth CSRF state tokens.
///
/// Tokens are stored with creation time and expire after a configurable TTL.
/// Each token is single-use: validation consumes it.
#[derive(Clone)]
pub struct OAuthStateStore {
    states: Arc<DashMap<String, OAuthStateEntry>>,
    ttl: Duration,
}

impl Default for OAuthStateStore {
    fn default() -> Self {
        Self::new()
    }
}

impl OAuthStateStore {
    /// Create a new store with a 10-minute TTL.
    pub fn new() -> Self {
        Self {
            states: Arc::new(DashMap::new()),
            ttl: Duration::from_secs(10 * 60),
        }
    }

    /// Generate a random 16-byte hex CSRF token, store it with the given
    /// origin, and return the token.
    pub fn generate(&self, origin: String) -> String {
        let bytes: [u8; 16] = rand::rng().random();
        let token: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        self.states.insert(
            token.clone(),
            OAuthStateEntry {
                created_at: Instant::now(),
                origin,
            },
        );
        token
    }

    /// Validate and consume a CSRF token. Returns the stored origin if the
    /// token was present and not expired.
    pub fn validate(&self, state: &str) -> Option<String> {
        let (_, entry) = self.states.remove(state)?;
        if entry.created_at.elapsed() < self.ttl {
            Some(entry.origin)
        } else {
            None
        }
    }

    /// Remove all expired entries from the store.
    #[allow(dead_code)] // Intended for periodic cleanup task (not yet wired)
    pub fn cleanup(&self) {
        let ttl = self.ttl;
        self.states
            .retain(|_, entry| entry.created_at.elapsed() < ttl);
    }
}
