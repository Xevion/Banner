//! End-to-end tests for `generate_candidates`, the RMP matching pipeline.
//!
//! Each test seeds a fixture and asserts the exact links, candidate statuses and
//! scores it produces, so a change in matching behaviour fails here rather than
//! surfacing as a wrong instructor on a live profile.

use crate::helpers::db::test_db;
use crate::helpers::{insert_instructor, insert_rmp_professor, insert_rmp_review, insert_taught_course};
use assert2::check;
use banner::data::rmp_matching::generate_candidates;
use sqlx::PgPool;

/// Every link row, ordered so comparisons are stable.
async fn links(pool: &PgPool) -> Vec<(i32, i32, String)> {
    sqlx::query_as(
        "SELECT instructor_id, rmp_legacy_id, source FROM instructor_rmp_links \
         ORDER BY instructor_id, rmp_legacy_id",
    )
    .fetch_all(pool)
    .await
    .expect("fetch links")
}

/// The stored `(status, score)` for one candidate pair, if a row exists.
async fn candidate(pool: &PgPool, instructor_id: i32, legacy_id: i32) -> Option<(String, f32)> {
    sqlx::query_as(
        "SELECT status, score FROM rmp_match_candidates \
         WHERE instructor_id = $1 AND rmp_legacy_id = $2",
    )
    .bind(instructor_id)
    .bind(legacy_id)
    .fetch_optional(pool)
    .await
    .expect("fetch candidate")
}

/// The review subjects and years denormalized onto a candidate row.
async fn candidate_review_data(pool: &PgPool, instructor_id: i32, legacy_id: i32) -> (Vec<String>, Vec<i16>) {
    sqlx::query_as(
        "SELECT review_subjects, review_years FROM rmp_match_candidates \
         WHERE instructor_id = $1 AND rmp_legacy_id = $2",
    )
    .bind(instructor_id)
    .bind(legacy_id)
    .fetch_one(pool)
    .await
    .expect("fetch candidate review data")
}

async fn match_status(pool: &PgPool, instructor_id: i32) -> String {
    let (status,): (String,) =
        sqlx::query_as("SELECT status FROM instructor_rmp_match_status WHERE instructor_id = $1")
            .bind(instructor_id)
            .fetch_one(pool)
            .await
            .expect("fetch instructor status");
    status
}

/// RMP profiles claimed by more than one instructor. Must always be zero.
async fn contested_profiles(pool: &PgPool) -> i64 {
    let (count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM (SELECT rmp_legacy_id FROM instructor_rmp_links \
         GROUP BY rmp_legacy_id HAVING COUNT(DISTINCT instructor_id) > 1) t",
    )
    .fetch_one(pool)
    .await
    .expect("count contested profiles");
    count
}

/// Candidates marked accepted with no link behind them. Must always be zero.
async fn orphan_accepted(pool: &PgPool) -> i64 {
    let (count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM rmp_match_candidates c WHERE c.status = 'accepted' \
         AND NOT EXISTS (SELECT 1 FROM instructor_rmp_links l \
             WHERE l.instructor_id = c.instructor_id AND l.rmp_legacy_id = c.rmp_legacy_id)",
    )
    .fetch_one(pool)
    .await
    .expect("count orphaned accepted candidates");
    count
}

/// Compare a stored REAL score against the expected composite.
fn close(actual: f32, expected: f32) -> bool {
    (actual - expected).abs() < 1e-5
}

/// A full-strength name with no rival profile links even though nothing
/// positively confirms the subject: score 0.85, below the auto-accept threshold.
#[tokio::test]
async fn sole_uncontradicted_candidate_auto_links() {
    let pool = test_db!().await;
    let instructor = insert_instructor(&pool, "Saldana, Liliana", Some("saldana@utsa.edu")).await;
    insert_rmp_professor(&pool, 1001, "Liliana", "Saldana", Some("Spanish"), 56).await;

    let stats = generate_candidates(&pool).await.expect("generate candidates");

    check!(stats.auto_matched == 1);
    check!(stats.candidates_created == 1);
    check!(links(&pool).await == vec![(instructor, 1001, "auto".to_owned())]);
    check!(match_status(&pool, instructor).await == "auto");

    let (status, score) = candidate(&pool, instructor, 1001).await.expect("candidate row");
    check!(status == "accepted");
    check!(close(score, 0.85));
}

/// Review course codes that land in another subject entirely drag the merged
/// subject evidence to 0.2, which the auto-eligibility gate refuses outright.
#[tokio::test]
async fn contradicting_review_subjects_block_auto_link() {
    let pool = test_db!().await;
    let instructor = insert_instructor(&pool, "Packham, Christopher", Some("packham@utsa.edu")).await;
    insert_taught_course(&pool, instructor, "CS", "10001").await;
    insert_rmp_professor(&pool, 1002, "Christopher", "Packham", Some("History"), 50).await;
    for _ in 0..3 {
        insert_rmp_review(&pool, 1002, "HIS1043", 2021).await;
    }

    let stats = generate_candidates(&pool).await.expect("generate candidates");

    check!(stats.auto_matched == 0);
    check!(stats.pending_review == 1);
    check!(links(&pool).await.is_empty());
    check!(match_status(&pool, instructor).await == "pending");

    let (status, score) = candidate(&pool, instructor, 1002).await.expect("candidate row");
    check!(status == "pending");
    check!(close(score, 0.76));

    // The review columns are filled by a second pass over the inserted rows.
    let (subjects, years) = candidate_review_data(&pool, instructor, 1002).await;
    check!(subjects == vec!["HIS".to_owned()]);
    check!(years == vec![2021i16]);
}

/// A department alone does not decide a match. RMP's strings frequently fail to
/// map onto a Banner subject even for the right person, so a sole candidate on
/// an exact name still links; only both subject signals disagreeing blocks it.
#[tokio::test]
async fn department_mismatch_alone_does_not_block_auto_link() {
    let pool = test_db!().await;
    let instructor = insert_instructor(&pool, "Fenwick, Alaric", Some("fenwick@utsa.edu")).await;
    insert_taught_course(&pool, instructor, "CS", "10001").await;
    insert_rmp_professor(&pool, 1003, "Alaric", "Fenwick", Some("History"), 50).await;

    generate_candidates(&pool).await.expect("generate candidates");

    check!(links(&pool).await == vec![(instructor, 1003, "auto".to_owned())]);
    check!(match_status(&pool, instructor).await == "auto");

    let (status, score) = candidate(&pool, instructor, 1003).await.expect("candidate row");
    check!(status == "accepted");
    check!(close(score, 0.85));
}

/// "Not Specified" is a missing department, not a conflicting one, so it must
/// not be scored as a mismatch.
#[tokio::test]
async fn placeholder_department_is_treated_as_absent() {
    let pool = test_db!().await;
    let instructor = insert_instructor(&pool, "Ortiz, Nadia", Some("ortiz@utsa.edu")).await;
    insert_taught_course(&pool, instructor, "CS", "10001").await;
    insert_rmp_professor(&pool, 1020, "Nadia", "Ortiz", Some("Not Specified"), 50).await;
    insert_rmp_review(&pool, 1020, "HIS1043", 2021).await;

    generate_candidates(&pool).await.expect("generate candidates");

    let (_, score) = candidate(&pool, instructor, 1020).await.expect("candidate row");
    // A real mismatch would drag the department signal to 0.2 and the score to 0.76.
    check!(close(score, 0.85));
}

/// Two instructor rows share a name and both clear auto-accept, but the profile
/// is unique: the heavier course load wins and the loser drops to review.
#[tokio::test]
async fn contested_profile_links_once_to_the_heavier_load() {
    let pool = test_db!().await;
    let winner = insert_instructor(&pool, "Smith, John", Some("john.a@utsa.edu")).await;
    let loser = insert_instructor(&pool, "Smith, John", Some("john.b@utsa.edu")).await;
    for crn in ["10001", "10002", "10003"] {
        insert_taught_course(&pool, winner, "CS", crn).await;
    }
    insert_taught_course(&pool, loser, "CS", "10004").await;
    insert_rmp_professor(&pool, 1004, "John", "Smith", Some("Computer Science"), 0).await;

    let stats = generate_candidates(&pool).await.expect("generate candidates");

    check!(stats.auto_matched == 1);
    check!(stats.candidates_created == 2);
    check!(links(&pool).await == vec![(winner, 1004, "auto".to_owned())]);
    check!(contested_profiles(&pool).await == 0);
    check!(orphan_accepted(&pool).await == 0);

    check!(match_status(&pool, winner).await == "auto");
    check!(match_status(&pool, loser).await == "pending");

    let (winner_status, winner_score) = candidate(&pool, winner, 1004).await.expect("winner row");
    check!(winner_status == "accepted");
    check!(close(winner_score, 0.95));

    let (loser_status, loser_score) = candidate(&pool, loser, 1004).await.expect("loser row");
    check!(loser_status == "pending");
    check!(close(loser_score, 0.95));
}

/// A manual link survives regeneration untouched, and the profile it holds is
/// off limits to any other instructor that would otherwise auto-link to it.
#[tokio::test]
async fn manual_link_survives_and_blocks_rival_claims() {
    let pool = test_db!().await;
    let owner = insert_instructor(&pool, "Ellis, Ronald", Some("ron.a@utsa.edu")).await;
    let rival = insert_instructor(&pool, "Ellis, Ronald", Some("ron.b@utsa.edu")).await;
    insert_taught_course(&pool, rival, "CS", "10001").await;
    insert_rmp_professor(&pool, 1005, "Ronald", "Ellis", Some("Computer Science"), 30).await;
    sqlx::query(
        "INSERT INTO instructor_rmp_links (instructor_id, rmp_legacy_id, source) \
         VALUES ($1, 1005, 'manual')",
    )
    .bind(owner)
    .execute(&pool)
    .await
    .expect("seed manual link");

    let stats = generate_candidates(&pool).await.expect("generate candidates");

    check!(stats.deleted_auto_links == 0);
    check!(stats.auto_matched == 0);
    check!(links(&pool).await == vec![(owner, 1005, "manual".to_owned())]);

    // A confirmed instructor is never rescored, so it gains no candidate row.
    check!(match_status(&pool, owner).await == "confirmed");
    check!(candidate(&pool, owner, 1005).await.is_none());

    check!(match_status(&pool, rival).await == "pending");
    let (rival_status, _) = candidate(&pool, rival, 1005).await.expect("rival row");
    check!(rival_status == "pending");
}

/// A rejected pair is the one candidate row regeneration preserves, and the pair
/// is skipped before scoring, so no replacement row and no link appear.
#[tokio::test]
async fn rejected_candidate_survives_regeneration_unscored() {
    let pool = test_db!().await;
    let instructor = insert_instructor(&pool, "Garcia, Maria", Some("garcia@utsa.edu")).await;
    insert_taught_course(&pool, instructor, "CS", "10001").await;
    insert_rmp_professor(&pool, 1006, "Maria", "Garcia", Some("Computer Science"), 40).await;
    sqlx::query(
        "INSERT INTO rmp_match_candidates (instructor_id, rmp_legacy_id, score, status) \
         VALUES ($1, 1006, 0.9, 'rejected')",
    )
    .bind(instructor)
    .execute(&pool)
    .await
    .expect("seed rejected candidate");

    let stats = generate_candidates(&pool).await.expect("generate candidates");

    check!(stats.deleted_pending_candidates == 0);
    check!(stats.candidates_created == 0);
    check!(stats.auto_matched == 0);
    check!(links(&pool).await.is_empty());

    let (status, score) = candidate(&pool, instructor, 1006).await.expect("candidate row");
    check!(status == "rejected");
    check!(close(score, 0.9));

    // Nothing is left open, so the instructor reads as settled, not untouched.
    check!(match_status(&pool, instructor).await == "rejected");
}

/// RMP holds duplicate profiles for one person; linking all of them is intended.
/// Two rivals depress uniqueness to 0.5, so each scores exactly 0.875.
#[tokio::test]
async fn duplicate_profiles_for_one_person_all_link() {
    let pool = test_db!().await;
    let instructor = insert_instructor(&pool, "Nguyen, Minh", Some("nguyen@utsa.edu")).await;
    insert_taught_course(&pool, instructor, "CS", "10001").await;
    insert_rmp_professor(&pool, 1007, "Minh", "Nguyen", Some("Computer Science"), 0).await;
    insert_rmp_professor(&pool, 1008, "Minh", "Nguyen", Some("Computer Science"), 0).await;

    let stats = generate_candidates(&pool).await.expect("generate candidates");

    // One person, so one auto match, whatever the number of profiles.
    check!(stats.auto_matched == 1);
    check!(stats.pending_review == 0);
    check!(
        links(&pool).await
            == vec![
                (instructor, 1007, "auto".to_owned()),
                (instructor, 1008, "auto".to_owned()),
            ]
    );
    check!(match_status(&pool, instructor).await == "auto");

    for legacy_id in [1007, 1008] {
        let (status, score) = candidate(&pool, instructor, legacy_id).await.expect("candidate row");
        check!(status == "accepted");
        check!(close(score, 0.875));
    }
}

/// One profile identifies the person, but a sibling whose own evidence points
/// at another field is a different teacher of the same name, so it stays out.
#[tokio::test]
async fn contradicted_sibling_profile_is_not_inherited() {
    let pool = test_db!().await;
    let instructor = insert_instructor(&pool, "Nguyen, Minh", Some("nguyen@utsa.edu")).await;
    insert_taught_course(&pool, instructor, "CS", "10001").await;
    insert_rmp_professor(&pool, 1013, "Minh", "Nguyen", Some("Computer Science"), 0).await;
    insert_rmp_professor(&pool, 1014, "Minh", "Nguyen", Some("History"), 0).await;
    for _ in 0..3 {
        insert_rmp_review(&pool, 1014, "HIS1043", 2021).await;
    }

    let stats = generate_candidates(&pool).await.expect("generate candidates");

    check!(stats.auto_matched == 1);
    check!(links(&pool).await == vec![(instructor, 1013, "auto".to_owned())]);
    check!(match_status(&pool, instructor).await == "auto");

    let (identifying_status, identifying_score) = candidate(&pool, instructor, 1013).await.expect("identifier");
    check!(identifying_status == "accepted");
    check!(close(identifying_score, 0.875));

    let (sibling_status, sibling_score) = candidate(&pool, instructor, 1014).await.expect("sibling");
    check!(sibling_status == "pending");
    check!(close(sibling_score, 0.635));
}

/// Inheritance is withheld when a second instructor also claims the sibling,
/// so only the directly eligible profile is linked.
#[tokio::test]
async fn contested_sibling_profile_is_not_inherited() {
    let pool = test_db!().await;
    let winner = insert_instructor(&pool, "Nguyen, Minh", Some("minh.a@utsa.edu")).await;
    let loser = insert_instructor(&pool, "Nguyen, Minh", Some("minh.b@utsa.edu")).await;
    insert_taught_course(&pool, winner, "CS", "10001").await;
    insert_taught_course(&pool, winner, "CS", "10002").await;
    insert_taught_course(&pool, loser, "CS", "10003").await;
    insert_rmp_professor(&pool, 1015, "Minh", "Nguyen", Some("Computer Science"), 0).await;
    insert_rmp_professor(&pool, 1016, "Minh", "Nguyen", Some("History"), 0).await;
    for _ in 0..3 {
        insert_rmp_review(&pool, 1016, "HIS1043", 2021).await;
    }

    let stats = generate_candidates(&pool).await.expect("generate candidates");

    check!(stats.auto_matched == 1);
    check!(links(&pool).await == vec![(winner, 1015, "auto".to_owned())]);
    check!(match_status(&pool, winner).await == "auto");
    check!(match_status(&pool, loser).await == "pending");
    check!(orphan_accepted(&pool).await == 0);

    for instructor in [winner, loser] {
        let (status, _) = candidate(&pool, instructor, 1016).await.expect("sibling candidate");
        check!(status == "pending");
    }
}

/// A nickname expansion on both sides discounts the name to 0.7. Confirmed
/// subject evidence routes the pair through the threshold, which 0.85 misses.
#[tokio::test]
async fn nickname_only_name_match_stays_below_the_threshold() {
    let pool = test_db!().await;
    let instructor = insert_instructor(&pool, "Packham, Christopher", Some("packham@utsa.edu")).await;
    insert_taught_course(&pool, instructor, "CS", "10001").await;
    insert_rmp_professor(&pool, 1017, "Chris", "Packham", Some("Computer Science"), 50).await;

    let stats = generate_candidates(&pool).await.expect("generate candidates");

    check!(stats.auto_matched == 0);
    check!(stats.pending_review == 1);
    check!(links(&pool).await.is_empty());
    check!(match_status(&pool, instructor).await == "pending");

    let (status, score) = candidate(&pool, instructor, 1017).await.expect("candidate row");
    check!(status == "pending");
    check!(close(score, 0.85));
}

/// With nothing eligible to match, the run still reports the deletions it made.
#[tokio::test]
async fn no_eligible_instructors_returns_empty_stats() {
    let pool = test_db!().await;
    insert_rmp_professor(&pool, 1018, "Minh", "Nguyen", Some("Computer Science"), 0).await;

    let stats = generate_candidates(&pool).await.expect("generate candidates");

    check!(stats.total_processed == 0);
    check!(stats.candidates_created == 0);
    check!(stats.auto_matched == 0);
    check!(stats.pending_review == 0);
    check!(stats.skipped_unparseable == 0);
    check!(stats.skipped_no_candidates == 0);
    check!(links(&pool).await.is_empty());
}

/// Two genuinely different RMP people share a first-name key. Both clear the
/// score gate, so the pipeline refuses to guess and defers the whole instructor.
#[tokio::test]
async fn distinct_people_sharing_a_name_defer_to_review() {
    let pool = test_db!().await;
    let instructor = insert_instructor(&pool, "Smith, Jane", Some("jane@utsa.edu")).await;
    insert_taught_course(&pool, instructor, "CS", "10001").await;
    insert_rmp_professor(&pool, 1009, "Jane", "Smith", Some("Computer Science"), 0).await;
    insert_rmp_professor(&pool, 1010, "Jane Marie", "Smith", Some("Computer Science"), 0).await;

    let stats = generate_candidates(&pool).await.expect("generate candidates");

    check!(stats.auto_matched == 0);
    check!(stats.candidates_created == 2);
    check!(stats.pending_review == 1);
    check!(links(&pool).await.is_empty());
    check!(match_status(&pool, instructor).await == "pending");

    for legacy_id in [1009, 1010] {
        let (status, score) = candidate(&pool, instructor, legacy_id).await.expect("candidate row");
        check!(status == "pending");
        check!(close(score, 0.875));
    }
}

/// Regeneration wipes every auto link and non-rejected candidate first, so state
/// from a previous run that no longer matches disappears.
#[tokio::test]
async fn stale_auto_state_is_cleared_before_rescoring() {
    let pool = test_db!().await;
    let instructor = insert_instructor(&pool, "Zhao, Peng", Some("zhao@utsa.edu")).await;
    insert_taught_course(&pool, instructor, "CS", "10001").await;
    insert_rmp_professor(&pool, 1011, "Someone", "Else", Some("Computer Science"), 20).await;
    sqlx::query(
        "INSERT INTO instructor_rmp_links (instructor_id, rmp_legacy_id, source) \
         VALUES ($1, 1011, 'auto')",
    )
    .bind(instructor)
    .execute(&pool)
    .await
    .expect("seed stale link");
    sqlx::query(
        "INSERT INTO rmp_match_candidates (instructor_id, rmp_legacy_id, score, status) \
         VALUES ($1, 1011, 0.91, 'pending')",
    )
    .bind(instructor)
    .execute(&pool)
    .await
    .expect("seed stale candidate");

    let stats = generate_candidates(&pool).await.expect("generate candidates");

    check!(stats.deleted_auto_links == 1);
    check!(stats.deleted_pending_candidates == 1);
    check!(stats.skipped_no_candidates == 1);
    check!(stats.candidates_created == 0);
    check!(links(&pool).await.is_empty());
    check!(candidate(&pool, instructor, 1011).await.is_none());
    check!(match_status(&pool, instructor).await == "unmatched");
}

/// An unparseable display name is skipped outright rather than failing the run.
#[tokio::test]
async fn unparseable_display_name_is_skipped() {
    let pool = test_db!().await;
    let instructor = insert_instructor(&pool, "SingleName", Some("single@utsa.edu")).await;
    insert_rmp_professor(&pool, 1012, "Single", "Name", Some("Computer Science"), 20).await;

    let stats = generate_candidates(&pool).await.expect("generate candidates");

    check!(stats.total_processed == 1);
    check!(stats.skipped_unparseable == 1);
    check!(stats.candidates_created == 0);
    check!(links(&pool).await.is_empty());
    check!(match_status(&pool, instructor).await == "unmatched");
}

/// The two invariants the corpus harness checks, on a hermetic fixture that mixes
/// a contested profile, a manual hold, a rejection and a duplicate-profile person.
#[tokio::test]
async fn mixed_fixture_holds_link_and_acceptance_invariants() {
    let pool = test_db!().await;
    let winner = insert_instructor(&pool, "Ellis, Ronald", Some("ron.a@utsa.edu")).await;
    let loser = insert_instructor(&pool, "Ellis, Ronald", Some("ron.b@utsa.edu")).await;
    let duplicated = insert_instructor(&pool, "Nguyen, Minh", Some("nguyen@utsa.edu")).await;
    let refused = insert_instructor(&pool, "Garcia, Maria", Some("garcia@utsa.edu")).await;
    let owner = insert_instructor(&pool, "Zhao, Peng", Some("peng.a@utsa.edu")).await;
    let rival = insert_instructor(&pool, "Zhao, Peng", Some("peng.b@utsa.edu")).await;

    insert_taught_course(&pool, winner, "CS", "10001").await;
    insert_taught_course(&pool, winner, "CS", "10002").await;
    insert_taught_course(&pool, loser, "CS", "10003").await;
    insert_taught_course(&pool, duplicated, "CS", "10004").await;
    insert_taught_course(&pool, refused, "CS", "10005").await;
    insert_taught_course(&pool, rival, "CS", "10006").await;

    insert_rmp_professor(&pool, 2001, "Ronald", "Ellis", Some("Computer Science"), 0).await;
    insert_rmp_professor(&pool, 2002, "Minh", "Nguyen", Some("Computer Science"), 0).await;
    insert_rmp_professor(&pool, 2003, "Minh", "Nguyen", Some("Computer Science"), 0).await;
    insert_rmp_professor(&pool, 2004, "Maria", "Garcia", Some("Computer Science"), 0).await;
    insert_rmp_professor(&pool, 2005, "Peng", "Zhao", Some("Computer Science"), 0).await;

    sqlx::query(
        "INSERT INTO rmp_match_candidates (instructor_id, rmp_legacy_id, score, status) \
         VALUES ($1, 2004, 0.9, 'rejected')",
    )
    .bind(refused)
    .execute(&pool)
    .await
    .expect("seed rejected candidate");
    sqlx::query(
        "INSERT INTO instructor_rmp_links (instructor_id, rmp_legacy_id, source) \
         VALUES ($1, 2005, 'manual')",
    )
    .bind(owner)
    .execute(&pool)
    .await
    .expect("seed manual link");

    let stats = generate_candidates(&pool).await.expect("generate candidates");

    check!(contested_profiles(&pool).await == 0);
    check!(orphan_accepted(&pool).await == 0);
    check!(
        links(&pool).await
            == vec![
                (winner, 2001, "auto".to_owned()),
                (duplicated, 2002, "auto".to_owned()),
                (duplicated, 2003, "auto".to_owned()),
                (owner, 2005, "manual".to_owned()),
            ]
    );

    // Contested loser and manually blocked rival are the only review entries.
    check!(stats.pending_review == 2);
    check!(match_status(&pool, loser).await == "pending");
    check!(match_status(&pool, rival).await == "pending");
    // Its one candidate was turned down, so it is settled rather than untouched.
    check!(match_status(&pool, refused).await == "rejected");
    check!(match_status(&pool, owner).await == "confirmed");
}
