//! Public instructor data layer: slug generation, directory listing, and profile queries.

use anyhow::{Context, Result};
use serde::Serialize;
use sqlx::PgPool;
use ts_rs::TS;

use crate::data::unsigned::Count;

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

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PublicInstructorListItem {
    pub id: i32,
    pub slug: String,
    pub display_name: String,
    pub email: Option<String>,
    pub subjects: Vec<String>,
    pub rmp: Option<super::course_types::RmpBrief>,
    pub bluebook: Option<super::course_types::BlueBookBrief>,
    pub rating: Option<super::course_types::InstructorRating>,
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
    pub rmp: Option<super::course_types::RmpFull>,
    pub bluebook: Option<super::course_types::BlueBookFull>,
    pub rating: Option<super::course_types::InstructorRating>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TeachingHistoryTerm {
    pub term_slug: String,
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
    pub section_count: Count,
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
    use sqlx::{Postgres, QueryBuilder};

    use super::scoring::{self, UnratedPolicy};

    let page = params.page.max(1);
    let per_page = params.per_page.clamp(1, 100);
    let offset = (page - 1) * per_page;

    // Determine sort clause and any additional filter it requires
    let mut extra_condition: Option<String> = None;
    let sort_clause = match params.sort.as_str() {
        "name_desc" => "i.display_name DESC".to_string(),
        sort_key if sort_key.starts_with("score_") => {
            let ascending = sort_key.ends_with("_asc");
            let (order, filter) = scoring::rating_sort_sql(ascending, UnratedPolicy::AsPrior);
            extra_condition = filter;
            order
        }
        _ => "i.display_name ASC".to_string(),
    };

    /// Append instructor list WHERE conditions to a QueryBuilder.
    fn push_instructor_conditions<'args>(
        builder: &mut QueryBuilder<'args, Postgres>,
        params: &'args PublicInstructorListParams,
        extra_condition: &Option<String>,
    ) {
        builder.push(
            " WHERE EXISTS (SELECT 1 FROM course_instructors ci WHERE ci.instructor_id = i.id) \
             AND i.slug IS NOT NULL",
        );

        if let Some(cond) = extra_condition {
            builder.push(" AND ");
            builder.push(cond.as_str());
        }

        if let Some(ref search) = params.search {
            builder.push(" AND (immutable_unaccent(i.display_name) % immutable_unaccent(");
            builder.push_bind(search);
            builder
                .push(") OR immutable_unaccent(i.display_name) ILIKE '%' || immutable_unaccent(");
            builder.push_bind(search);
            builder.push(") || '%')");
        }

        if let Some(ref subject) = params.subject {
            builder.push(
                " AND EXISTS (SELECT 1 FROM course_instructors ci2 \
                 JOIN courses c2 ON c2.id = ci2.course_id \
                 WHERE ci2.instructor_id = i.id AND c2.subject = ",
            );
            builder.push_bind(subject);
            builder.push(")");
        }
    }

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
        display_score: Option<f32>,
        sort_score: Option<f32>,
        ci_lower: Option<f32>,
        ci_upper: Option<f32>,
        confidence: Option<f32>,
        score_source: Option<String>,
        sc_rmp_count: Option<i32>,
        sc_bb_count: Option<i32>,
    }

    // Data query
    let mut data_builder: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT \
            i.id, i.slug, i.display_name, i.email, \
            COALESCE(\
                (SELECT array_agg(DISTINCT c.subject ORDER BY c.subject) \
                 FROM course_instructors ci JOIN courses c ON c.id = ci.course_id \
                 WHERE ci.instructor_id = i.id), \
                ARRAY[]::text[]\
            ) as subjects, \
            rmp.avg_rating, rmp.num_ratings, rmp.primary_legacy_id as rmp_legacy_id, \
            bb.bb_avg_instructor_rating, bb.bb_total_responses, \
            sc.display_score, sc.sort_score, sc.ci_lower, sc.ci_upper, \
            sc.confidence, sc.source as score_source, \
            sc.rmp_count as sc_rmp_count, sc.bb_count as sc_bb_count \
         FROM instructors i \
         LEFT JOIN instructor_rmp_summary rmp ON rmp.instructor_id = i.id \
         LEFT JOIN (\
             SELECT ibl.instructor_id, \
                 AVG(be.instructor_rating)::real as bb_avg_instructor_rating, \
                 SUM(be.instructor_response_count)::bigint as bb_total_responses \
             FROM instructor_bluebook_links ibl \
             JOIN bluebook_evaluations be ON ibl.instructor_name = be.instructor_name \
                 AND (ibl.subject IS NULL OR ibl.subject = be.subject) \
             WHERE ibl.status IN ('approved', 'auto') \
                 AND be.instructor_rating IS NOT NULL \
                 AND be.instructor_response_count > 0 \
             GROUP BY ibl.instructor_id\
         ) bb ON bb.instructor_id = i.id \
         LEFT JOIN instructor_scores sc ON sc.instructor_id = i.id",
    );
    push_instructor_conditions(&mut data_builder, params, &extra_condition);
    data_builder.push(" ORDER BY ");
    data_builder.push(sort_clause.as_str());
    data_builder.push(" LIMIT ");
    data_builder.push_bind(per_page);
    data_builder.push(" OFFSET ");
    data_builder.push_bind(offset);

    let rows = data_builder
        .build_query_as::<Row>()
        .fetch_all(pool)
        .await
        .context("failed to list public instructors")?;

    // Count query
    let mut count_builder: QueryBuilder<Postgres> =
        QueryBuilder::new("SELECT COUNT(*) FROM instructors i");
    push_instructor_conditions(&mut count_builder, params, &extra_condition);

    let (total,): (i64,) = count_builder
        .build_query_as()
        .fetch_one(pool)
        .await
        .context("failed to count public instructors")?;

    let instructors = rows
        .into_iter()
        .map(|r| {
            let rmp = r.rmp_legacy_id.map(|legacy_id| {
                let (avg_rating, num_ratings) = super::course_types::sanitize_rmp_ratings(
                    r.avg_rating.map(|v| v as f32),
                    r.num_ratings,
                );
                super::course_types::RmpBrief {
                    avg_rating,
                    num_ratings,
                    legacy_id,
                }
            });
            let bluebook = match (r.bb_avg_instructor_rating, r.bb_total_responses) {
                (Some(avg), Some(n)) if avg > 0.0 && n > 0 => {
                    Count::try_from(n).ok().map(|total_responses| {
                        super::course_types::BlueBookBrief {
                            avg_instructor_rating: avg,
                            total_responses,
                        }
                    })
                }
                _ => None,
            };
            let rating = match (
                r.display_score,
                r.sort_score,
                r.ci_lower,
                r.ci_upper,
                r.confidence,
                r.score_source,
            ) {
                (Some(ds), Some(ss), Some(cl), Some(cu), Some(conf), Some(src)) => Some(
                    super::scoring::build_rating_from_score_row(&super::scoring::ScoreRow {
                        display_score: ds,
                        sort_score: ss,
                        ci_lower: cl,
                        ci_upper: cu,
                        confidence: conf,
                        source: src,
                        rmp_count: r.sc_rmp_count.unwrap_or(0),
                        bb_count: r.sc_bb_count.unwrap_or(0),
                    }),
                ),
                _ => None,
            };
            PublicInstructorListItem {
                id: r.id,
                slug: r.slug.unwrap_or_default(),
                display_name: r.display_name,
                email: r.email,
                subjects: r.subjects,
                rmp,
                bluebook,
                rating,
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
        let (avg_rating, num_ratings) = super::course_types::sanitize_rmp_ratings(
            r.avg_rating.map(|v| v as f32),
            r.num_ratings,
        );
        Some(super::course_types::RmpFull {
            avg_rating,
            avg_difficulty: r.avg_difficulty.map(|v| v as f32),
            would_take_again_pct: r.would_take_again_pct.map(|v| v as f32),
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

    // Precomputed composite score (fetched before BB summary so calibrated_bb is available)
    #[derive(sqlx::FromRow)]
    struct DbScoreRow {
        display_score: f32,
        sort_score: f32,
        ci_lower: f32,
        ci_upper: f32,
        confidence: f32,
        source: String,
        rmp_count: i32,
        bb_count: i32,
        calibrated_bb: Option<f32>,
    }

    let score_row = sqlx::query_as::<_, DbScoreRow>(
        "SELECT display_score, sort_score, ci_lower, ci_upper, confidence, source, rmp_count, bb_count, calibrated_bb FROM instructor_scores WHERE instructor_id = $1",
    )
    .bind(inst.id)
    .fetch_optional(pool)
    .await
    .context("failed to fetch instructor score")?;

    let rating = score_row.as_ref().map(|s| {
        super::scoring::build_rating_from_score_row(&super::scoring::ScoreRow {
            display_score: s.display_score,
            sort_score: s.sort_score,
            ci_lower: s.ci_lower,
            ci_upper: s.ci_upper,
            confidence: s.confidence,
            source: s.source.clone(),
            rmp_count: s.rmp_count,
            bb_count: s.bb_count,
        })
    });

    let bluebook_summary = match bb {
        Some(ref r) => match (r.avg_instructor_rating, r.total_responses) {
            (Some(avg), Some(n)) if avg > 0.0 && n > 0 => {
                let total_responses = Count::try_from(n).ok();
                let eval_count = Count::try_from(r.eval_count.unwrap_or(0)).ok();
                match (total_responses, eval_count) {
                    (Some(total_responses), Some(eval_count)) => {
                        let calibrated_rating = score_row
                            .as_ref()
                            .and_then(|s| s.calibrated_bb)
                            .unwrap_or_else(|| (-2.58 + 1.45 * avg as f64).clamp(1.0, 5.0) as f32);
                        Some(super::course_types::BlueBookFull {
                            calibrated_rating,
                            avg_instructor_rating: avg,
                            avg_course_rating: r.avg_course_rating,
                            total_responses,
                            eval_count,
                        })
                    }
                    _ => None,
                }
            }
            _ => None,
        },
        None => None,
    };

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
            rating,
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
        section_count: Count,
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
        let parsed_term = row.term_code.parse::<Term>().ok();
        let term_slug = parsed_term
            .as_ref()
            .map(|t| t.slug())
            .unwrap_or_else(|| row.term_code.clone());
        let term_description = parsed_term
            .as_ref()
            .map(|t| t.description())
            .unwrap_or_else(|| row.term_code.clone());

        if let Some(last) = terms.last_mut()
            && last.term_slug == term_slug
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
            term_slug,
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

/// Resolve a batch of instructor slugs to their display names.
pub async fn resolve_instructor_slugs(
    pool: &PgPool,
    slugs: &[String],
) -> Result<Vec<(String, String)>> {
    let rows: Vec<(String, String)> =
        sqlx::query_as("SELECT slug, display_name FROM instructors WHERE slug = ANY($1)")
            .bind(slugs)
            .fetch_all(pool)
            .await
            .context("failed to resolve instructor slugs")?;
    Ok(rows)
}

/// An instructor slug with its most recent modification timestamp for sitemap generation.
pub struct InstructorSitemapEntry {
    pub slug: String,
    pub last_modified: Option<chrono::DateTime<chrono::Utc>>,
}

/// List all instructor slugs with per-instructor lastmod timestamps.
///
/// The lastmod is the most recent of: score computation, BlueBook link update,
/// RMP profile sync, and course scrape time.
pub async fn list_all_instructor_sitemap_entries(
    pool: &PgPool,
) -> Result<Vec<InstructorSitemapEntry>> {
    #[derive(sqlx::FromRow)]
    struct Row {
        slug: String,
        last_modified: Option<chrono::DateTime<chrono::Utc>>,
    }

    let rows = sqlx::query_as::<_, Row>(
        r#"
        SELECT
            i.slug,
            GREATEST(
                sc.computed_at,
                bb.max_updated_at,
                rmp.max_synced_at,
                cr.max_scraped_at
            ) AS last_modified
        FROM instructors i
        LEFT JOIN instructor_scores sc ON sc.instructor_id = i.id
        LEFT JOIN (
            SELECT instructor_id, MAX(updated_at) AS max_updated_at
            FROM instructor_bluebook_links
            GROUP BY instructor_id
        ) bb ON bb.instructor_id = i.id
        LEFT JOIN (
            SELECT irl.instructor_id, MAX(rp.last_synced_at) AS max_synced_at
            FROM instructor_rmp_links irl
            JOIN rmp_professors rp ON rp.legacy_id = irl.rmp_legacy_id
            GROUP BY irl.instructor_id
        ) rmp ON rmp.instructor_id = i.id
        LEFT JOIN (
            SELECT ci.instructor_id, MAX(c.last_scraped_at) AS max_scraped_at
            FROM course_instructors ci
            JOIN courses c ON c.id = ci.course_id
            GROUP BY ci.instructor_id
        ) cr ON cr.instructor_id = i.id
        WHERE i.slug IS NOT NULL
        ORDER BY i.slug
        "#,
    )
    .fetch_all(pool)
    .await
    .context("failed to list instructor sitemap entries")?;

    Ok(rows
        .into_iter()
        .map(|r| InstructorSitemapEntry {
            slug: r.slug,
            last_modified: r.last_modified,
        })
        .collect())
}

pub enum IdentifierKind {
    Slug,
    NumericId(i32),
    EmailPrefix,
}

pub fn classify_identifier(s: &str) -> IdentifierKind {
    if let Ok(id) = s.parse::<i32>() {
        IdentifierKind::NumericId(id)
    } else if s.contains('.') {
        IdentifierKind::EmailPrefix
    } else {
        IdentifierKind::Slug
    }
}

/// Resolve any identifier form to (instructor_id, canonical_slug).
/// Returns None if not found or if the instructor has no slug yet.
pub async fn resolve_instructor_identifier(
    pool: &PgPool,
    raw: &str,
) -> Result<Option<(i32, String)>> {
    #[derive(sqlx::FromRow)]
    struct Row {
        id: i32,
        slug: Option<String>,
    }

    let row: Option<Row> = match classify_identifier(raw) {
        IdentifierKind::Slug => {
            sqlx::query_as("SELECT id, slug FROM instructors WHERE slug = $1")
                .bind(raw)
                .fetch_optional(pool)
                .await?
        }
        IdentifierKind::NumericId(id) => {
            sqlx::query_as("SELECT id, slug FROM instructors WHERE id = $1")
                .bind(id)
                .fetch_optional(pool)
                .await?
        }
        IdentifierKind::EmailPrefix => {
            sqlx::query_as(
                "SELECT id, slug FROM instructors \
                 WHERE LOWER(SPLIT_PART(email, '@', 1)) = LOWER($1)",
            )
            .bind(raw)
            .fetch_optional(pool)
            .await?
        }
    };

    Ok(row.and_then(|r| r.slug.map(|slug| (r.id, slug))))
}
