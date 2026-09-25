//! An instructor's `BlueBook` figures, as the score, the directory and the profile report them.

use crate::helpers::db::test_db;
use banner::data::admin_bluebook::reject_link;
use banner::data::bluebook::catalogue_has_subject;
use banner::data::courses::get_course_instructors;
use banner::data::instructors::{PublicInstructorListParams, get_public_instructor_by_slug, list_public_instructors};
use banner::data::scoring::recompute_all_scores;
use sqlx::PgPool;

const NAME: &str = "Doe, Jane";
const SLUG: &str = "doe-jane";

async fn insert_taught_course(pool: &PgPool, instructor_id: i32, crn: &str, subject: &str, cross_list: Option<&str>) {
    let (course_id,): (i32,) = sqlx::query_as(
        "INSERT INTO courses (crn, subject, course_number, title, term_code, cross_list,
             enrollment, max_enrollment, wait_count, wait_capacity, last_scraped_at)
         VALUES ($1, $2, '3000', 'Test Course', '202510', $3, 10, 30, 0, 0, NOW())
         RETURNING id",
    )
    .bind(crn)
    .bind(subject)
    .bind(cross_list)
    .fetch_one(pool)
    .await
    .expect("insert course failed");

    sqlx::query(
        "INSERT INTO course_instructors (course_id, instructor_id, banner_id, is_primary)
         VALUES ($1, $2, $3, true)",
    )
    .bind(course_id)
    .bind(instructor_id)
    .bind(format!("@{instructor_id}"))
    .execute(pool)
    .await
    .expect("link instructor failed");
}

async fn insert_evaluation(pool: &PgPool, crn: &str, subject: &str, rating: f32, responses: i32) {
    sqlx::query(
        "INSERT INTO bluebook_evaluations (subject, course_number, section, term, crn, instructor_name,
             instructor_rating, instructor_response_count, course_rating, course_response_count)
         VALUES ($1, '3000', $2, '202510', $2, $3, $4, $5, $4, $5)",
    )
    .bind(subject)
    .bind(crn)
    .bind(NAME)
    .bind(rating)
    .bind(responses)
    .execute(pool)
    .await
    .expect("insert_evaluation failed");
}

/// One evaluation published under two cross-listed CRNs, plus one ordinary section.
async fn seed(pool: &PgPool) {
    let (id,): (i32,) = sqlx::query_as("INSERT INTO instructors (display_name, slug) VALUES ($1, $2) RETURNING id")
        .bind(NAME)
        .bind(SLUG)
        .fetch_one(pool)
        .await
        .expect("insert instructor failed");
    sqlx::query(
        "INSERT INTO instructor_bluebook_links (instructor_id, instructor_name, status) VALUES ($1, $2, 'approved')",
    )
    .bind(id)
    .bind(NAME)
    .execute(pool)
    .await
    .expect("insert link failed");

    insert_taught_course(pool, id, "10001", "CS", Some("7A")).await;
    insert_taught_course(pool, id, "10002", "IS", Some("7A")).await;
    insert_taught_course(pool, id, "10003", "CS", None).await;
    insert_evaluation(pool, "10001", "CS", 3.0, 20).await;
    insert_evaluation(pool, "10002", "IS", 3.0, 20).await;
    insert_evaluation(pool, "10003", "CS", 5.0, 20).await;
}

#[tokio::test]
async fn test_recompute_scores_cross_listed_evaluation_counts_once() {
    let pool = test_db!().await;
    seed(&pool).await;

    recompute_all_scores(&pool).await.unwrap();

    let (rating, responses): (Option<f32>, i32) = sqlx::query_as("SELECT bb_rating, bb_count FROM instructor_scores")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(rating, Some(4.0));
    assert_eq!(responses, 40);
}

#[tokio::test]
async fn test_list_instructors_reports_recomputed_bluebook_summary() {
    let pool = test_db!().await;
    seed(&pool).await;
    recompute_all_scores(&pool).await.unwrap();

    let params = PublicInstructorListParams {
        search: None,
        subject: None,
        sort: "name_asc".to_owned(),
        page: 1,
        per_page: 10,
    };
    let page = list_public_instructors(&pool, &params).await.unwrap();

    let bluebook = page.items[0].bluebook.as_ref().expect("bluebook summary");
    assert!((bluebook.avg_instructor_rating - 4.0).abs() < 1e-6);
    assert_eq!(bluebook.total_responses.get(), 40);
}

#[tokio::test]
async fn test_instructor_profile_cross_listed_evaluation_counts_once() {
    let pool = test_db!().await;
    seed(&pool).await;

    let profile = get_public_instructor_by_slug(&pool, SLUG)
        .await
        .unwrap()
        .expect("profile");

    let bluebook = profile.instructor.bluebook.expect("bluebook summary");
    assert!((bluebook.avg_instructor_rating - 4.0).abs() < 1e-6);
    assert_eq!(bluebook.total_responses.get(), 40);
    assert_eq!(bluebook.eval_count.get(), 2);
}

#[tokio::test]
async fn test_catalogue_has_subject_only_for_listed_subjects() {
    let pool = test_db!().await;
    seed(&pool).await;

    assert!(catalogue_has_subject(&pool, "CS").await.unwrap());
    assert!(!catalogue_has_subject(&pool, "MTC").await.unwrap());
}

#[tokio::test]
async fn test_course_instructors_report_recomputed_bluebook_summary() {
    let pool = test_db!().await;
    seed(&pool).await;
    recompute_all_scores(&pool).await.unwrap();
    let (course_id,): (i32,) = sqlx::query_as("SELECT id FROM courses WHERE crn = '10003'")
        .fetch_one(&pool)
        .await
        .unwrap();

    let instructors = get_course_instructors(&pool, course_id).await.unwrap();

    assert!((instructors[0].bb_avg_instructor_rating.unwrap() - 4.0).abs() < 1e-6);
    assert_eq!(instructors[0].bb_total_responses, Some(40));
}

#[tokio::test]
async fn test_reject_link_recomputes_scores() {
    let pool = test_db!().await;
    seed(&pool).await;
    let (link_id,): (i32,) = sqlx::query_as("UPDATE instructor_bluebook_links SET status = 'auto' RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();
    recompute_all_scores(&pool).await.unwrap();

    reject_link(&pool, link_id).await.unwrap();

    let (scored,): (i64,) = sqlx::query_as("SELECT count(*) FROM instructor_scores")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(scored, 0);
}
