//! Database operations for RateMyProfessors data.

use crate::rmp::{RmpProfessor, RmpProfessorDetail, RmpReview};
use anyhow::Result;
use sqlx::PgPool;
use std::collections::HashSet;

/// Bulk upsert RMP professors using the UNNEST pattern.
///
/// Deduplicates by `legacy_id` before inserting — the RMP API can return
/// the same professor on multiple pages.
pub async fn batch_upsert_rmp_professors(
    professors: &[RmpProfessor],
    db_pool: &PgPool,
) -> Result<()> {
    if professors.is_empty() {
        return Ok(());
    }

    // Deduplicate: keep last occurrence per legacy_id (latest page wins)
    let mut seen = HashSet::new();
    let deduped: Vec<&RmpProfessor> = professors
        .iter()
        .rev()
        .filter(|p| seen.insert(p.legacy_id))
        .collect();

    let legacy_ids: Vec<i32> = deduped.iter().map(|p| p.legacy_id).collect();
    let graphql_ids: Vec<&str> = deduped.iter().map(|p| p.graphql_id.as_str()).collect();
    let first_names: Vec<String> = deduped
        .iter()
        .map(|p| p.first_name.trim().to_string())
        .collect();
    let first_name_refs: Vec<&str> = first_names.iter().map(|s| s.as_str()).collect();
    let last_names: Vec<String> = deduped
        .iter()
        .map(|p| p.last_name.trim().to_string())
        .collect();
    let last_name_refs: Vec<&str> = last_names.iter().map(|s| s.as_str()).collect();
    let departments: Vec<Option<&str>> = deduped.iter().map(|p| p.department.as_deref()).collect();
    let avg_ratings: Vec<Option<f32>> = deduped.iter().map(|p| p.avg_rating).collect();
    let avg_difficulties: Vec<Option<f32>> = deduped.iter().map(|p| p.avg_difficulty).collect();
    let num_ratings: Vec<i32> = deduped.iter().map(|p| p.num_ratings).collect();
    let would_take_again_pcts: Vec<Option<f32>> =
        deduped.iter().map(|p| p.would_take_again_pct).collect();

    sqlx::query(
        r#"
        INSERT INTO rmp_professors (
            legacy_id, graphql_id, first_name, last_name, department,
            avg_rating, avg_difficulty, num_ratings, would_take_again_pct,
            last_synced_at
        )
        SELECT
            v.legacy_id, v.graphql_id, v.first_name, v.last_name, v.department,
            v.avg_rating, v.avg_difficulty, v.num_ratings, v.would_take_again_pct,
            NOW()
        FROM UNNEST(
            $1::int4[], $2::text[], $3::text[], $4::text[], $5::text[],
            $6::real[], $7::real[], $8::int4[], $9::real[]
        ) AS v(
            legacy_id, graphql_id, first_name, last_name, department,
            avg_rating, avg_difficulty, num_ratings, would_take_again_pct
        )
        ON CONFLICT (legacy_id)
        DO UPDATE SET
            graphql_id = EXCLUDED.graphql_id,
            first_name = EXCLUDED.first_name,
            last_name = EXCLUDED.last_name,
            department = EXCLUDED.department,
            avg_rating = EXCLUDED.avg_rating,
            avg_difficulty = EXCLUDED.avg_difficulty,
            num_ratings = EXCLUDED.num_ratings,
            would_take_again_pct = EXCLUDED.would_take_again_pct,
            last_synced_at = EXCLUDED.last_synced_at
        "#,
    )
    .bind(&legacy_ids)
    .bind(&graphql_ids)
    .bind(&first_name_refs)
    .bind(&last_name_refs)
    .bind(&departments)
    .bind(&avg_ratings)
    .bind(&avg_difficulties)
    .bind(&num_ratings)
    .bind(&would_take_again_pcts)
    .execute(db_pool)
    .await
    .map_err(|e| anyhow::anyhow!("Failed to batch upsert RMP professors: {}", e))?;

    Ok(())
}

/// Retrieve RMP rating data for an instructor by instructor id.
///
/// Returns `(avg_rating, num_ratings)` for the best linked RMP profile
/// (most ratings). Returns `None` if no link exists.
#[allow(dead_code)]
pub async fn get_instructor_rmp_data(
    db_pool: &PgPool,
    instructor_id: i32,
) -> Result<Option<(f32, i32)>> {
    let row: Option<(f32, i32)> = sqlx::query_as(
        r#"
        SELECT rp.avg_rating, rp.num_ratings
        FROM instructor_rmp_links irl
        JOIN rmp_professors rp ON rp.legacy_id = irl.rmp_legacy_id
        WHERE irl.instructor_id = $1
          AND rp.avg_rating IS NOT NULL
        ORDER BY rp.num_ratings DESC NULLS LAST
        LIMIT 1
        "#,
    )
    .bind(instructor_id)
    .fetch_optional(db_pool)
    .await?;
    Ok(row)
}

/// Unmatch an instructor from an RMP profile.
///
/// Removes the link from `instructor_rmp_links` and updates the instructor's
/// `rmp_match_status` to 'unmatched' if no links remain.
///
/// If `rmp_legacy_id` is `Some`, removes only that specific link.
/// If `None`, removes all links for the instructor.
pub async fn unmatch_instructor(
    db_pool: &PgPool,
    instructor_id: i32,
    rmp_legacy_id: Option<i32>,
) -> Result<()> {
    let mut tx = db_pool.begin().await?;

    // Delete specific link or all links
    if let Some(legacy_id) = rmp_legacy_id {
        sqlx::query(
            "DELETE FROM instructor_rmp_links WHERE instructor_id = $1 AND rmp_legacy_id = $2",
        )
        .bind(instructor_id)
        .bind(legacy_id)
        .execute(&mut *tx)
        .await?;
    } else {
        sqlx::query("DELETE FROM instructor_rmp_links WHERE instructor_id = $1")
            .bind(instructor_id)
            .execute(&mut *tx)
            .await?;
    }

    // Check if any links remain
    let (remaining,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM instructor_rmp_links WHERE instructor_id = $1")
            .bind(instructor_id)
            .fetch_one(&mut *tx)
            .await?;

    // Update instructor status if no links remain
    if remaining == 0 {
        sqlx::query("UPDATE instructors SET rmp_match_status = 'unmatched' WHERE id = $1")
            .bind(instructor_id)
            .execute(&mut *tx)
            .await?;
    }

    // Reset accepted candidates back to pending when unmatching
    // This allows the candidates to be re-matched later
    if let Some(legacy_id) = rmp_legacy_id {
        // Reset only the specific candidate
        sqlx::query(
            "UPDATE rmp_match_candidates 
             SET status = 'pending', resolved_at = NULL, resolved_by = NULL 
             WHERE instructor_id = $1 AND rmp_legacy_id = $2 AND status = 'accepted'",
        )
        .bind(instructor_id)
        .bind(legacy_id)
        .execute(&mut *tx)
        .await?;
    } else {
        // Reset all accepted candidates for this instructor
        sqlx::query(
            "UPDATE rmp_match_candidates 
             SET status = 'pending', resolved_at = NULL, resolved_by = NULL 
             WHERE instructor_id = $1 AND status = 'accepted'",
        )
        .bind(instructor_id)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}

/// Get professors eligible for review scraping.
///
/// Returns `(legacy_id, graphql_id)` pairs for professors whose
/// `reviews_last_scraped_at` is NULL or past their individual interval.
pub async fn get_professors_eligible_for_review_scrape(
    db_pool: &PgPool,
    limit: i64,
) -> Result<Vec<(i32, String)>> {
    let rows: Vec<(i32, String)> = sqlx::query_as(
        r#"
        SELECT legacy_id, graphql_id FROM rmp_professors
        WHERE reviews_last_scraped_at IS NULL
           OR (reviews_last_scraped_at + review_scrape_interval) <= NOW()
        ORDER BY reviews_last_scraped_at ASC NULLS FIRST
        LIMIT $1
        "#,
    )
    .bind(limit)
    .fetch_all(db_pool)
    .await?;

    Ok(rows)
}

/// Update extended profile columns on `rmp_professors` for one professor.
pub async fn upsert_professor_detail(detail: &RmpProfessorDetail, db_pool: &PgPool) -> Result<()> {
    let course_codes_json = serde_json::to_value(&detail.course_codes)?;

    sqlx::query(
        r#"
        UPDATE rmp_professors SET
            ratings_r1 = $1,
            ratings_r2 = $2,
            ratings_r3 = $3,
            ratings_r4 = $4,
            ratings_r5 = $5,
            course_codes = $6
        WHERE legacy_id = $7
        "#,
    )
    .bind(detail.ratings_r1)
    .bind(detail.ratings_r2)
    .bind(detail.ratings_r3)
    .bind(detail.ratings_r4)
    .bind(detail.ratings_r5)
    .bind(&course_codes_json)
    .bind(detail.legacy_id)
    .execute(db_pool)
    .await?;

    Ok(())
}

/// Delete all reviews for a professor and bulk insert new ones.
///
/// Uses a transaction to ensure atomicity. Delete-and-reinsert is simpler
/// than composite-key upsert since RMP reviews lack stable IDs.
pub async fn replace_professor_reviews(
    legacy_id: i32,
    reviews: &[RmpReview],
    db_pool: &PgPool,
) -> Result<()> {
    let mut tx = db_pool.begin().await?;

    sqlx::query("DELETE FROM rmp_reviews WHERE rmp_legacy_id = $1")
        .bind(legacy_id)
        .execute(&mut *tx)
        .await?;

    for review in reviews {
        sqlx::query(
            r#"
            INSERT INTO rmp_reviews (
                rmp_legacy_id, comment, class, grade, rating_tags,
                helpful_rating, clarity_rating, difficulty_rating,
                would_take_again, is_for_credit, is_for_online_class,
                attendance_mandatory, flag_status, textbook_use,
                thumbs_up_total, thumbs_down_total, posted_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)
            "#,
        )
        .bind(legacy_id)
        .bind(&review.comment)
        .bind(&review.class)
        .bind(&review.grade)
        .bind(&review.rating_tags)
        .bind(review.helpful_rating)
        .bind(review.clarity_rating)
        .bind(review.difficulty_rating)
        .bind(review.would_take_again)
        .bind(review.is_for_credit)
        .bind(review.is_for_online_class)
        .bind(&review.attendance_mandatory)
        .bind(&review.flag_status)
        .bind(review.textbook_use)
        .bind(review.thumbs_up_total)
        .bind(review.thumbs_down_total)
        .bind(review.posted_at)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}

/// Mark a professor's reviews as scraped and compute the next scrape interval.
///
/// Only called after `replace_professor_reviews` succeeds to prevent
/// failed inserts from pushing the next scrape forward.
pub async fn mark_professor_reviews_scraped(
    legacy_id: i32,
    num_ratings: i32,
    db_pool: &PgPool,
) -> Result<()> {
    let interval_days = match num_ratings {
        0 => 14,
        1..=5 => 7,
        6..=20 => 3,
        _ => 1,
    };

    sqlx::query(
        r#"
        UPDATE rmp_professors SET
            reviews_last_scraped_at = NOW(),
            review_scrape_interval = make_interval(days => $1)
        WHERE legacy_id = $2
        "#,
    )
    .bind(interval_days)
    .bind(legacy_id)
    .execute(db_pool)
    .await?;

    Ok(())
}
