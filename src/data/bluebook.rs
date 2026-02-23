//! Database operations for BlueBook evaluation data.

use anyhow::Result;
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

/// Bulk upsert BlueBook evaluations using the UNNEST pattern.
///
/// On conflict (same section/term/instructor), updates all evaluation fields
/// and resets `scraped_at` to NOW().
#[allow(dead_code)]
pub async fn batch_upsert_bluebook_evaluations(
    evaluations: &[BlueBookEvaluation],
    db_pool: &PgPool,
) -> Result<()> {
    if evaluations.is_empty() {
        return Ok(());
    }

    let subjects: Vec<&str> = evaluations.iter().map(|e| e.subject.as_str()).collect();
    let course_numbers: Vec<&str> = evaluations
        .iter()
        .map(|e| e.course_number.as_str())
        .collect();
    let sections: Vec<&str> = evaluations.iter().map(|e| e.section.as_str()).collect();
    let crns: Vec<&str> = evaluations.iter().map(|e| e.crn.as_str()).collect();
    let terms: Vec<&str> = evaluations.iter().map(|e| e.term.as_str()).collect();
    let instructor_names: Vec<&str> = evaluations
        .iter()
        .map(|e| e.instructor_name.as_str())
        .collect();
    let instructor_ratings: Vec<Option<f32>> =
        evaluations.iter().map(|e| e.instructor_rating).collect();
    let instructor_response_counts: Vec<Option<i32>> = evaluations
        .iter()
        .map(|e| e.instructor_response_count)
        .collect();
    let course_ratings: Vec<Option<f32>> = evaluations.iter().map(|e| e.course_rating).collect();
    let course_response_counts: Vec<Option<i32>> = evaluations
        .iter()
        .map(|e| e.course_response_count)
        .collect();
    let departments: Vec<Option<&str>> = evaluations
        .iter()
        .map(|e| e.department.as_deref())
        .collect();

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
    .execute(db_pool)
    .await
    .map_err(|e| anyhow::anyhow!("Failed to batch upsert BlueBook evaluations: {}", e))?;

    Ok(())
}
