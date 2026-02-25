//! Tests that text search functions handle accented/diacritic characters correctly.
//!
//! Users with US keyboards should be able to search "Jose Garcia" and find "José García".

mod helpers;

use banner::banner::models::meetings::FacultyItem;
use banner::data::batch::batch_upsert_courses;
use banner::data::courses::{search_courses, suggest_courses, suggest_instructors};
use banner::data::instructors::{PublicInstructorListParams, list_public_instructors};
use helpers::make_course;
use sqlx::PgPool;

/// Build a `FacultyItem` for attaching to a test course.
fn make_faculty(banner_id: &str, display_name: &str, email: Option<&str>) -> FacultyItem {
    FacultyItem {
        banner_id: banner_id.to_owned(),
        category: Some("01".to_owned()),
        class: "net.hedtech.banner.general.overall.SectionMeetingTimeDecorator".to_owned(),
        course_reference_number: 0,
        display_name: display_name.to_owned(),
        email_address: email.map(|e| e.to_owned()),
        primary_indicator: true,
        term: "202620".to_owned(),
    }
}

/// Attach faculty to a course.
fn with_faculty(
    mut course: banner::banner::Course,
    faculty: Vec<FacultyItem>,
) -> banner::banner::Course {
    course.faculty = faculty;
    course
}

/// Insert test data with accented characters in both course titles and instructor names.
async fn insert_accented_test_data(pool: &PgPool) {
    let term = "202620";

    let courses = vec![
        with_faculty(
            make_course(
                "30001",
                term,
                "SPAN",
                "3300",
                "Introducción a la Lingüística",
                (20, 30, 0, 5),
            ),
            vec![make_faculty(
                "@F001",
                "García López, José",
                Some("jose.garcia@utsa.edu"),
            )],
        ),
        with_faculty(
            make_course(
                "30002",
                term,
                "MUS",
                "2100",
                "Études in Music Theory",
                (15, 25, 0, 5),
            ),
            vec![make_faculty(
                "@F002",
                "Müller, François",
                Some("francois.muller@utsa.edu"),
            )],
        ),
        with_faculty(
            make_course(
                "30003",
                term,
                "CS",
                "2200",
                "Data Structures",
                (25, 30, 0, 5),
            ),
            vec![make_faculty(
                "@F003",
                "O'Brien, Séan",
                Some("sean.obrien@utsa.edu"),
            )],
        ),
        with_faculty(
            make_course(
                "30004",
                term,
                "MATH",
                "3400",
                "Álgebra Lineal",
                (18, 25, 0, 5),
            ),
            vec![make_faculty(
                "@F004",
                "Hernández, María",
                Some("maria.hernandez@utsa.edu"),
            )],
        ),
    ];

    batch_upsert_courses(&courses, pool)
        .await
        .expect("failed to insert accented test courses");
}

#[sqlx::test]
async fn test_search_courses_title_unaccented_finds_accented(pool: PgPool) {
    insert_accented_test_data(&pool).await;

    // Search "Introduccion" (no accent) should find "Introducción a la Lingüística"
    let (results, total) = search_courses(
        &pool,
        "202620",
        None,                 // subject
        Some("Introduccion"), // title_query — no accent
        None,
        None,
        false,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        100,
        0,
        None,
        None,
    )
    .await
    .expect("search_courses failed");

    assert!(
        total >= 1,
        "expected at least 1 result for 'Introduccion', got {total}"
    );
    assert!(
        results.iter().any(|c| c.crn == "30001"),
        "should find CRN 30001 (Introducción a la Lingüística)"
    );
}

#[sqlx::test]
async fn test_search_courses_title_unaccented_finds_umlaut(pool: PgPool) {
    insert_accented_test_data(&pool).await;

    // Search "Etudes" (no accent) should find "Études in Music Theory"
    let (results, total) = search_courses(
        &pool,
        "202620",
        None,
        Some("Etudes"),
        None,
        None,
        false,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        100,
        0,
        None,
        None,
    )
    .await
    .expect("search_courses failed");

    assert!(
        total >= 1,
        "expected at least 1 result for 'Etudes', got {total}"
    );
    assert!(
        results.iter().any(|c| c.crn == "30002"),
        "should find CRN 30002 (Études in Music Theory)"
    );
}

#[sqlx::test]
async fn test_search_courses_title_unaccented_finds_algebra(pool: PgPool) {
    insert_accented_test_data(&pool).await;

    // Search "Algebra" (no accent) should find "Álgebra Lineal"
    let (results, total) = search_courses(
        &pool,
        "202620",
        None,
        Some("Algebra"),
        None,
        None,
        false,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        100,
        0,
        None,
        None,
    )
    .await
    .expect("search_courses failed");

    assert!(
        total >= 1,
        "expected at least 1 result for 'Algebra', got {total}"
    );
    assert!(
        results.iter().any(|c| c.crn == "30004"),
        "should find CRN 30004 (Álgebra Lineal)"
    );
}

#[sqlx::test]
async fn test_search_courses_instructor_filter_unaccented(pool: PgPool) {
    insert_accented_test_data(&pool).await;

    // Filter by instructor "Garcia" (no accent) should find courses taught by "García López, José"
    let (results, total) = search_courses(
        &pool,
        "202620",
        None,
        None, // no title query
        None,
        None,
        false,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        Some("Garcia"), // instructor filter — no accent
        100,
        0,
        None,
        None,
    )
    .await
    .expect("search_courses failed");

    assert!(
        total >= 1,
        "expected at least 1 result for instructor 'Garcia', got {total}"
    );
    assert!(
        results.iter().any(|c| c.crn == "30001"),
        "should find CRN 30001 (taught by García López, José)"
    );
}

#[sqlx::test]
async fn test_search_courses_instructor_filter_muller(pool: PgPool) {
    insert_accented_test_data(&pool).await;

    // Filter by instructor "Muller" should find courses taught by "Müller, François"
    let (results, total) = search_courses(
        &pool,
        "202620",
        None,
        None,
        None,
        None,
        false,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        Some("Muller"),
        100,
        0,
        None,
        None,
    )
    .await
    .expect("search_courses failed");

    assert!(
        total >= 1,
        "expected at least 1 result for instructor 'Muller', got {total}"
    );
    assert!(
        results.iter().any(|c| c.crn == "30002"),
        "should find CRN 30002 (taught by Müller, François)"
    );
}

#[sqlx::test]
async fn test_suggest_courses_unaccented_finds_accented_title(pool: PgPool) {
    insert_accented_test_data(&pool).await;

    // Suggest "Introduccion" should find the course with accented title
    let suggestions = suggest_courses(&pool, "202620", "Introduccion", 10)
        .await
        .expect("suggest_courses failed");

    assert!(
        !suggestions.is_empty(),
        "expected suggestions for 'Introduccion', got none"
    );
    assert!(
        suggestions.iter().any(|s| s.title.contains("Introducción")),
        "should suggest 'Introducción a la Lingüística'"
    );
}

#[sqlx::test]
async fn test_suggest_courses_unaccented_finds_etudes(pool: PgPool) {
    insert_accented_test_data(&pool).await;

    let suggestions = suggest_courses(&pool, "202620", "Etudes", 10)
        .await
        .expect("suggest_courses failed");

    assert!(
        !suggestions.is_empty(),
        "expected suggestions for 'Etudes', got none"
    );
    assert!(
        suggestions.iter().any(|s| s.title.contains("Études")),
        "should suggest 'Études in Music Theory'"
    );
}

#[sqlx::test]
async fn test_suggest_instructors_unaccented_finds_accented_name(pool: PgPool) {
    insert_accented_test_data(&pool).await;

    // Suggest "Garcia" should find "García López, José"
    let suggestions = suggest_instructors(&pool, "202620", "Garcia", 10)
        .await
        .expect("suggest_instructors failed");

    assert!(
        !suggestions.is_empty(),
        "expected suggestions for 'Garcia', got none"
    );
    assert!(
        suggestions
            .iter()
            .any(|s| s.display_name.contains("García")),
        "should suggest 'García López, José'"
    );
}

#[sqlx::test]
async fn test_suggest_instructors_unaccented_finds_muller(pool: PgPool) {
    insert_accented_test_data(&pool).await;

    let suggestions = suggest_instructors(&pool, "202620", "Muller", 10)
        .await
        .expect("suggest_instructors failed");

    assert!(
        !suggestions.is_empty(),
        "expected suggestions for 'Muller', got none"
    );
    assert!(
        suggestions
            .iter()
            .any(|s| s.display_name.contains("Müller")),
        "should suggest 'Müller, François'"
    );
}

#[sqlx::test]
async fn test_suggest_instructors_unaccented_finds_sean(pool: PgPool) {
    insert_accented_test_data(&pool).await;

    // "Sean" should find "Séan"
    let suggestions = suggest_instructors(&pool, "202620", "Sean", 10)
        .await
        .expect("suggest_instructors failed");

    assert!(
        !suggestions.is_empty(),
        "expected suggestions for 'Sean', got none"
    );
    assert!(
        suggestions.iter().any(|s| s.display_name.contains("Séan")),
        "should suggest 'O'Brien, Séan'"
    );
}

#[sqlx::test]
async fn test_list_public_instructors_unaccented_search(pool: PgPool) {
    insert_accented_test_data(&pool).await;

    // Search "Hernandez" (no accent) should find "Hernández, María"
    let response = list_public_instructors(
        &pool,
        &PublicInstructorListParams {
            search: Some("Hernandez".to_owned()),
            subject: None,
            sort: "name_asc".to_owned(),
            page: 1,
            per_page: 24,
        },
    )
    .await
    .expect("list_public_instructors failed");

    assert!(
        response.total >= 1,
        "expected at least 1 instructor for 'Hernandez', got {}",
        response.total
    );
    assert!(
        response
            .instructors
            .iter()
            .any(|i| i.display_name.contains("Hernández")),
        "should find 'Hernández, María'"
    );
}

#[sqlx::test]
async fn test_list_public_instructors_unaccented_search_jose(pool: PgPool) {
    insert_accented_test_data(&pool).await;

    // Search "Jose" should find "García López, José"
    let response = list_public_instructors(
        &pool,
        &PublicInstructorListParams {
            search: Some("Jose".to_owned()),
            subject: None,
            sort: "name_asc".to_owned(),
            page: 1,
            per_page: 24,
        },
    )
    .await
    .expect("list_public_instructors failed");

    assert!(
        response.total >= 1,
        "expected at least 1 instructor for 'Jose', got {}",
        response.total
    );
    assert!(
        response
            .instructors
            .iter()
            .any(|i| i.display_name.contains("José")),
        "should find 'García López, José'"
    );
}

#[sqlx::test]
async fn test_list_public_instructors_unaccented_search_francois(pool: PgPool) {
    insert_accented_test_data(&pool).await;

    // Search "Francois" should find "Müller, François"
    let response = list_public_instructors(
        &pool,
        &PublicInstructorListParams {
            search: Some("Francois".to_owned()),
            subject: None,
            sort: "name_asc".to_owned(),
            page: 1,
            per_page: 24,
        },
    )
    .await
    .expect("list_public_instructors failed");

    assert!(
        response.total >= 1,
        "expected at least 1 instructor for 'Francois', got {}",
        response.total
    );
    assert!(
        response
            .instructors
            .iter()
            .any(|i| i.display_name.contains("François")),
        "should find 'Müller, François'"
    );
}
