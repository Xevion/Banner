//! Candidate generation: scores every eligible instructor against RMP's
//! profiles and writes candidates, auto-links and review statuses.

use super::score::{
    MIN_CANDIDATE_THRESHOLD, MatchScore, SCORE_EPSILON, ScoreBreakdown, compute_match_score, extract_review_subjects,
};
use crate::data::names::{KeyOrigin, NameParts, matching_keys, parse_banner_name, parse_rmp_name};
use crate::rmp::RmpCourseCode;
use anyhow::{Context, Result};
use sqlx::{PgConnection, PgPool};
use std::collections::{HashMap, HashSet};
use tracing::{debug, info};

/// Statistics returned from candidate generation.
#[derive(Debug)]
pub struct MatchingStats {
    /// Total instructors processed (excludes confirmed/rejected).
    pub total_processed: usize,
    /// Pending candidate rows deleted before regeneration.
    pub deleted_pending_candidates: usize,
    /// Auto-generated links deleted before regeneration.
    pub deleted_auto_links: usize,
    /// Candidates inserted in this run.
    pub candidates_created: usize,
    /// Instructors that were auto-linked (score >= AUTO_ACCEPT_THRESHOLD).
    pub auto_matched: usize,
    /// Instructors with candidates below auto-accept threshold (status set to 'pending').
    pub pending_review: usize,
    /// Instructors skipped because their display_name couldn't be parsed.
    pub skipped_unparseable: usize,
    /// Instructors skipped because no RMP name keys matched.
    pub skipped_no_candidates: usize,
}

/// Candidate row tuple: (instructor_id, rmp_legacy_id, score, breakdown, review_subjects, review_years).
type CandidateRow = (i32, i32, f32, sqlx::types::Json<ScoreBreakdown>, Vec<String>, Vec<i16>);

/// Raw row fetched from `rmp_professors` for the matching pipeline.
#[derive(sqlx::FromRow)]
struct RmpProfRow {
    legacy_id: i32,
    first_name: String,
    last_name: String,
    department: Option<String>,
    num_ratings: i32,
    course_codes: Option<sqlx::types::Json<Vec<RmpCourseCode>>>,
}

/// Lightweight row for building the in-memory RMP name index.
struct RmpProfForMatching {
    legacy_id: i32,
    department: Option<String>,
    num_ratings: i32,
    /// Subject prefixes from RMP review course codes, with review counts.
    review_subjects: Vec<(String, u32)>,
    /// The origin of the key that placed this professor in the index bucket.
    key_origin: KeyOrigin,
    /// Normalized (last, first), used to tell duplicate profiles from namesakes.
    norm_name: (String, String),
}

/// RMP professors bucketed by normalized (last, first) key; one profile appears
/// under every key variant its name produces.
type NameIndex = HashMap<(String, String), Vec<RmpProfForMatching>>;

/// Rows discarded while clearing the previous run's algorithmic output.
struct ClearedState {
    candidates: usize,
    links: usize,
}

/// Review evidence per RMP profile, drawn from `rmp_reviews`.
struct ReviewData {
    subjects: HashMap<i32, HashMap<String, u32>>,
    years: HashMap<i32, HashSet<i16>>,
}

/// RMP profiles whose name matches one instructor, deduplicated by legacy id.
struct MatchedProfs<'a> {
    profs: HashMap<i32, &'a RmpProfForMatching>,
    /// Best key origin per profile: Primary if any key pair was Primary on both
    /// sides, Nickname otherwise.
    best_origin: HashMap<i32, KeyOrigin>,
}

/// Candidate rows and claims accumulated across the scoring loop.
#[derive(Default)]
struct Collected {
    candidates: Vec<CandidateRow>,
    /// Instructors holding at least one above-threshold candidate.
    with_candidates: HashSet<i32>,
    /// Instructors claiming each RMP profile.
    claimants: HashMap<i32, HashSet<i32>>,
}

/// Auto-link decisions for one instructor.
struct AutoDecision {
    accepted: Vec<(i32, i32, f32)>,
    /// Sibling profiles of an identified person, held back until every claimant
    /// is known.
    inherited: Vec<(i32, i32, f32)>,
}

/// Generate match candidates for all unmatched and pending instructors.
///
/// Runs entirely inside a transaction to prevent data loss if the process
/// crashes mid-way. The steps are:
///
/// 1. Delete all non-rejected candidate rows (only explicit human rejections
///    are preserved across rescores).
/// 2. Delete all non-manual links from `instructor_rmp_links`.
/// 3. Reset all non-confirmed/rejected instructors back to `'unmatched'`.
/// 4. Load all instructors where status not in `{confirmed, rejected}`.
/// 5. Build a name index from all RMP professors.
/// 6. Score every instructor-RMP pair and collect candidates above
///    [`MIN_CANDIDATE_THRESHOLD`]. Skip rejected pairs.
/// 7. Batch-insert new candidate rows.
/// 8. Auto-link every candidate scoring >=
///    [`AUTO_ACCEPT_THRESHOLD`](super::score::AUTO_ACCEPT_THRESHOLD); set
///    instructor status to `'auto'`.
/// 9. Set remaining instructors that received at least one candidate (but no
///    auto-link) to `'pending'`.
pub async fn generate_candidates(db_pool: &PgPool) -> Result<MatchingStats> {
    let mut tx = db_pool.begin().await?;

    let cleared = clear_previous_run(&mut tx).await?;

    // 'confirmed' and 'rejected' are human decisions -- never touch them.
    let instructors: Vec<(i32, String)> = sqlx::query_as(
        "SELECT i.id, i.display_name FROM instructors i \
         JOIN instructor_rmp_match_status ms ON ms.instructor_id = i.id \
         WHERE ms.status NOT IN ('confirmed', 'rejected')",
    )
    .fetch_all(&mut *tx)
    .await
    .context("failed to fetch eligible instructors for matching")?;

    if instructors.is_empty() {
        tx.commit().await?;
        info!("No eligible instructors to generate candidates for");
        return Ok(MatchingStats {
            total_processed: 0,
            deleted_pending_candidates: cleared.candidates,
            deleted_auto_links: cleared.links,
            candidates_created: 0,
            auto_matched: 0,
            pending_review: 0,
            skipped_unparseable: 0,
            skipped_no_candidates: 0,
        });
    }

    let instructor_ids: Vec<i32> = instructors.iter().map(|(id, _)| *id).collect();
    let total_processed = instructors.len();

    let subject_map = load_instructor_subjects(&mut tx, &instructor_ids).await?;
    // Course load breaks ties when two instructor rows claim one RMP profile.
    let course_count_map = load_course_counts(&mut tx, &instructor_ids).await?;
    let reviews = load_review_data(&mut tx).await?;
    let name_index = build_name_index(&mut tx, &reviews).await?;
    // Rejected pairs are the only candidates preserved from the clearing step:
    // they are explicit human decisions to NOT link a pair.
    let rejected_pairs = load_rejected_pairs(&mut tx).await?;

    let empty_subjects: Vec<(String, u32)> = Vec::new();
    let mut collected = Collected::default();
    let mut auto_accept: Vec<(i32, i32, f32)> = Vec::new();
    let mut inherited: Vec<(i32, i32, f32)> = Vec::new();
    let mut skipped_unparseable = 0usize;
    let mut skipped_no_candidates = 0usize;

    for (instructor_id, display_name) in &instructors {
        let Some(instructor_parts) = parse_banner_name(display_name) else {
            skipped_unparseable += 1;
            debug!(instructor_id, display_name, "Unparseable display name, skipping");
            continue;
        };

        let subjects = subject_map.get(instructor_id).unwrap_or(&empty_subjects);
        let matched = collect_matched_profs(&name_index, &instructor_parts);

        if matched.profs.is_empty() {
            skipped_no_candidates += 1;
            continue;
        }

        let scored = score_instructor(
            *instructor_id,
            subjects,
            &matched,
            &rejected_pairs,
            &reviews,
            &mut collected,
        );

        let decision = decide_auto_links(*instructor_id, &scored, &matched);
        auto_accept.extend(decision.accepted);
        inherited.extend(decision.inherited);
    }

    let mut inherited_count = 0usize;
    for (instructor_id, legacy_id, score) in inherited {
        let uncontested = collected.claimants.get(&legacy_id).is_none_or(|ids| ids.len() == 1);
        if uncontested {
            auto_accept.push((instructor_id, legacy_id, score));
            inherited_count += 1;
        }
    }
    if inherited_count > 0 {
        debug!(inherited_count, "Linked sibling RMP profiles");
    }

    let candidates_created = collected.candidates.len();
    insert_candidates(&mut tx, collected.candidates).await?;

    // One RMP profile belongs to one instructor. Where several claim the same
    // profile, keep the strongest and leave the rest for review.
    let manual_held: HashSet<i32> =
        sqlx::query_as("SELECT rmp_legacy_id FROM instructor_rmp_links WHERE source = 'manual'")
            .fetch_all(&mut *tx)
            .await
            .context("failed to fetch manually linked rmp profiles")?
            .into_iter()
            .map(|(id,): (i32,)| id)
            .collect();

    let best_claim = resolve_best_claims(&auto_accept, &manual_held, &course_count_map);
    let linked_ids = insert_auto_links(&mut tx, &best_claim).await?;

    // One instructor can hold several profiles, so count people, not link rows.
    let auto_instructor_ids: HashSet<i32> = linked_ids.iter().copied().collect();
    let auto_matched = auto_instructor_ids.len();

    // Instructors with candidates but no auto-link get a distinct status so
    // they appear in the review queue.
    let pending_instructor_ids: Vec<i32> = collected
        .with_candidates
        .iter()
        .filter(|id| !auto_instructor_ids.contains(id))
        .copied()
        .collect();
    let pending_review = pending_instructor_ids.len();

    tx.commit().await?;
    crate::data::rmp::refresh_rmp_summary(db_pool).await?;

    let stats = MatchingStats {
        total_processed,
        deleted_pending_candidates: cleared.candidates,
        deleted_auto_links: cleared.links,
        candidates_created,
        auto_matched,
        pending_review,
        skipped_unparseable,
        skipped_no_candidates,
    };

    info!(
        total_processed = stats.total_processed,
        deleted_pending_candidates = stats.deleted_pending_candidates,
        deleted_auto_links = stats.deleted_auto_links,
        candidates_created = stats.candidates_created,
        auto_matched = stats.auto_matched,
        pending_review = stats.pending_review,
        skipped_unparseable = stats.skipped_unparseable,
        skipped_no_candidates = stats.skipped_no_candidates,
        "RMP candidate generation complete"
    );

    Ok(stats)
}

/// Drop every algorithm-generated candidate and link, and reset the statuses
/// they set. Only explicit human decisions survive.
async fn clear_previous_run(conn: &mut PgConnection) -> Result<ClearedState> {
    let candidates = sqlx::query("DELETE FROM rmp_match_candidates WHERE status != 'rejected'")
        .execute(&mut *conn)
        .await
        .context("failed to delete non-rejected match candidates")?
        .rows_affected() as usize;

    let links = sqlx::query("DELETE FROM instructor_rmp_links WHERE source != 'manual'")
        .execute(&mut *conn)
        .await
        .context("failed to delete non-manual rmp links")?
        .rows_affected() as usize;

    Ok(ClearedState { candidates, links })
}

/// Subjects each instructor teaches, with the number of courses in each.
async fn load_instructor_subjects(
    conn: &mut PgConnection,
    instructor_ids: &[i32],
) -> Result<HashMap<i32, Vec<(String, u32)>>> {
    let rows: Vec<(i32, String, i64)> = sqlx::query_as(
        r#"
        SELECT ci.instructor_id, c.subject, COUNT(*)
        FROM course_instructors ci
        JOIN courses c ON c.id = ci.course_id
        WHERE ci.instructor_id = ANY($1)
        GROUP BY ci.instructor_id, c.subject
        "#,
    )
    .bind(instructor_ids)
    .fetch_all(&mut *conn)
    .await
    .context("failed to fetch instructor subjects for matching")?;

    let mut subject_map: HashMap<i32, Vec<(String, u32)>> = HashMap::new();
    for (iid, subject, count) in rows {
        subject_map.entry(iid).or_default().push((subject, count.max(0) as u32));
    }

    Ok(subject_map)
}

/// Total courses taught per instructor.
async fn load_course_counts(conn: &mut PgConnection, instructor_ids: &[i32]) -> Result<HashMap<i32, i64>> {
    let counts = sqlx::query_as(
        "SELECT instructor_id, COUNT(*) FROM course_instructors \
         WHERE instructor_id = ANY($1) GROUP BY instructor_id",
    )
    .bind(instructor_ids)
    .fetch_all(&mut *conn)
    .await
    .context("failed to fetch instructor course counts")?
    .into_iter()
    .collect();

    Ok(counts)
}

/// Subject prefixes and posting years for every reviewed RMP profile.
async fn load_review_data(conn: &mut PgConnection) -> Result<ReviewData> {
    let rows: Vec<(i32, Option<String>, Option<i16>)> = sqlx::query_as(
        r#"
        SELECT rmp_legacy_id,
               class,
               EXTRACT(YEAR FROM posted_at)::SMALLINT as year
        FROM rmp_reviews
        WHERE posted_at IS NOT NULL OR class IS NOT NULL
        "#,
    )
    .fetch_all(&mut *conn)
    .await
    .context("failed to fetch rmp review data for matching")?;

    let mut subjects: HashMap<i32, HashMap<String, u32>> = HashMap::new();
    let mut years: HashMap<i32, HashSet<i16>> = HashMap::new();
    for (legacy_id, class, year) in &rows {
        if let Some(class_str) = class {
            let prefix: String = class_str.chars().take_while(|c| c.is_alphabetic()).collect();
            if !prefix.is_empty() {
                *subjects
                    .entry(*legacy_id)
                    .or_default()
                    .entry(prefix.to_uppercase())
                    .or_default() += 1;
            }
        }
        if let Some(y) = year {
            years.entry(*legacy_id).or_default().insert(*y);
        }
    }

    Ok(ReviewData { subjects, years })
}

/// Index every RMP professor under each key variant their name produces.
async fn build_name_index(conn: &mut PgConnection, reviews: &ReviewData) -> Result<NameIndex> {
    let prof_rows: Vec<RmpProfRow> = sqlx::query_as(
        "SELECT legacy_id, first_name, last_name, department, num_ratings, course_codes \
         FROM rmp_professors",
    )
    .fetch_all(&mut *conn)
    .await
    .context("failed to fetch rmp professors for name index")?;

    let mut name_index: NameIndex = HashMap::new();
    let mut rmp_parse_failures = 0usize;
    for row in &prof_rows {
        match parse_rmp_name(&row.first_name, &row.last_name) {
            Some(parts) => {
                // Prefer subjects from actual reviews; fall back to course_codes from RMP detail.
                let review_subjects = if let Some(subjs) = reviews.subjects.get(&row.legacy_id) {
                    subjs.iter().map(|(k, v)| (k.clone(), *v)).collect()
                } else {
                    extract_review_subjects(row.course_codes.as_deref().map(|c| c.as_slice()))
                };
                let keys = matching_keys(&parts);
                let norm_name = (
                    crate::data::names::normalize_for_matching(&parts.last),
                    crate::data::names::normalize_for_matching(&parts.first),
                );
                for key in keys {
                    name_index
                        .entry((key.last, key.first))
                        .or_default()
                        .push(RmpProfForMatching {
                            legacy_id: row.legacy_id,
                            department: row.department.clone(),
                            num_ratings: row.num_ratings,
                            review_subjects: review_subjects.clone(),
                            key_origin: key.origin,
                            norm_name: norm_name.clone(),
                        });
                }
            }
            None => {
                rmp_parse_failures += 1;
                debug!(
                    legacy_id = row.legacy_id,
                    first_name = row.first_name,
                    last_name = row.last_name,
                    "Unparseable RMP professor name, skipping"
                );
            }
        }
    }

    if rmp_parse_failures > 0 {
        debug!(count = rmp_parse_failures, "RMP professors with unparseable names");
    }

    Ok(name_index)
}

/// Pairs a human explicitly refused to link.
async fn load_rejected_pairs(conn: &mut PgConnection) -> Result<HashSet<(i32, i32)>> {
    let rows: Vec<(i32, i32)> = sqlx::query_as(
        "SELECT instructor_id, rmp_legacy_id \
         FROM rmp_match_candidates \
         WHERE status = 'rejected'",
    )
    .fetch_all(&mut *conn)
    .await
    .context("failed to fetch rejected match pairs")?;

    Ok(rows.into_iter().collect())
}

/// Gather the RMP profiles reachable from any of an instructor's name keys.
fn collect_matched_profs<'a>(name_index: &'a NameIndex, parts: &NameParts) -> MatchedProfs<'a> {
    let mut best_origin: HashMap<i32, KeyOrigin> = HashMap::new();
    let mut profs: HashMap<i32, &RmpProfForMatching> = HashMap::new();

    let instructor_keys = matching_keys(parts);
    for ikey in &instructor_keys {
        let lookup = (ikey.last.clone(), ikey.first.clone());
        if let Some(bucket) = name_index.get(&lookup) {
            for prof in bucket {
                let pair_origin = if ikey.origin == KeyOrigin::Primary && prof.key_origin == KeyOrigin::Primary {
                    KeyOrigin::Primary
                } else {
                    KeyOrigin::Nickname
                };

                let entry = best_origin.entry(prof.legacy_id).or_insert(pair_origin);
                if pair_origin == KeyOrigin::Primary {
                    *entry = KeyOrigin::Primary;
                }

                profs.entry(prof.legacy_id).or_insert(prof);
            }
        }
    }

    MatchedProfs { profs, best_origin }
}

/// Score one instructor against every profile carrying their name, recording
/// the candidates that clear the threshold.
fn score_instructor(
    instructor_id: i32,
    subjects: &[(String, u32)],
    matched: &MatchedProfs<'_>,
    rejected_pairs: &HashSet<(i32, i32)>,
    reviews: &ReviewData,
    collected: &mut Collected,
) -> Vec<(i32, MatchScore)> {
    let candidate_count = matched.profs.len();
    let mut scored: Vec<(i32, MatchScore)> = Vec::new();

    for (&legacy_id, &prof) in &matched.profs {
        if rejected_pairs.contains(&(instructor_id, legacy_id)) {
            continue;
        }

        let nickname_match = matched.best_origin.get(&legacy_id) == Some(&KeyOrigin::Nickname);

        let ms = compute_match_score(
            subjects,
            prof.department.as_deref(),
            candidate_count,
            prof.num_ratings,
            nickname_match,
            &prof.review_subjects,
        );

        if ms.score < MIN_CANDIDATE_THRESHOLD {
            continue;
        }

        let prof_review_subjects: Vec<String> = reviews
            .subjects
            .get(&prof.legacy_id)
            .map(|s| {
                let mut v: Vec<String> = s.keys().cloned().collect();
                v.sort();
                v
            })
            .unwrap_or_default();
        let prof_review_years: Vec<i16> = reviews
            .years
            .get(&prof.legacy_id)
            .map(|s| {
                let mut v: Vec<i16> = s.iter().copied().collect();
                v.sort();
                v
            })
            .unwrap_or_default();

        collected.candidates.push((
            instructor_id,
            prof.legacy_id,
            ms.score,
            sqlx::types::Json(ms.breakdown.clone()),
            prof_review_subjects,
            prof_review_years,
        ));
        collected.with_candidates.insert(instructor_id);

        collected
            .claimants
            .entry(prof.legacy_id)
            .or_default()
            .insert(instructor_id);
        scored.push((prof.legacy_id, ms));
    }

    scored
}

/// Decide which of an instructor's scored profiles may link without review.
///
/// RMP often holds several profiles for one person, and the summary view
/// aggregates them by rating weight, so linking all of them is correct.
/// Two distinct people sharing a name is the case a human must settle.
fn decide_auto_links(instructor_id: i32, scored: &[(i32, MatchScore)], matched: &MatchedProfs<'_>) -> AutoDecision {
    let candidate_count = matched.profs.len();
    let passing: Vec<&(i32, MatchScore)> = scored
        .iter()
        .filter(|(_, ms)| ms.auto_eligible(candidate_count))
        .collect();

    let distinct_people: HashSet<&(String, String)> = passing
        .iter()
        .filter_map(|(lid, _)| matched.profs.get(lid).map(|p| &p.norm_name))
        .collect();

    let mut decision = AutoDecision {
        accepted: Vec::new(),
        inherited: Vec::new(),
    };

    if distinct_people.len() == 1 {
        let identified = distinct_people.iter().next().copied().cloned();
        for (legacy_id, ms) in passing {
            decision.accepted.push((instructor_id, *legacy_id, ms.score));
        }
        // A confirmed profile establishes who this is, so RMP's other
        // profiles for the same name are the same person -- unless another
        // instructor is also claiming one.
        if let Some(name) = identified {
            for (legacy_id, ms) in scored {
                if ms.auto_eligible(candidate_count) {
                    continue;
                }
                // Identity carries across a shared name, but evidence that
                // actively disagrees still rules a profile out.
                if ms.breakdown.subject + SCORE_EPSILON < 0.5 {
                    continue;
                }
                let same_person = matched.profs.get(legacy_id).is_some_and(|p| p.norm_name == name);
                if same_person {
                    decision.inherited.push((instructor_id, *legacy_id, ms.score));
                }
            }
        }
    } else if distinct_people.len() > 1 {
        debug!(
            instructor_id,
            people = distinct_people.len(),
            "Distinct RMP people share this name, deferring to review"
        );
    }

    decision
}

/// Batch-insert the run's candidate rows and their review evidence.
async fn insert_candidates(conn: &mut PgConnection, new_candidates: Vec<CandidateRow>) -> Result<()> {
    if new_candidates.is_empty() {
        return Ok(());
    }

    let c_instructor_ids: Vec<i32> = new_candidates.iter().map(|(iid, _, _, _, _, _)| *iid).collect();
    let c_legacy_ids: Vec<i32> = new_candidates.iter().map(|(_, lid, _, _, _, _)| *lid).collect();
    let c_scores: Vec<f32> = new_candidates.iter().map(|(_, _, s, _, _, _)| *s).collect();
    let c_breakdowns: Vec<serde_json::Value> = new_candidates
        .iter()
        .map(|(_, _, _, b, _, _)| serde_json::to_value(&b.0))
        .collect::<Result<_, _>>()
        .context("failed to serialize match score breakdowns")?;
    // Encode review arrays as JSONB for bulk binding (SQLx can't bind Vec<Vec<T>>).
    let c_review_subjects_json: Vec<serde_json::Value> = new_candidates
        .iter()
        .map(|(_, _, _, _, rs, _)| serde_json::to_value(rs).unwrap_or_default())
        .collect();
    let c_review_years_json: Vec<serde_json::Value> = new_candidates
        .into_iter()
        .map(|(_, _, _, _, _, ry)| serde_json::to_value(&ry).unwrap_or_default())
        .collect();

    sqlx::query(
        r#"
        INSERT INTO rmp_match_candidates (instructor_id, rmp_legacy_id, score, score_breakdown)
        SELECT v.instructor_id, v.rmp_legacy_id, v.score, v.score_breakdown
        FROM UNNEST($1::int4[], $2::int4[], $3::real[], $4::jsonb[])
            AS v(instructor_id, rmp_legacy_id, score, score_breakdown)
        ON CONFLICT (instructor_id, rmp_legacy_id) DO UPDATE SET
            score = EXCLUDED.score,
            score_breakdown = EXCLUDED.score_breakdown
        "#,
    )
    .bind(&c_instructor_ids)
    .bind(&c_legacy_ids)
    .bind(&c_scores)
    .bind(&c_breakdowns)
    .execute(&mut *conn)
    .await
    .context("failed to batch insert match candidates")?;

    // Batch-update review subjects and years using JSONB->array conversion.
    sqlx::query(
        r#"
        UPDATE rmp_match_candidates mc
        SET review_subjects = sub.subjects,
            review_years = sub.years
        FROM (
            SELECT v.instructor_id, v.rmp_legacy_id,
                   COALESCE(ARRAY(SELECT jsonb_array_elements_text(v.rs)), '{}') as subjects,
                   COALESCE(ARRAY(SELECT (jsonb_array_elements_text(v.ry))::SMALLINT), '{}') as years
            FROM UNNEST($1::int4[], $2::int4[], $3::jsonb[], $4::jsonb[])
                AS v(instructor_id, rmp_legacy_id, rs, ry)
        ) sub
        WHERE mc.instructor_id = sub.instructor_id
          AND mc.rmp_legacy_id = sub.rmp_legacy_id
        "#,
    )
    .bind(&c_instructor_ids)
    .bind(&c_legacy_ids)
    .bind(&c_review_subjects_json)
    .bind(&c_review_years_json)
    .execute(&mut *conn)
    .await
    .context("failed to batch update candidate review data")?;

    Ok(())
}

/// Pick one instructor per contested RMP profile: highest score, then heaviest
/// course load, then lowest id. Manually linked profiles are left alone.
fn resolve_best_claims(
    auto_accept: &[(i32, i32, f32)],
    manual_held: &HashSet<i32>,
    course_count_map: &HashMap<i32, i64>,
) -> HashMap<i32, (i32, f32)> {
    let mut best_claim: HashMap<i32, (i32, f32)> = HashMap::new();
    let mut manual_skipped = 0usize;
    let mut contested = 0usize;

    for &(instructor_id, legacy_id, score) in auto_accept {
        if manual_held.contains(&legacy_id) {
            manual_skipped += 1;
            continue;
        }
        if best_claim.contains_key(&legacy_id) {
            contested += 1;
        }
        let courses = course_count_map.get(&instructor_id).copied().unwrap_or(0);
        let entry = best_claim.entry(legacy_id).or_insert((instructor_id, score));
        let (best_iid, best_score) = *entry;
        let best_courses = course_count_map.get(&best_iid).copied().unwrap_or(0);
        if (score, courses, -instructor_id) > (best_score, best_courses, -best_iid) {
            *entry = (instructor_id, score);
        }
    }

    if contested > 0 || manual_skipped > 0 {
        debug!(contested, manual_skipped, "Auto-link claims dropped before insert");
    }

    best_claim
}

/// Insert the winning claims as auto links, returning the instructors that
/// actually gained one. The insert may no-op on conflict.
async fn insert_auto_links(conn: &mut PgConnection, best_claim: &HashMap<i32, (i32, f32)>) -> Result<Vec<i32>> {
    if best_claim.is_empty() {
        return Ok(Vec::new());
    }

    let aa_legacy_ids: Vec<i32> = best_claim.keys().copied().collect();
    let aa_instructor_ids: Vec<i32> = aa_legacy_ids.iter().map(|lid| best_claim[lid].0).collect();

    let actually_linked: Vec<(i32, i32)> = sqlx::query_as(
        r#"
        INSERT INTO instructor_rmp_links (instructor_id, rmp_legacy_id, source)
        SELECT v.instructor_id, v.rmp_legacy_id, 'auto'
        FROM UNNEST($1::int4[], $2::int4[]) AS v(instructor_id, rmp_legacy_id)
        ON CONFLICT (rmp_legacy_id) DO NOTHING
        RETURNING instructor_id, rmp_legacy_id
        "#,
    )
    .bind(&aa_instructor_ids)
    .bind(&aa_legacy_ids)
    .fetch_all(&mut *conn)
    .await
    .context("failed to insert auto rmp links")?;

    // Only mark candidates accepted once the link actually landed.
    let linked_instructor_ids: Vec<i32> = actually_linked.iter().map(|(iid, _)| *iid).collect();
    let linked_legacy_ids: Vec<i32> = actually_linked.iter().map(|(_, lid)| *lid).collect();

    sqlx::query(
        r#"
        UPDATE rmp_match_candidates mc
        SET status = 'accepted', resolved_at = NOW()
        FROM UNNEST($1::int4[], $2::int4[]) AS v(instructor_id, rmp_legacy_id)
        WHERE mc.instructor_id = v.instructor_id
          AND mc.rmp_legacy_id = v.rmp_legacy_id
        "#,
    )
    .bind(&linked_instructor_ids)
    .bind(&linked_legacy_ids)
    .execute(&mut *conn)
    .await
    .context("failed to update auto-accepted candidates")?;

    Ok(linked_instructor_ids)
}
