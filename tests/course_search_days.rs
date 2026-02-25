//! Test day-of-week filtering in `search_courses`.
//!
//! The SQL day filter uses AND semantics: a meeting must contain ALL requested
//! days for the course to match. A course matches if ANY of its meetings
//! satisfies the filter.

mod helpers;

use banner::data::batch::batch_upsert_courses;
use banner::data::courses::search_courses;
use helpers::{MeetingTimeBuilder, make_course, with_meetings};
use sqlx::PgPool;

/// Insert the standard dataset used by all day-filter tests.
///
/// | CRN   | Subject | Number | Meeting days              |
/// |-------|---------|--------|---------------------------|
/// | 10001 | CS      | 1100   | MWF 09:00-09:50           |
/// | 10002 | CS      | 2200   | TTh 10:30-11:45           |
/// | 10003 | MATH    | 1300   | MW 14:00-15:15            |
/// | 10004 | ART     | 2100   | Mon only 13:30-16:15      |
/// | 10005 | MUS     | 1050   | Sat only 09:00-12:00      |
/// | 10006 | ENG     | 1010   | No meetings (TBA)         |
/// | 10007 | PHYS    | 1600   | MWF 08:00-08:50 + TTh 10:00-11:15 |
async fn insert_test_courses(pool: &PgPool) {
    let term = "202620";

    let courses = vec![
        // MWF 09:00-09:50
        with_meetings(
            make_course("10001", term, "CS", "1100", "Intro to CS", (20, 30, 0, 10)),
            vec![
                MeetingTimeBuilder::new()
                    .days([true, false, true, false, true, false, false])
                    .time("0900", "0950")
                    .location("SCI", "101")
                    .build(),
            ],
        ),
        // TTh 10:30-11:45
        with_meetings(
            make_course(
                "10002",
                term,
                "CS",
                "2200",
                "Data Structures",
                (25, 30, 0, 10),
            ),
            vec![
                MeetingTimeBuilder::new()
                    .days([false, true, false, true, false, false, false])
                    .time("1030", "1145")
                    .location("SCI", "202")
                    .build(),
            ],
        ),
        // MW 14:00-15:15
        with_meetings(
            make_course("10003", term, "MATH", "1300", "Calculus I", (30, 35, 0, 5)),
            vec![
                MeetingTimeBuilder::new()
                    .days([true, false, true, false, false, false, false])
                    .time("1400", "1515")
                    .location("MATH", "300")
                    .build(),
            ],
        ),
        // Monday only 13:30-16:15
        with_meetings(
            make_course("10004", term, "ART", "2100", "Studio Art", (15, 20, 0, 5)),
            vec![
                MeetingTimeBuilder::new()
                    .days([true, false, false, false, false, false, false])
                    .time("1330", "1615")
                    .location("ART", "110")
                    .build(),
            ],
        ),
        // Saturday only 09:00-12:00
        with_meetings(
            make_course(
                "10005",
                term,
                "MUS",
                "1050",
                "Music Appreciation",
                (40, 50, 0, 10),
            ),
            vec![
                MeetingTimeBuilder::new()
                    .days([false, false, false, false, false, true, false])
                    .time("0900", "1200")
                    .location("MUS", "100")
                    .build(),
            ],
        ),
        // No meetings (TBA)
        make_course(
            "10006",
            term,
            "ENG",
            "1010",
            "English Composition",
            (25, 30, 0, 10),
        ),
        // Two separate meetings: MWF 08:00-08:50 and TTh 10:00-11:15
        with_meetings(
            make_course("10007", term, "PHYS", "1600", "Physics I", (28, 35, 0, 5)),
            vec![
                MeetingTimeBuilder::new()
                    .days([true, false, true, false, true, false, false])
                    .time("0800", "0850")
                    .location("SCI", "400")
                    .build(),
                MeetingTimeBuilder::new()
                    .days([false, true, false, true, false, false, false])
                    .time("1000", "1115")
                    .location("SCI", "401")
                    .build(),
            ],
        ),
    ];

    batch_upsert_courses(&courses, pool)
        .await
        .expect("Failed to insert test courses");
}

/// Helper: run `search_courses` with only the `days` filter set.
async fn search_by_days(pool: &PgPool, days: Option<&[String]>) -> (Vec<String>, i64) {
    let (results, total) = search_courses(
        pool, "202620", None,  // subject
        None,  // title_query
        None,  // course_number_low
        None,  // course_number_high
        false, // open_only
        None,  // instructional_method
        None,  // campus
        None,  // wait_count_max
        days,  // days
        None,  // time_start
        None,  // time_end
        None,  // part_of_term
        None,  // attributes
        None,  // credit_hour_min
        None,  // credit_hour_max
        None,  // instructor
        100,   // limit
        0,     // offset
        None,  // sort_by
        None,  // sort_dir
    )
    .await
    .expect("search_courses failed");

    let crns: Vec<String> = results.iter().map(|c| c.crn.clone()).collect();
    (crns, total)
}

#[sqlx::test]
async fn test_filter_single_day_monday(pool: PgPool) {
    insert_test_courses(&pool).await;

    let days = vec!["monday".to_owned()];
    let (crns, total) = search_by_days(&pool, Some(&days)).await;

    assert_eq!(
        total, 4,
        "Expected 4 courses with Monday meetings, got {total}"
    );
    assert_eq!(crns.len(), 4, "Result count should match total: {crns:?}");
    assert!(
        crns.contains(&"10001".to_owned()),
        "MWF course should match Monday"
    );
    assert!(
        crns.contains(&"10003".to_owned()),
        "MW course should match Monday"
    );
    assert!(
        crns.contains(&"10004".to_owned()),
        "Monday-only course should match"
    );
    assert!(
        crns.contains(&"10007".to_owned()),
        "PHYS with MWF meeting should match Monday"
    );
    assert!(
        !crns.contains(&"10002".to_owned()),
        "TTh course should not match Monday"
    );
    assert!(
        !crns.contains(&"10005".to_owned()),
        "Saturday course should not match Monday"
    );
    assert!(
        !crns.contains(&"10006".to_owned()),
        "TBA course should not match Monday"
    );
}

#[sqlx::test]
async fn test_filter_single_day_saturday(pool: PgPool) {
    insert_test_courses(&pool).await;

    let days = vec!["saturday".to_owned()];
    let (crns, total) = search_by_days(&pool, Some(&days)).await;

    assert_eq!(
        total, 1,
        "Expected 1 course with Saturday meeting, got {total}"
    );
    assert_eq!(crns.len(), 1, "Result count should match total: {crns:?}");
    assert!(
        crns.contains(&"10005".to_owned()),
        "Saturday-only course should match"
    );
}

#[sqlx::test]
async fn test_filter_multi_day_and_semantics(pool: PgPool) {
    insert_test_courses(&pool).await;

    let days = vec![
        "monday".to_owned(),
        "wednesday".to_owned(),
        "friday".to_owned(),
    ];
    let (crns, total) = search_by_days(&pool, Some(&days)).await;

    assert_eq!(
        total, 2,
        "Expected 2 courses with MWF meetings, got {total}"
    );
    assert_eq!(crns.len(), 2, "Result count should match total: {crns:?}");
    assert!(
        crns.contains(&"10001".to_owned()),
        "MWF course should match MWF filter"
    );
    assert!(
        crns.contains(&"10007".to_owned()),
        "PHYS with MWF meeting should match MWF filter"
    );
    assert!(
        !crns.contains(&"10003".to_owned()),
        "MW course should NOT match MWF -- missing friday"
    );
    assert!(
        !crns.contains(&"10004".to_owned()),
        "Monday-only course should NOT match MWF"
    );
}

#[sqlx::test]
async fn test_filter_tuesday_thursday(pool: PgPool) {
    insert_test_courses(&pool).await;

    let days = vec!["tuesday".to_owned(), "thursday".to_owned()];
    let (crns, total) = search_by_days(&pool, Some(&days)).await;

    assert_eq!(
        total, 2,
        "Expected 2 courses with TTh meetings, got {total}"
    );
    assert_eq!(crns.len(), 2, "Result count should match total: {crns:?}");
    assert!(
        crns.contains(&"10002".to_owned()),
        "TTh course should match TTh filter"
    );
    assert!(
        crns.contains(&"10007".to_owned()),
        "PHYS with TTh meeting should match TTh filter"
    );
}

#[sqlx::test]
async fn test_no_day_filter_returns_all(pool: PgPool) {
    insert_test_courses(&pool).await;

    let (crns, total) = search_by_days(&pool, None).await;

    assert_eq!(
        total, 7,
        "Expected all 7 courses when no day filter, got {total}"
    );
    assert_eq!(crns.len(), 7, "Result count should match total: {crns:?}");
    for expected_crn in [
        "10001", "10002", "10003", "10004", "10005", "10006", "10007",
    ] {
        assert!(
            crns.contains(&expected_crn.to_owned()),
            "Course {expected_crn} should be present with no day filter"
        );
    }
}

#[sqlx::test]
async fn test_filter_day_excludes_tba(pool: PgPool) {
    insert_test_courses(&pool).await;

    // Try every individual day -- TBA course (10006) should never appear
    let all_days = [
        "monday",
        "tuesday",
        "wednesday",
        "thursday",
        "friday",
        "saturday",
        "sunday",
    ];
    for day in all_days {
        let days = vec![day.to_owned()];
        let (crns, _total) = search_by_days(&pool, Some(&days)).await;
        assert!(
            !crns.contains(&"10006".to_owned()),
            "TBA course should never match day filter '{day}'"
        );
    }
}

#[sqlx::test]
async fn test_filter_nonexistent_day_combo(pool: PgPool) {
    insert_test_courses(&pool).await;

    let days = vec!["saturday".to_owned(), "sunday".to_owned()];
    let (crns, total) = search_by_days(&pool, Some(&days)).await;

    assert_eq!(
        total, 0,
        "No course meets on both Saturday AND Sunday, got {total}"
    );
    assert!(crns.is_empty(), "Should return no results: {crns:?}");
}
