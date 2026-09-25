//! Database models and schema.

pub mod admin_audits;
pub mod admin_bluebook;
pub mod admin_rmp;
pub mod admin_scraper;
pub mod audit;
pub mod batch;
pub mod bluebook;
pub mod cohort;
mod context;
pub mod course_types;
pub mod courses;
pub mod events;
pub mod health;
pub mod instructor_merge;
pub mod instructors;
pub mod kv;
pub mod metrics;
pub mod models;
pub mod names;
pub mod reference;
pub mod reference_types;
pub mod rmp;
pub mod rmp_matching;
pub mod scoring;
pub mod scrape_jobs;
pub mod scraper_stats;
pub mod sessions;
pub mod term_subjects;
pub mod terms;
pub mod unsigned;
pub mod users;
pub mod watches;

pub use context::DbContext;

/// Escape LIKE/ILIKE metacharacters so user input is treated as literal text.
#[must_use]
pub fn escape_like(s: &str) -> String {
    s.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")
}

#[cfg(test)]
mod tests {
    use super::escape_like;

    #[test]
    fn test_escape_like_no_metacharacters() {
        assert_eq!(escape_like("John Smith"), "John Smith");
    }

    #[test]
    fn test_escape_like_percent() {
        assert_eq!(escape_like("100%"), "100\\%");
    }

    #[test]
    fn test_escape_like_underscore() {
        assert_eq!(escape_like("foo_bar"), "foo\\_bar");
    }

    #[test]
    fn test_escape_like_backslash() {
        assert_eq!(escape_like("path\\to"), "path\\\\to");
    }

    #[test]
    fn test_escape_like_all_metacharacters() {
        assert_eq!(escape_like("%_\\"), "\\%\\_\\\\");
    }

    #[test]
    fn test_escape_like_empty_string() {
        assert_eq!(escape_like(""), "");
    }
}
