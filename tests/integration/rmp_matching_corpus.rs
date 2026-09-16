//! Calibration harness for RMP candidate generation.
//!
//! Ignored by default: it rewrites all non-manual match state, so point
//! `RMP_CORPUS_DATABASE_URL` at a scratch restore, never a live database.
//!
//! Run with `cargo nextest run --run-ignored all rmp_corpus`.

use banner::data::rmp_matching::generate_candidates;
use sqlx::postgres::PgPoolOptions;

#[tokio::test]
#[ignore = "mutates match state; requires RMP_CORPUS_DATABASE_URL"]
async fn rmp_corpus_regenerate_reports_outcome_mix() {
    let url = std::env::var("RMP_CORPUS_DATABASE_URL")
        .expect("set RMP_CORPUS_DATABASE_URL to a scratch database restore");

    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(&url)
        .await
        .expect("connect to corpus database");

    let stats = generate_candidates(&pool)
        .await
        .expect("generate candidates");

    let statuses: Vec<(String, i64)> = sqlx::query_as(
        "SELECT status, COUNT(*) FROM instructor_rmp_match_status GROUP BY 1 ORDER BY 2 DESC",
    )
    .fetch_all(&pool)
    .await
    .expect("fetch status mix");

    // Several auto-links on one instructor are correct when RMP holds duplicate
    // profiles for that person, so only the per-profile invariant is asserted.
    let contested_profiles: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM (SELECT rmp_legacy_id FROM instructor_rmp_links \
         GROUP BY rmp_legacy_id HAVING COUNT(DISTINCT instructor_id) > 1) t",
    )
    .fetch_one(&pool)
    .await
    .expect("count contested profiles");

    let orphan_accepted: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM rmp_match_candidates c WHERE c.status = 'accepted' \
         AND NOT EXISTS (SELECT 1 FROM instructor_rmp_links l \
             WHERE l.instructor_id = c.instructor_id AND l.rmp_legacy_id = c.rmp_legacy_id)",
    )
    .fetch_one(&pool)
    .await
    .expect("count orphaned accepted candidates");

    let multi_linked: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM (SELECT instructor_id FROM instructor_rmp_links \
         WHERE source = 'auto' GROUP BY instructor_id HAVING COUNT(*) > 1) t",
    )
    .fetch_one(&pool)
    .await
    .expect("count multi-linked instructors");

    println!("{stats:#?}");
    println!("status mix: {statuses:?}");
    println!("instructors with several profiles: {}", multi_linked.0);

    assert_eq!(
        contested_profiles.0, 0,
        "one RMP profile must belong to a single instructor"
    );
    assert_eq!(
        orphan_accepted.0, 0,
        "a candidate marked accepted must have a matching link"
    );
}

/// Exercises duplicate detection and the automatic merge against a snapshot.
#[tokio::test]
#[ignore = "mutates instructor rows; requires RMP_CORPUS_DATABASE_URL"]
async fn rmp_corpus_merge_duplicate_instructors() {
    use banner::data::instructor_merge::{
        DuplicateTier, auto_merge_duplicates, find_duplicate_pairs,
    };

    let url = std::env::var("RMP_CORPUS_DATABASE_URL")
        .expect("set RMP_CORPUS_DATABASE_URL to a scratch database restore");
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(&url)
        .await
        .expect("connect to corpus database");

    let before = find_duplicate_pairs(&pool).await.expect("find duplicates");
    let same_account = before
        .iter()
        .filter(|p| p.tier == DuplicateTier::SameAccount)
        .count();
    println!("pairs: {}, same-account: {same_account}", before.len());

    let stats = auto_merge_duplicates(&pool, None)
        .await
        .expect("merge duplicates");
    println!("{stats:?}");

    let after = find_duplicate_pairs(&pool)
        .await
        .expect("re-find duplicates");
    let remaining = after
        .iter()
        .filter(|p| p.tier == DuplicateTier::SameAccount)
        .count();

    let orphans: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM course_instructors ci \
         WHERE NOT EXISTS (SELECT 1 FROM instructors i WHERE i.id = ci.instructor_id)",
    )
    .fetch_one(&pool)
    .await
    .expect("count orphaned course links");

    assert_eq!(stats.merged, same_account, "every safe pair should merge");
    assert_eq!(remaining, 0, "no same-account duplicates should survive");
    assert_eq!(orphans.0, 0, "merging must not orphan course links");
}
