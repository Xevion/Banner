//! Database query functions for courses, used by the web API.

use super::context::DbContext;
use super::events::{AuditLogEvent, DomainEvent};
use crate::banner::Course as BannerCourse;
use crate::data::batch::batch_upsert_courses as batch_upsert_impl;
use crate::data::models::{Course, CourseInstructorDetail, UpsertCounts};
use anyhow::Result;
use sqlx::PgPool;
use std::collections::HashMap;
use ts_rs::TS;

/// Column to sort search results by.
#[derive(Debug, Clone, Copy, serde::Deserialize, serde::Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum SortColumn {
    CourseCode,
    Title,
    Instructor,
    Time,
    Seats,
}

/// Sort direction.
#[derive(Debug, Clone, Copy, serde::Deserialize, serde::Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum SortDirection {
    Asc,
    Desc,
}

/// Aggregate min/max ranges for filter sliders, computed per-term.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FilterRanges {
    pub course_number_min: i32,
    pub course_number_max: i32,
    pub credit_hour_min: i32,
    pub credit_hour_max: i32,
    pub wait_count_max: i32,
}

/// Shared WHERE clause for course search filters.
///
/// Parameters $1-$17 match the bind order in `search_courses`.
///
/// Note: Course number filtering extracts numeric prefix to support alphanumeric
/// course numbers (e.g., "015X", "399H"). The numeric part is compared against
/// the range, so "399H" matches a search for courses 300-400.
const SEARCH_WHERE: &str = r#"
    WHERE term_code = $1
      AND ($2::text[] IS NULL OR subject = ANY($2))
      AND ($3::text IS NULL OR title_search @@ plainto_tsquery('simple', $3) OR title ILIKE '%' || $3 || '%')
      AND ($4::int IS NULL OR (substring(course_number from '^\d+'))::int >= $4)
      AND ($5::int IS NULL OR (substring(course_number from '^\d+'))::int <= $5)
      AND ($6::bool = false OR max_enrollment > enrollment)
      AND ($7::text[] IS NULL OR instructional_method = ANY($7))
      AND ($8::text[] IS NULL OR campus = ANY($8))
      AND ($9::int IS NULL OR wait_count <= $9)
      AND ($10::text[] IS NULL OR EXISTS (
          SELECT 1 FROM jsonb_array_elements(meeting_times) AS mt,
               LATERAL jsonb_array_elements_text(mt->'days') AS d(day)
          WHERE d.day = ANY($10)
          GROUP BY mt
          HAVING COUNT(DISTINCT d.day) = array_length($10, 1)
      ))
      AND ($11::text IS NULL OR EXISTS (
          SELECT 1 FROM jsonb_array_elements(meeting_times) AS mt
          WHERE (mt->'timeRange'->>'start') >= $11
      ))
      AND ($12::text IS NULL OR EXISTS (
          SELECT 1 FROM jsonb_array_elements(meeting_times) AS mt
          WHERE (mt->'timeRange'->>'end') <= $12
      ))
      AND ($13::text[] IS NULL OR part_of_term = ANY($13))
      AND ($14::text[] IS NULL OR EXISTS (
          SELECT 1 FROM jsonb_array_elements_text(attributes) a
          WHERE a = ANY($14)
      ))
      AND ($15::int IS NULL OR COALESCE(credit_hours, credit_hour_low, 0) >= $15)
      AND ($16::int IS NULL OR COALESCE(credit_hours, credit_hour_high, 0) <= $16)
      AND ($17::text IS NULL OR EXISTS (
          SELECT 1 FROM course_instructors ci
          JOIN instructors i ON i.id = ci.instructor_id
          WHERE ci.course_id = courses.id
            AND i.display_name ILIKE '%' || $17 || '%'
      ))
"#;

/// Build a safe ORDER BY clause from typed sort parameters.
///
/// All column names are hardcoded string literals — no caller input is interpolated.
fn sort_clause(column: Option<SortColumn>, direction: Option<SortDirection>) -> String {
    let dir = match direction.unwrap_or(SortDirection::Asc) {
        SortDirection::Asc => "ASC",
        SortDirection::Desc => "DESC",
    };

    match column {
        Some(SortColumn::CourseCode) => {
            format!("subject {dir}, course_number {dir}, sequence_number {dir}")
        }
        Some(SortColumn::Title) => format!("title {dir}"),
        Some(SortColumn::Instructor) => {
            format!(
                "(SELECT i.display_name FROM course_instructors ci \
                 JOIN instructors i ON i.id = ci.instructor_id \
                 WHERE ci.course_id = courses.id AND ci.is_primary = true \
                 LIMIT 1) {dir} NULLS LAST"
            )
        }
        Some(SortColumn::Time) => {
            format!("(meeting_times->0->'timeRange'->>'start') {dir} NULLS LAST")
        }
        Some(SortColumn::Seats) => {
            format!("(max_enrollment - enrollment) {dir}")
        }
        None => "subject ASC, course_number ASC, sequence_number ASC".to_string(),
    }
}

/// Search courses by term with optional filters.
///
/// Returns `(courses, total_count)` for pagination. Uses FTS tsvector for word
/// search and falls back to trigram ILIKE for substring matching.
#[allow(clippy::too_many_arguments)]
pub async fn search_courses(
    db_pool: &PgPool,
    term_code: &str,
    subject: Option<&[String]>,
    title_query: Option<&str>,
    course_number_low: Option<i32>,
    course_number_high: Option<i32>,
    open_only: bool,
    instructional_method: Option<&[String]>,
    campus: Option<&[String]>,
    wait_count_max: Option<i32>,
    days: Option<&[String]>,
    time_start: Option<&str>,
    time_end: Option<&str>,
    part_of_term: Option<&[String]>,
    attributes: Option<&[String]>,
    credit_hour_min: Option<i32>,
    credit_hour_max: Option<i32>,
    instructor: Option<&str>,
    limit: i32,
    offset: i32,
    sort_by: Option<SortColumn>,
    sort_dir: Option<SortDirection>,
) -> Result<(Vec<Course>, i64)> {
    let order_by = sort_clause(sort_by, sort_dir);

    let data_query =
        format!("SELECT * FROM courses {SEARCH_WHERE} ORDER BY {order_by} LIMIT $18 OFFSET $19");
    let count_query = format!("SELECT COUNT(*) FROM courses {SEARCH_WHERE}");

    let courses = sqlx::query_as::<_, Course>(&data_query)
        .bind(term_code) // $1
        .bind(subject) // $2
        .bind(title_query) // $3
        .bind(course_number_low) // $4
        .bind(course_number_high) // $5
        .bind(open_only) // $6
        .bind(instructional_method) // $7
        .bind(campus) // $8
        .bind(wait_count_max) // $9
        .bind(days) // $10
        .bind(time_start) // $11
        .bind(time_end) // $12
        .bind(part_of_term) // $13
        .bind(attributes) // $14
        .bind(credit_hour_min) // $15
        .bind(credit_hour_max) // $16
        .bind(instructor) // $17
        .bind(limit) // $18
        .bind(offset) // $19
        .fetch_all(db_pool)
        .await?;

    let total: (i64,) = sqlx::query_as(&count_query)
        .bind(term_code) // $1
        .bind(subject) // $2
        .bind(title_query) // $3
        .bind(course_number_low) // $4
        .bind(course_number_high) // $5
        .bind(open_only) // $6
        .bind(instructional_method) // $7
        .bind(campus) // $8
        .bind(wait_count_max) // $9
        .bind(days) // $10
        .bind(time_start) // $11
        .bind(time_end) // $12
        .bind(part_of_term) // $13
        .bind(attributes) // $14
        .bind(credit_hour_min) // $15
        .bind(credit_hour_max) // $16
        .bind(instructor) // $17
        .fetch_one(db_pool)
        .await?;

    Ok((courses, total.0))
}

/// Get a single course by CRN and term.
pub async fn get_course_by_crn(
    db_pool: &PgPool,
    crn: &str,
    term_code: &str,
) -> Result<Option<Course>> {
    let course =
        sqlx::query_as::<_, Course>("SELECT * FROM courses WHERE crn = $1 AND term_code = $2")
            .bind(crn)
            .bind(term_code)
            .fetch_optional(db_pool)
            .await?;
    Ok(course)
}

/// Get instructors for a single course by course ID.
pub async fn get_course_instructors(
    db_pool: &PgPool,
    course_id: i32,
) -> Result<Vec<CourseInstructorDetail>> {
    let rows = sqlx::query_as::<_, CourseInstructorDetail>(
        r#"
        SELECT i.id as instructor_id, ci.banner_id, i.display_name, i.first_name, i.last_name,
               i.email, ci.is_primary,
               rmp.avg_rating, rmp.num_ratings, rmp.rmp_legacy_id,
               ci.course_id
        FROM course_instructors ci
        JOIN instructors i ON i.id = ci.instructor_id
        LEFT JOIN LATERAL (
            SELECT rp.avg_rating, rp.num_ratings, rp.legacy_id as rmp_legacy_id
            FROM instructor_rmp_links irl
            JOIN rmp_professors rp ON rp.legacy_id = irl.rmp_legacy_id
            WHERE irl.instructor_id = i.id
            ORDER BY rp.num_ratings DESC NULLS LAST, rp.legacy_id ASC
            LIMIT 1
        ) rmp ON true
        WHERE ci.course_id = $1
        ORDER BY ci.is_primary DESC, i.display_name
        "#,
    )
    .bind(course_id)
    .fetch_all(db_pool)
    .await?;
    Ok(rows)
}

/// Batch-fetch instructors for multiple courses in a single query.
///
/// Returns a map of `course_id → Vec<CourseInstructorDetail>`.
pub async fn get_instructors_for_courses(
    db_pool: &PgPool,
    course_ids: &[i32],
) -> Result<HashMap<i32, Vec<CourseInstructorDetail>>> {
    if course_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let rows = sqlx::query_as::<_, CourseInstructorDetail>(
        r#"
        SELECT i.id as instructor_id, ci.banner_id, i.display_name, i.first_name, i.last_name,
               i.email, ci.is_primary,
               rmp.avg_rating, rmp.num_ratings, rmp.rmp_legacy_id,
               ci.course_id
        FROM course_instructors ci
        JOIN instructors i ON i.id = ci.instructor_id
        LEFT JOIN LATERAL (
            SELECT rp.avg_rating, rp.num_ratings, rp.legacy_id as rmp_legacy_id
            FROM instructor_rmp_links irl
            JOIN rmp_professors rp ON rp.legacy_id = irl.rmp_legacy_id
            WHERE irl.instructor_id = i.id
            ORDER BY rp.num_ratings DESC NULLS LAST, rp.legacy_id ASC
            LIMIT 1
        ) rmp ON true
        WHERE ci.course_id = ANY($1)
        ORDER BY ci.course_id, ci.is_primary DESC, i.display_name
        "#,
    )
    .bind(course_ids)
    .fetch_all(db_pool)
    .await?;

    let mut map: HashMap<i32, Vec<CourseInstructorDetail>> = HashMap::new();
    for row in rows {
        // course_id is always present in the batch query
        let cid = row.course_id.unwrap_or_default();
        map.entry(cid).or_default().push(row);
    }
    Ok(map)
}

/// Get subjects for a term, sorted by total enrollment (descending).
///
/// Returns only subjects that have courses in the given term, with their
/// descriptions from reference_data and enrollment totals for ranking.
pub async fn get_subjects_by_enrollment(
    db_pool: &PgPool,
    term_code: &str,
) -> Result<Vec<(String, String, i64)>> {
    let rows: Vec<(String, String, i64)> = sqlx::query_as(
        r#"
        SELECT c.subject,
               COALESCE(rd.description, c.subject),
               COALESCE(SUM(c.enrollment), 0) as total_enrollment
        FROM courses c
        LEFT JOIN reference_data rd ON rd.category = 'subject' AND rd.code = c.subject
        WHERE c.term_code = $1
        GROUP BY c.subject, rd.description
        ORDER BY total_enrollment DESC
        "#,
    )
    .bind(term_code)
    .fetch_all(db_pool)
    .await?;
    Ok(rows)
}

/// Get all sections of the same course (same term, subject, and course number).
pub async fn get_related_sections(
    db_pool: &PgPool,
    term_code: &str,
    subject: &str,
    course_number: &str,
) -> Result<Vec<Course>> {
    let courses = sqlx::query_as::<_, Course>(
        "SELECT * FROM courses WHERE term_code = $1 AND subject = $2 AND course_number = $3 ORDER BY sequence_number ASC NULLS LAST",
    )
    .bind(term_code)
    .bind(subject)
    .bind(course_number)
    .fetch_all(db_pool)
    .await?;
    Ok(courses)
}

/// Get all distinct term codes that have courses in the DB.
pub async fn get_available_terms(db_pool: &PgPool) -> Result<Vec<String>> {
    let rows: Vec<(String,)> =
        sqlx::query_as("SELECT DISTINCT term_code FROM courses ORDER BY term_code DESC")
            .fetch_all(db_pool)
            .await?;
    Ok(rows.into_iter().map(|(tc,)| tc).collect())
}

type RangeRow = (
    Option<i32>,
    Option<i32>,
    Option<i32>,
    Option<i32>,
    Option<i32>,
);

/// Get aggregate filter ranges for a term (course number, credit hours, waitlist).
pub async fn get_filter_ranges(db_pool: &PgPool, term_code: &str) -> Result<FilterRanges> {
    let row: RangeRow = sqlx::query_as(
        r#"
        SELECT
            MIN(course_number::int),
            MAX(course_number::int),
            MIN(COALESCE(credit_hours, credit_hour_low, 0)),
            MAX(COALESCE(credit_hours, credit_hour_high, 0)),
            MAX(wait_count)
        FROM courses
        WHERE term_code = $1
          AND course_number ~ '^\d+$'
        "#,
    )
    .bind(term_code)
    .fetch_one(db_pool)
    .await?;

    let cn_max = row.1.unwrap_or(9000);
    let ch_min = row.2.unwrap_or(0);
    let ch_max = row.3.unwrap_or(8);
    let wc_max_raw = row.4.unwrap_or(0);

    // Round course number to hundreds: floor min, ceil max
    let cn_max_rounded = ((cn_max + 99) / 100) * 100;

    // Waitlist ceiling: (max / 10 + 1) * 10
    let wc_max = if wc_max_raw > 0 {
        (wc_max_raw / 10 + 1) * 10
    } else {
        0
    };

    Ok(FilterRanges {
        course_number_min: 0,
        course_number_max: cn_max_rounded,
        credit_hour_min: ch_min,
        credit_hour_max: ch_max,
        wait_count_max: wc_max,
    })
}

/// A suggested course result for autocomplete.
#[derive(Debug, serde::Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CourseSuggestion {
    pub subject: String,
    pub course_number: String,
    pub title: String,
    pub section_count: i32,
    pub score: f32,
}

/// A suggested instructor result for autocomplete.
#[derive(Debug, serde::Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct InstructorSuggestion {
    pub id: i32,
    pub display_name: String,
    pub score: f32,
}

/// Get course title suggestions using trigram similarity.
pub async fn suggest_courses(
    db_pool: &PgPool,
    term_code: &str,
    query: &str,
    limit: i32,
) -> Result<Vec<CourseSuggestion>> {
    let rows: Vec<(String, String, String, i32, f32)> = sqlx::query_as(
        r#"
        SELECT subject, course_number, title, COUNT(*)::int as section_count,
               MAX(GREATEST(similarity(title, $2), similarity(subject || ' ' || course_number, $2))) as score
        FROM courses
        WHERE term_code = $1
          AND (title % $2 OR title ILIKE '%' || $2 || '%'
               OR (subject || ' ' || course_number) % $2
               OR (subject || ' ' || course_number) ILIKE '%' || $2 || '%')
        GROUP BY subject, course_number, title
        ORDER BY score DESC
        LIMIT $3
        "#,
    )
    .bind(term_code)
    .bind(query)
    .bind(limit)
    .fetch_all(db_pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(subject, course_number, title, section_count, score)| CourseSuggestion {
                subject,
                course_number,
                title,
                section_count,
                score,
            },
        )
        .collect())
}

/// Get instructor suggestions using trigram similarity.
pub async fn suggest_instructors(
    db_pool: &PgPool,
    term_code: &str,
    query: &str,
    limit: i32,
) -> Result<Vec<InstructorSuggestion>> {
    let rows: Vec<(i32, String, f32)> = sqlx::query_as(
        r#"
        SELECT i.id, i.display_name, MAX(similarity(i.display_name, $2)) as score
        FROM instructors i
        JOIN course_instructors ci ON ci.instructor_id = i.id
        JOIN courses c ON c.id = ci.course_id
        WHERE c.term_code = $1
          AND (i.display_name % $2 OR i.display_name ILIKE '%' || $2 || '%')
        GROUP BY i.id, i.display_name
        ORDER BY score DESC
        LIMIT $3
        "#,
    )
    .bind(term_code)
    .bind(query)
    .bind(limit)
    .fetch_all(db_pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(id, display_name, score)| InstructorSuggestion {
            id,
            display_name,
            score,
        })
        .collect())
}

/// Course operations with automatic event emission.
pub struct CourseOps<'a> {
    ctx: &'a DbContext,
}

impl<'a> CourseOps<'a> {
    pub(crate) fn new(ctx: &'a DbContext) -> Self {
        Self { ctx }
    }

    /// Batch upsert courses and emit audit log events.
    ///
    /// This wraps the existing `batch_upsert_courses` function but handles
    /// event emission automatically.
    pub async fn batch_upsert(&self, courses: &[BannerCourse]) -> Result<UpsertCounts> {
        let (counts, audit_entries) = batch_upsert_impl(courses, self.ctx.pool()).await?;

        if !audit_entries.is_empty() {
            self.ctx
                .events()
                .publish(DomainEvent::AuditLog(AuditLogEvent {
                    entries: audit_entries,
                }));
        }

        Ok(counts)
    }
}
