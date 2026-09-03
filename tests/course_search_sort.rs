//! Execute every `SortKey` against a real database.
//!
//! The unit tests in `src/data/courses.rs` only inspect the text `SortSpec`
//! produces, so a mistyped `KeyDef.expr` passes them and fails at runtime.
//! These tests run each key through `search_courses` and pin the row order the
//! denormalized summary columns are supposed to give.

mod helpers;

use assert2::check;
use banner::banner::Course;
use banner::data::batch::batch_upsert_courses;
use banner::data::courses::{
    SearchFilter, SortDirection, SortKey, SortSpec, SortTerm, search_courses,
};
use helpers::{MeetingTimeBuilder, make_course, with_meetings};
use sqlx::PgPool;

const TERM: &str = "202620";

/// Return a copy of `course` with its section sequence number replaced.
fn with_sequence(mut course: Course, sequence: &str) -> Course {
    course.sequence_number = sequence.to_owned();
    course
}

/// Insert the dataset every ordering assertion below is written against.
///
/// | CRN   | Code           | Meetings                        | begin | end  | dur | weekly | mask | seats |
/// |-------|----------------|---------------------------------|-------|------|-----|--------|------|-------|
/// | 30001 | CS 1100 001    | MWF 09:00-09:50                 | 540   | 590  | 50  | 50     | 21   | 5     |
/// | 30002 | CS 2200 001    | TTh 08:00-09:15                 | 480   | 555  | 75  | 75     | 10   | 20    |
/// | 30003 | MATH 1300 001  | M 13:00-14:00, W 10:00-11:30    | 600   | 690  | 90  | 150    | 5    | 28    |
/// | 30004 | ART 2100 001   | F 16:00-18:00                   | 960   | 1080 | 120 | 120    | 16   | 15    |
/// | 30005 | ENG 1010 001   | none                            | NULL  | NULL | -   | -      | NULL | 25    |
/// | 30006 | CS 1100 002    | MWF 09:00-09:50                 | 540   | 590  | 50  | 50     | 21   | 10    |
///
/// 30003 lists its later meeting first so the earliest-meeting semantics of
/// `first_begin_minutes` are pinned rather than array position.
async fn insert_fixture(pool: &PgPool) {
    let courses = vec![
        with_meetings(
            make_course("30001", TERM, "CS", "1100", "Intro to CS", (25, 30, 3, 10)),
            vec![
                MeetingTimeBuilder::new()
                    .days([true, false, true, false, true, false, false])
                    .time("0900", "0950")
                    .location("SCI", "101")
                    .build(),
            ],
        ),
        with_meetings(
            make_course(
                "30002",
                TERM,
                "CS",
                "2200",
                "Data Structures",
                (10, 30, 0, 10),
            ),
            vec![
                MeetingTimeBuilder::new()
                    .days([false, true, false, true, false, false, false])
                    .time("0800", "0915")
                    .location("SCI", "202")
                    .build(),
            ],
        ),
        with_meetings(
            make_course("30003", TERM, "MATH", "1300", "Calculus I", (12, 40, 7, 10)),
            vec![
                MeetingTimeBuilder::new()
                    .days([true, false, false, false, false, false, false])
                    .time("1300", "1400")
                    .location("MATH", "300")
                    .build(),
                MeetingTimeBuilder::new()
                    .days([false, false, true, false, false, false, false])
                    .time("1000", "1130")
                    .location("MATH", "301")
                    .build(),
            ],
        ),
        with_meetings(
            make_course("30004", TERM, "ART", "2100", "Studio Art", (5, 20, 1, 5)),
            vec![
                MeetingTimeBuilder::new()
                    .days([false, false, false, false, true, false, false])
                    .time("1600", "1800")
                    .location("ART", "110")
                    .build(),
            ],
        ),
        make_course("30005", TERM, "ENG", "1010", "English Comp", (0, 25, 0, 10)),
        with_sequence(
            with_meetings(
                make_course("30006", TERM, "CS", "1100", "Intro to CS", (20, 30, 2, 10)),
                vec![
                    MeetingTimeBuilder::new()
                        .days([true, false, true, false, true, false, false])
                        .time("0900", "0950")
                        .location("SCI", "102")
                        .build(),
                ],
            ),
            "002",
        ),
    ];

    batch_upsert_courses(&courses, pool)
        .await
        .expect("Failed to insert fixture courses");
}

/// Run `search_courses` over the whole term with `spec`, returning CRNs in order.
async fn crns_sorted_by(pool: &PgPool, spec: &str) -> Vec<String> {
    let sort: SortSpec = spec.parse().expect("sort spec parses");
    let filter = SearchFilter {
        term_code: TERM,
        ..Default::default()
    };

    let (rows, _total) = search_courses(pool, &filter, 100, 0, &sort)
        .await
        .unwrap_or_else(|e| panic!("search_courses failed for sort '{spec}': {e:#}"));

    rows.iter().map(|c| c.crn.clone()).collect()
}

#[sqlx::test]
async fn test_every_sort_key_executes_against_the_schema(pool: PgPool) {
    insert_fixture(&pool).await;

    let filter = SearchFilter {
        term_code: TERM,
        ..Default::default()
    };

    for key in SortKey::ALL {
        for direction in [SortDirection::Asc, SortDirection::Desc] {
            let spec = SortSpec::new(vec![SortTerm {
                key: *key,
                direction,
            }]);

            let (rows, total) = search_courses(&pool, &filter, 100, 0, &spec)
                .await
                .unwrap_or_else(|e| {
                    panic!(
                        "sort by {key:?} {direction:?} ({}) failed: {e:#}",
                        spec.to_sql()
                    )
                });

            check!(
                rows.len() == 6,
                "{key:?} {direction:?} returned wrong count"
            );
            check!(total == 6, "{key:?} {direction:?} miscounted");
        }
    }
}

#[sqlx::test]
async fn test_start_time_orders_by_earliest_meeting(pool: PgPool) {
    insert_fixture(&pool).await;

    let asc = crns_sorted_by(&pool, "start_time").await;
    check!(asc == ["30002", "30001", "30006", "30003", "30004", "30005"]);

    let desc = crns_sorted_by(&pool, "-start_time").await;
    check!(desc == ["30004", "30003", "30001", "30006", "30002", "30005"]);
}

#[sqlx::test]
async fn test_end_time_orders_by_earliest_meeting_end(pool: PgPool) {
    insert_fixture(&pool).await;

    let asc = crns_sorted_by(&pool, "end_time").await;
    check!(asc == ["30002", "30001", "30006", "30003", "30004", "30005"]);

    let desc = crns_sorted_by(&pool, "-end_time").await;
    check!(desc == ["30004", "30003", "30001", "30006", "30002", "30005"]);
}

/// 30003 is longer than 30004 by the week and shorter by the single meeting, so
/// the two keys cannot be reading the same column.
#[sqlx::test]
async fn test_duration_and_weekly_minutes_diverge(pool: PgPool) {
    insert_fixture(&pool).await;

    let duration = crns_sorted_by(&pool, "duration").await;
    check!(duration == ["30001", "30006", "30002", "30003", "30004", "30005"]);

    let weekly = crns_sorted_by(&pool, "weekly_minutes").await;
    check!(weekly == ["30001", "30006", "30002", "30004", "30003", "30005"]);

    let weekly_desc = crns_sorted_by(&pool, "-weekly_minutes").await;
    check!(weekly_desc == ["30003", "30004", "30002", "30001", "30006", "30005"]);
}

/// Ascending is Monday-first bit order, not day count: the Friday-only section
/// sorts after both two-day patterns.
#[sqlx::test]
async fn test_days_orders_by_the_monday_first_mask(pool: PgPool) {
    insert_fixture(&pool).await;

    let asc = crns_sorted_by(&pool, "days").await;
    check!(asc == ["30003", "30002", "30004", "30001", "30006", "30005"]);

    let desc = crns_sorted_by(&pool, "-days").await;
    check!(desc == ["30001", "30006", "30004", "30002", "30003", "30005"]);
}

#[sqlx::test]
async fn test_seats_open_orders_by_remaining_capacity(pool: PgPool) {
    insert_fixture(&pool).await;

    let asc = crns_sorted_by(&pool, "seats_open").await;
    check!(asc == ["30001", "30006", "30004", "30002", "30005", "30003"]);

    let desc = crns_sorted_by(&pool, "-seats_open").await;
    check!(desc == ["30003", "30005", "30002", "30004", "30006", "30001"]);
}

#[sqlx::test]
async fn test_course_code_orders_by_catalog_position(pool: PgPool) {
    insert_fixture(&pool).await;

    let asc = crns_sorted_by(&pool, "course_code").await;
    check!(asc == ["30004", "30001", "30006", "30002", "30005", "30003"]);

    // course_code spans three columns, so descending must reverse all of them
    // rather than only the last.
    let desc = crns_sorted_by(&pool, "-course_code").await;
    check!(desc == ["30003", "30005", "30002", "30006", "30001", "30004"]);
}

/// Absence is not a low value, so the unscheduled section stays last when the
/// ordering is reversed.
#[sqlx::test]
async fn test_untimed_section_sinks_in_both_directions(pool: PgPool) {
    insert_fixture(&pool).await;

    for key in [
        "start_time",
        "end_time",
        "duration",
        "weekly_minutes",
        "days",
    ] {
        let asc = crns_sorted_by(&pool, key).await;
        check!(asc.last() == Some(&"30005".to_owned()), "{key} ascending");

        let desc = crns_sorted_by(&pool, &format!("-{key}")).await;
        check!(desc.last() == Some(&"30005".to_owned()), "{key} descending");
    }
}

/// Sections sharing a sort value fall back to catalog order, the same way round
/// regardless of the leading term's direction.
#[sqlx::test]
async fn test_ties_break_on_catalog_order(pool: PgPool) {
    insert_fixture(&pool).await;

    for spec in ["start_time", "-start_time", "duration", "-duration"] {
        let crns = crns_sorted_by(&pool, spec).await;
        let first = crns
            .iter()
            .position(|c| c == "30001")
            .expect("30001 present");
        let second = crns
            .iter()
            .position(|c| c == "30006")
            .expect("30006 present");

        check!(second == first + 1, "{spec} split the tied sections");
        check!(first < second, "{spec} reversed the catalog tiebreaker");
    }
}

/// A section that loses its last meeting must have its summary cleared, not
/// left reading the times it used to have.
#[sqlx::test]
async fn test_losing_every_meeting_clears_the_summary(pool: PgPool) {
    insert_fixture(&pool).await;

    let before = crns_sorted_by(&pool, "start_time").await;
    check!(before[1] == "30001", "fixture did not seed a timed 30001");

    let stripped = make_course("30001", TERM, "CS", "1100", "Intro to CS", (25, 30, 3, 10));
    batch_upsert_courses(&[stripped], &pool)
        .await
        .expect("Failed to re-upsert 30001 without meetings");

    let asc = crns_sorted_by(&pool, "start_time").await;
    check!(asc == ["30002", "30006", "30003", "30004", "30001", "30005"]);

    let desc = crns_sorted_by(&pool, "-start_time").await;
    check!(desc == ["30004", "30003", "30006", "30002", "30001", "30005"]);

    let days = crns_sorted_by(&pool, "days").await;
    check!(days == ["30003", "30002", "30004", "30006", "30001", "30005"]);
}
