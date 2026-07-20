use super::Job;
use crate::banner::{BannerApi, SearchQuery, Term};
use crate::data::DbContext;
use crate::data::models::UpsertCounts;
use anyhow::Result;
use tracing::debug;

/// Job implementation for scraping subject data.
///
/// The job is the stored payload itself; legacy rows without a `term` fall back
/// to `Term::get_current()`.
pub use crate::data::models::SubjectTarget as SubjectJob;

impl SubjectJob {
    /// Create a new subject job for a specific term.
    pub fn new(subject: String, term: String) -> Self {
        Self {
            subject,
            term: Some(term),
        }
    }

    /// Get the effective term, falling back to current term for legacy jobs.
    pub fn effective_term(&self) -> String {
        self.term
            .clone()
            .unwrap_or_else(|| Term::get_current().inner().to_string())
    }
}

#[async_trait::async_trait]
impl Job for SubjectJob {
    #[tracing::instrument(skip(self, banner_api, db), fields(subject = %self.subject, term))]
    async fn process(&self, banner_api: &BannerApi, db: &DbContext) -> Result<UpsertCounts> {
        let subject_code = &self.subject;
        let term = self.effective_term();

        tracing::Span::current().record("term", term.as_str());

        let query = SearchQuery::new().subject(subject_code);

        let courses = banner_api
            .search_all(&term, &query, "subjectDescription", false)
            .await?;

        debug!(count = courses.len(), "Found courses");
        let counts = db.courses().batch_upsert(&courses).await?;
        Ok(counts)
    }
}
