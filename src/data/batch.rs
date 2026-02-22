//! Batch database operations for improved performance.

use crate::banner::Course;
use crate::banner::models::meetings::TimeRange;
use crate::data::course_types::{DateRange, MeetingLocation};
use crate::data::models::{DayOfWeek, DbMeetingTime, UpsertCounts};
use crate::data::names::{decode_html_entities, parse_banner_name};
use crate::utils::fmt_duration;
use crate::web::audit::{AuditLogEntry, AuditRow};
use anyhow::Result;
use chrono::NaiveDate;
use sqlx::PgConnection;
use sqlx::PgPool;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

/// Parse a date string in MM/DD/YYYY format to `NaiveDate`.
fn parse_mm_dd_yyyy(s: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(s, "%m/%d/%Y").ok()
}

/// Convert a Banner API course's meeting times to the DB JSONB shape.
fn to_db_meeting_times(course: &Course) -> serde_json::Value {
    let meetings: Vec<DbMeetingTime> = course
        .meetings_faculty
        .iter()
        .map(|mf| {
            let mt = &mf.meeting_time;

            // Build days BTreeSet from boolean flags
            let mut days = BTreeSet::new();
            if mt.monday {
                days.insert(DayOfWeek::Monday);
            }
            if mt.tuesday {
                days.insert(DayOfWeek::Tuesday);
            }
            if mt.wednesday {
                days.insert(DayOfWeek::Wednesday);
            }
            if mt.thursday {
                days.insert(DayOfWeek::Thursday);
            }
            if mt.friday {
                days.insert(DayOfWeek::Friday);
            }
            if mt.saturday {
                days.insert(DayOfWeek::Saturday);
            }
            if mt.sunday {
                days.insert(DayOfWeek::Sunday);
            }

            // Parse time range from HHMM strings
            let time_range = match (mt.begin_time.as_deref(), mt.end_time.as_deref()) {
                (Some(begin), Some(end)) => {
                    let result = TimeRange::from_hhmm(begin, end);
                    if result.is_none() {
                        warn!(
                            crn = %mt.course_reference_number,
                            begin, end,
                            "Failed to parse meeting time range"
                        );
                    }
                    result
                }
                _ => None,
            };

            // Parse date range from MM/DD/YYYY strings
            let date_range = match (
                parse_mm_dd_yyyy(&mt.start_date),
                parse_mm_dd_yyyy(&mt.end_date),
            ) {
                (Some(start), Some(end)) => DateRange::new(start, end).unwrap_or_else(|err| {
                    warn!(
                        crn = %mt.course_reference_number,
                        start_date = %mt.start_date,
                        end_date = %mt.end_date,
                        ?err,
                        "Invalid date range, swapping start/end"
                    );
                    // Swap so the invariant holds
                    DateRange {
                        start: end,
                        end: start,
                    }
                }),
                _ => {
                    warn!(
                        crn = %mt.course_reference_number,
                        start_date = %mt.start_date,
                        end_date = %mt.end_date,
                        "Failed to parse meeting date range, using epoch fallback"
                    );
                    let epoch = NaiveDate::from_ymd_opt(1970, 1, 1).unwrap();
                    DateRange {
                        start: epoch,
                        end: epoch,
                    }
                }
            };

            // Build location if any field is present
            let location = {
                let loc = MeetingLocation {
                    building: mt.building.clone(),
                    building_description: mt.building_description.clone(),
                    room: mt.room.clone(),
                    campus: mt.campus.clone(),
                };
                if loc.building.is_some()
                    || loc.building_description.is_some()
                    || loc.room.is_some()
                    || loc.campus.is_some()
                {
                    Some(loc)
                } else {
                    None
                }
            };

            DbMeetingTime {
                time_range,
                date_range,
                days,
                location,
                meeting_type: mt.meeting_type.clone(),
                meeting_schedule_type: mt.meeting_schedule_type.clone(),
            }
        })
        .collect();
    serde_json::to_value(meetings).unwrap_or_default()
}

/// Convert a Banner API course's section attributes to a JSONB array of code strings.
fn to_db_attributes(course: &Course) -> serde_json::Value {
    let codes: Vec<&str> = course
        .section_attributes
        .iter()
        .map(|a| a.code.as_str())
        .collect();
    serde_json::to_value(codes).unwrap_or_default()
}

/// Extract the campus code from the first meeting time (Banner doesn't put it on the course directly).
fn extract_campus_code(course: &Course) -> Option<String> {
    course
        .meetings_faculty
        .first()
        .and_then(|mf| mf.meeting_time.campus.clone())
}

/// Row returned by the CTE-based upsert query, carrying both old and new values
/// for every auditable field. `old_id` is `None` for fresh inserts.
#[derive(sqlx::FromRow, Debug)]
struct UpsertDiffRow {
    id: i32,
    old_id: Option<i32>,
    crn: String,
    term_code: String,

    // enrollment fields
    old_enrollment: Option<i32>,
    new_enrollment: i32,
    old_max_enrollment: Option<i32>,
    new_max_enrollment: i32,
    old_wait_count: Option<i32>,
    new_wait_count: i32,
    old_wait_capacity: Option<i32>,
    new_wait_capacity: i32,

    // text fields (non-nullable in DB)
    old_subject: Option<String>,
    new_subject: String,
    old_course_number: Option<String>,
    new_course_number: String,
    old_title: Option<String>,
    new_title: String,

    // nullable text fields
    old_sequence_number: Option<String>,
    new_sequence_number: Option<String>,
    old_part_of_term: Option<String>,
    new_part_of_term: Option<String>,
    old_instructional_method: Option<String>,
    new_instructional_method: Option<String>,
    old_campus: Option<String>,
    new_campus: Option<String>,

    // nullable int fields
    old_credit_hours: Option<i32>,
    new_credit_hours: Option<i32>,
    old_credit_hour_low: Option<i32>,
    new_credit_hour_low: Option<i32>,
    old_credit_hour_high: Option<i32>,
    new_credit_hour_high: Option<i32>,

    // cross-list fields
    old_cross_list: Option<String>,
    new_cross_list: Option<String>,
    old_cross_list_capacity: Option<i32>,
    new_cross_list_capacity: Option<i32>,
    old_cross_list_count: Option<i32>,
    new_cross_list_count: Option<i32>,

    // link fields
    old_link_identifier: Option<String>,
    new_link_identifier: Option<String>,
    old_is_section_linked: Option<bool>,
    new_is_section_linked: Option<bool>,

    // JSONB fields
    old_meeting_times: Option<serde_json::Value>,
    new_meeting_times: serde_json::Value,
    old_attributes: Option<serde_json::Value>,
    new_attributes: serde_json::Value,
}

struct AuditEntry {
    course_id: i32,
    field_changed: &'static str,
    old_value: Option<serde_json::Value>,
    new_value: serde_json::Value,
}

struct MetricEntry {
    course_id: i32,
    enrollment: i32,
    wait_count: i32,
    seats_available: i32,
}

/// Compare old vs new for a single field, pushing an `AuditEntry` when they differ.
///
/// Three variants:
/// - `diff_field!(audits, row, field_name, old_field, new_field)` — `Option<T>` old vs `T` new
/// - `diff_field!(opt audits, row, field_name, old_field, new_field)` — `Option<T>` old vs `Option<T>` new
/// - `diff_field!(json audits, row, field_name, old_field, new_field)` — `Option<Value>` old vs `Value` new
///
/// All variants skip when `old_id` is None (fresh insert).
macro_rules! diff_field {
    // Standard: Option<T> old vs T new (non-nullable columns)
    ($audits:ident, $row:ident, $field:expr, $old:ident, $new:ident) => {
        if $row.old_id.is_some() {
            let old_val = $row.$old.as_ref().map(|v| serde_json::json!(v));
            let new_val = serde_json::json!($row.$new);
            if old_val.as_ref() != Some(&new_val) {
                $audits.push(AuditEntry {
                    course_id: $row.id,
                    field_changed: $field,
                    old_value: old_val,
                    new_value: new_val,
                });
            }
        }
    };
    // Nullable: Option<T> old vs Option<T> new
    (opt $audits:ident, $row:ident, $field:expr, $old:ident, $new:ident) => {
        if $row.old_id.is_some() {
            let old_val = $row.$old.as_ref().map(|v| serde_json::json!(v));
            let new_val = $row.$new.as_ref().map(|v| serde_json::json!(v));
            if old_val != new_val {
                $audits.push(AuditEntry {
                    course_id: $row.id,
                    field_changed: $field,
                    old_value: old_val,
                    new_value: new_val.unwrap_or(serde_json::Value::Null),
                });
            }
        }
    };
    // JSONB: Option<Value> old vs Value new
    (json $audits:ident, $row:ident, $field:expr, $old:ident, $new:ident) => {
        if $row.old_id.is_some() {
            let old_val = $row.$old.clone();
            let new_val = $row.$new.clone();
            if old_val.as_ref() != Some(&new_val) {
                $audits.push(AuditEntry {
                    course_id: $row.id,
                    field_changed: $field,
                    old_value: old_val,
                    new_value: new_val,
                });
            }
        }
    };
}

/// Compute audit entries (field-level diffs) and metric entries from upsert diff rows.
fn compute_diffs(rows: &[UpsertDiffRow]) -> (Vec<AuditEntry>, Vec<MetricEntry>) {
    let mut audits = Vec::new();
    let mut metrics = Vec::new();

    for row in rows {
        if row.old_id.is_none() {
            let snapshot = serde_json::json!({
                "enrollment": row.new_enrollment,
                "maxEnrollment": row.new_max_enrollment,
                "waitCount": row.new_wait_count,
                "waitCapacity": row.new_wait_capacity,
                "title": row.new_title,
                "subject": row.new_subject,
                "courseNumber": row.new_course_number,
                "instructionalMethod": row.new_instructional_method,
                "campus": row.new_campus,
                "creditHours": row.new_credit_hours,
            });
            audits.push(AuditEntry {
                course_id: row.id,
                field_changed: "initial",
                old_value: None,
                new_value: snapshot,
            });
        }

        // Non-nullable fields
        diff_field!(audits, row, "enrollment", old_enrollment, new_enrollment);
        diff_field!(
            audits,
            row,
            "max_enrollment",
            old_max_enrollment,
            new_max_enrollment
        );
        diff_field!(audits, row, "wait_count", old_wait_count, new_wait_count);
        diff_field!(
            audits,
            row,
            "wait_capacity",
            old_wait_capacity,
            new_wait_capacity
        );
        diff_field!(audits, row, "subject", old_subject, new_subject);
        diff_field!(
            audits,
            row,
            "course_number",
            old_course_number,
            new_course_number
        );
        diff_field!(audits, row, "title", old_title, new_title);

        // Nullable text fields
        diff_field!(opt audits, row, "sequence_number", old_sequence_number, new_sequence_number);
        diff_field!(opt audits, row, "part_of_term", old_part_of_term, new_part_of_term);
        diff_field!(opt audits, row, "instructional_method", old_instructional_method, new_instructional_method);
        diff_field!(opt audits, row, "campus", old_campus, new_campus);

        // Nullable int fields
        diff_field!(opt audits, row, "credit_hours", old_credit_hours, new_credit_hours);
        diff_field!(opt audits, row, "credit_hour_low", old_credit_hour_low, new_credit_hour_low);
        diff_field!(opt audits, row, "credit_hour_high", old_credit_hour_high, new_credit_hour_high);

        // Cross-list fields
        diff_field!(opt audits, row, "cross_list", old_cross_list, new_cross_list);
        diff_field!(opt audits, row, "cross_list_capacity", old_cross_list_capacity, new_cross_list_capacity);
        diff_field!(opt audits, row, "cross_list_count", old_cross_list_count, new_cross_list_count);

        // Link fields
        diff_field!(opt audits, row, "link_identifier", old_link_identifier, new_link_identifier);
        diff_field!(opt audits, row, "is_section_linked", old_is_section_linked, new_is_section_linked);

        // JSONB fields
        diff_field!(json audits, row, "meeting_times", old_meeting_times, new_meeting_times);
        diff_field!(json audits, row, "attributes", old_attributes, new_attributes);

        // Emit a metric entry on fresh insert (baseline) or when enrollment data changed
        let is_new = row.old_id.is_none();
        let enrollment_changed = row.old_id.is_some()
            && (row.old_enrollment != Some(row.new_enrollment)
                || row.old_wait_count != Some(row.new_wait_count)
                || row.old_max_enrollment != Some(row.new_max_enrollment));

        if is_new || enrollment_changed {
            metrics.push(MetricEntry {
                course_id: row.id,
                enrollment: row.new_enrollment,
                wait_count: row.new_wait_count,
                seats_available: row.new_max_enrollment - row.new_enrollment,
            });
        }
    }

    (audits, metrics)
}

async fn insert_audits(audits: &[AuditEntry], conn: &mut PgConnection) -> Result<Vec<i32>> {
    if audits.is_empty() {
        return Ok(Vec::new());
    }

    let course_ids: Vec<i32> = audits.iter().map(|a| a.course_id).collect();
    let fields: Vec<&str> = audits.iter().map(|a| a.field_changed).collect();
    let old_values: Vec<Option<serde_json::Value>> =
        audits.iter().map(|a| a.old_value.clone()).collect();
    let new_values: Vec<serde_json::Value> = audits.iter().map(|a| a.new_value.clone()).collect();

    let rows: Vec<(i32,)> = sqlx::query_as(
        r#"
        INSERT INTO course_audits (course_id, timestamp, field_changed, old_value, new_value)
        SELECT v.course_id, NOW(), v.field_changed, v.old_value, v.new_value
        FROM UNNEST($1::int4[], $2::text[], $3::jsonb[], $4::jsonb[])
            AS v(course_id, field_changed, old_value, new_value)
        RETURNING id
        "#,
    )
    .bind(&course_ids)
    .bind(&fields)
    .bind(&old_values)
    .bind(&new_values)
    .fetch_all(&mut *conn)
    .await
    .map_err(|e| anyhow::anyhow!("Failed to batch insert course_audits: {}", e))?;

    Ok(rows.into_iter().map(|(id,)| id).collect())
}

async fn insert_metrics(metrics: &[MetricEntry], conn: &mut PgConnection) -> Result<()> {
    if metrics.is_empty() {
        return Ok(());
    }

    let course_ids: Vec<i32> = metrics.iter().map(|m| m.course_id).collect();
    let enrollments: Vec<i32> = metrics.iter().map(|m| m.enrollment).collect();
    let wait_counts: Vec<i32> = metrics.iter().map(|m| m.wait_count).collect();
    let seats_available: Vec<i32> = metrics.iter().map(|m| m.seats_available).collect();

    sqlx::query(
        r#"
        INSERT INTO course_metrics (course_id, timestamp, enrollment, wait_count, seats_available)
        SELECT v.course_id, NOW(), v.enrollment, v.wait_count, v.seats_available
        FROM UNNEST($1::int4[], $2::int4[], $3::int4[], $4::int4[])
            AS v(course_id, enrollment, wait_count, seats_available)
        "#,
    )
    .bind(&course_ids)
    .bind(&enrollments)
    .bind(&wait_counts)
    .bind(&seats_available)
    .execute(&mut *conn)
    .await
    .map_err(|e| anyhow::anyhow!("Failed to batch insert course_metrics: {}", e))?;

    Ok(())
}

/// Batch upsert courses in a single database query.
///
/// Performs a bulk INSERT...ON CONFLICT DO UPDATE for all courses, including
/// new fields (meeting times, attributes, instructor data). Captures pre-update
/// state for audit/metric tracking, all within a single transaction.
///
/// # Performance
/// - Reduces N database round-trips to 5 (old-data CTE + upsert, audits, metrics, instructors, junction)
/// - Typical usage: 50-200 courses per batch
pub async fn batch_upsert_courses(
    courses: &[Course],
    db_pool: &PgPool,
) -> Result<(UpsertCounts, Vec<AuditLogEntry>)> {
    if courses.is_empty() {
        info!("No courses to upsert, skipping batch operation");
        return Ok((UpsertCounts::default(), Vec::new()));
    }

    let start = Instant::now();
    let course_count = courses.len();

    let mut tx = db_pool.begin().await?;

    // Step 1: Upsert courses with CTE, returning diff rows
    let diff_rows = upsert_courses(courses, &mut tx).await?;

    // Step 2: Build (crn, term_code) → course_id map for instructor linking.
    // RETURNING order from INSERT ... ON CONFLICT is not guaranteed to match
    // the input array order, so we must key by (crn, term_code) rather than
    // relying on positional correspondence.
    let crn_term_to_id: HashMap<(&str, &str), i32> = diff_rows
        .iter()
        .map(|r| ((r.crn.as_str(), r.term_code.as_str()), r.id))
        .collect();

    // Step 3: Compute audit/metric diffs
    let (mut audits, metrics) = compute_diffs(&diff_rows);

    // Step 4: Upsert instructors (returns email -> id map)
    let email_to_id = upsert_instructors(courses, &mut tx).await?;

    // Step 5: Link courses to instructors via junction table, collecting instructor audits
    let instructor_audits =
        upsert_course_instructors(courses, &crn_term_to_id, &email_to_id, &mut tx).await?;
    audits.extend(instructor_audits);

    // Count courses that had at least one field change (existing rows only)
    let changed_ids: HashSet<i32> = audits.iter().map(|a| a.course_id).collect();
    let existing_count = diff_rows.iter().filter(|r| r.old_id.is_some()).count() as i32;
    let courses_changed = changed_ids.len() as i32;

    let counts = UpsertCounts {
        courses_fetched: course_count as i32,
        courses_changed,
        courses_unchanged: existing_count - courses_changed,
        audits_generated: audits.len() as i32,
        metrics_generated: metrics.len() as i32,
    };

    // Step 6: Insert audits and metrics
    let audit_ids = insert_audits(&audits, &mut tx).await?;
    insert_metrics(&metrics, &mut tx).await?;

    tx.commit().await?;

    let audit_entries = if !audit_ids.is_empty() {
        fetch_audit_entries_by_ids(db_pool, &audit_ids)
            .await
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    let duration = start.elapsed();

    if counts.courses_changed > 0 {
        if counts.audits_generated > 0 {
            info!(
                courses_count = course_count,
                courses_changed = counts.courses_changed,
                duration = fmt_duration(duration),
                audit_entries = counts.audits_generated,
                "Batch upserted courses"
            );
        } else {
            info!(
                courses_count = course_count,
                courses_changed = counts.courses_changed,
                duration = fmt_duration(duration),
                "Batch upserted courses"
            );
        }
    } else if counts.audits_generated > 0 {
        debug!(
            courses_count = course_count,
            courses_changed = counts.courses_changed,
            duration = fmt_duration(duration),
            audit_entries = counts.audits_generated,
            "Batch upserted courses"
        );
    } else {
        debug!(
            courses_count = course_count,
            courses_changed = counts.courses_changed,
            duration = fmt_duration(duration),
            "Batch upserted courses"
        );
    }

    if duration > Duration::from_millis(500) {
        warn!(
            courses_count = course_count,
            duration = fmt_duration(duration),
            "Slow batch upsert query"
        );
    }

    Ok((counts, audit_entries))
}

async fn fetch_audit_entries_by_ids(
    db_pool: &PgPool,
    audit_ids: &[i32],
) -> Result<Vec<AuditLogEntry>> {
    let rows: Vec<AuditRow> = sqlx::query_as(
        "SELECT a.id, a.course_id, a.timestamp, a.field_changed, a.old_value, a.new_value, \
                c.subject, c.course_number, c.crn, c.title, c.term_code \
         FROM course_audits a \
         LEFT JOIN courses c ON c.id = a.course_id \
         WHERE a.id = ANY($1)",
    )
    .bind(audit_ids)
    .fetch_all(db_pool)
    .await
    .map_err(|e| anyhow::anyhow!("Failed to fetch audit entries: {}", e))?;

    Ok(rows.into_iter().map(AuditLogEntry::from).collect())
}

/// Upsert all courses and return diff rows with old and new values for auditing.
async fn upsert_courses(courses: &[Course], conn: &mut PgConnection) -> Result<Vec<UpsertDiffRow>> {
    let crns: Vec<&str> = courses
        .iter()
        .map(|c| c.course_reference_number.as_str())
        .collect();
    let subjects: Vec<&str> = courses.iter().map(|c| c.subject.as_str()).collect();
    let course_numbers: Vec<&str> = courses.iter().map(|c| c.course_number.as_str()).collect();
    let titles: Vec<String> = courses
        .iter()
        .map(|c| decode_html_entities(&c.course_title))
        .collect();
    let term_codes: Vec<&str> = courses.iter().map(|c| c.term.as_str()).collect();
    let enrollments: Vec<i32> = courses.iter().map(|c| c.enrollment).collect();
    let max_enrollments: Vec<i32> = courses.iter().map(|c| c.maximum_enrollment).collect();
    let wait_counts: Vec<i32> = courses.iter().map(|c| c.wait_count).collect();
    let wait_capacities: Vec<i32> = courses.iter().map(|c| c.wait_capacity).collect();

    // New scalar fields
    let sequence_numbers: Vec<Option<&str>> = courses
        .iter()
        .map(|c| Some(c.sequence_number.as_str()))
        .collect();
    let parts_of_term: Vec<Option<&str>> = courses
        .iter()
        .map(|c| Some(c.part_of_term.as_str()))
        .collect();
    let instructional_methods: Vec<Option<&str>> = courses
        .iter()
        .map(|c| c.instructional_method.as_deref())
        .collect();
    let campuses: Vec<Option<String>> = courses.iter().map(extract_campus_code).collect();
    let credit_hours: Vec<Option<i32>> = courses.iter().map(|c| c.credit_hours).collect();
    let credit_hour_lows: Vec<Option<i32>> = courses.iter().map(|c| c.credit_hour_low).collect();
    let credit_hour_highs: Vec<Option<i32>> = courses.iter().map(|c| c.credit_hour_high).collect();
    let cross_lists: Vec<Option<&str>> = courses.iter().map(|c| c.cross_list.as_deref()).collect();
    let cross_list_capacities: Vec<Option<i32>> =
        courses.iter().map(|c| c.cross_list_capacity).collect();
    let cross_list_counts: Vec<Option<i32>> = courses.iter().map(|c| c.cross_list_count).collect();
    let link_identifiers: Vec<Option<&str>> = courses
        .iter()
        .map(|c| c.link_identifier.as_deref())
        .collect();
    let is_section_linkeds: Vec<Option<bool>> =
        courses.iter().map(|c| Some(c.is_section_linked)).collect();

    // JSONB fields
    let meeting_times_json: Vec<serde_json::Value> =
        courses.iter().map(to_db_meeting_times).collect();
    let attributes_json: Vec<serde_json::Value> = courses.iter().map(to_db_attributes).collect();

    let rows = sqlx::query_as::<_, UpsertDiffRow>(
        r#"
        WITH old_data AS (
            SELECT id, enrollment, max_enrollment, wait_count, wait_capacity,
                   subject, course_number, title,
                   sequence_number, part_of_term, instructional_method, campus,
                   credit_hours, credit_hour_low, credit_hour_high,
                   cross_list, cross_list_capacity, cross_list_count,
                   link_identifier, is_section_linked,
                   meeting_times, attributes,
                   crn, term_code
            FROM courses
            WHERE (crn, term_code) IN (SELECT * FROM UNNEST($1::text[], $5::text[]))
        ),
        upserted AS (
            INSERT INTO courses (
                crn, subject, course_number, title, term_code,
                enrollment, max_enrollment, wait_count, wait_capacity, last_scraped_at,
                sequence_number, part_of_term, instructional_method, campus,
                credit_hours, credit_hour_low, credit_hour_high,
                cross_list, cross_list_capacity, cross_list_count,
                link_identifier, is_section_linked,
                meeting_times, attributes
            )
            SELECT
                v.crn, v.subject, v.course_number, v.title, v.term_code,
                v.enrollment, v.max_enrollment, v.wait_count, v.wait_capacity, NOW(),
                v.sequence_number, v.part_of_term, v.instructional_method, v.campus,
                v.credit_hours, v.credit_hour_low, v.credit_hour_high,
                v.cross_list, v.cross_list_capacity, v.cross_list_count,
                v.link_identifier, v.is_section_linked,
                v.meeting_times, v.attributes
            FROM UNNEST(
                $1::text[], $2::text[], $3::text[], $4::text[], $5::text[],
                $6::int4[], $7::int4[], $8::int4[], $9::int4[],
                $10::text[], $11::text[], $12::text[], $13::text[],
                $14::int4[], $15::int4[], $16::int4[],
                $17::text[], $18::int4[], $19::int4[],
                $20::text[], $21::bool[],
                $22::jsonb[], $23::jsonb[]
            ) AS v(
                crn, subject, course_number, title, term_code,
                enrollment, max_enrollment, wait_count, wait_capacity,
                sequence_number, part_of_term, instructional_method, campus,
                credit_hours, credit_hour_low, credit_hour_high,
                cross_list, cross_list_capacity, cross_list_count,
                link_identifier, is_section_linked,
                meeting_times, attributes
            )
            ON CONFLICT (crn, term_code)
            DO UPDATE SET
                subject = EXCLUDED.subject,
                course_number = EXCLUDED.course_number,
                title = EXCLUDED.title,
                enrollment = EXCLUDED.enrollment,
                max_enrollment = EXCLUDED.max_enrollment,
                wait_count = EXCLUDED.wait_count,
                wait_capacity = EXCLUDED.wait_capacity,
                last_scraped_at = EXCLUDED.last_scraped_at,
                sequence_number = EXCLUDED.sequence_number,
                part_of_term = EXCLUDED.part_of_term,
                instructional_method = EXCLUDED.instructional_method,
                campus = EXCLUDED.campus,
                credit_hours = EXCLUDED.credit_hours,
                credit_hour_low = EXCLUDED.credit_hour_low,
                credit_hour_high = EXCLUDED.credit_hour_high,
                cross_list = EXCLUDED.cross_list,
                cross_list_capacity = EXCLUDED.cross_list_capacity,
                cross_list_count = EXCLUDED.cross_list_count,
                link_identifier = EXCLUDED.link_identifier,
                is_section_linked = EXCLUDED.is_section_linked,
                meeting_times = EXCLUDED.meeting_times,
                attributes = EXCLUDED.attributes
            RETURNING *
        )
        SELECT u.id,
               o.id AS old_id,
               u.crn, u.term_code,
               o.enrollment AS old_enrollment, u.enrollment AS new_enrollment,
               o.max_enrollment AS old_max_enrollment, u.max_enrollment AS new_max_enrollment,
               o.wait_count AS old_wait_count, u.wait_count AS new_wait_count,
               o.wait_capacity AS old_wait_capacity, u.wait_capacity AS new_wait_capacity,
               o.subject AS old_subject, u.subject AS new_subject,
               o.course_number AS old_course_number, u.course_number AS new_course_number,
               o.title AS old_title, u.title AS new_title,
               o.sequence_number AS old_sequence_number, u.sequence_number AS new_sequence_number,
               o.part_of_term AS old_part_of_term, u.part_of_term AS new_part_of_term,
               o.instructional_method AS old_instructional_method, u.instructional_method AS new_instructional_method,
               o.campus AS old_campus, u.campus AS new_campus,
               o.credit_hours AS old_credit_hours, u.credit_hours AS new_credit_hours,
               o.credit_hour_low AS old_credit_hour_low, u.credit_hour_low AS new_credit_hour_low,
               o.credit_hour_high AS old_credit_hour_high, u.credit_hour_high AS new_credit_hour_high,
               o.cross_list AS old_cross_list, u.cross_list AS new_cross_list,
               o.cross_list_capacity AS old_cross_list_capacity, u.cross_list_capacity AS new_cross_list_capacity,
               o.cross_list_count AS old_cross_list_count, u.cross_list_count AS new_cross_list_count,
               o.link_identifier AS old_link_identifier, u.link_identifier AS new_link_identifier,
               o.is_section_linked AS old_is_section_linked, u.is_section_linked AS new_is_section_linked,
               o.meeting_times AS old_meeting_times, u.meeting_times AS new_meeting_times,
               o.attributes AS old_attributes, u.attributes AS new_attributes
        FROM upserted u
        LEFT JOIN old_data o ON u.crn = o.crn AND u.term_code = o.term_code
        "#,
    )
    .bind(&crns)
    .bind(&subjects)
    .bind(&course_numbers)
    .bind(&titles)
    .bind(&term_codes)
    .bind(&enrollments)
    .bind(&max_enrollments)
    .bind(&wait_counts)
    .bind(&wait_capacities)
    .bind(&sequence_numbers)
    .bind(&parts_of_term)
    .bind(&instructional_methods)
    .bind(&campuses)
    .bind(&credit_hours)
    .bind(&credit_hour_lows)
    .bind(&credit_hour_highs)
    .bind(&cross_lists)
    .bind(&cross_list_capacities)
    .bind(&cross_list_counts)
    .bind(&link_identifiers)
    .bind(&is_section_linkeds)
    .bind(&meeting_times_json)
    .bind(&attributes_json)
    .fetch_all(&mut *conn)
    .await
    .map_err(|e| anyhow::anyhow!("Failed to batch upsert courses: {}", e))?;

    Ok(rows)
}

/// Deduplicate and upsert all instructors from the batch by email.
/// Returns a map of lowercased_email -> instructor id for junction linking.
async fn upsert_instructors(
    courses: &[Course],
    conn: &mut PgConnection,
) -> Result<HashMap<String, i32>> {
    let mut seen = HashSet::new();
    let mut display_names: Vec<String> = Vec::new();
    let mut first_names: Vec<Option<String>> = Vec::new();
    let mut last_names: Vec<Option<String>> = Vec::new();
    let mut emails_lower: Vec<String> = Vec::new();
    let mut skipped_no_email = 0u32;

    for course in courses {
        for faculty in &course.faculty {
            if let Some(email) = &faculty.email_address {
                let email_lower = email.to_lowercase();
                if seen.insert(email_lower.clone()) {
                    let parts = parse_banner_name(&faculty.display_name);
                    display_names.push(decode_html_entities(&faculty.display_name));
                    first_names.push(parts.as_ref().map(|p| p.first.clone()));
                    last_names.push(parts.as_ref().map(|p| p.last.clone()));
                    emails_lower.push(email_lower);
                }
            } else {
                skipped_no_email += 1;
            }
        }
    }

    if skipped_no_email > 0 {
        warn!(
            count = skipped_no_email,
            "Skipped instructors with no email address"
        );
    }

    if display_names.is_empty() {
        return Ok(HashMap::new());
    }

    let email_refs: Vec<&str> = emails_lower.iter().map(|s| s.as_str()).collect();
    let first_name_refs: Vec<Option<&str>> = first_names.iter().map(|s| s.as_deref()).collect();
    let last_name_refs: Vec<Option<&str>> = last_names.iter().map(|s| s.as_deref()).collect();

    let rows: Vec<(i32, String)> = sqlx::query_as(
        r#"
        INSERT INTO instructors (display_name, email, first_name, last_name)
        SELECT * FROM UNNEST($1::text[], $2::text[], $3::text[], $4::text[])
        ON CONFLICT (email)
        DO UPDATE SET
            display_name = EXCLUDED.display_name,
            first_name = EXCLUDED.first_name,
            last_name = EXCLUDED.last_name
        RETURNING id, email
        "#,
    )
    .bind(&display_names)
    .bind(&email_refs)
    .bind(&first_name_refs)
    .bind(&last_name_refs)
    .fetch_all(&mut *conn)
    .await
    .map_err(|e| anyhow::anyhow!("Failed to batch upsert instructors: {}", e))?;

    Ok(rows.into_iter().map(|(id, email)| (email, id)).collect())
}

/// Link courses to their instructors via the junction table.
/// Returns audit entries for any instructor changes detected.
async fn upsert_course_instructors(
    courses: &[Course],
    crn_term_to_id: &HashMap<(&str, &str), i32>,
    email_to_id: &HashMap<String, i32>,
    conn: &mut PgConnection,
) -> Result<Vec<AuditEntry>> {
    let mut cids = Vec::new();
    let mut instructor_ids: Vec<i32> = Vec::new();
    let mut banner_ids: Vec<&str> = Vec::new();
    let mut primaries = Vec::new();

    // Build new instructor names per course for auditing
    let mut new_names: HashMap<i32, Vec<String>> = HashMap::new();

    for course in courses {
        let key = (
            course.course_reference_number.as_str(),
            course.term.as_str(),
        );
        let Some(&course_id) = crn_term_to_id.get(&key) else {
            warn!(
                crn = %course.course_reference_number,
                term = %course.term,
                "No course_id found for CRN/term pair during instructor linking"
            );
            continue;
        };

        let names = new_names.entry(course_id).or_default();
        for faculty in &course.faculty {
            if let Some(email) = &faculty.email_address {
                let email_lower = email.to_lowercase();
                if let Some(&instructor_id) = email_to_id.get(&email_lower) {
                    cids.push(course_id);
                    instructor_ids.push(instructor_id);
                    banner_ids.push(faculty.banner_id.as_str());
                    primaries.push(faculty.primary_indicator);
                    names.push(decode_html_entities(&faculty.display_name));
                }
            }
        }
    }

    if cids.is_empty() {
        return Ok(Vec::new());
    }

    // Collect unique course IDs for the batch
    let unique_cids: Vec<i32> = cids
        .iter()
        .copied()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();

    // Fetch existing instructor names before deletion
    let old_rows: Vec<(i32, String)> = sqlx::query_as(
        "SELECT ci.course_id, i.display_name \
         FROM course_instructors ci \
         JOIN instructors i ON i.id = ci.instructor_id \
         WHERE ci.course_id = ANY($1) \
         ORDER BY ci.course_id, ci.is_primary DESC, i.display_name",
    )
    .bind(&unique_cids)
    .fetch_all(&mut *conn)
    .await?;

    let mut old_names: HashMap<i32, Vec<String>> = HashMap::new();
    for (course_id, name) in old_rows {
        old_names.entry(course_id).or_default().push(name);
    }

    // Delete existing links for these courses then re-insert
    sqlx::query("DELETE FROM course_instructors WHERE course_id = ANY($1)")
        .bind(&cids)
        .execute(&mut *conn)
        .await?;

    sqlx::query(
        r#"
        INSERT INTO course_instructors (course_id, instructor_id, banner_id, is_primary)
        SELECT * FROM UNNEST($1::int4[], $2::int4[], $3::text[], $4::bool[])
        ON CONFLICT (course_id, instructor_id)
        DO UPDATE SET
            banner_id = EXCLUDED.banner_id,
            is_primary = EXCLUDED.is_primary
        "#,
    )
    .bind(&cids)
    .bind(&instructor_ids)
    .bind(&banner_ids)
    .bind(&primaries)
    .execute(&mut *conn)
    .await
    .map_err(|e| anyhow::anyhow!("Failed to batch upsert course_instructors: {}", e))?;

    // Compare old vs new instructor names and emit audit entries
    let mut audits = Vec::new();
    for &course_id in &unique_cids {
        let old = old_names.get(&course_id).cloned().unwrap_or_default();
        let mut new = new_names.get(&course_id).cloned().unwrap_or_default();
        new.sort();

        if old != new {
            audits.push(AuditEntry {
                course_id,
                field_changed: "instructors",
                old_value: Some(serde_json::to_value(&old).unwrap_or_default()),
                new_value: serde_json::to_value(&new).unwrap_or_default(),
            });
        }
    }

    Ok(audits)
}
