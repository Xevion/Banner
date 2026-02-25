//! Public instructor data layer: slug generation, directory listing, and profile queries.

use anyhow::{Context, Result};
use serde::Serialize;
use sqlx::PgPool;
use ts_rs::TS;

const NANOID_ALPHABET: &[char] = &[
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i',
    'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z',
];
const NANOID_LEN: usize = 3;

/// Convert a display name to a URL-safe slug with a nanoid suffix.
///
/// "Doe, Jane Marie" -> "doe-jane-marie-x6k"
pub fn generate_slug(display_name: &str) -> String {
    let base: String = display_name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();

    // Collapse consecutive hyphens and trim
    let mut slug = String::with_capacity(base.len() + NANOID_LEN + 1);
    let mut prev_hyphen = true; // treat start as hyphen to trim leading
    for ch in base.chars() {
        if ch == '-' {
            if !prev_hyphen {
                slug.push('-');
            }
            prev_hyphen = true;
        } else {
            slug.push(ch);
            prev_hyphen = false;
        }
    }
    // Trim trailing hyphen
    if slug.ends_with('-') {
        slug.pop();
    }

    let suffix = nanoid::nanoid!(NANOID_LEN, NANOID_ALPHABET);
    format!("{slug}-{suffix}")
}

/// Backfill slugs for all instructors that don't have one yet.
pub async fn backfill_instructor_slugs(pool: &PgPool) -> Result<u64> {
    let rows: Vec<(i32, String)> =
        sqlx::query_as("SELECT id, display_name FROM instructors WHERE slug IS NULL")
            .fetch_all(pool)
            .await
            .context("failed to fetch instructors without slugs")?;

    if rows.is_empty() {
        return Ok(0);
    }

    let count = rows.len() as u64;
    let ids: Vec<i32> = rows.iter().map(|(id, _)| *id).collect();
    let slugs: Vec<String> = rows.iter().map(|(_, name)| generate_slug(name)).collect();

    sqlx::query(
        r#"
        UPDATE instructors SET slug = data.slug
        FROM (SELECT UNNEST($1::int[]) AS id, UNNEST($2::text[]) AS slug) data
        WHERE instructors.id = data.id
        "#,
    )
    .bind(&ids)
    .bind(&slugs)
    .execute(pool)
    .await
    .context("failed to backfill instructor slugs")?;

    Ok(count)
}

/// Lightweight RMP summary for instructor list cards.
///
/// Present whenever an RMP profile link exists. Rating fields are `None` when the
/// profile has no reviews.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RmpListSummary {
    pub avg_rating: Option<f32>,
    pub num_ratings: Option<i32>,
    pub legacy_id: i32,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PublicInstructorListItem {
    pub id: i32,
    pub slug: String,
    pub display_name: String,
    pub email: Option<String>,
    pub subjects: Vec<String>,
    pub rmp: Option<RmpListSummary>,
    pub bluebook: Option<super::course_types::BlueBookListSummary>,
    pub composite: Option<super::course_types::CompositeRating>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PublicInstructorListResponse {
    pub instructors: Vec<PublicInstructorListItem>,
    #[ts(as = "i32")]
    pub total: i64,
    pub page: i32,
    pub per_page: i32,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PublicInstructorProfile {
    pub id: i32,
    pub slug: String,
    pub display_name: String,
    pub email: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub subjects: Vec<String>,
    pub rmp: Option<PublicRmpSummary>,
    pub bluebook: Option<super::course_types::PublicBlueBookSummary>,
    pub composite: Option<super::course_types::CompositeRating>,
}

/// Full RMP summary for instructor detail pages.
///
/// Present whenever an RMP profile link exists. Rating fields are `None` when the
/// profile has no reviews.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PublicRmpSummary {
    pub avg_rating: Option<f32>,
    pub avg_difficulty: Option<f32>,
    pub would_take_again_pct: Option<f32>,
    pub num_ratings: Option<i32>,
    pub legacy_id: i32,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TeachingHistoryTerm {
    pub term_code: String,
    pub term_description: String,
    pub courses: Vec<TeachingHistoryCourse>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TeachingHistoryCourse {
    pub subject: String,
    pub course_number: String,
    pub title: String,
    pub section_count: i32,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PublicInstructorProfileResponse {
    pub instructor: PublicInstructorProfile,
    pub teaching_history: Vec<TeachingHistoryTerm>,
}

#[derive(Debug, serde::Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PublicInstructorListParams {
    pub search: Option<String>,
    pub subject: Option<String>,
    #[serde(default = "default_sort")]
    pub sort: String,
    #[serde(default = "default_page")]
    pub page: i32,
    #[serde(default = "default_per_page")]
    pub per_page: i32,
}

fn default_sort() -> String {
    "name_asc".to_string()
}
fn default_page() -> i32 {
    1
}
fn default_per_page() -> i32 {
    24
}

/// List instructors for the public directory: paginated, searchable, filterable.
pub async fn list_public_instructors(
    pool: &PgPool,
    params: &PublicInstructorListParams,
) -> Result<PublicInstructorListResponse> {
    let page = params.page.max(1);
    let per_page = params.per_page.clamp(1, 100);
    let offset = (page - 1) * per_page;

    let sort_clause = match params.sort.as_str() {
        "name_desc" => "i.display_name DESC",
        "rating_asc" => "rmp.avg_rating ASC NULLS LAST, i.display_name ASC",
        "rating_desc" => "rmp.avg_rating DESC NULLS LAST, i.display_name ASC",
        _ => "i.display_name ASC",
    };

    // Build dynamic WHERE
    let mut conditions = vec![
        // Only instructors that have taught at least one section
        "EXISTS (SELECT 1 FROM course_instructors ci WHERE ci.instructor_id = i.id)".to_string(),
        // Must have a slug
        "i.slug IS NOT NULL".to_string(),
    ];
    let mut bind_idx = 0u32;

    if params.search.is_some() {
        bind_idx += 1;
        conditions.push(format!(
            "(immutable_unaccent(i.display_name) % immutable_unaccent(${bind_idx}) OR immutable_unaccent(i.display_name) ILIKE '%' || immutable_unaccent(${bind_idx}) || '%')"
        ));
    }
    if params.subject.is_some() {
        bind_idx += 1;
        conditions.push(format!(
            "EXISTS (SELECT 1 FROM course_instructors ci2 JOIN courses c2 ON c2.id = ci2.course_id WHERE ci2.instructor_id = i.id AND c2.subject = ${bind_idx})"
        ));
    }

    let where_clause = format!("WHERE {}", conditions.join(" AND "));

    let query_str = format!(
        r#"
        SELECT
            i.id, i.slug, i.display_name, i.email,
            COALESCE(
                (SELECT array_agg(DISTINCT c.subject ORDER BY c.subject)
                 FROM course_instructors ci JOIN courses c ON c.id = ci.course_id
                 WHERE ci.instructor_id = i.id),
                ARRAY[]::text[]
            ) as subjects,
            rmp.avg_rating, rmp.num_ratings, rmp.primary_legacy_id as rmp_legacy_id,
            bb.bb_avg_instructor_rating, bb.bb_total_responses
        FROM instructors i
        LEFT JOIN instructor_rmp_summary rmp ON rmp.instructor_id = i.id
        LEFT JOIN (
            SELECT ibl.instructor_id,
                AVG(be.instructor_rating)::real as bb_avg_instructor_rating,
                SUM(be.instructor_response_count)::bigint as bb_total_responses
            FROM instructor_bluebook_links ibl
            JOIN bluebook_evaluations be ON ibl.instructor_name = be.instructor_name
                AND (ibl.subject IS NULL OR ibl.subject = be.subject)
            WHERE ibl.status IN ('approved', 'auto')
                AND be.instructor_rating IS NOT NULL
                AND be.instructor_response_count > 0
            GROUP BY ibl.instructor_id
        ) bb ON bb.instructor_id = i.id
        {where_clause}
        ORDER BY {sort_clause}
        LIMIT {per_page} OFFSET {offset}
        "#
    );

    #[derive(sqlx::FromRow)]
    struct Row {
        id: i32,
        slug: Option<String>,
        display_name: String,
        email: Option<String>,
        subjects: Vec<String>,
        avg_rating: Option<f64>,
        num_ratings: Option<i32>,
        rmp_legacy_id: Option<i32>,
        bb_avg_instructor_rating: Option<f32>,
        bb_total_responses: Option<i64>,
    }

    let mut query = sqlx::query_as::<_, Row>(&query_str);
    if let Some(ref search) = params.search {
        query = query.bind(search);
    }
    if let Some(ref subject) = params.subject {
        query = query.bind(subject);
    }

    let rows = query
        .fetch_all(pool)
        .await
        .context("failed to list public instructors")?;

    // Count total
    let count_str = format!("SELECT COUNT(*) FROM instructors i {where_clause}");
    let mut count_query = sqlx::query_as::<_, (i64,)>(&count_str);
    if let Some(ref search) = params.search {
        count_query = count_query.bind(search);
    }
    if let Some(ref subject) = params.subject {
        count_query = count_query.bind(subject);
    }

    let (total,) = count_query
        .fetch_one(pool)
        .await
        .context("failed to count public instructors")?;

    let instructors = rows
        .into_iter()
        .map(|r| {
            let rmp = r.rmp_legacy_id.map(|legacy_id| {
                let (avg_rating, num_ratings) =
                    super::course_types::sanitize_rmp_ratings(r.avg_rating.map(|v| v as f32), r.num_ratings);
                RmpListSummary {
                    avg_rating,
                    num_ratings,
                    legacy_id,
                }
            });
            let bluebook = match (r.bb_avg_instructor_rating, r.bb_total_responses) {
                (Some(avg), Some(n)) if avg > 0.0 && n > 0 => {
                    Some(super::course_types::BlueBookListSummary {
                        avg_instructor_rating: avg,
                        total_responses: n as i32,
                    })
                }
                _ => None,
            };
            let composite = super::course_types::compute_composite(
                rmp.as_ref().and_then(|r| r.avg_rating),
                rmp.as_ref().and_then(|r| r.num_ratings),
                r.bb_avg_instructor_rating,
                r.bb_total_responses.unwrap_or(0) as i32,
            );
            PublicInstructorListItem {
                id: r.id,
                slug: r.slug.unwrap_or_default(),
                display_name: r.display_name,
                email: r.email,
                subjects: r.subjects,
                rmp,
                bluebook,
                composite,
            }
        })
        .collect();

    Ok(PublicInstructorListResponse {
        instructors,
        total,
        page,
        per_page,
    })
}

/// Get a single instructor's full public profile by slug.
pub async fn get_public_instructor_by_slug(
    pool: &PgPool,
    slug: &str,
) -> Result<Option<PublicInstructorProfileResponse>> {
    #[derive(sqlx::FromRow)]
    struct InstructorRow {
        id: i32,
        slug: Option<String>,
        display_name: String,
        email: Option<String>,
        first_name: Option<String>,
        last_name: Option<String>,
    }

    let instructor = sqlx::query_as::<_, InstructorRow>(
        "SELECT id, slug, display_name, email, first_name, last_name FROM instructors WHERE slug = $1",
    )
    .bind(slug)
    .fetch_optional(pool)
    .await
    .context("failed to fetch instructor by slug")?;

    let inst = match instructor {
        Some(row) => row,
        None => return Ok(None),
    };

    // Subjects
    let subjects: Vec<(String,)> = sqlx::query_as(
        "SELECT DISTINCT c.subject FROM course_instructors ci JOIN courses c ON c.id = ci.course_id WHERE ci.instructor_id = $1 ORDER BY c.subject",
    )
    .bind(inst.id)
    .fetch_all(pool)
    .await
    .context("failed to fetch instructor subjects")?;

    // Best RMP profile (from materialized view)
    #[derive(sqlx::FromRow)]
    struct RmpRow {
        avg_rating: Option<f64>,
        avg_difficulty: Option<f64>,
        would_take_again_pct: Option<f64>,
        num_ratings: Option<i32>,
        legacy_id: Option<i32>,
    }

    let rmp = sqlx::query_as::<_, RmpRow>(
        r#"
        SELECT rmp.avg_rating, rmp.avg_difficulty, rmp.would_take_again_pct,
               rmp.num_ratings, rmp.primary_legacy_id as legacy_id
        FROM instructor_rmp_summary rmp
        WHERE rmp.instructor_id = $1
        "#,
    )
    .bind(inst.id)
    .fetch_optional(pool)
    .await
    .context("failed to fetch instructor rmp")?;

    let rmp_summary = rmp.and_then(|r| {
        let legacy_id = r.legacy_id?;
        let (avg_rating, num_ratings) =
            super::course_types::sanitize_rmp_ratings(r.avg_rating.map(|v| v as f32), r.num_ratings);
        Some(PublicRmpSummary {
            avg_rating,
            avg_difficulty: if avg_rating.is_some() {
                r.avg_difficulty.map(|v| v as f32)
            } else {
                None
            },
            would_take_again_pct: if avg_rating.is_some() {
                r.would_take_again_pct.map(|v| v as f32)
            } else {
                None
            },
            num_ratings,
            legacy_id,
        })
    });

    // BlueBook evaluations
    #[derive(sqlx::FromRow)]
    struct BlueBookRow {
        avg_instructor_rating: Option<f32>,
        avg_course_rating: Option<f32>,
        total_responses: Option<i64>,
        eval_count: Option<i64>,
    }

    let bb = sqlx::query_as::<_, BlueBookRow>(
        r#"
        SELECT
            AVG(be.instructor_rating)::real as avg_instructor_rating,
            AVG(be.course_rating)::real as avg_course_rating,
            SUM(be.instructor_response_count)::bigint as total_responses,
            COUNT(*)::bigint as eval_count
        FROM bluebook_evaluations be
        JOIN instructor_bluebook_links ibl ON ibl.instructor_name = be.instructor_name
            AND (ibl.subject IS NULL OR ibl.subject = be.subject)
        WHERE ibl.instructor_id = $1
            AND ibl.status IN ('approved', 'auto')
            AND be.instructor_rating IS NOT NULL
            AND be.instructor_response_count > 0
        "#,
    )
    .bind(inst.id)
    .fetch_optional(pool)
    .await
    .context("failed to fetch instructor bluebook")?;

    let (bluebook_summary, bb_avg, bb_count) = match bb {
        Some(ref r) => {
            let summary = match (r.avg_instructor_rating, r.total_responses) {
                (Some(avg), Some(n)) if avg > 0.0 && n > 0 => {
                    Some(super::course_types::PublicBlueBookSummary {
                        avg_instructor_rating: avg,
                        avg_course_rating: r.avg_course_rating,
                        total_responses: n as i32,
                        eval_count: r.eval_count.unwrap_or(0) as i32,
                    })
                }
                _ => None,
            };
            (
                summary,
                r.avg_instructor_rating,
                r.total_responses.unwrap_or(0) as i32,
            )
        }
        None => (None, None, 0),
    };

    let composite = super::course_types::compute_composite(
        rmp_summary.as_ref().and_then(|r| r.avg_rating),
        rmp_summary.as_ref().and_then(|r| r.num_ratings),
        bb_avg,
        bb_count,
    );

    // Teaching history
    let teaching_history = get_teaching_history(pool, inst.id).await?;

    Ok(Some(PublicInstructorProfileResponse {
        instructor: PublicInstructorProfile {
            id: inst.id,
            slug: inst.slug.unwrap_or_default(),
            display_name: inst.display_name,
            email: inst.email,
            first_name: inst.first_name,
            last_name: inst.last_name,
            subjects: subjects.into_iter().map(|(s,)| s).collect(),
            rmp: rmp_summary,
            bluebook: bluebook_summary,
            composite,
        },
        teaching_history,
    }))
}

/// Get teaching history grouped by term for an instructor.
async fn get_teaching_history(
    pool: &PgPool,
    instructor_id: i32,
) -> Result<Vec<TeachingHistoryTerm>> {
    #[derive(sqlx::FromRow)]
    struct Row {
        term_code: String,
        subject: String,
        course_number: String,
        title: String,
        section_count: i32,
    }

    let rows = sqlx::query_as::<_, Row>(
        r#"
        SELECT c.term_code, c.subject, c.course_number, c.title, COUNT(*)::int as section_count
        FROM course_instructors ci
        JOIN courses c ON c.id = ci.course_id
        WHERE ci.instructor_id = $1
        GROUP BY c.term_code, c.subject, c.course_number, c.title
        ORDER BY c.term_code DESC, c.subject ASC, c.course_number ASC
        "#,
    )
    .bind(instructor_id)
    .fetch_all(pool)
    .await
    .context("failed to fetch teaching history")?;

    use crate::banner::models::terms::Term;

    let mut terms: Vec<TeachingHistoryTerm> = Vec::new();
    for row in rows {
        let term_description = row
            .term_code
            .parse::<Term>()
            .map(|t| t.description())
            .unwrap_or_else(|_| row.term_code.clone());

        if let Some(last) = terms.last_mut()
            && last.term_code == row.term_code
        {
            last.courses.push(TeachingHistoryCourse {
                subject: row.subject,
                course_number: row.course_number,
                title: row.title,
                section_count: row.section_count,
            });
            continue;
        }

        terms.push(TeachingHistoryTerm {
            term_code: row.term_code.clone(),
            term_description,
            courses: vec![TeachingHistoryCourse {
                subject: row.subject,
                course_number: row.course_number,
                title: row.title,
                section_count: row.section_count,
            }],
        });
    }

    Ok(terms)
}

/// Get sections taught by an instructor in a given term.
///
/// Returns course IDs for use with `get_instructors_for_courses`.
pub async fn get_instructor_sections(
    pool: &PgPool,
    instructor_id: i32,
    term_code: &str,
) -> Result<Vec<super::models::Course>> {
    let courses = sqlx::query_as::<_, super::models::Course>(
        r#"
        SELECT c.*
        FROM courses c
        JOIN course_instructors ci ON ci.course_id = c.id
        WHERE ci.instructor_id = $1 AND c.term_code = $2
        ORDER BY c.subject, c.course_number, c.sequence_number
        "#,
    )
    .bind(instructor_id)
    .bind(term_code)
    .fetch_all(pool)
    .await
    .context("failed to fetch instructor sections")?;

    Ok(courses)
}

/// Look up an instructor's ID by slug. Returns None if not found.
pub async fn get_instructor_id_by_slug(pool: &PgPool, slug: &str) -> Result<Option<i32>> {
    let row: Option<(i32,)> = sqlx::query_as("SELECT id FROM instructors WHERE slug = $1")
        .bind(slug)
        .fetch_optional(pool)
        .await
        .context("failed to look up instructor by slug")?;
    Ok(row.map(|(id,)| id))
}
