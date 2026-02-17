use super::Job;
use crate::banner::{BannerApi, SearchQuery, Term};
use crate::data::DbContext;
use crate::data::models::UpsertCounts;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::debug;

/// Job implementation for scraping subject data.
///
/// The `term` field is optional for backward compatibility with legacy jobs
/// that don't include it. Legacy jobs fall back to `Term::get_current()`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubjectJob {
    pub subject: String,
    /// Term code (e.g., "202510"). If absent, falls back to current term.
    #[serde(default)]
    pub term: Option<String>,
}

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

        let query = SearchQuery::new().subject(subject_code).max_results(500);

        let search_result = banner_api
            .search(&term, &query, "subjectDescription", false)
            .await?;

        let counts = if let Some(courses_from_api) = search_result.data {
            debug!(count = courses_from_api.len(), "Found courses");
            db.courses().batch_upsert(&courses_from_api).await?
        } else {
            UpsertCounts::default()
        };
        Ok(counts)
    }

    fn description(&self) -> String {
        match &self.term {
            Some(t) => format!("Scrape subject: {} (term {})", self.subject, t),
            None => format!("Scrape subject: {} (current term)", self.subject),
        }
    }
}
