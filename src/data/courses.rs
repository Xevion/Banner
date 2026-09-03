//! Database query functions for courses, used by the web API.

use super::context::DbContext;
use super::events::{AuditLogEvent, DomainEvent};
use crate::banner::Course as BannerCourse;
use crate::data::batch::batch_upsert_courses as batch_upsert_impl;
use crate::data::models::{Course, CourseInstructorDetail, UpsertCounts};
use anyhow::{Context, Result};
use sqlx::{PgPool, Postgres, QueryBuilder};
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;
use ts_rs::TS;

/// An orderable quantity.
///
/// Deliberately not a column: several keys may share one column's header, and
/// some belong to no column at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum SortKey {
    CourseCode,
    Title,
    InstructorName,
    InstructorRating,
    StartTime,
    EndTime,
    Duration,
    WeeklyMinutes,
    Days,
    SeatsOpen,
    FillRatio,
    WaitCount,
}

/// How a key orders, and what each direction means to a reader.
///
/// Labels live beside the expression so a key cannot gain an ordering without
/// also saying what that ordering means. "Ascending" never reaches the UI.
struct KeyDef {
    /// Wire name. Must match the serde representation; a test holds them together.
    name: &'static str,
    /// ORDER BY columns, each taking the term's direction. A key spanning several
    /// columns must list them separately or only the last one would reverse.
    /// Hardcoded literals, never built from caller input.
    exprs: &'static [&'static str],
    asc: &'static str,
    desc: &'static str,
}

impl SortKey {
    pub const ALL: &'static [SortKey] = &[
        Self::CourseCode,
        Self::Title,
        Self::InstructorName,
        Self::InstructorRating,
        Self::StartTime,
        Self::EndTime,
        Self::Duration,
        Self::WeeklyMinutes,
        Self::Days,
        Self::SeatsOpen,
        Self::FillRatio,
        Self::WaitCount,
    ];

    const fn def(self) -> KeyDef {
        match self {
            Self::CourseCode => KeyDef {
                name: "course_code",
                exprs: &["subject", "course_number", "sequence_number"],
                asc: "A to Z",
                desc: "Z to A",
            },
            Self::Title => KeyDef {
                name: "title",
                exprs: &["title"],
                asc: "A to Z",
                desc: "Z to A",
            },
            // display_name is stored "Last, First", so this orders by last name
            // then first without needing to split it.
            Self::InstructorName => KeyDef {
                name: "instructor_name",
                exprs: &["(SELECT i.display_name FROM course_instructors ci \
                        JOIN instructors i ON i.id = ci.instructor_id \
                        WHERE ci.course_id = courses.id AND ci.is_primary = true LIMIT 1)"],
                asc: "Name, A to Z",
                desc: "Name, Z to A",
            },
            // sort_score is the CI lower bound, which is what ratings are ranked on.
            Self::InstructorRating => KeyDef {
                name: "instructor_rating",
                exprs: &["(SELECT s.sort_score FROM course_instructors ci \
                        JOIN instructor_scores s ON s.instructor_id = ci.instructor_id \
                        WHERE ci.course_id = courses.id AND ci.is_primary = true LIMIT 1)"],
                asc: "Lowest rated",
                desc: "Highest rated",
            },
            Self::StartTime => KeyDef {
                name: "start_time",
                exprs: &["first_begin_minutes"],
                asc: "Earliest first",
                desc: "Latest first",
            },
            Self::EndTime => KeyDef {
                name: "end_time",
                exprs: &["first_end_minutes"],
                asc: "Ends earliest",
                desc: "Ends latest",
            },
            Self::Duration => KeyDef {
                name: "duration",
                exprs: &["meeting_minutes"],
                asc: "Shortest first",
                desc: "Longest first",
            },
            Self::WeeklyMinutes => KeyDef {
                name: "weekly_minutes",
                exprs: &["weekly_minutes"],
                asc: "Least time per week",
                desc: "Most time per week",
            },
            // The mask is Monday-first bit order, so ascending groups Monday
            // patterns before Tuesday ones rather than ordering by day count.
            Self::Days => KeyDef {
                name: "days",
                exprs: &["day_mask"],
                asc: "Earliest weekday first",
                desc: "Latest weekday first",
            },
            Self::SeatsOpen => KeyDef {
                name: "seats_open",
                exprs: &["(max_enrollment - enrollment)"],
                asc: "Nearly full",
                desc: "Most seats open",
            },
            // Guarded against the sections whose max enrollment is zero.
            Self::FillRatio => KeyDef {
                name: "fill_ratio",
                exprs: &["(CASE WHEN max_enrollment > 0 \
                        THEN enrollment::float8 / max_enrollment ELSE NULL END)"],
                asc: "Emptiest first",
                desc: "Fullest first",
            },
            Self::WaitCount => KeyDef {
                name: "wait_count",
                exprs: &["wait_count"],
                asc: "Shortest waitlist",
                desc: "Longest waitlist",
            },
        }
    }

    /// Wire name, e.g. `start_time`.
    pub const fn name(self) -> &'static str {
        self.def().name
    }

    /// What this ordering means to a reader, for a header tooltip or a menu.
    pub const fn label(self, direction: SortDirection) -> &'static str {
        let def = self.def();
        match direction {
            SortDirection::Asc => def.asc,
            SortDirection::Desc => def.desc,
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|key| key.name() == name)
    }

    fn describe(self) -> SortKeyOption {
        SortKeyOption {
            key: self,
            asc_label: self.label(SortDirection::Asc).to_owned(),
            desc_label: self.label(SortDirection::Desc).to_owned(),
        }
    }

    /// Every key with its labels, so the client names a sort without restating it.
    pub fn catalog() -> Vec<SortKeyOption> {
        Self::ALL.iter().copied().map(Self::describe).collect()
    }
}

/// A sort key as offered to the client: what it is, and what each way round means.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SortKeyOption {
    pub key: SortKey,
    pub asc_label: String,
    pub desc_label: String,
}

/// Sort direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
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
    pub credit_hour_min: f64,
    pub credit_hour_max: f64,
    pub wait_count_max: i32,
}

/// Filter parameters for course search queries.
///
/// Borrows all data from the caller -- the filter is short-lived (one request).
#[derive(Debug, Default)]
pub struct SearchFilter<'a> {
    pub term_code: &'a str,
    pub subjects: Option<&'a [String]>,
    pub query: Option<&'a str>,
    pub course_number_low: Option<i32>,
    pub course_number_high: Option<i32>,
    pub open_only: bool,
    pub instructional_method: Option<&'a [String]>,
    pub campus: Option<&'a [String]>,
    pub wait_count_max: Option<i32>,
    pub days: Option<&'a [String]>,
    pub time_start: Option<&'a str>,
    pub time_end: Option<&'a str>,
    pub part_of_term: Option<&'a [String]>,
    pub attributes: Option<&'a [String]>,
    pub credit_hour_min: Option<f64>,
    pub credit_hour_max: Option<f64>,
    pub instructors: Option<&'a [String]>,
}

/// Append search filter WHERE conditions to a QueryBuilder.
///
/// Course number filtering extracts the numeric prefix to support alphanumeric
/// course numbers (e.g., "015X", "399H"). The numeric part is compared against
/// the range, so "399H" matches a search for courses 300-400.
fn push_search_conditions<'args>(
    builder: &mut QueryBuilder<'args, Postgres>,
    filter: &'args SearchFilter<'_>,
) {
    builder.push(" WHERE term_code = ");
    builder.push_bind(filter.term_code);

    if let Some(subjects) = filter.subjects {
        builder.push(" AND subject = ANY(");
        builder.push_bind(subjects);
        builder.push(")");
    }

    if let Some(query) = filter.query {
        builder.push(" AND (title_search @@ plainto_tsquery('simple_unaccent', ");
        builder.push_bind(query);
        builder.push(") OR immutable_unaccent(title) ILIKE '%' || immutable_unaccent(");
        builder.push_bind(query);
        builder.push(") || '%')");
    }

    if let Some(low) = filter.course_number_low {
        builder.push(r" AND (substring(course_number from '^\d+'))::int >= ");
        builder.push_bind(low);
    }

    if let Some(high) = filter.course_number_high {
        builder.push(r" AND (substring(course_number from '^\d+'))::int <= ");
        builder.push_bind(high);
    }

    if filter.open_only {
        builder.push(" AND max_enrollment > enrollment");
    }

    if let Some(method) = filter.instructional_method {
        builder.push(" AND instructional_method = ANY(");
        builder.push_bind(method);
        builder.push(")");
    }

    if let Some(campus) = filter.campus {
        builder.push(" AND campus = ANY(");
        builder.push_bind(campus);
        builder.push(")");
    }

    if let Some(wc) = filter.wait_count_max {
        builder.push(" AND wait_count <= ");
        builder.push_bind(wc);
    }

    if let Some(days) = filter.days {
        builder.push(
            " AND EXISTS (\
             SELECT 1 FROM jsonb_array_elements(meeting_times) AS mt, \
             LATERAL jsonb_array_elements_text(mt->'days') AS d(day) \
             WHERE d.day = ANY(",
        );
        builder.push_bind(days);
        builder.push(") GROUP BY mt HAVING COUNT(DISTINCT d.day) = array_length(");
        builder.push_bind(days);
        builder.push(", 1))");
    }

    if let Some(start) = filter.time_start {
        builder.push(
            " AND EXISTS (\
             SELECT 1 FROM jsonb_array_elements(meeting_times) AS mt \
             WHERE (mt->'timeRange'->>'start') >= ",
        );
        builder.push_bind(start);
        builder.push(")");
    }

    if let Some(end) = filter.time_end {
        builder.push(
            " AND EXISTS (\
             SELECT 1 FROM jsonb_array_elements(meeting_times) AS mt \
             WHERE (mt->'timeRange'->>'end') <= ",
        );
        builder.push_bind(end);
        builder.push(")");
    }

    if let Some(pot) = filter.part_of_term {
        builder.push(" AND part_of_term = ANY(");
        builder.push_bind(pot);
        builder.push(")");
    }

    if let Some(attrs) = filter.attributes {
        builder.push(
            " AND EXISTS (\
             SELECT 1 FROM jsonb_array_elements_text(attributes) a \
             WHERE a = ANY(",
        );
        builder.push_bind(attrs);
        builder.push("))");
    }

    if let Some(min) = filter.credit_hour_min {
        builder.push(" AND COALESCE(credit_hours, credit_hour_low, 0) >= ");
        builder.push_bind(min);
    }

    if let Some(max) = filter.credit_hour_max {
        builder.push(" AND COALESCE(credit_hours, credit_hour_high, 0) <= ");
        builder.push_bind(max);
    }

    if let Some(instructors) = filter.instructors {
        builder.push(
            " AND EXISTS (\
             SELECT 1 FROM course_instructors ci \
             JOIN instructors i ON i.id = ci.instructor_id \
             WHERE ci.course_id = courses.id \
             AND i.slug = ANY(",
        );
        builder.push_bind(instructors);
        builder.push("))");
    }
}

/// Catalog order, and the tiebreaker every sort ends on.
const DEFAULT_ORDER: &str = "subject ASC, course_number ASC, sequence_number ASC";

/// Terms beyond this are dropped: a deeper sort cannot change the order, since
/// the tiebreaker is already total, but it does cost a sort pass each.
const MAX_SORT_TERMS: usize = 4;

/// One key ordered one way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SortTerm {
    pub key: SortKey,
    pub direction: SortDirection,
}

impl SortTerm {
    fn to_sql(self) -> String {
        let dir = match self.direction {
            SortDirection::Asc => "ASC",
            SortDirection::Desc => "DESC",
        };
        // NULLS LAST in both directions: a section with no value for the key is
        // not "lowest", it is absent, and reversing should not float it to page one.
        self.key
            .def()
            .exprs
            .iter()
            .map(|expr| format!("{expr} {dir} NULLS LAST"))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

impl fmt::Display for SortTerm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if matches!(self.direction, SortDirection::Desc) {
            f.write_str("-")?;
        }
        f.write_str(self.key.name())
    }
}

impl FromStr for SortTerm {
    type Err = SortParseError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let (direction, name) = match s.strip_prefix('-') {
            Some(rest) => (SortDirection::Desc, rest),
            None => (SortDirection::Asc, s),
        };
        SortKey::from_name(name)
            .map(|key| SortTerm { key, direction })
            .ok_or_else(|| SortParseError::UnknownKey(name.to_owned()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SortParseError {
    UnknownKey(String),
}

impl fmt::Display for SortParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownKey(name) => {
                let known: Vec<&str> = SortKey::ALL.iter().map(|k| k.name()).collect();
                write!(
                    f,
                    "unknown sort key '{name}'; expected one of {}",
                    known.join(", ")
                )
            }
        }
    }
}

impl std::error::Error for SortParseError {}

/// An ordered sort.
///
/// The catalog tiebreaker is appended by `to_sql` rather than stored, so no
/// caller can build a spec that pages unstably. An empty spec is catalog order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SortSpec(Vec<SortTerm>);

impl SortSpec {
    pub fn new(mut terms: Vec<SortTerm>) -> Self {
        terms.truncate(MAX_SORT_TERMS);
        Self(terms)
    }

    /// The ORDER BY body. Every fragment is a hardcoded literal from `KeyDef`,
    /// so nothing a caller sends can reach the SQL text.
    pub fn to_sql(&self) -> String {
        let mut parts: Vec<String> = self.0.iter().map(|term| term.to_sql()).collect();
        parts.push(DEFAULT_ORDER.to_owned());
        parts.join(", ")
    }
}

impl fmt::Display for SortSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, term) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str(",")?;
            }
            write!(f, "{term}")?;
        }
        Ok(())
    }
}

impl FromStr for SortSpec {
    type Err = SortParseError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let terms = s
            .split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(SortTerm::from_str)
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(Self::new(terms))
    }
}

#[cfg(test)]
mod sort_tests {
    use super::*;

    #[test]
    fn wire_names_match_serde() {
        for key in SortKey::ALL {
            let json = serde_json::to_string(key).expect("key serializes");
            assert_eq!(json, format!("\"{}\"", key.name()), "{key:?} drifted");
        }
    }

    #[test]
    fn every_key_is_reachable_by_name() {
        for key in SortKey::ALL {
            assert_eq!(SortKey::from_name(key.name()), Some(*key));
        }
        assert_eq!(SortKey::from_name("nope"), None);
    }

    #[test]
    fn terms_round_trip_through_the_wire_format() {
        let raw = "start_time,-instructor_rating,duration";
        let spec: SortSpec = raw.parse().expect("parses");
        assert_eq!(spec.to_string(), raw);
    }

    #[test]
    fn the_leading_term_drives_the_ordering() {
        let asc = "start_time".parse::<SortSpec>().expect("parses").to_sql();
        let desc = "-start_time".parse::<SortSpec>().expect("parses").to_sql();
        assert!(
            asc.starts_with("first_begin_minutes ASC NULLS LAST"),
            "{asc}"
        );
        assert!(
            desc.starts_with("first_begin_minutes DESC NULLS LAST"),
            "{desc}"
        );
    }

    #[test]
    fn an_unknown_key_names_itself_and_the_alternatives() {
        let err = "start_time,bogus".parse::<SortSpec>().unwrap_err();
        let message = err.to_string();
        assert!(message.contains("bogus"), "{message}");
        assert!(message.contains("start_time"), "{message}");
    }

    /// The tiebreaker is what keeps offset paging stable across shared keys, so
    /// no spec may omit it -- including the empty one.
    #[test]
    fn every_spec_ends_on_the_tiebreaker() {
        assert!(SortSpec::default().to_sql().ends_with(DEFAULT_ORDER));
        for key in SortKey::ALL {
            for direction in [SortDirection::Asc, SortDirection::Desc] {
                let spec = SortSpec::new(vec![SortTerm {
                    key: *key,
                    direction,
                }]);
                assert!(
                    spec.to_sql().ends_with(DEFAULT_ORDER),
                    "{key:?} {direction:?}"
                );
            }
        }
    }

    /// A key spanning several columns must reverse all of them, not just the last.
    #[test]
    fn a_multi_column_key_reverses_every_column() {
        let sql = "-course_code".parse::<SortSpec>().expect("parses").to_sql();
        let leading = sql
            .strip_suffix(&format!(", {DEFAULT_ORDER}"))
            .expect("ends on the tiebreaker");
        for column in SortKey::CourseCode.def().exprs {
            assert!(
                leading.contains(&format!("{column} DESC NULLS LAST")),
                "{column} is not descending in {leading}"
            );
        }
        assert!(!leading.contains("ASC"), "{leading}");
    }

    /// Absence is not a low value: reversing a sort must not float unscheduled
    /// sections to the first page.
    #[test]
    fn null_rows_sink_in_both_directions() {
        for direction in [SortDirection::Asc, SortDirection::Desc] {
            let spec = SortSpec::new(vec![SortTerm {
                key: SortKey::StartTime,
                direction,
            }]);
            assert!(spec.to_sql().contains("NULLS LAST"));
        }
    }

    #[test]
    fn extra_terms_are_dropped_rather_than_sorted_on() {
        let raw = "start_time,end_time,duration,weekly_minutes,days,seats_open";
        let spec: SortSpec = raw.parse().expect("parses");
        assert_eq!(spec.to_string().split(',').count(), MAX_SORT_TERMS);
    }

    #[test]
    fn labels_never_say_ascending() {
        for key in SortKey::ALL {
            for direction in [SortDirection::Asc, SortDirection::Desc] {
                let label = key.label(direction).to_lowercase();
                assert!(!label.contains("ascend"), "{key:?}: {label}");
                assert!(!label.contains("descend"), "{key:?}: {label}");
            }
        }
    }
}

/// Search courses by term with optional filters.
///
/// Returns `(courses, total_count)` for pagination. Uses FTS tsvector for word
/// search and falls back to trigram ILIKE for substring matching.
pub async fn search_courses(
    db_pool: &PgPool,
    filter: &SearchFilter<'_>,
    limit: i32,
    offset: i32,
    sort: &SortSpec,
) -> Result<(Vec<Course>, i64)> {
    let order_by = sort.to_sql();

    // Data query
    let mut data_builder: QueryBuilder<Postgres> = QueryBuilder::new("SELECT * FROM courses");
    push_search_conditions(&mut data_builder, filter);
    data_builder.push(" ORDER BY ");
    data_builder.push(&order_by);
    data_builder.push(" LIMIT ");
    data_builder.push_bind(limit);
    data_builder.push(" OFFSET ");
    data_builder.push_bind(offset);

    let courses = data_builder
        .build_query_as::<Course>()
        .fetch_all(db_pool)
        .await
        .context("failed to search courses")?;

    // Count query
    let mut count_builder: QueryBuilder<Postgres> =
        QueryBuilder::new("SELECT COUNT(*) FROM courses");
    push_search_conditions(&mut count_builder, filter);

    let total: (i64,) = count_builder
        .build_query_as()
        .fetch_one(db_pool)
        .await
        .context("failed to count search results")?;

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
            .await
            .context("failed to fetch course by crn and term")?;
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
               rmp.avg_rating, rmp.num_ratings, rmp.primary_legacy_id as rmp_legacy_id,
               bb.bb_avg_instructor_rating, bb.bb_total_responses,
               i.slug,
               ci.course_id,
               sc.display_score as sc_display_score, sc.sort_score as sc_sort_score,
               sc.ci_lower as sc_ci_lower, sc.ci_upper as sc_ci_upper,
               sc.confidence as sc_confidence, sc.source as sc_source,
               sc.rmp_count as sc_rmp_count, sc.bb_count as sc_bb_count
        FROM course_instructors ci
        JOIN instructors i ON i.id = ci.instructor_id
        LEFT JOIN instructor_rmp_summary rmp ON rmp.instructor_id = i.id
        LEFT JOIN LATERAL (
            SELECT
                AVG(be.instructor_rating)::real as bb_avg_instructor_rating,
                SUM(be.instructor_response_count)::bigint as bb_total_responses
            FROM bluebook_evaluations be
            JOIN instructor_bluebook_links ibl ON ibl.instructor_name = be.instructor_name
                AND (ibl.subject IS NULL OR ibl.subject = be.subject)
            WHERE ibl.instructor_id = i.id
                AND ibl.status IN ('approved', 'auto')
                AND be.instructor_rating IS NOT NULL
                AND be.instructor_response_count > 0
        ) bb ON true
        LEFT JOIN instructor_scores sc ON sc.instructor_id = i.id
        WHERE ci.course_id = $1
        ORDER BY ci.is_primary DESC, i.display_name
        "#,
    )
    .bind(course_id)
    .fetch_all(db_pool)
    .await
    .context("failed to fetch instructors for course")?;
    Ok(rows)
}

/// Batch-fetch instructors for multiple courses in a single query.
///
/// Returns a map of `course_id -> Vec<CourseInstructorDetail>`.
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
               rmp.avg_rating, rmp.num_ratings, rmp.primary_legacy_id as rmp_legacy_id,
               bb.bb_avg_instructor_rating, bb.bb_total_responses,
               i.slug,
               ci.course_id,
               sc.display_score as sc_display_score, sc.sort_score as sc_sort_score,
               sc.ci_lower as sc_ci_lower, sc.ci_upper as sc_ci_upper,
               sc.confidence as sc_confidence, sc.source as sc_source,
               sc.rmp_count as sc_rmp_count, sc.bb_count as sc_bb_count
        FROM course_instructors ci
        JOIN instructors i ON i.id = ci.instructor_id
        LEFT JOIN instructor_rmp_summary rmp ON rmp.instructor_id = i.id
        LEFT JOIN LATERAL (
            SELECT
                AVG(be.instructor_rating)::real as bb_avg_instructor_rating,
                SUM(be.instructor_response_count)::bigint as bb_total_responses
            FROM bluebook_evaluations be
            JOIN instructor_bluebook_links ibl ON ibl.instructor_name = be.instructor_name
                AND (ibl.subject IS NULL OR ibl.subject = be.subject)
            WHERE ibl.instructor_id = i.id
                AND ibl.status IN ('approved', 'auto')
                AND be.instructor_rating IS NOT NULL
                AND be.instructor_response_count > 0
        ) bb ON true
        LEFT JOIN instructor_scores sc ON sc.instructor_id = i.id
        WHERE ci.course_id = ANY($1)
        ORDER BY ci.course_id, ci.is_primary DESC, i.display_name
        "#,
    )
    .bind(course_ids)
    .fetch_all(db_pool)
    .await
    .context("failed to batch fetch instructors for courses")?;

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
        SELECT s.subject,
               COALESCE(rd.description, s.subject),
               s.total_enrollment
        FROM term_subject_summary s
        LEFT JOIN reference_data rd ON rd.category = 'subject' AND rd.code = s.subject
        WHERE s.term_code = $1
        ORDER BY s.total_enrollment DESC, s.subject
        "#,
    )
    .bind(term_code)
    .fetch_all(db_pool)
    .await
    .context("failed to fetch subjects by enrollment")?;
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
    .await
    .context("failed to fetch related sections")?;
    Ok(courses)
}

/// Get all distinct term codes that have courses in the DB.
pub async fn get_available_terms(db_pool: &PgPool) -> Result<Vec<String>> {
    let rows: Vec<(String,)> =
        sqlx::query_as("SELECT DISTINCT term_code FROM courses ORDER BY term_code DESC")
            .fetch_all(db_pool)
            .await
            .context("failed to fetch available terms")?;
    Ok(rows.into_iter().map(|(tc,)| tc).collect())
}

/// List all CRNs for a given term, for sitemap generation.
pub async fn list_crns_for_term(db_pool: &PgPool, term_code: &str) -> Result<Vec<String>> {
    let rows: Vec<(String,)> =
        sqlx::query_as("SELECT crn FROM courses WHERE term_code = $1 ORDER BY crn")
            .bind(term_code)
            .fetch_all(db_pool)
            .await
            .context("failed to list crns for term")?;
    Ok(rows.into_iter().map(|(crn,)| crn).collect())
}

/// List all distinct subject codes, for sitemap generation.
pub async fn list_all_subjects(db_pool: &PgPool) -> Result<Vec<String>> {
    let rows: Vec<(String,)> =
        sqlx::query_as("SELECT DISTINCT subject FROM courses ORDER BY subject")
            .fetch_all(db_pool)
            .await
            .context("failed to list all subjects")?;
    Ok(rows.into_iter().map(|(s,)| s).collect())
}

type RangeRow = (
    Option<i32>,
    Option<i32>,
    Option<f64>,
    Option<f64>,
    Option<i32>,
);

/// Get aggregate filter ranges for a term (course number, credit hours, waitlist).
pub async fn get_filter_ranges(db_pool: &PgPool, term_code: &str) -> Result<FilterRanges> {
    // An unknown term produces no row here, whereas the aggregate this replaced
    // returned a single all-NULL row. Both funnel into the same defaults below.
    let row: RangeRow = sqlx::query_as(
        r#"
        SELECT course_number_min, course_number_max,
               credit_hour_min, credit_hour_max, wait_count_max
        FROM term_summary
        WHERE term_code = $1
        "#,
    )
    .bind(term_code)
    .fetch_optional(db_pool)
    .await
    .context("failed to fetch filter ranges for term")?
    .unwrap_or((None, None, None, None, None));

    let cn_max = row.1.unwrap_or(9000);
    let ch_min = row.2.unwrap_or(0.0);
    let ch_max = row.3.unwrap_or(8.0);
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
    pub slug: String,
    pub display_name: String,
    pub section_count: i32,
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
               MAX(GREATEST(similarity(immutable_unaccent(title), immutable_unaccent($2)), similarity(subject || ' ' || course_number, $2))) as score
        FROM courses
        WHERE term_code = $1
          AND (immutable_unaccent(title) % immutable_unaccent($2) OR immutable_unaccent(title) ILIKE '%' || immutable_unaccent($2) || '%'
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
    .await
    .context("failed to suggest courses")?;

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
    let rows: Vec<(i32, String, String, i32, f32)> = sqlx::query_as(
        r#"
        SELECT i.id, i.slug, i.display_name,
               COUNT(DISTINCT c.id)::int as section_count,
               MAX(similarity(immutable_unaccent(i.display_name), immutable_unaccent($2))) as score
        FROM instructors i
        JOIN course_instructors ci ON ci.instructor_id = i.id
        JOIN courses c ON c.id = ci.course_id
        WHERE c.term_code = $1
          AND i.slug IS NOT NULL
          AND (immutable_unaccent(i.display_name) % immutable_unaccent($2) OR immutable_unaccent(i.display_name) ILIKE '%' || immutable_unaccent($2) || '%')
        GROUP BY i.id, i.slug, i.display_name
        ORDER BY score DESC
        LIMIT $3
        "#,
    )
    .bind(term_code)
    .bind(query)
    .bind(limit)
    .fetch_all(db_pool)
    .await
    .context("failed to suggest instructors")?;

    Ok(rows
        .into_iter()
        .map(
            |(id, slug, display_name, section_count, score)| InstructorSuggestion {
                id,
                slug,
                display_name,
                section_count,
                score,
            },
        )
        .collect())
}

/// Suggest instructors with an optional term filter.
/// When a term is provided, section_count is scoped to that term.
/// When no term is provided, section_count is across all terms.
pub async fn suggest_instructors_global(
    db_pool: &PgPool,
    term_code: Option<&str>,
    query: &str,
    limit: i32,
) -> Result<Vec<InstructorSuggestion>> {
    let rows: Vec<(i32, String, String, i32, f32)> = sqlx::query_as(
        r#"
        SELECT i.id, i.slug, i.display_name,
               COUNT(DISTINCT c.id)::int as section_count,
               MAX(similarity(immutable_unaccent(i.display_name), immutable_unaccent($2))) as score
        FROM instructors i
        JOIN course_instructors ci ON ci.instructor_id = i.id
        JOIN courses c ON c.id = ci.course_id
        WHERE ($1::text IS NULL OR c.term_code = $1)
          AND i.slug IS NOT NULL
          AND (immutable_unaccent(i.display_name) % immutable_unaccent($2) OR immutable_unaccent(i.display_name) ILIKE '%' || immutable_unaccent($2) || '%')
        GROUP BY i.id, i.slug, i.display_name
        ORDER BY score DESC
        LIMIT $3
        "#,
    )
    .bind(term_code)
    .bind(query)
    .bind(limit)
    .fetch_all(db_pool)
    .await
    .context("failed to suggest instructors globally")?;

    Ok(rows
        .into_iter()
        .map(
            |(id, slug, display_name, section_count, score)| InstructorSuggestion {
                id,
                slug,
                display_name,
                section_count,
                score,
            },
        )
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

/// Recompute the cached per-term aggregates backing the search-options endpoint.
///
/// Terms other than the one being scraped are untouched, so past terms keep the
/// values computed when they were last active.
pub async fn refresh_term_summary(pool: &PgPool, term_code: &str) -> Result<()> {
    sqlx::query("SELECT refresh_term_summary($1)")
        .bind(term_code)
        .execute(pool)
        .await
        .context("failed to refresh term summary")?;
    Ok(())
}

/// Count all courses in the database.
pub async fn count_all(pool: &PgPool) -> Result<i64> {
    let (count,): (i64,) =
        sqlx::query_as("SELECT COALESCE(SUM(course_count), 0)::bigint FROM term_summary")
            .fetch_one(pool)
            .await
            .context("failed to count all courses")?;
    Ok(count)
}

/// Look up a course's internal ID by term code and CRN.
pub async fn get_id_by_crn(pool: &PgPool, term_code: &str, crn: &str) -> Result<Option<i32>> {
    let row: Option<(i32,)> =
        sqlx::query_as("SELECT id FROM courses WHERE term_code = $1 AND crn = $2")
            .bind(term_code)
            .bind(crn)
            .fetch_optional(pool)
            .await
            .context("failed to get course id by crn")?;
    Ok(row.map(|(id,)| id))
}

/// Count courses grouped by subject for a given term.
///
/// Returns a map of subject code -> count.
pub async fn count_by_subject(pool: &PgPool, term_code: &str) -> Result<HashMap<String, i64>> {
    let rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT subject, COUNT(*)::BIGINT AS cnt FROM courses WHERE term_code = $1 GROUP BY subject",
    )
    .bind(term_code)
    .fetch_all(pool)
    .await
    .context("failed to count courses by subject")?;
    Ok(rows.into_iter().collect())
}
