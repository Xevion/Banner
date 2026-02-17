#[allow(dead_code)]
mod helpers;

use banner::data::DbContext;
use banner::data::events::EventBuffer;
use banner::data::models::{ScrapePriority, TargetType};
use serde_json::json;
use sqlx::PgPool;
use std::sync::Arc;

fn make_ctx(pool: PgPool) -> DbContext {
    let events = Arc::new(EventBuffer::new(100));
    DbContext::new(pool, events)
}

#[sqlx::test]
async fn lock_next_empty_queue(pool: PgPool) {
    let ctx = make_ctx(pool);
    let result = ctx.scrape_jobs().lock_next().await.unwrap();
    assert!(result.is_none());
}

#[sqlx::test]
async fn lock_next_returns_job_and_sets_locked_at(pool: PgPool) {
    let id = helpers::insert_scrape_job(
        &pool,
        TargetType::Subject,
        json!({"subject": "CS"}),
        ScrapePriority::Medium,
        false,
        0,
        3,
    )
    .await;

    let ctx = make_ctx(pool.clone());
    let job = ctx
        .scrape_jobs()
        .lock_next()
        .await
        .unwrap()
        .expect("should return a job");

    assert_eq!(job.id, id);
    assert!(matches!(job.target_type, TargetType::Subject));
    assert_eq!(job.target_payload, json!({"subject": "CS"}));

    // Verify locked_at was set in the database
    let (locked_at,): (Option<chrono::DateTime<chrono::Utc>>,) =
        sqlx::query_as("SELECT locked_at FROM scrape_jobs WHERE id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(locked_at.is_some(), "locked_at should be set after fetch");
}

#[sqlx::test]
async fn lock_next_skips_locked_jobs(pool: PgPool) {
    helpers::insert_scrape_job(
        &pool,
        TargetType::Subject,
        json!({"subject": "CS"}),
        ScrapePriority::Medium,
        true, // locked
        0,
        3,
    )
    .await;

    let ctx = make_ctx(pool);
    let result = ctx.scrape_jobs().lock_next().await.unwrap();
    assert!(result.is_none(), "locked jobs should be skipped");
}

#[sqlx::test]
async fn lock_next_skips_future_execute_at(pool: PgPool) {
    // Insert a job with execute_at in the future via raw SQL
    sqlx::query(
        "INSERT INTO scrape_jobs (target_type, target_payload, priority, execute_at)
         VALUES ('Subject', '{\"subject\": \"CS\"}', 'Medium', NOW() + INTERVAL '1 hour')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let ctx = make_ctx(pool);
    let result = ctx.scrape_jobs().lock_next().await.unwrap();
    assert!(result.is_none(), "future execute_at jobs should be skipped");
}

#[sqlx::test]
async fn lock_next_priority_desc_ordering(pool: PgPool) {
    // Insert low priority first, then critical
    helpers::insert_scrape_job(
        &pool,
        TargetType::Subject,
        json!({"subject": "LOW"}),
        ScrapePriority::Low,
        false,
        0,
        3,
    )
    .await;

    helpers::insert_scrape_job(
        &pool,
        TargetType::Subject,
        json!({"subject": "CRIT"}),
        ScrapePriority::Critical,
        false,
        0,
        3,
    )
    .await;

    let ctx = make_ctx(pool);
    let job = ctx
        .scrape_jobs()
        .lock_next()
        .await
        .unwrap()
        .expect("should return a job");

    assert_eq!(
        job.target_payload,
        json!({"subject": "CRIT"}),
        "Critical priority should be fetched before Low"
    );
}

#[sqlx::test]
async fn lock_next_execute_at_asc_ordering(pool: PgPool) {
    // Insert an older job and a newer job, both same priority
    sqlx::query(
        "INSERT INTO scrape_jobs (target_type, target_payload, priority, execute_at)
         VALUES ('Subject', '{\"subject\": \"OLDER\"}', 'Medium', NOW() - INTERVAL '2 hours')",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO scrape_jobs (target_type, target_payload, priority, execute_at)
         VALUES ('Subject', '{\"subject\": \"NEWER\"}', 'Medium', NOW() - INTERVAL '1 hour')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let ctx = make_ctx(pool);
    let job = ctx
        .scrape_jobs()
        .lock_next()
        .await
        .unwrap()
        .expect("should return a job");

    assert_eq!(
        job.target_payload,
        json!({"subject": "OLDER"}),
        "Older execute_at should be fetched first"
    );
}

#[sqlx::test]
async fn delete_removes_row(pool: PgPool) {
    let id = helpers::insert_scrape_job(
        &pool,
        TargetType::SingleCrn,
        json!({"crn": "12345"}),
        ScrapePriority::High,
        false,
        0,
        3,
    )
    .await;

    let ctx = make_ctx(pool.clone());
    ctx.scrape_jobs().delete(id).await.unwrap();

    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM scrape_jobs WHERE id = $1")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0, "row should be deleted");
}

#[sqlx::test]
async fn delete_nonexistent_id_no_error(pool: PgPool) {
    let ctx = make_ctx(pool);
    ctx.scrape_jobs().delete(999_999).await.unwrap();
}

#[sqlx::test]
async fn unlock_clears_locked_at(pool: PgPool) {
    let id = helpers::insert_scrape_job(
        &pool,
        TargetType::CrnList,
        json!({"crns": [1, 2, 3]}),
        ScrapePriority::Medium,
        true, // locked
        0,
        3,
    )
    .await;

    let ctx = make_ctx(pool.clone());
    ctx.scrape_jobs().unlock(id).await.unwrap();

    let (locked_at,): (Option<chrono::DateTime<chrono::Utc>>,) =
        sqlx::query_as("SELECT locked_at FROM scrape_jobs WHERE id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(locked_at.is_none(), "locked_at should be cleared");
}

#[sqlx::test]
async fn find_existing_payloads_returns_matching(pool: PgPool) {
    let payload_a = json!({"subject": "CS"});
    let payload_b = json!({"subject": "MAT"});
    let payload_c = json!({"subject": "ENG"});

    // Insert A and B as Subject jobs
    helpers::insert_scrape_job(
        &pool,
        TargetType::Subject,
        payload_a.clone(),
        ScrapePriority::Medium,
        false,
        0,
        3,
    )
    .await;
    helpers::insert_scrape_job(
        &pool,
        TargetType::Subject,
        payload_b.clone(),
        ScrapePriority::Medium,
        false,
        0,
        3,
    )
    .await;
    // Insert C as a different target type
    helpers::insert_scrape_job(
        &pool,
        TargetType::SingleCrn,
        payload_c.clone(),
        ScrapePriority::Medium,
        false,
        0,
        3,
    )
    .await;

    let candidates = vec![payload_a.clone(), payload_b.clone(), payload_c.clone()];
    let ctx = make_ctx(pool);
    let existing = ctx
        .scrape_jobs()
        .find_existing_payloads(TargetType::Subject, &candidates)
        .await
        .unwrap();

    assert!(existing.contains(&payload_a.to_string()));
    assert!(existing.contains(&payload_b.to_string()));
    // payload_c is SingleCrn, not Subject — should not match
    assert!(!existing.contains(&payload_c.to_string()));
}

#[sqlx::test]
async fn find_existing_payloads_includes_locked(pool: PgPool) {
    let payload = json!({"subject": "CS"});

    helpers::insert_scrape_job(
        &pool,
        TargetType::Subject,
        payload.clone(),
        ScrapePriority::Medium,
        true, // locked
        0,
        3,
    )
    .await;

    let candidates = vec![payload.clone()];
    let ctx = make_ctx(pool);
    let existing = ctx
        .scrape_jobs()
        .find_existing_payloads(TargetType::Subject, &candidates)
        .await
        .unwrap();

    assert!(
        existing.contains(&payload.to_string()),
        "locked jobs should be included in deduplication"
    );
}

#[sqlx::test]
async fn find_existing_payloads_empty_candidates(pool: PgPool) {
    // Insert a job so the table isn't empty
    helpers::insert_scrape_job(
        &pool,
        TargetType::Subject,
        json!({"subject": "CS"}),
        ScrapePriority::Medium,
        false,
        0,
        3,
    )
    .await;

    let ctx = make_ctx(pool);
    let existing = ctx
        .scrape_jobs()
        .find_existing_payloads(TargetType::Subject, &[])
        .await
        .unwrap();

    assert!(
        existing.is_empty(),
        "empty candidates should return empty result"
    );
}

#[sqlx::test]
async fn batch_insert_inserts_multiple(pool: PgPool) {
    let jobs = vec![
        (
            json!({"subject": "CS"}),
            TargetType::Subject,
            ScrapePriority::High,
        ),
        (
            json!({"subject": "MAT"}),
            TargetType::Subject,
            ScrapePriority::Medium,
        ),
        (
            json!({"crn": "12345"}),
            TargetType::SingleCrn,
            ScrapePriority::Low,
        ),
    ];

    let ctx = make_ctx(pool.clone());
    ctx.scrape_jobs().batch_insert(&jobs).await.unwrap();

    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM scrape_jobs")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 3);
}

#[sqlx::test]
async fn batch_insert_empty_slice(pool: PgPool) {
    let ctx = make_ctx(pool.clone());
    ctx.scrape_jobs().batch_insert(&[]).await.unwrap();

    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM scrape_jobs")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}
