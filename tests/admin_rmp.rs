mod helpers;

use banner::data::rmp::unmatch_instructor;
use sqlx::PgPool;

/// Test that unmatching an instructor resets accepted candidates back to pending.
///
/// When a user unmatches an instructor, accepted candidates should be reset to
/// 'pending' so they can be re-matched later. This prevents the bug where
/// candidates remain 'accepted' but have no corresponding link.
#[sqlx::test]
async fn unmatch_resets_accepted_candidates_to_pending(pool: PgPool) {
    // ARRANGE: Create an instructor
    let (instructor_id,): (i32,) = sqlx::query_as(
        "INSERT INTO instructors (display_name, email) 
         VALUES ('Test, Instructor', 'test@utsa.edu') 
         RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .expect("failed to create instructor");

    // ARRANGE: Create an RMP professor
    let (rmp_legacy_id,): (i32,) = sqlx::query_as(
        "INSERT INTO rmp_professors (legacy_id, graphql_id, first_name, last_name, num_ratings) 
         VALUES (9999999, 'test-graphql-id', 'Test', 'Professor', 10) 
         RETURNING legacy_id",
    )
    .fetch_one(&pool)
    .await
    .expect("failed to create rmp professor");

    // ARRANGE: Create a match candidate with 'accepted' status
    sqlx::query(
        "INSERT INTO rmp_match_candidates (instructor_id, rmp_legacy_id, score, status) 
         VALUES ($1, $2, 0.85, 'accepted')",
    )
    .bind(instructor_id)
    .bind(rmp_legacy_id)
    .execute(&pool)
    .await
    .expect("failed to create candidate");

    // ARRANGE: Create a link in instructor_rmp_links
    sqlx::query(
        "INSERT INTO instructor_rmp_links (instructor_id, rmp_legacy_id, source) 
         VALUES ($1, $2, 'manual')",
    )
    .bind(instructor_id)
    .bind(rmp_legacy_id)
    .execute(&pool)
    .await
    .expect("failed to create link");

    // ARRANGE: Update instructor status to 'confirmed'
    sqlx::query("UPDATE instructors SET rmp_match_status = 'confirmed' WHERE id = $1")
        .bind(instructor_id)
        .execute(&pool)
        .await
        .expect("failed to update instructor status");

    // ACT: Unmatch the specific RMP profile
    unmatch_instructor(&pool, instructor_id, Some(rmp_legacy_id))
        .await
        .expect("unmatch should succeed");

    // ASSERT: Candidate should be reset to pending
    let (candidate_status,): (String,) = sqlx::query_as(
        "SELECT status FROM rmp_match_candidates 
         WHERE instructor_id = $1 AND rmp_legacy_id = $2",
    )
    .bind(instructor_id)
    .bind(rmp_legacy_id)
    .fetch_one(&pool)
    .await
    .expect("failed to fetch candidate status");
    assert_eq!(
        candidate_status, "pending",
        "candidate should be reset to pending after unmatch"
    );

    // ASSERT: Link should be deleted
    let (link_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM instructor_rmp_links WHERE instructor_id = $1")
            .bind(instructor_id)
            .fetch_one(&pool)
            .await
            .expect("failed to count links");
    assert_eq!(link_count, 0, "link should be deleted");

    // ASSERT: Instructor status should be unmatched
    let (instructor_status,): (String,) =
        sqlx::query_as("SELECT rmp_match_status FROM instructors WHERE id = $1")
            .bind(instructor_id)
            .fetch_one(&pool)
            .await
            .expect("failed to fetch instructor status");
    assert_eq!(
        instructor_status, "unmatched",
        "instructor should be unmatched"
    );
}
