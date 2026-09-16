use banner::data::admin_rmp::{AdminRmpError, accept_candidate, reject_all_candidates};
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

/// Accepting a candidate whose RMP profile another instructor already holds must
/// name that instructor in a typed error, not in a formatted message.
#[sqlx::test]
async fn accept_candidate_reports_the_holder_when_the_profile_is_taken(pool: PgPool) {
    let (claimant_id,): (i32,) = sqlx::query_as(
        "INSERT INTO instructors (display_name, email) \
         VALUES ('Fictional, Bryn', 'bryn@utsa.edu') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .expect("failed to create claiming instructor");

    let (instructor_id,): (i32,) = sqlx::query_as(
        "INSERT INTO instructors (display_name, email) \
         VALUES ('Fictional, Bryn', 'bryn@my.utsa.edu') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .expect("failed to create instructor");

    sqlx::query(
        "INSERT INTO rmp_professors (legacy_id, graphql_id, first_name, last_name, num_ratings) \
         VALUES (9999998, 'taken-graphql-id', 'Bryn', 'Fictional', 12)",
    )
    .execute(&pool)
    .await
    .expect("failed to create rmp professor");

    sqlx::query(
        "INSERT INTO instructor_rmp_links (instructor_id, rmp_legacy_id, source) \
         VALUES ($1, 9999998, 'manual')",
    )
    .bind(claimant_id)
    .execute(&pool)
    .await
    .expect("failed to create link");

    sqlx::query(
        "INSERT INTO rmp_match_candidates (instructor_id, rmp_legacy_id, score, status) \
         VALUES ($1, 9999998, 0.9, 'pending')",
    )
    .bind(instructor_id)
    .execute(&pool)
    .await
    .expect("failed to create candidate");

    let err = accept_candidate(&pool, instructor_id, 9999998, 1)
        .await
        .expect_err("accepting a taken profile should fail");

    match err.downcast_ref::<AdminRmpError>() {
        Some(AdminRmpError::AlreadyLinked {
            instructor_id: holder,
            display_name,
            email,
        }) => {
            assert_eq!(*holder, claimant_id);
            assert_eq!(display_name, "Fictional, Bryn");
            assert_eq!(email.as_deref(), Some("bryn@utsa.edu"));
        }
        other => panic!("expected AlreadyLinked, got {other:?}"),
    }
}

/// An instructor with a confirmed match cannot be rejected wholesale.
#[sqlx::test]
async fn reject_all_refuses_an_instructor_with_confirmed_matches(pool: PgPool) {
    let (instructor_id,): (i32,) = sqlx::query_as(
        "INSERT INTO instructors (display_name, email, rmp_match_status) \
         VALUES ('Test, Instructor', 'test@utsa.edu', 'confirmed') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .expect("failed to create instructor");

    let err = reject_all_candidates(&pool, instructor_id, 1)
        .await
        .expect_err("rejecting a confirmed instructor should fail");

    assert!(
        matches!(
            err.downcast_ref::<AdminRmpError>(),
            Some(AdminRmpError::ConfirmedMatches)
        ),
        "expected ConfirmedMatches, got {err:?}"
    );
}
