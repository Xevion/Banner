//! Tests for various search filters individually and in combination.
//!
//! Covers `open_only`, `subject`, `time_start`, `time_end`, day filters,
//! and multi-filter combinations including pagination.

mod helpers;

use banner::data::batch::batch_upsert_courses;
use banner::data::courses::{SearchFilter, search_courses};
use helpers::{MeetingTimeBuilder, make_course, with_meetings};
use sqlx::PgPool;

/// Run `search_courses` using a `SearchFilter`, returning (CRNs, total_count).
async fn search(pool: &PgPool, filter: &SearchFilter<'_>) -> (Vec<String>, i64) {
    let (results, total) = search_courses(pool, filter, 100, 0, None, None)
        .await
        .expect("search_courses failed");

    let crns: Vec<String> = results.iter().map(|c| c.crn.clone()).collect();
    (crns, total)
}

/// Insert the shared dataset used by all filter tests.
///
/// | CRN   | Subject | Num  | Title           | Enrl | Max | Days    | Time          |
/// |-------|---------|------|-----------------|------|-----|---------|---------------|
/// | 20001 | CS      | 1100 | Intro to CS     | 25   | 30  | MWF     | 09:00-09:50   |
/// | 20002 | CS      | 2200 | Data Structures | 30   | 30  | TTh     | 14:00-15:15   |
/// | 20003 | CS      | 3300 | Algorithms      | 20   | 25  | MW      | 16:00-17:15   |
/// | 20004 | MATH    | 1100 | College Algebra | 28   | 35  | MWF     | 08:00-08:50   |
/// | 20005 | MATH    | 2400 | Linear Algebra  | 22   | 25  | TTh     | 10:00-11:15   |
/// | 20006 | ENG     | 1010 | English Comp    | 20   | 20  | MW      | 11:00-12:15   |
/// | 20007 | PHYS    | 1600 | Physics I       | 15   | 30  | MWF+TTh | 09:00-09:50 + 14:00-15:15 |
/// | 20008 | ART     | 2100 | Studio Art      | 10   | 15  | Sat     | 09:00-12:00   |
/// | 20009 | CS      | 4400 | Senior Project  | 8    | 15  | (none)  | (none)        |
async fn insert_test_courses(pool: &PgPool) {
    let term = "202620";

    let courses = vec![
        // 20001: CS 1100, MWF 09:00-09:50, open (25/30)
        with_meetings(
            make_course("20001", term, "CS", "1100", "Intro to CS", (25, 30, 0, 10)),
            vec![
                MeetingTimeBuilder::new()
                    .days([true, false, true, false, true, false, false])
                    .time("0900", "0950")
                    .location("SCI", "101")
                    .build(),
            ],
        ),
        // 20002: CS 2200, TTh 14:00-15:15, full (30/30)
        with_meetings(
            make_course(
                "20002",
                term,
                "CS",
                "2200",
                "Data Structures",
                (30, 30, 0, 10),
            ),
            vec![
                MeetingTimeBuilder::new()
                    .days([false, true, false, true, false, false, false])
                    .time("1400", "1515")
                    .location("SCI", "202")
                    .build(),
            ],
        ),
        // 20003: CS 3300, MW 16:00-17:15, open (20/25)
        with_meetings(
            make_course("20003", term, "CS", "3300", "Algorithms", (20, 25, 0, 5)),
            vec![
                MeetingTimeBuilder::new()
                    .days([true, false, true, false, false, false, false])
                    .time("1600", "1715")
                    .location("SCI", "303")
                    .build(),
            ],
        ),
        // 20004: MATH 1100, MWF 08:00-08:50, open (28/35)
        with_meetings(
            make_course(
                "20004",
                term,
                "MATH",
                "1100",
                "College Algebra",
                (28, 35, 0, 5),
            ),
            vec![
                MeetingTimeBuilder::new()
                    .days([true, false, true, false, true, false, false])
                    .time("0800", "0850")
                    .location("MATH", "200")
                    .build(),
            ],
        ),
        // 20005: MATH 2400, TTh 10:00-11:15, open (22/25)
        with_meetings(
            make_course(
                "20005",
                term,
                "MATH",
                "2400",
                "Linear Algebra",
                (22, 25, 0, 5),
            ),
            vec![
                MeetingTimeBuilder::new()
                    .days([false, true, false, true, false, false, false])
                    .time("1000", "1115")
                    .location("MATH", "300")
                    .build(),
            ],
        ),
        // 20006: ENG 1010, MW 11:00-12:15, full (20/20)
        with_meetings(
            make_course("20006", term, "ENG", "1010", "English Comp", (20, 20, 0, 5)),
            vec![
                MeetingTimeBuilder::new()
                    .days([true, false, true, false, false, false, false])
                    .time("1100", "1215")
                    .location("LIB", "100")
                    .build(),
            ],
        ),
        // 20007: PHYS 1600, two meetings: MWF 09:00-09:50 + TTh 14:00-15:15, open (15/30)
        with_meetings(
            make_course("20007", term, "PHYS", "1600", "Physics I", (15, 30, 0, 10)),
            vec![
                MeetingTimeBuilder::new()
                    .days([true, false, true, false, true, false, false])
                    .time("0900", "0950")
                    .location("SCI", "400")
                    .build(),
                MeetingTimeBuilder::new()
                    .days([false, true, false, true, false, false, false])
                    .time("1400", "1515")
                    .location("SCI", "401")
                    .build(),
            ],
        ),
        // 20008: ART 2100, Sat 09:00-12:00, open (10/15)
        with_meetings(
            make_course("20008", term, "ART", "2100", "Studio Art", (10, 15, 0, 5)),
            vec![
                MeetingTimeBuilder::new()
                    .days([false, false, false, false, false, true, false])
                    .time("0900", "1200")
                    .location("ART", "110")
                    .build(),
            ],
        ),
        // 20009: CS 4400, no meetings (TBA), open (8/15)
        make_course("20009", term, "CS", "4400", "Senior Project", (8, 15, 0, 5)),
    ];

    batch_upsert_courses(&courses, pool)
        .await
        .expect("Failed to insert test courses");
}

#[sqlx::test]
async fn test_filter_open_only(pool: PgPool) {
    insert_test_courses(&pool).await;

    let (crns, total) = search(
        &pool,
        &SearchFilter {
            term_code: "202620",
            open_only: true,
            ..Default::default()
        },
    )
    .await;

    assert_eq!(total, 7, "7 open courses (excludes 20002 and 20006)");
    assert_eq!(crns.len(), 7);
    assert!(!crns.contains(&"20002".to_owned()), "20002 is full (30/30)");
    assert!(!crns.contains(&"20006".to_owned()), "20006 is full (20/20)");
}

#[sqlx::test]
async fn test_filter_by_subject(pool: PgPool) {
    insert_test_courses(&pool).await;

    let subjects = vec!["CS".to_owned()];
    let (crns, total) = search(
        &pool,
        &SearchFilter {
            term_code: "202620",
            subjects: Some(&subjects),
            ..Default::default()
        },
    )
    .await;

    assert_eq!(total, 4, "4 CS courses: 20001, 20002, 20003, 20009");
    assert_eq!(crns.len(), 4);
    for expected in ["20001", "20002", "20003", "20009"] {
        assert!(
            crns.contains(&expected.to_owned()),
            "missing CS course {expected}"
        );
    }
}

#[sqlx::test]
async fn test_filter_by_multiple_subjects(pool: PgPool) {
    insert_test_courses(&pool).await;

    let subjects = vec!["CS".to_owned(), "MATH".to_owned()];
    let (crns, total) = search(
        &pool,
        &SearchFilter {
            term_code: "202620",
            subjects: Some(&subjects),
            ..Default::default()
        },
    )
    .await;

    assert_eq!(total, 6, "4 CS + 2 MATH = 6 courses");
    assert_eq!(crns.len(), 6);
    for expected in ["20001", "20002", "20003", "20004", "20005", "20009"] {
        assert!(
            crns.contains(&expected.to_owned()),
            "missing course {expected}"
        );
    }
}

#[sqlx::test]
async fn test_filter_by_time_start(pool: PgPool) {
    insert_test_courses(&pool).await;

    // Courses with at least one meeting starting at or after 10:00:00
    let (crns, total) = search(
        &pool,
        &SearchFilter {
            term_code: "202620",
            time_start: Some("10:00:00"),
            ..Default::default()
        },
    )
    .await;

    // 20002 (14:00), 20003 (16:00), 20005 (10:00), 20006 (11:00), 20007 (has 14:00 meeting)
    assert_eq!(total, 5, "5 courses have a meeting starting >= 10:00");
    assert_eq!(crns.len(), 5);
    for expected in ["20002", "20003", "20005", "20006", "20007"] {
        assert!(
            crns.contains(&expected.to_owned()),
            "missing course {expected}"
        );
    }
    // Excluded: 20001 (09:00), 20004 (08:00), 20008 (09:00), 20009 (TBA)
    for excluded in ["20001", "20004", "20008", "20009"] {
        assert!(
            !crns.contains(&excluded.to_owned()),
            "{excluded} should not match"
        );
    }
}

#[sqlx::test]
async fn test_filter_by_time_end(pool: PgPool) {
    insert_test_courses(&pool).await;

    // Courses with at least one meeting ending at or before 12:00:00
    let (crns, total) = search(
        &pool,
        &SearchFilter {
            term_code: "202620",
            time_end: Some("12:00:00"),
            ..Default::default()
        },
    )
    .await;

    // 20001 (09:50), 20004 (08:50), 20005 (11:15), 20007 (has 09:50 meeting), 20008 (12:00)
    assert_eq!(total, 5, "5 courses have a meeting ending <= 12:00");
    assert_eq!(crns.len(), 5);
    for expected in ["20001", "20004", "20005", "20007", "20008"] {
        assert!(
            crns.contains(&expected.to_owned()),
            "missing course {expected}"
        );
    }
    // Excluded: 20002 (15:15), 20003 (17:15), 20006 (12:15), 20009 (TBA)
    for excluded in ["20002", "20003", "20006", "20009"] {
        assert!(
            !crns.contains(&excluded.to_owned()),
            "{excluded} should not match"
        );
    }
}

#[sqlx::test]
async fn test_combined_subject_and_days(pool: PgPool) {
    insert_test_courses(&pool).await;

    let subjects = vec!["CS".to_owned()];
    let days = vec!["monday".to_owned()];
    let (crns, total) = search(
        &pool,
        &SearchFilter {
            term_code: "202620",
            subjects: Some(&subjects),
            days: Some(&days),
            ..Default::default()
        },
    )
    .await;

    // CS courses with Monday: 20001 (MWF), 20003 (MW)
    assert_eq!(total, 2, "2 CS courses meet on Monday");
    assert_eq!(crns.len(), 2);
    assert!(crns.contains(&"20001".to_owned()), "CS 1100 MWF has Monday");
    assert!(crns.contains(&"20003".to_owned()), "CS 3300 MW has Monday");
    assert!(
        !crns.contains(&"20002".to_owned()),
        "CS 2200 TTh has no Monday"
    );
    assert!(
        !crns.contains(&"20009".to_owned()),
        "CS 4400 TBA has no Monday"
    );
}

#[sqlx::test]
async fn test_combined_open_only_and_days(pool: PgPool) {
    insert_test_courses(&pool).await;

    let days = vec!["tuesday".to_owned(), "thursday".to_owned()];
    let (crns, total) = search(
        &pool,
        &SearchFilter {
            term_code: "202620",
            open_only: true,
            days: Some(&days),
            ..Default::default()
        },
    )
    .await;

    // Open TTh courses: 20005 (MATH, open), 20007 (PHYS, open, has TTh meeting)
    // NOT 20002 (CS, full)
    assert_eq!(total, 2, "2 open courses with TTh meetings");
    assert_eq!(crns.len(), 2);
    assert!(crns.contains(&"20005".to_owned()), "MATH 2400 is open TTh");
    assert!(
        crns.contains(&"20007".to_owned()),
        "PHYS 1600 is open with TTh meeting"
    );
    assert!(!crns.contains(&"20002".to_owned()), "CS 2200 is full");
}

#[sqlx::test]
async fn test_combined_days_and_time_range(pool: PgPool) {
    insert_test_courses(&pool).await;

    let days = vec![
        "monday".to_owned(),
        "wednesday".to_owned(),
        "friday".to_owned(),
    ];
    let (crns, total) = search(
        &pool,
        &SearchFilter {
            term_code: "202620",
            days: Some(&days),
            time_start: Some("09:00:00"),
            time_end: Some("10:00:00"),
            ..Default::default()
        },
    )
    .await;

    // MWF courses where some meeting starts >= 09:00 AND some meeting ends <= 10:00.
    // 20001 (MWF 09:00-09:50): start 09:00 >= 09:00 (ok), end 09:50 <= 10:00 (ok)
    // 20004 (MWF 08:00-08:50): start 08:00 < 09:00 (x)
    // 20007 (has MWF 09:00-09:50): start 09:00 >= 09:00 (ok), end 09:50 <= 10:00 (ok)
    assert_eq!(total, 2, "2 MWF courses fit the 09:00-10:00 window");
    assert_eq!(crns.len(), 2);
    assert!(
        crns.contains(&"20001".to_owned()),
        "CS 1100 MWF 09:00-09:50"
    );
    assert!(
        crns.contains(&"20007".to_owned()),
        "PHYS 1600 MWF 09:00-09:50 meeting"
    );
    assert!(
        !crns.contains(&"20004".to_owned()),
        "MATH 1100 starts at 08:00"
    );
}

#[sqlx::test]
async fn test_combined_triple_filter(pool: PgPool) {
    insert_test_courses(&pool).await;

    let subjects = vec!["CS".to_owned(), "PHYS".to_owned()];
    let days = vec!["monday".to_owned()];
    let (crns, total) = search(
        &pool,
        &SearchFilter {
            term_code: "202620",
            subjects: Some(&subjects),
            open_only: true,
            days: Some(&days),
            ..Default::default()
        },
    )
    .await;

    // CS/PHYS + open + Monday: 20001 (CS, open, MWF), 20003 (CS, open, MW), 20007 (PHYS, open, MWF)
    // NOT 20002 (full), NOT 20009 (TBA)
    assert_eq!(total, 3, "3 open CS/PHYS courses with Monday meetings");
    assert_eq!(crns.len(), 3);
    for expected in ["20001", "20003", "20007"] {
        assert!(
            crns.contains(&expected.to_owned()),
            "missing course {expected}"
        );
    }
}

#[sqlx::test]
async fn test_pagination_with_filters(pool: PgPool) {
    insert_test_courses(&pool).await;

    let days = vec!["monday".to_owned()];

    // Monday courses: 20001 (MWF), 20003 (MW), 20004 (MWF), 20006 (MW), 20007 (has MWF)
    // That's 5 courses total.

    let filter = SearchFilter {
        term_code: "202620",
        days: Some(&days),
        ..Default::default()
    };

    // First page
    let (results, total1) = search_courses(&pool, &filter, 2, 0, None, None)
        .await
        .expect("search_courses failed");
    let page1_crns: Vec<String> = results.iter().map(|c| c.crn.clone()).collect();

    assert_eq!(total1, 5, "5 courses have Monday meetings");
    assert_eq!(page1_crns.len(), 2, "page 1 returns limit=2 results");

    // Second page
    let (results, total2) = search_courses(&pool, &filter, 2, 2, None, None)
        .await
        .expect("search_courses failed");
    let page2_crns: Vec<String> = results.iter().map(|c| c.crn.clone()).collect();

    assert_eq!(total2, 5, "total count is stable across pages");
    assert_eq!(page2_crns.len(), 2, "page 2 returns limit=2 results");

    // Third page (remainder)
    let (results, total3) = search_courses(&pool, &filter, 2, 4, None, None)
        .await
        .expect("search_courses failed");
    let page3_crns: Vec<String> = results.iter().map(|c| c.crn.clone()).collect();

    assert_eq!(total3, 5, "total count is stable across pages");
    assert_eq!(page3_crns.len(), 1, "page 3 returns the remaining 1 result");

    // No overlap between pages
    let mut all_crns = Vec::new();
    all_crns.extend(page1_crns);
    all_crns.extend(page2_crns);
    all_crns.extend(page3_crns);
    assert_eq!(all_crns.len(), 5, "all pages together yield 5 results");

    all_crns.sort();
    all_crns.dedup();
    assert_eq!(all_crns.len(), 5, "no duplicate CRNs across pages");
}
