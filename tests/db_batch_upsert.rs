mod helpers;

use banner::data::batch::batch_upsert_courses;
use sqlx::PgPool;

/// Columns selected by the course verification query:
/// (crn, subject, course_number, title, enrollment, max_enrollment, wait_count, wait_capacity)
type CourseRow = (String, String, String, String, i32, i32, i32, i32);

#[sqlx::test]
async fn test_batch_upsert_empty_slice(pool: PgPool) {
    batch_upsert_courses(&[], &pool).await.unwrap();

    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM courses")
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(count.0, 0);
}

#[sqlx::test]
async fn test_batch_upsert_inserts_new_courses(pool: PgPool) {
    let courses = vec![
        helpers::make_course(
            "10001",
            "202510",
            "CS",
            "1083",
            "Intro to CS",
            (25, 30, 0, 5),
        ),
        helpers::make_course(
            "10002",
            "202510",
            "MAT",
            "1214",
            "Calculus I",
            (40, 45, 3, 10),
        ),
    ];

    batch_upsert_courses(&courses, &pool).await.unwrap();

    let rows: Vec<CourseRow> = sqlx::query_as(
        "SELECT crn, subject, course_number, title, enrollment, max_enrollment, wait_count, wait_capacity
         FROM courses ORDER BY crn",
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(rows.len(), 2);

    let (crn, subject, course_number, title, enrollment, max_enrollment, wait_count, wait_capacity) =
        &rows[0];
    assert_eq!(crn, "10001");
    assert_eq!(subject, "CS");
    assert_eq!(course_number, "1083");
    assert_eq!(title, "Intro to CS");
    assert_eq!(*enrollment, 25);
    assert_eq!(*max_enrollment, 30);
    assert_eq!(*wait_count, 0);
    assert_eq!(*wait_capacity, 5);

    let (crn, subject, ..) = &rows[1];
    assert_eq!(crn, "10002");
    assert_eq!(subject, "MAT");
}

#[sqlx::test]
async fn test_batch_upsert_updates_existing(pool: PgPool) {
    let initial = vec![helpers::make_course(
        "20001",
        "202510",
        "CS",
        "3443",
        "App Programming",
        (10, 35, 0, 5),
    )];
    batch_upsert_courses(&initial, &pool).await.unwrap();

    // Upsert the same CRN+term with updated enrollment
    let updated = vec![helpers::make_course(
        "20001",
        "202510",
        "CS",
        "3443",
        "App Programming",
        (30, 35, 2, 5),
    )];
    batch_upsert_courses(&updated, &pool).await.unwrap();

    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM courses")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count.0, 1, "upsert should not create a duplicate row");

    let (enrollment, wait_count): (i32, i32) =
        sqlx::query_as("SELECT enrollment, wait_count FROM courses WHERE crn = '20001'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(enrollment, 30);
    assert_eq!(wait_count, 2);
}

#[sqlx::test]
async fn test_batch_upsert_mixed_insert_and_update(pool: PgPool) {
    let initial = vec![
        helpers::make_course(
            "30001",
            "202510",
            "CS",
            "1083",
            "Intro to CS",
            (10, 30, 0, 5),
        ),
        helpers::make_course(
            "30002",
            "202510",
            "CS",
            "2073",
            "Computer Architecture",
            (20, 30, 0, 5),
        ),
    ];
    batch_upsert_courses(&initial, &pool).await.unwrap();

    // Update both existing courses and add a new one
    let mixed = vec![
        helpers::make_course(
            "30001",
            "202510",
            "CS",
            "1083",
            "Intro to CS",
            (15, 30, 1, 5),
        ),
        helpers::make_course(
            "30002",
            "202510",
            "CS",
            "2073",
            "Computer Architecture",
            (25, 30, 0, 5),
        ),
        helpers::make_course(
            "30003",
            "202510",
            "MAT",
            "1214",
            "Calculus I",
            (40, 45, 3, 10),
        ),
    ];
    batch_upsert_courses(&mixed, &pool).await.unwrap();

    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM courses")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count.0, 3, "should have 2 updated + 1 new = 3 total rows");

    // Verify updated values
    let (enrollment,): (i32,) =
        sqlx::query_as("SELECT enrollment FROM courses WHERE crn = '30001'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(enrollment, 15);

    let (enrollment,): (i32,) =
        sqlx::query_as("SELECT enrollment FROM courses WHERE crn = '30002'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(enrollment, 25);

    // Verify new row
    let (subject,): (String,) = sqlx::query_as("SELECT subject FROM courses WHERE crn = '30003'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(subject, "MAT");
}

#[sqlx::test]
async fn test_batch_upsert_unique_constraint_crn_term(pool: PgPool) {
    // Same CRN, different term codes -> should produce two separate rows
    let courses = vec![
        helpers::make_course(
            "40001",
            "202510",
            "CS",
            "1083",
            "Intro to CS",
            (25, 30, 0, 5),
        ),
        helpers::make_course(
            "40001",
            "202520",
            "CS",
            "1083",
            "Intro to CS",
            (10, 30, 0, 5),
        ),
    ];

    batch_upsert_courses(&courses, &pool).await.unwrap();

    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM courses WHERE crn = '40001'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        count.0, 2,
        "same CRN with different term codes should be separate rows"
    );

    let rows: Vec<(String, i32)> = sqlx::query_as(
        "SELECT term_code, enrollment FROM courses WHERE crn = '40001' ORDER BY term_code",
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(rows[0].0, "202510");
    assert_eq!(rows[0].1, 25);
    assert_eq!(rows[1].0, "202520");
    assert_eq!(rows[1].1, 10);
}

#[sqlx::test]
async fn test_batch_upsert_creates_audit_and_metric_entries(pool: PgPool) {
    // Insert initial data -- should create a baseline metric but no audits
    let initial = vec![helpers::make_course(
        "50001",
        "202510",
        "CS",
        "3443",
        "App Programming",
        (10, 35, 0, 5),
    )];
    batch_upsert_courses(&initial, &pool).await.unwrap();

    let (audit_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM course_audits")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        audit_count, 1,
        "initial insert should create one 'initial' audit entry"
    );

    let (field_changed,): (String,) =
        sqlx::query_as("SELECT field_changed FROM course_audits LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(field_changed, "initial");

    let (metric_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM course_metrics")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        metric_count, 1,
        "initial insert should create a baseline metric"
    );

    // Verify baseline metric values
    let (enrollment, wait_count, seats): (i32, i32, i32) = sqlx::query_as(
        "SELECT enrollment, wait_count, seats_available FROM course_metrics ORDER BY timestamp LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(enrollment, 10);
    assert_eq!(wait_count, 0);
    assert_eq!(seats, 25); // 35 - 10

    // Update enrollment and wait_count
    let updated = vec![helpers::make_course(
        "50001",
        "202510",
        "CS",
        "3443",
        "App Programming",
        (20, 35, 2, 5),
    )];
    batch_upsert_courses(&updated, &pool).await.unwrap();

    // Should have audit entries: 1 initial + at least 2 change entries (enrollment, wait_count)
    let (audit_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM course_audits")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(
        audit_count >= 3,
        "should have 1 initial + audit entries for enrollment and wait_count changes, got {audit_count}"
    );

    // Should have 2 metric entries: baseline + change
    let (metric_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM course_metrics")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(metric_count, 2, "should have baseline + 1 change metric");

    // Verify the latest metric values
    let (enrollment, wait_count, seats): (i32, i32, i32) = sqlx::query_as(
        "SELECT enrollment, wait_count, seats_available FROM course_metrics ORDER BY timestamp DESC LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(enrollment, 20);
    assert_eq!(wait_count, 2);
    assert_eq!(seats, 15); // 35 - 20
}

#[sqlx::test]
async fn test_batch_upsert_no_change_no_audit(pool: PgPool) {
    // Insert then re-insert identical data -- should produce baseline metric but no audits or extra metrics
    let course = vec![helpers::make_course(
        "60001",
        "202510",
        "CS",
        "1083",
        "Intro to CS",
        (25, 30, 0, 5),
    )];
    batch_upsert_courses(&course, &pool).await.unwrap();
    batch_upsert_courses(&course, &pool).await.unwrap();

    let (audit_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM course_audits")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        audit_count, 1,
        "should have only the initial insert audit entry, no change audits"
    );

    let (metric_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM course_metrics")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        metric_count, 1,
        "identical re-upsert should only have the baseline metric"
    );
}
