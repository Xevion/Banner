//! An instructor's per-course standing among everyone else who teaches the course.

use crate::helpers::db::test_db;
use banner::data::cohort::{Evidence, InstructorCohort};
use banner::data::instructors::get_public_instructor_by_slug;
use banner::data::scoring::recompute_all_scores;
use banner::data::unsigned::Count;
use sqlx::PgPool;

async fn insert_instructor(pool: &PgPool, name: &str, slug: &str) {
    let (id,): (i32,) = sqlx::query_as("INSERT INTO instructors (display_name, slug) VALUES ($1, $2) RETURNING id")
        .bind(name)
        .bind(slug)
        .fetch_one(pool)
        .await
        .expect("insert instructor failed");
    sqlx::query(
        "INSERT INTO instructor_bluebook_links (instructor_id, instructor_name, status) VALUES ($1, $2, 'approved')",
    )
    .bind(id)
    .bind(name)
    .execute(pool)
    .await
    .expect("insert link failed");
}

async fn insert_section(pool: &PgPool, crn: &str, course_number: &str, cross_list: Option<&str>) {
    sqlx::query(
        "INSERT INTO courses (crn, subject, course_number, title, term_code, cross_list,
             enrollment, max_enrollment, wait_count, wait_capacity, last_scraped_at)
         VALUES ($1, 'CS', $2, 'Test Course', '202510', $3, 10, 30, 0, 0, NOW())",
    )
    .bind(crn)
    .bind(course_number)
    .bind(cross_list)
    .execute(pool)
    .await
    .expect("insert course failed");
}

async fn insert_evaluation(pool: &PgPool, name: &str, crn: &str, course_number: &str, rating: f32, responses: i32) {
    sqlx::query(
        "INSERT INTO bluebook_evaluations (subject, course_number, section, term, crn, instructor_name,
             instructor_rating, instructor_response_count)
         VALUES ('CS', $1, $2, '202510', $2, $3, $4, $5)",
    )
    .bind(course_number)
    .bind(crn)
    .bind(name)
    .bind(rating)
    .bind(responses)
    .execute(pool)
    .await
    .expect("insert evaluation failed");
}

/// CS 3000 is taught by Ada (two sections, one of them listed under two CRNs), Bea, and
/// Cy (below the response floor). CS 4000 is Ada's alone.
async fn seed(pool: &PgPool) {
    insert_instructor(pool, "Ada, A", "ada").await;
    insert_instructor(pool, "Bea, B", "bea").await;
    insert_instructor(pool, "Cy, C", "cy").await;

    insert_section(pool, "10001", "3000", Some("7A")).await;
    insert_section(pool, "10002", "3000", Some("7A")).await;
    insert_section(pool, "10003", "3000", None).await;
    insert_section(pool, "10004", "3000", None).await;
    insert_section(pool, "10005", "3000", None).await;
    insert_section(pool, "10006", "4000", None).await;

    insert_evaluation(pool, "Ada, A", "10001", "3000", 3.0, 20).await;
    insert_evaluation(pool, "Ada, A", "10002", "3000", 3.0, 20).await;
    insert_evaluation(pool, "Ada, A", "10003", "3000", 5.0, 20).await;
    insert_evaluation(pool, "Bea, B", "10004", "3000", 4.5, 30).await;
    insert_evaluation(pool, "Cy, C", "10005", "3000", 2.0, 10).await;
    insert_evaluation(pool, "Ada, A", "10006", "4000", 4.0, 20).await;

    recompute_all_scores(pool).await.unwrap();
}

async fn cohort_of(pool: &PgPool, slug: &str) -> InstructorCohort {
    get_public_instructor_by_slug(pool, slug)
        .await
        .unwrap()
        .expect("profile")
        .instructor
        .cohort
}

fn close(actual: f32, expected: f32) -> bool {
    (actual - expected).abs() < 1e-4
}

#[tokio::test]
async fn test_cohort_course_ranks_against_other_instructors_of_the_course() {
    let pool = test_db!().await;
    seed(&pool).await;

    let cohort = cohort_of(&pool, "ada").await;
    let course = cohort
        .courses
        .iter()
        .find(|c| c.course_number == "3000")
        .expect("CS 3000");

    assert!(
        close(course.rating, 4.0),
        "cross-listed twins count once: {}",
        course.rating
    );
    assert_eq!(course.responses.get(), 40);
    assert_eq!(course.sections.get(), 2);
    assert_eq!(course.cohort_size.get(), 2, "Cy is under the response floor");
    assert!(close(course.cohort_mean, 4.25));
    assert_eq!(course.rank, Some(Count::new(2)));
    assert!(close(course.delta.unwrap(), -0.25));
    assert_eq!(course.evidence, Evidence::Insufficient);
    let peers: Vec<_> = course.peers.iter().map(|p| p.slug.as_deref()).collect();
    assert_eq!(peers, [Some("bea")]);
}

#[tokio::test]
async fn test_cohort_of_one_is_not_ranked() {
    let pool = test_db!().await;
    seed(&pool).await;

    let cohort = cohort_of(&pool, "ada").await;
    let course = cohort
        .courses
        .iter()
        .find(|c| c.course_number == "4000")
        .expect("CS 4000");

    assert_eq!(course.cohort_size.get(), 1);
    assert_eq!(course.rank, None);
    assert_eq!(course.delta, None);
    assert!(course.peers.is_empty());
}

#[tokio::test]
async fn test_cohort_standing_places_instructor_among_all_instructors() {
    let pool = test_db!().await;
    seed(&pool).await;

    let ada = cohort_of(&pool, "ada").await.standing.expect("Ada has a cohort");
    let bea = cohort_of(&pool, "bea").await.standing.expect("Bea has a cohort");

    assert!(
        close(ada.delta, -0.25),
        "only the course with a cohort counts: {}",
        ada.delta
    );
    assert_eq!(ada.compared_courses.get(), 1);
    assert_eq!(
        ada.evidence,
        Evidence::Insufficient,
        "one two-section course cannot place her"
    );
    assert_eq!(ada.percentile, 0);
    assert_eq!(bea.percentile, 100);
}

#[tokio::test]
async fn test_instructor_below_response_floor_has_no_cohort() {
    let pool = test_db!().await;
    seed(&pool).await;

    let cohort = cohort_of(&pool, "cy").await;

    assert!(cohort.standing.is_none());
    assert!(cohort.courses.is_empty());
}
