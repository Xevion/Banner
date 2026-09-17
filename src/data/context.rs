//! Database context with automatic event emission.

use sqlx::PgPool;
use std::sync::Arc;

use super::courses::CourseOps;
use super::events::EventBuffer;
use super::scrape_jobs::ScrapeJobOps;

/// Database context that wraps pool and event buffer.
///
/// All database operations that should emit events go through this context.
#[derive(Clone)]
pub struct DbContext {
    pool: PgPool,
    events: Arc<EventBuffer>,
}

impl DbContext {
    /// Create a new `DbContext`.
    #[must_use]
    pub fn new(pool: PgPool, events: Arc<EventBuffer>) -> Self {
        Self { pool, events }
    }

    /// Get the underlying database pool.
    #[must_use]
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Get the event buffer.
    #[must_use]
    pub fn events(&self) -> &EventBuffer {
        &self.events
    }

    /// Get scrape job operations.
    #[must_use]
    pub fn scrape_jobs(&self) -> ScrapeJobOps<'_> {
        ScrapeJobOps::new(self)
    }

    /// Get course operations.
    #[must_use]
    pub fn courses(&self) -> CourseOps<'_> {
        CourseOps::new(self)
    }
}
