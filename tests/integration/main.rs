//! Every integration suite, compiled as one test binary.
//!
//! Cargo builds each file directly under tests/ as its own crate, which
//! means linking the whole library once per file. They are modules here
//! so that cost is paid a single time.

mod helpers;

mod admin_audits;
mod admin_rmp;
mod bluebook_aggregates;
mod course_search_alphanumeric;
mod course_search_days;
mod course_search_filters;
mod course_search_sort;
mod db_batch_upsert;
mod db_context_events;
mod db_health;
mod db_scrape_jobs;
mod instructor_cohorts;
mod rmp_matching_corpus;
mod rmp_matching_pipeline;
mod search_accent_handling;
