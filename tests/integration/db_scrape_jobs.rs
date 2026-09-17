use crate::helpers::db::test_db;
use banner::data::DbContext;
use banner::data::events::EventBuffer;
use banner::data::models::{ScrapePriority, SubjectTarget, TargetPayload, TargetType};
use serde_json::json;
use sqlx::PgPool;
use std::sync::Arc;

fn make_ctx(pool: PgPool) -> DbContext {
    let events = Arc::new(EventBuffer::new(100));
    DbContext::new(pool, events)
}

fn subject_payload(subject: &str) -> TargetPayload {
    TargetPayload::Subject(SubjectTarget {
        subject: subject.to_string(),
        term: None,
    })
}

/// The exact JSON a typed payload is stored as, for direct SQL inserts and dedup assertions.
fn payload_json(payload: &TargetPayload) -> serde_json::Value {
    serde_json::to_value(payload).unwrap()
}

#[tokio::test]
async fn lock_next_empty_queue() {
    let pool = test_db!().await;
    let ctx = make_ctx(pool);
    let result = ctx.scrape_jobs().lock_next().await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn lock_next_returns_job_and_sets_locked_at() {
    let pool = test_db!().await;
    let id = crate::helpers::insert_scrape_job(
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
    assert_eq!(job.target_payload.subject(), Some("CS"));

    // Verify locked_at was set in the database
    let (locked_at,): (Option<chrono::DateTime<chrono::Utc>>,) =
        sqlx::query_as("SELECT locked_at FROM scrape_jobs WHERE id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(locked_at.is_some(), "locked_at should be set after fetch");
}

#[tokio::test]
async fn lock_next_skips_locked_jobs() {
    let pool = test_db!().await;
    crate::helpers::insert_scrape_job(
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

#[tokio::test]
async fn lock_next_skips_future_execute_at() {
    let pool = test_db!().await;
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

#[tokio::test]
async fn lock_next_priority_desc_ordering() {
    let pool = test_db!().await;
    // Insert low priority first, then critical
    crate::helpers::insert_scrape_job(
        &pool,
        TargetType::Subject,
        json!({"subject": "LOW"}),
        ScrapePriority::Low,
        false,
        0,
        3,
    )
    .await;

    crate::helpers::insert_scrape_job(
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
        job.target_payload.subject(),
        Some("CRIT"),
        "Critical priority should be fetched before Low"
    );
}

#[tokio::test]
async fn lock_next_execute_at_asc_ordering() {
    let pool = test_db!().await;
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
        job.target_payload.subject(),
        Some("OLDER"),
        "Older execute_at should be fetched first"
    );
}

#[tokio::test]
async fn delete_removes_row() {
    let pool = test_db!().await;
    let id = crate::helpers::insert_scrape_job(
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

#[tokio::test]
async fn delete_nonexistent_id_no_error() {
    let pool = test_db!().await;
    let ctx = make_ctx(pool);
    ctx.scrape_jobs().delete(999_999).await.unwrap();
}

#[tokio::test]
async fn unlock_clears_locked_at() {
    let pool = test_db!().await;
    let id = crate::helpers::insert_scrape_job(
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

#[tokio::test]
async fn find_existing_payloads_returns_matching() {
    let pool = test_db!().await;
    let payload_a = subject_payload("CS");
    let payload_b = subject_payload("MAT");
    let payload_c = subject_payload("ENG");

    // Insert A and B as Subject jobs
    crate::helpers::insert_scrape_job(
        &pool,
        TargetType::Subject,
        payload_json(&payload_a),
        ScrapePriority::Medium,
        false,
        0,
        3,
    )
    .await;
    crate::helpers::insert_scrape_job(
        &pool,
        TargetType::Subject,
        payload_json(&payload_b),
        ScrapePriority::Medium,
        false,
        0,
        3,
    )
    .await;
    // Insert C as a different target type
    crate::helpers::insert_scrape_job(
        &pool,
        TargetType::SingleCrn,
        payload_json(&payload_c),
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

    assert!(existing.contains(&payload_json(&payload_a).to_string()));
    assert!(existing.contains(&payload_json(&payload_b).to_string()));
    // payload_c is SingleCrn, not Subject, so it should not match
    assert!(!existing.contains(&payload_json(&payload_c).to_string()));
}

#[tokio::test]
async fn find_existing_payloads_includes_locked() {
    let pool = test_db!().await;
    let payload = subject_payload("CS");

    crate::helpers::insert_scrape_job(
        &pool,
        TargetType::Subject,
        payload_json(&payload),
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
        existing.contains(&payload_json(&payload).to_string()),
        "locked jobs should be included in deduplication"
    );
}

#[tokio::test]
async fn find_existing_payloads_empty_candidates() {
    let pool = test_db!().await;
    // Insert a job so the table isn't empty
    crate::helpers::insert_scrape_job(
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

    assert!(existing.is_empty(), "empty candidates should return empty result");
}

#[tokio::test]
async fn batch_insert_inserts_multiple() {
    let pool = test_db!().await;
    let jobs = vec![
        (subject_payload("CS"), TargetType::Subject, ScrapePriority::High),
        (subject_payload("MAT"), TargetType::Subject, ScrapePriority::Medium),
        (
            TargetPayload::SingleCrn(banner::data::models::SingleCrnTarget {
                crn: "12345".to_string(),
                term: None,
            }),
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

#[tokio::test]
async fn batch_insert_empty_slice() {
    let pool = test_db!().await;
    let ctx = make_ctx(pool.clone());
    ctx.scrape_jobs().batch_insert(&[]).await.unwrap();

    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM scrape_jobs")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn test_queue_depth_empty_queue_returns_zero() {
    let pool = test_db!().await;
    let depth = banner::data::scrape_jobs::queue_depth(&pool).await.unwrap();
    assert_eq!(depth.count, 0);
    assert_eq!(depth.oldest_seconds, None);
}

#[tokio::test]
async fn test_queue_depth_counts_ready_job_and_ages_it() {
    let pool = test_db!().await;
    sqlx::query(
        "INSERT INTO scrape_jobs (target_type, target_payload, priority, execute_at, queued_at)
         VALUES ('Subject', '{\"subject\": \"CS\"}', 'Medium', NOW(), NOW() - INTERVAL '90 seconds')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let depth = banner::data::scrape_jobs::queue_depth(&pool).await.unwrap();
    assert_eq!(depth.count, 1);
    let oldest_seconds = depth.oldest_seconds.expect("expected an oldest queued job");
    assert!(oldest_seconds >= 89.0, "expected age near 90s, got {oldest_seconds}");
}

#[tokio::test]
async fn test_queue_depth_excludes_locked_job() {
    let pool = test_db!().await;
    crate::helpers::insert_scrape_job(
        &pool,
        TargetType::Subject,
        json!({"subject": "CS"}),
        ScrapePriority::Medium,
        true, // locked
        0,
        3,
    )
    .await;

    let depth = banner::data::scrape_jobs::queue_depth(&pool).await.unwrap();
    assert_eq!(depth.count, 0, "actively locked jobs are not backlog");
}

#[tokio::test]
async fn test_queue_depth_excludes_future_execute_at() {
    let pool = test_db!().await;
    sqlx::query(
        "INSERT INTO scrape_jobs (target_type, target_payload, priority, execute_at)
         VALUES ('Subject', '{\"subject\": \"CS\"}', 'Medium', NOW() + INTERVAL '1 hour')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let depth = banner::data::scrape_jobs::queue_depth(&pool).await.unwrap();
    assert_eq!(depth.count, 0, "not-yet-due jobs are not backlog");
}
