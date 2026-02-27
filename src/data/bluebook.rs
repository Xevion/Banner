//! Database operations for BlueBook evaluation data.

use std::collections::HashMap;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::PgPool;

/// A parsed BlueBook course evaluation record.
#[derive(Debug, Clone)]
pub struct BlueBookEvaluation {
    pub subject: String,
    pub course_number: String,
    pub section: String,
    pub crn: String,
    pub term: String,
    pub instructor_name: String,
    pub instructor_rating: Option<f32>,
    pub instructor_response_count: Option<i32>,
    pub course_rating: Option<f32>,
    pub course_response_count: Option<i32>,
    pub department: Option<String>,
}

/// Deduplicate evaluations by the unique constraint key
/// `(subject, course_number, section, term, instructor_name)`.
///
/// When duplicates exist (e.g. Summer I and Summer II collapsing to the same
/// term code), keeps whichever entry has more response data.
fn deduplicate(evaluations: &[BlueBookEvaluation]) -> Vec<&BlueBookEvaluation> {
    let mut best: HashMap<(&str, &str, &str, &str, &str), &BlueBookEvaluation> = HashMap::new();

    for eval in evaluations {
        let key = (
            eval.subject.as_str(),
            eval.course_number.as_str(),
            eval.section.as_str(),
            eval.term.as_str(),
            eval.instructor_name.as_str(),
        );
        let entry = best.entry(key).or_insert(eval);
        // Prefer the entry with more response data
        let existing_responses =
            entry.instructor_response_count.unwrap_or(0) + entry.course_response_count.unwrap_or(0);
        let new_responses =
            eval.instructor_response_count.unwrap_or(0) + eval.course_response_count.unwrap_or(0);
        if new_responses > existing_responses {
            *entry = eval;
        }
    }

    best.into_values().collect()
}

/// Bulk upsert BlueBook evaluations using the UNNEST pattern.
///
/// Deduplicates by the unique constraint key before inserting -- PostgreSQL's
/// `ON CONFLICT DO UPDATE` cannot handle the same row appearing twice in one
/// statement. On conflict, updates all evaluation fields and resets `scraped_at`.
#[allow(dead_code)]
pub async fn batch_upsert_bluebook_evaluations(
    pool: &PgPool,
    evaluations: &[BlueBookEvaluation],
) -> Result<()> {
    if evaluations.is_empty() {
        return Ok(());
    }

    let deduped = deduplicate(evaluations);

    let subjects: Vec<&str> = deduped.iter().map(|e| e.subject.as_str()).collect();
    let course_numbers: Vec<&str> = deduped.iter().map(|e| e.course_number.as_str()).collect();
    let sections: Vec<&str> = deduped.iter().map(|e| e.section.as_str()).collect();
    let crns: Vec<&str> = deduped.iter().map(|e| e.crn.as_str()).collect();
    let terms: Vec<&str> = deduped.iter().map(|e| e.term.as_str()).collect();
    let instructor_names: Vec<&str> = deduped.iter().map(|e| e.instructor_name.as_str()).collect();
    let instructor_ratings: Vec<Option<f32>> =
        deduped.iter().map(|e| e.instructor_rating).collect();
    let instructor_response_counts: Vec<Option<i32>> = deduped
        .iter()
        .map(|e| e.instructor_response_count)
        .collect();
    let course_ratings: Vec<Option<f32>> = deduped.iter().map(|e| e.course_rating).collect();
    let course_response_counts: Vec<Option<i32>> =
        deduped.iter().map(|e| e.course_response_count).collect();
    let departments: Vec<Option<&str>> = deduped.iter().map(|e| e.department.as_deref()).collect();

    sqlx::query(
        r#"
        INSERT INTO bluebook_evaluations (
            subject, course_number, section, crn, term, instructor_name,
            instructor_rating, instructor_response_count,
            course_rating, course_response_count,
            department, scraped_at
        )
        SELECT
            v.subject, v.course_number, v.section, v.crn, v.term, v.instructor_name,
            v.instructor_rating, v.instructor_response_count,
            v.course_rating, v.course_response_count,
            v.department, NOW()
        FROM UNNEST(
            $1::text[], $2::text[], $3::text[], $4::text[], $5::text[], $6::text[],
            $7::real[], $8::int4[], $9::real[], $10::int4[], $11::text[]
        ) AS v(
            subject, course_number, section, crn, term, instructor_name,
            instructor_rating, instructor_response_count,
            course_rating, course_response_count,
            department
        )
        ON CONFLICT ON CONSTRAINT uq_bluebook_eval
        DO UPDATE SET
            crn = EXCLUDED.crn,
            instructor_rating = EXCLUDED.instructor_rating,
            instructor_response_count = EXCLUDED.instructor_response_count,
            course_rating = EXCLUDED.course_rating,
            course_response_count = EXCLUDED.course_response_count,
            department = EXCLUDED.department,
            scraped_at = EXCLUDED.scraped_at
        "#,
    )
    .bind(&subjects)
    .bind(&course_numbers)
    .bind(&sections)
    .bind(&crns)
    .bind(&terms)
    .bind(&instructor_names)
    .bind(&instructor_ratings)
    .bind(&instructor_response_counts)
    .bind(&course_ratings)
    .bind(&course_response_counts)
    .bind(&departments)
    .execute(pool)
    .await
    .context("Failed to batch upsert BlueBook evaluations")?;

    Ok(())
}

/// Load the last-scraped timestamp for every subject in `bluebook_subject_scrapes`.
pub async fn get_all_subject_scrape_times(pool: &PgPool) -> Result<HashMap<String, DateTime<Utc>>> {
    let rows = sqlx::query_as::<_, (String, DateTime<Utc>)>(
        "SELECT subject, last_scraped_at FROM bluebook_subject_scrapes",
    )
    .fetch_all(pool)
    .await
    .context("Failed to load subject scrape times")?;

    Ok(rows.into_iter().collect())
}

/// Returns the MAX(term) code per subject from `bluebook_evaluations`.
///
/// Used to classify subjects as recent vs. historical when deciding scrape intervals.
pub async fn get_subject_max_terms(pool: &PgPool) -> Result<HashMap<String, String>> {
    let rows = sqlx::query_as::<_, (String, Option<String>)>(
        "SELECT subject, MAX(term) FROM bluebook_evaluations GROUP BY subject",
    )
    .fetch_all(pool)
    .await
    .context("Failed to load subject max terms")?;

    Ok(rows
        .into_iter()
        .filter_map(|(subject, max_term)| max_term.map(|t| (subject, t)))
        .collect())
}

/// Upsert `last_scraped_at = NOW()` for the given subject in `bluebook_subject_scrapes`.
pub async fn mark_subject_scraped(pool: &PgPool, subject: &str) -> Result<()> {
    sqlx::query(
        "INSERT INTO bluebook_subject_scrapes (subject, last_scraped_at)
         VALUES ($1, NOW())
         ON CONFLICT (subject) DO UPDATE SET last_scraped_at = NOW()",
    )
    .bind(subject)
    .execute(pool)
    .await
    .context("Failed to mark subject scraped")?;

    Ok(())
}
