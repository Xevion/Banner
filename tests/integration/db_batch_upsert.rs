use crate::helpers::db::test_db;
use banner::data::batch::batch_upsert_courses;
use sqlx::PgPool;

/// Columns selected by the course verification query:
/// (crn, subject, `course_number`, title, enrollment, `max_enrollment`, `wait_count`, `wait_capacity`)
type CourseRow = (String, String, String, String, i32, i32, i32, i32);

#[tokio::test]
async fn test_batch_upsert_empty_slice() {
    let pool = test_db!().await;
    batch_upsert_courses(&[], &pool).await.unwrap();

    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM courses")
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(count.0, 0);
}

#[tokio::test]
async fn test_batch_upsert_inserts_new_courses() {
    let pool = test_db!().await;
    let courses = vec![
        crate::helpers::make_course("10001", "202510", "CS", "1083", "Intro to CS", (25, 30, 0, 5)),
        crate::helpers::make_course("10002", "202510", "MAT", "1214", "Calculus I", (40, 45, 3, 10)),
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

    let (crn, subject, course_number, title, enrollment, max_enrollment, wait_count, wait_capacity) = &rows[0];
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

#[tokio::test]
async fn test_batch_upsert_updates_existing() {
    let pool = test_db!().await;
    let initial = vec![crate::helpers::make_course(
        "20001",
        "202510",
        "CS",
        "3443",
        "App Programming",
        (10, 35, 0, 5),
    )];
    batch_upsert_courses(&initial, &pool).await.unwrap();

    // Upsert the same CRN+term with updated enrollment
    let updated = vec![crate::helpers::make_course(
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

#[tokio::test]
async fn test_batch_upsert_mixed_insert_and_update() {
    let pool = test_db!().await;
    let initial = vec![
        crate::helpers::make_course("30001", "202510", "CS", "1083", "Intro to CS", (10, 30, 0, 5)),
        crate::helpers::make_course("30002", "202510", "CS", "2073", "Computer Architecture", (20, 30, 0, 5)),
    ];
    batch_upsert_courses(&initial, &pool).await.unwrap();

    // Update both existing courses and add a new one
    let mixed = vec![
        crate::helpers::make_course("30001", "202510", "CS", "1083", "Intro to CS", (15, 30, 1, 5)),
        crate::helpers::make_course("30002", "202510", "CS", "2073", "Computer Architecture", (25, 30, 0, 5)),
        crate::helpers::make_course("30003", "202510", "MAT", "1214", "Calculus I", (40, 45, 3, 10)),
    ];
    batch_upsert_courses(&mixed, &pool).await.unwrap();

    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM courses")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count.0, 3, "should have 2 updated + 1 new = 3 total rows");

    // Verify updated values
    let (enrollment,): (i32,) = sqlx::query_as("SELECT enrollment FROM courses WHERE crn = '30001'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(enrollment, 15);

    let (enrollment,): (i32,) = sqlx::query_as("SELECT enrollment FROM courses WHERE crn = '30002'")
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

#[tokio::test]
async fn test_batch_upsert_unique_constraint_crn_term() {
    let pool = test_db!().await;
    // Same CRN, different term codes -> should produce two separate rows
    let courses = vec![
        crate::helpers::make_course("40001", "202510", "CS", "1083", "Intro to CS", (25, 30, 0, 5)),
        crate::helpers::make_course("40001", "202520", "CS", "1083", "Intro to CS", (10, 30, 0, 5)),
    ];

    batch_upsert_courses(&courses, &pool).await.unwrap();

    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM courses WHERE crn = '40001'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count.0, 2, "same CRN with different term codes should be separate rows");

    let rows: Vec<(String, i32)> =
        sqlx::query_as("SELECT term_code, enrollment FROM courses WHERE crn = '40001' ORDER BY term_code")
            .fetch_all(&pool)
            .await
            .unwrap();

    assert_eq!(rows[0].0, "202510");
    assert_eq!(rows[0].1, 25);
    assert_eq!(rows[1].0, "202520");
    assert_eq!(rows[1].1, 10);
}

#[tokio::test]
async fn test_batch_upsert_creates_audit_and_metric_entries() {
    let pool = test_db!().await;
    // Insert initial data, which should create a baseline metric but no audits
    let initial = vec![crate::helpers::make_course(
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
    assert_eq!(audit_count, 1, "initial insert should create one 'initial' audit entry");

    let (field_changed,): (String,) = sqlx::query_as("SELECT field_changed FROM course_audits LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(field_changed, "initial");

    let (metric_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM course_metrics")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(metric_count, 1, "initial insert should create a baseline metric");

    // Verify baseline metric values
    let (enrollment, wait_count, seats): (i32, i32, i32) =
        sqlx::query_as("SELECT enrollment, wait_count, seats_available FROM course_metrics ORDER BY timestamp LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(enrollment, 10);
    assert_eq!(wait_count, 0);
    assert_eq!(seats, 25); // 35 - 10

    // Update enrollment and wait_count
    let updated = vec![crate::helpers::make_course(
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

#[tokio::test]
async fn test_batch_upsert_no_change_no_audit() {
    let pool = test_db!().await;
    // Insert then re-insert identical data, which should produce baseline metric but no audits or extra metrics
    let course = vec![crate::helpers::make_course(
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

/// One person reached through both UTSA domains must resolve to a single row.
#[tokio::test]
async fn test_upsert_instructors_merges_student_and_staff_domains() {
    let pool = test_db!().await;
    let mut first = crate::helpers::make_course("20001", "202510", "SPN", "1014", "Elementary", (5, 30, 0, 0));
    first.faculty = vec![crate::helpers::make_faculty(
        "Cano, Lilian",
        Some("lilian.cano@my.utsa.edu"),
        20001,
        "202510",
    )];

    let mut second = crate::helpers::make_course("20002", "202520", "SPN", "2013", "Intermediate", (5, 30, 0, 0));
    second.faculty = vec![crate::helpers::make_faculty(
        "Cano, Lilian",
        Some("lilian.cano@utsa.edu"),
        20002,
        "202520",
    )];

    batch_upsert_courses(&[first], &pool).await.unwrap();
    batch_upsert_courses(&[second], &pool).await.unwrap();

    let rows: Vec<(i32, String)> = sqlx::query_as("SELECT id, display_name FROM instructors ORDER BY id")
        .fetch_all(&pool)
        .await
        .unwrap();

    assert_eq!(
        rows.len(),
        1,
        "both domains should resolve to one instructor, got {rows:?}"
    );

    let sections: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM course_instructors WHERE instructor_id = $1")
        .bind(rows[0].0)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(sections.0, 2, "both sections should land on the same record");
}

/// Both spellings arriving in one batch must not collide in the upsert.
#[tokio::test]
async fn test_upsert_instructors_handles_both_domains_in_one_batch() {
    let pool = test_db!().await;
    let mut a = crate::helpers::make_course("20003", "202510", "SPN", "1014", "Elementary", (5, 30, 0, 0));
    a.faculty = vec![crate::helpers::make_faculty(
        "Cano, Lilian",
        Some("lilian.cano@my.utsa.edu"),
        20003,
        "202510",
    )];

    let mut b = crate::helpers::make_course("20004", "202510", "SPN", "2013", "Intermediate", (5, 30, 0, 0));
    b.faculty = vec![crate::helpers::make_faculty(
        "Cano, Lilian",
        Some("lilian.cano@utsa.edu"),
        20004,
        "202510",
    )];

    batch_upsert_courses(&[a, b], &pool).await.unwrap();

    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM instructors")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count.0, 1);
}

/// Distinct accounts that merely share a name must stay separate.
#[tokio::test]
async fn test_upsert_instructors_keeps_distinct_accounts_apart() {
    let pool = test_db!().await;
    let mut a = crate::helpers::make_course("20005", "202510", "BIO", "1404", "Biology", (5, 30, 0, 0));
    a.faculty = vec![crate::helpers::make_faculty(
        "Thompson, Patricia",
        Some("iki700@my.utsa.edu"),
        20005,
        "202510",
    )];

    let mut b = crate::helpers::make_course("20006", "202510", "HIS", "1043", "History", (5, 30, 0, 0));
    b.faculty = vec![crate::helpers::make_faculty(
        "Thompson, Patricia",
        Some("patricia.thompson@utsa.edu"),
        20006,
        "202510",
    )];

    batch_upsert_courses(&[a, b], &pool).await.unwrap();

    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM instructors")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count.0, 2, "different accounts must remain separate records");
}

/// A tombstone speaks only for an identity whose row is gone. Both spellings of
/// one account share a canonical form, so the survivor must not match the
/// tombstone left by its own absorbed twin and stop taking updates.
#[tokio::test]
async fn test_survivor_still_takes_updates_after_absorbing_its_twin() {
    let pool = test_db!().await;
    let mut a = crate::helpers::make_course("20017", "202510", "AST", "1013", "Stars", (5, 30, 0, 0));
    a.faculty = vec![crate::helpers::make_faculty(
        "Fictional, Dara",
        Some("dara.fictional@utsa.edu"),
        20017,
        "202510",
    )];
    batch_upsert_courses(&[a], &pool).await.unwrap();

    let survivor: (i32,) = sqlx::query_as("SELECT id FROM instructors LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();

    sqlx::query(
        "INSERT INTO instructor_merges \
             (survivor_id, absorbed_email, absorbed_display_name, tier) \
         VALUES ($1, 'dara.fictional@my.utsa.edu', 'Fictional, Dara', 'same_account')",
    )
    .bind(survivor.0)
    .execute(&pool)
    .await
    .unwrap();

    let mut renamed = crate::helpers::make_course("20018", "202520", "AST", "1013", "Stars", (5, 30, 0, 0));
    renamed.faculty = vec![crate::helpers::make_faculty(
        "Fictional, Dara Q",
        Some("dara.fictional@utsa.edu"),
        20018,
        "202520",
    )];
    batch_upsert_courses(&[renamed], &pool).await.unwrap();

    let rows: Vec<(i32, String)> = sqlx::query_as("SELECT id, display_name FROM instructors")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(rows.len(), 1, "no new record should appear");
    assert_eq!(
        rows[0].1, "Fictional, Dara Q",
        "the live record must keep taking updates from the scrape"
    );
}

/// A merge is a decision about identity, so a later scrape must honour it
/// rather than recreating the record it absorbed.
#[tokio::test]
async fn test_absorbed_account_is_not_recreated_by_a_later_scrape() {
    let pool = test_db!().await;
    let mut a = crate::helpers::make_course("20007", "202510", "AST", "1013", "Stars", (5, 30, 0, 0));
    a.faculty = vec![crate::helpers::make_faculty(
        "Fictional, Aster",
        Some("abc123@my.utsa.edu"),
        20007,
        "202510",
    )];

    let mut b = crate::helpers::make_course("20008", "202510", "AST", "2013", "Planets", (5, 30, 0, 0));
    b.faculty = vec![crate::helpers::make_faculty(
        "Fictional, Aster",
        Some("aster.fictional@utsa.edu"),
        20008,
        "202510",
    )];

    batch_upsert_courses(&[a, b], &pool).await.unwrap();

    let ids: Vec<(i32, Option<String>)> = sqlx::query_as("SELECT id, email FROM instructors ORDER BY id")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(ids.len(), 2, "distinct accounts start as separate records");

    let survivor = ids[0].0;
    let absorbed = ids[1].0;
    banner::data::instructor_merge::merge_instructors(&pool, survivor, absorbed, None, false)
        .await
        .unwrap();

    // The absorbed address comes back on the next scrape.
    let mut again = crate::helpers::make_course("20009", "202520", "AST", "2013", "Planets", (5, 30, 0, 0));
    again.faculty = vec![crate::helpers::make_faculty(
        "Fictional, Aster",
        Some("aster.fictional@utsa.edu"),
        20009,
        "202520",
    )];
    batch_upsert_courses(&[again], &pool).await.unwrap();

    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM instructors")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count.0, 1, "the merge must survive a rescrape");

    let landed: (i32,) = sqlx::query_as(
        "SELECT ci.instructor_id FROM course_instructors ci \
         JOIN courses c ON c.id = ci.course_id WHERE c.crn = '20009'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(landed.0, survivor, "the new section belongs to the survivor");
}

/// Records with no address are keyed on the display name, and a merge of one
/// has to hold on the same key.
#[tokio::test]
async fn test_absorbed_nameless_record_is_not_recreated() {
    let pool = test_db!().await;
    let mut a = crate::helpers::make_course("20010", "202510", "AST", "1013", "Stars", (5, 30, 0, 0));
    a.faculty = vec![crate::helpers::make_faculty("Fictional, Bryn", None, 20010, "202510")];

    let mut b = crate::helpers::make_course("20011", "202510", "AST", "3013", "Galaxies", (5, 30, 0, 0));
    b.faculty = vec![crate::helpers::make_faculty(
        "Fictional, Bryn Q",
        Some("bryn.fictional@utsa.edu"),
        20011,
        "202510",
    )];

    batch_upsert_courses(&[a, b], &pool).await.unwrap();

    let ids: Vec<(i32,)> = sqlx::query_as("SELECT id FROM instructors WHERE email IS NULL ORDER BY id")
        .fetch_all(&pool)
        .await
        .unwrap();
    let nameless = ids[0].0;
    let survivor: (i32,) = sqlx::query_as("SELECT id FROM instructors WHERE email IS NOT NULL LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();

    banner::data::instructor_merge::merge_instructors(&pool, survivor.0, nameless, None, true)
        .await
        .unwrap();

    let mut again = crate::helpers::make_course("20012", "202520", "AST", "1013", "Stars", (5, 30, 0, 0));
    again.faculty = vec![crate::helpers::make_faculty("Fictional, Bryn", None, 20012, "202520")];
    batch_upsert_courses(&[again], &pool).await.unwrap();

    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM instructors WHERE email IS NULL")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count.0, 0, "the absorbed nameless record must not return");
}

/// Merging a survivor onward must carry what it had already absorbed, or the
/// first decision is quietly dropped.
#[tokio::test]
async fn test_chained_merge_keeps_the_earlier_decision() {
    let pool = test_db!().await;
    let mut a = crate::helpers::make_course("20013", "202510", "AST", "1013", "Stars", (5, 30, 0, 0));
    a.faculty = vec![crate::helpers::make_faculty(
        "Fictional, Cyrus",
        Some("xyz789@my.utsa.edu"),
        20013,
        "202510",
    )];

    let mut b = crate::helpers::make_course("20014", "202510", "AST", "2013", "Planets", (5, 30, 0, 0));
    b.faculty = vec![crate::helpers::make_faculty(
        "Fictional, Cyrus",
        Some("cyrus.fictional@utsa.edu"),
        20014,
        "202510",
    )];

    let mut c = crate::helpers::make_course("20015", "202510", "AST", "3013", "Galaxies", (5, 30, 0, 0));
    c.faculty = vec![crate::helpers::make_faculty(
        "Fictional, Cyrus",
        Some("cyrus.fictional2@utsa.edu"),
        20015,
        "202510",
    )];

    batch_upsert_courses(&[a, b, c], &pool).await.unwrap();

    let ids: Vec<(i32,)> = sqlx::query_as("SELECT id FROM instructors ORDER BY id")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(ids.len(), 3);
    let (first, second, third) = (ids[0].0, ids[1].0, ids[2].0);

    banner::data::instructor_merge::merge_instructors(&pool, second, first, None, false)
        .await
        .unwrap();
    banner::data::instructor_merge::merge_instructors(&pool, third, second, None, false)
        .await
        .unwrap();

    // The address absorbed by the first merge returns on a later scrape.
    let mut again = crate::helpers::make_course("20016", "202520", "AST", "1013", "Stars", (5, 30, 0, 0));
    again.faculty = vec![crate::helpers::make_faculty(
        "Fictional, Cyrus",
        Some("xyz789@my.utsa.edu"),
        20016,
        "202520",
    )];
    batch_upsert_courses(&[again], &pool).await.unwrap();

    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM instructors")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count.0, 1, "both merges must still hold");

    let landed: (i32,) = sqlx::query_as(
        "SELECT ci.instructor_id FROM course_instructors ci \
         JOIN courses c ON c.id = ci.course_id WHERE c.crn = '20016'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(landed.0, third, "the section lands on the final survivor");
}

/// Two records sharing a display name across distinct accounts. The shape
/// review keeps offering until someone records that they are two people.
async fn seed_distinct_namesakes(pool: &PgPool) -> (i32, i32) {
    let mut a = crate::helpers::make_course("20020", "202510", "AST", "1013", "Stars", (5, 30, 0, 0));
    a.faculty = vec![crate::helpers::make_faculty(
        "Fictional, Devan",
        Some("devan.fictional@utsa.edu"),
        20020,
        "202510",
    )];

    let mut b = crate::helpers::make_course("20021", "202510", "AST", "2013", "Planets", (5, 30, 0, 0));
    b.faculty = vec![crate::helpers::make_faculty(
        "Fictional, Devan",
        Some("devan.fictional2@utsa.edu"),
        20021,
        "202510",
    )];

    batch_upsert_courses(&[a, b], pool).await.unwrap();

    let ids: Vec<(i32,)> = sqlx::query_as("SELECT id FROM instructors ORDER BY id")
        .fetch_all(pool)
        .await
        .unwrap();
    assert_eq!(ids.len(), 2, "distinct accounts start as separate records");
    (ids[0].0, ids[1].0)
}

#[tokio::test]
async fn test_dismissed_pair_leaves_the_duplicate_list() {
    use banner::data::instructor_merge::{dismiss_pair, find_dismissed_pairs, find_duplicate_pairs};

    let pool = test_db!().await;
    let (a, b) = seed_distinct_namesakes(&pool).await;
    let before = find_duplicate_pairs(&pool).await.unwrap();
    assert_eq!(before.len(), 1, "the namesakes start out awaiting review");

    dismiss_pair(&pool, a, b, None).await.unwrap();

    let after = find_duplicate_pairs(&pool).await.unwrap();
    assert!(after.is_empty(), "a dismissed pair must not be offered again");

    let dismissed = find_dismissed_pairs(&pool).await.unwrap();
    assert_eq!(dismissed.len(), 1, "review still needs it to offer an undo");
}

/// The decision is about identity, so a later scrape must not resurrect it.
#[tokio::test]
async fn test_dismissal_survives_a_later_scrape() {
    use banner::data::instructor_merge::{dismiss_pair, find_duplicate_pairs};

    let pool = test_db!().await;
    let (a, b) = seed_distinct_namesakes(&pool).await;
    dismiss_pair(&pool, a, b, None).await.unwrap();

    let mut again = crate::helpers::make_course("20022", "202520", "AST", "1013", "Stars", (5, 30, 0, 0));
    again.faculty = vec![crate::helpers::make_faculty(
        "Fictional, Devan",
        Some("devan.fictional@utsa.edu"),
        20022,
        "202520",
    )];
    batch_upsert_courses(&[again], &pool).await.unwrap();

    let pairs = find_duplicate_pairs(&pool).await.unwrap();
    assert!(pairs.is_empty(), "the dismissal must survive a rescrape");
}

#[tokio::test]
async fn test_undismissing_returns_the_pair_to_review() {
    use banner::data::instructor_merge::{dismiss_pair, find_duplicate_pairs, undismiss_pair};

    let pool = test_db!().await;
    let (a, b) = seed_distinct_namesakes(&pool).await;
    dismiss_pair(&pool, a, b, None).await.unwrap();

    // Submitted the other way round, so the stored ordering has to be normalised.
    let removed = undismiss_pair(&pool, b, a).await.unwrap();
    assert!(removed, "the dismissal must be found either way round");

    let pairs = find_duplicate_pairs(&pool).await.unwrap();
    assert_eq!(pairs.len(), 1, "a mistaken dismissal must be reversible");
}

#[tokio::test]
async fn test_dismissing_a_pair_twice_keeps_one_decision() {
    use banner::data::instructor_merge::dismiss_pair;

    let pool = test_db!().await;
    let (a, b) = seed_distinct_namesakes(&pool).await;
    dismiss_pair(&pool, a, b, None).await.unwrap();
    dismiss_pair(&pool, b, a, None).await.unwrap();

    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM instructor_dismissals")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count.0, 1, "argument order must not double the decision");
}

/// A dismissal speaks about two live records, so merging one away must take it.
#[tokio::test]
async fn test_merging_one_side_of_a_dismissed_pair_drops_the_decision() {
    use banner::data::instructor_merge::{dismiss_pair, merge_instructors};

    let pool = test_db!().await;
    let (a, b) = seed_distinct_namesakes(&pool).await;
    dismiss_pair(&pool, a, b, None).await.unwrap();

    merge_instructors(&pool, b, a, None, false).await.unwrap();

    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM instructor_dismissals")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count.0, 0, "no decision may outlive the record it names");
}

/// Merging is irreversible, so two records that do not share a name must not
/// fold together on an id alone.
#[tokio::test]
async fn test_merging_unrelated_records_needs_confirmation() {
    use banner::data::instructor_merge::merge_instructors;

    let pool = test_db!().await;
    let mut a = crate::helpers::make_course("20020", "202510", "MMI", "1013", "Media", (5, 30, 0, 0));
    a.faculty = vec![crate::helpers::make_faculty(
        "Fictional, Esme",
        Some("esme.fictional@utsa.edu"),
        20020,
        "202510",
    )];
    let mut b = crate::helpers::make_course("20021", "202510", "ENG", "1013", "Writing", (5, 30, 0, 0));
    b.faculty = vec![crate::helpers::make_faculty(
        "Fictional, Rafferty",
        Some("rafferty.fictional@utsa.edu"),
        20021,
        "202510",
    )];
    batch_upsert_courses(&[a, b], &pool).await.unwrap();

    let ids: Vec<(i32,)> = sqlx::query_as("SELECT id FROM instructors ORDER BY id")
        .fetch_all(&pool)
        .await
        .unwrap();
    let (first, second) = (ids[0].0, ids[1].0);

    let refused = merge_instructors(&pool, first, second, None, false).await;
    assert!(refused.is_err(), "unrelated records must not merge unasked");

    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM instructors")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count.0, 2, "the refused merge must not have deleted anything");

    merge_instructors(&pool, first, second, None, true)
        .await
        .expect("an explicit confirmation still merges");

    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM instructors")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count.0, 1);
}
