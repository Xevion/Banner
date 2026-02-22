//! Test course search with alphanumeric course numbers (e.g., "015X", "399H").

mod helpers;

use banner::data::batch::batch_upsert_courses;
use banner::data::courses::search_courses;
use helpers::make_course;

#[sqlx::test]
async fn test_search_alphanumeric_course_numbers(pool: sqlx::PgPool) {
    let term = "202620";

    // Insert courses with both numeric and alphanumeric course numbers
    let courses = vec![
        make_course("10001", term, "CS", "0100", "Intro to CS", (20, 30, 0, 10)),
        make_course(
            "10002",
            term,
            "CS",
            "015X",
            "Special Topics",
            (15, 25, 0, 5),
        ),
        make_course(
            "10003",
            term,
            "CS",
            "0200",
            "Data Structures",
            (25, 30, 0, 10),
        ),
        make_course(
            "10004",
            term,
            "CS",
            "0399",
            "Advanced Topics",
            (18, 25, 0, 5),
        ),
        make_course("10005", term, "CS", "399H", "Honors Course", (12, 20, 0, 5)),
        make_course(
            "10006",
            term,
            "CS",
            "5500",
            "Graduate Seminar",
            (10, 15, 0, 3),
        ),
    ];

    batch_upsert_courses(&courses, &pool)
        .await
        .expect("Failed to insert test courses");

    // Test: Search for course numbers 100-5500 (should include alphanumeric)
    let (results, _total) = search_courses(
        &pool,
        term,
        None,       // subject
        None,       // title_query
        Some(100),  // course_number_low
        Some(5500), // course_number_high
        false,      // open_only
        None,       // instructional_method
        None,       // campus
        None,       // wait_count_max
        None,       // days
        None,       // time_start
        None,       // time_end
        None,       // part_of_term
        None,       // attributes
        None,       // credit_hour_min
        None,       // credit_hour_max
        None,       // instructor
        100,        // limit
        0,          // offset
        None,       // sort_by
        None,       // sort_dir
    )
    .await
    .expect("Search failed");

    // Should include:
    // - 0100 (100 >= 100)
    // - 0200 (200 in range)
    // - 0399 (399 in range)
    // - 399H (numeric prefix 399 in range)
    // - 5500 (5500 <= 5500)
    //
    // Should exclude:
    // - 015X (numeric prefix 15 < 100)

    let crns: Vec<&str> = results.iter().map(|c| c.crn.as_str()).collect();

    assert_eq!(
        results.len(),
        5,
        "Expected 5 courses in range, got {}: {:?}",
        results.len(),
        crns
    );
    assert!(crns.contains(&"10001"), "Should include CS 0100");
    assert!(crns.contains(&"10003"), "Should include CS 0200");
    assert!(crns.contains(&"10004"), "Should include CS 0399");
    assert!(
        crns.contains(&"10005"),
        "Should include CS 399H (numeric prefix 399)"
    );
    assert!(crns.contains(&"10006"), "Should include CS 5500");
    assert!(
        !crns.contains(&"10002"),
        "Should exclude CS 015X (numeric prefix 15 < 100)"
    );
}
