//! Scrape job stream logic.

use sqlx::PgPool;
use std::collections::HashSet;

use crate::web::stream::filters::ScrapeJobsFilter;
use crate::web::ws::{ScrapeJobDto, ScrapeJobEvent};

pub async fn build_snapshot(
    db_pool: &PgPool,
    filter: &ScrapeJobsFilter,
) -> Result<Vec<ScrapeJobDto>, sqlx::Error> {
    let rows = crate::data::scrape_jobs::list_ordered(db_pool, 200)
        .await
        .map_err(|e| sqlx::Error::Protocol(e.to_string()))?;

    let jobs = rows
        .iter()
        .map(ScrapeJobDto::from)
        .filter(|job| matches_filter(filter, job))
        .collect();

    Ok(jobs)
}

pub fn matches_filter(filter: &ScrapeJobsFilter, job: &ScrapeJobDto) -> bool {
    if let Some(ref statuses) = filter.status
        && !statuses.is_empty()
        && !statuses.contains(&job.status)
    {
        return false;
    }

    if let Some(ref priorities) = filter.priority
        && !priorities.is_empty()
        && !priorities.contains(&job.priority)
    {
        return false;
    }

    if let Some(ref target_types) = filter.target_type
        && !target_types.is_empty()
        && !target_types.contains(&job.target_type)
    {
        return false;
    }

    let (term, subject) = payload_term_subject(job);

    if let Some(ref term_filter) = filter.term
        && term.as_deref() != Some(term_filter.as_str())
    {
        return false;
    }

    if let Some(ref subjects) = filter.subject
        && !subjects.is_empty()
    {
        let Some(subject) = subject else {
            return false;
        };
        if !subjects.iter().any(|val| val == &subject) {
            return false;
        }
    }

    true
}

pub async fn event_matches(
    db_pool: &PgPool,
    filter: &ScrapeJobsFilter,
    known_ids: &mut HashSet<i32>,
    event: &ScrapeJobEvent,
    job_details: &mut Option<ScrapeJobDto>,
) -> bool {
    match event {
        ScrapeJobEvent::Created { job } => {
            let matches = matches_filter(filter, job);
            if matches {
                known_ids.insert(job.id);
            }
            matches
        }
        ScrapeJobEvent::Locked { id, .. }
        | ScrapeJobEvent::Retried { id, .. }
        | ScrapeJobEvent::Exhausted { id, .. } => {
            if known_ids.contains(id) {
                true
            } else {
                if job_details.is_none() {
                    *job_details = fetch_by_id(db_pool, *id).await.ok();
                }
                if let Some(job) = job_details.as_ref() {
                    let matches = matches_filter(filter, job);
                    if matches {
                        known_ids.insert(*id);
                    }
                    matches
                } else {
                    false
                }
            }
        }
        ScrapeJobEvent::Completed { id, .. } | ScrapeJobEvent::Deleted { id } => {
            known_ids.remove(id)
        }
    }
}

fn payload_term_subject(job: &ScrapeJobDto) -> (Option<String>, Option<String>) {
    let Some(obj) = job.target_payload.as_object() else {
        return (None, None);
    };

    let term = obj
        .get("term")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let subject = obj
        .get("subject")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    (term, subject)
}

async fn fetch_by_id(db_pool: &PgPool, id: i32) -> Result<ScrapeJobDto, sqlx::Error> {
    let row = crate::data::scrape_jobs::get_by_id(db_pool, id)
        .await
        .map_err(|e| sqlx::Error::Protocol(e.to_string()))?
        .ok_or(sqlx::Error::RowNotFound)?;

    Ok(ScrapeJobDto::from(&row))
}
