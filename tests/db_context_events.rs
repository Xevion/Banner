//! Integration tests for DbContext event emission.

use banner::data::DbContext;
use banner::data::events::{DomainEvent, EventBuffer};
use banner::data::models::{ScrapePriority, TargetType};
use banner::data::unsigned::Count;
use banner::web::ws::ScrapeJobEvent;
use serde_json::json;
use std::sync::Arc;

mod helpers;

#[sqlx::test]
async fn db_context_emits_event_on_job_lock(pool: sqlx::PgPool) {
    let events = Arc::new(EventBuffer::new(100));
    let ctx = DbContext::new(pool.clone(), events.clone());

    // Insert a test job
    helpers::insert_scrape_job(
        &pool,
        TargetType::Subject,
        json!({ "subject": "CS", "term": "202620" }),
        ScrapePriority::Low,
        false, // not locked
        0,     // retry_count
        3,     // max_retries
    )
    .await;

    let (cursor, _) = events.subscribe();

    // Lock the job via DbContext
    let job = ctx.scrape_jobs().lock_next().await.unwrap();
    assert!(job.is_some(), "Expected to lock a job");

    // Verify event was emitted
    let event = events.read(cursor);
    assert!(
        matches!(
            event,
            Some(DomainEvent::ScrapeJob(ScrapeJobEvent::Locked { .. }))
        ),
        "Expected Locked event, got {:?}",
        event
    );
}

#[sqlx::test]
async fn db_context_emits_event_on_job_complete(pool: sqlx::PgPool) {
    let events = Arc::new(EventBuffer::new(100));
    let ctx = DbContext::new(pool.clone(), events.clone());

    // Insert and lock a job
    let job_id = helpers::insert_scrape_job(
        &pool,
        TargetType::Subject,
        json!({ "subject": "MATH", "term": "202620" }),
        ScrapePriority::Medium,
        false,
        0,
        3,
    )
    .await;

    // Subscribe before any operations to track events from position 0
    let (cursor, _) = events.subscribe();

    // Lock the job first (this emits Locked event at cursor)
    let job = ctx.scrape_jobs().lock_next().await.unwrap();
    assert!(job.is_some());
    assert_eq!(job.unwrap().id, job_id);

    // Complete the job (this emits Completed event at cursor + 1)
    ctx.scrape_jobs().complete(job_id).await.unwrap();

    // Verify Completed event was emitted (at position after Locked)
    let event = events.read(cursor + 1);
    assert!(
        matches!(event, Some(DomainEvent::ScrapeJob(ScrapeJobEvent::Completed { id, .. })) if id == job_id),
        "Expected Completed event for job {}, got {:?}",
        job_id,
        event
    );
}

#[sqlx::test]
async fn db_context_emits_event_on_job_retry(pool: sqlx::PgPool) {
    let events = Arc::new(EventBuffer::new(100));
    let ctx = DbContext::new(pool.clone(), events.clone());

    // Insert and lock a job
    let job_id = helpers::insert_scrape_job(
        &pool,
        TargetType::Subject,
        json!({ "subject": "PHYS", "term": "202620" }),
        ScrapePriority::High,
        false,
        0,
        5,
    )
    .await;

    // Subscribe before any operations to track events from position 0
    let (cursor, _) = events.subscribe();

    // Lock the job (emits Locked event at cursor)
    let job = ctx.scrape_jobs().lock_next().await.unwrap();
    assert!(job.is_some());

    // Retry the job with incremented count (emits Retried event at cursor + 1)
    ctx.scrape_jobs()
        .retry(job_id, Count::new(1), chrono::Utc::now())
        .await
        .unwrap();

    // Verify Retried event was emitted (at position after Locked)
    let event = events.read(cursor + 1);
    assert!(
        matches!(event, Some(DomainEvent::ScrapeJob(ScrapeJobEvent::Retried { id, retry_count, .. })) if id == job_id && retry_count == Count::new(1)),
        "Expected Retried event for job {} with retry_count 1, got {:?}",
        job_id,
        event
    );
}

#[sqlx::test]
async fn db_context_emits_events_on_job_exhaust(pool: sqlx::PgPool) {
    let events = Arc::new(EventBuffer::new(100));
    let ctx = DbContext::new(pool.clone(), events.clone());

    // Insert and lock a job
    let job_id = helpers::insert_scrape_job(
        &pool,
        TargetType::Subject,
        json!({ "subject": "CHEM", "term": "202620" }),
        ScrapePriority::Low,
        false,
        2, // already retried twice
        3, // max 3
    )
    .await;

    // Subscribe before any operations to track events from position 0
    let (cursor, _) = events.subscribe();

    // Lock the job (emits Locked event at cursor)
    let job = ctx.scrape_jobs().lock_next().await.unwrap();
    assert!(job.is_some());

    // Exhaust the job (emits Exhausted at cursor + 1, then Deleted at cursor + 2)
    ctx.scrape_jobs().exhaust(job_id).await.unwrap();

    // Verify Exhausted event was emitted (at position after Locked)
    let event1 = events.read(cursor + 1);
    assert!(
        matches!(event1, Some(DomainEvent::ScrapeJob(ScrapeJobEvent::Exhausted { id })) if id == job_id),
        "Expected Exhausted event for job {}, got {:?}",
        job_id,
        event1
    );

    // Verify Deleted event was also emitted (at position after Exhausted)
    let event2 = events.read(cursor + 2);
    assert!(
        matches!(event2, Some(DomainEvent::ScrapeJob(ScrapeJobEvent::Deleted { id })) if id == job_id),
        "Expected Deleted event for job {}, got {:?}",
        job_id,
        event2
    );
}

#[sqlx::test]
async fn db_context_emits_event_on_job_delete(pool: sqlx::PgPool) {
    let events = Arc::new(EventBuffer::new(100));
    let ctx = DbContext::new(pool.clone(), events.clone());

    // Insert a job
    let job_id = helpers::insert_scrape_job(
        &pool,
        TargetType::Subject,
        json!({ "subject": "BIO", "term": "202620" }),
        ScrapePriority::Low,
        false,
        0,
        3,
    )
    .await;

    let (cursor, _) = events.subscribe();

    // Delete the job directly
    ctx.scrape_jobs().delete(job_id).await.unwrap();

    // Verify Deleted event was emitted
    let event = events.read(cursor);
    assert!(
        matches!(event, Some(DomainEvent::ScrapeJob(ScrapeJobEvent::Deleted { id })) if id == job_id),
        "Expected Deleted event for job {}, got {:?}",
        job_id,
        event
    );
}
