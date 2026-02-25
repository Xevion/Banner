//! Confidence scoring and candidate generation for RMP instructor matching.

use crate::data::names::{KeyOrigin, matching_keys, parse_banner_name, parse_rmp_name};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};
use tracing::{debug, info};

/// Breakdown of individual scoring signals.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScoreBreakdown {
    pub name: f32,
    pub department: f32,
    pub uniqueness: f32,
    pub volume: f32,
    /// Score from RMP review course codes overlapping with instructor subjects.
    /// Only available for professors with scraped review data.
    pub review_courses: f32,
}

/// Result of scoring a single instructor-RMP candidate pair.
#[derive(Debug, Clone)]
pub struct MatchScore {
    pub score: f32,
    pub breakdown: ScoreBreakdown,
}

/// Minimum composite score to store a candidate row.
const MIN_CANDIDATE_THRESHOLD: f32 = 0.40;

/// Score at or above which a candidate is auto-accepted.
const AUTO_ACCEPT_THRESHOLD: f32 = 0.85;

const WEIGHT_NAME: f32 = 0.45;
const WEIGHT_DEPARTMENT: f32 = 0.20;
const WEIGHT_UNIQUENESS: f32 = 0.10;
const WEIGHT_VOLUME: f32 = 0.10;
/// Weight for review course code overlap with instructor subjects.
/// Provides stronger evidence than department matching since it uses
/// actual course codes from student reviews.
const WEIGHT_REVIEW_COURSES: f32 = 0.15;

/// Check if an instructor's subjects overlap with an RMP department.
///
/// Returns `1.0` for a match, `0.2` for a mismatch, `0.5` when the RMP
/// department is unknown.
fn department_similarity(subjects: &[String], rmp_department: Option<&str>) -> f32 {
    let Some(dept) = rmp_department else {
        return 0.5;
    };
    let dept_lower = dept.to_lowercase();

    // Quick check: does any subject appear directly in the department string
    // or vice-versa?
    for subj in subjects {
        let subj_lower = subj.to_lowercase();
        if dept_lower.contains(&subj_lower) || subj_lower.contains(&dept_lower) {
            return 1.0;
        }

        // Handle common UTSA abbreviation mappings.
        if matches_known_abbreviation(&subj_lower, &dept_lower) {
            return 1.0;
        }
    }

    0.2
}

/// Expand common subject abbreviations used at UTSA and check for overlap.
fn matches_known_abbreviation(subject: &str, department: &str) -> bool {
    const MAPPINGS: &[(&str, &[&str])] = &[
        // Computer Science & Engineering
        ("cs", &["computer science"]),
        ("ece", &["early childhood education", "early childhood"]),
        ("ee", &["electrical engineering", "electrical"]),
        ("me", &["mechanical engineering", "mechanical"]),
        ("ce", &["civil engineering", "civil"]),
        ("egr", &["engineering"]),
        ("bme", &["biomedical engineering", "engineering"]),
        ("cme", &["chemical engineering", "engineering"]),
        ("cpe", &["computer engineering", "engineering"]),
        ("ise", &["industrial", "systems engineering", "engineering"]),
        ("mate", &["materials engineering", "engineering"]),
        // Sciences (include generic "science" for RMP catch-all departments)
        ("bio", &["biology", "biological", "science"]),
        ("chem", &["chemistry", "science"]),
        ("che", &["chemistry", "science"]),
        ("bch", &["biochemistry", "chemistry", "science"]),
        ("phys", &["physics", "science"]),
        ("phy", &["physics", "science"]),
        ("math", &["mathematics"]),
        ("sta", &["statistics"]),
        ("geo", &["geology", "science"]),
        ("ast", &["astronomy", "science"]),
        ("es", &["environmental science", "science"]),
        // English & Humanities
        ("eng", &["english", "literature"]),
        ("his", &["history"]),
        ("phi", &["philosophy"]),
        ("cla", &["classics"]),
        ("hum", &["humanities"]),
        ("wgss", &["women's studies"]),
        // Social Sciences (include generic "social science")
        ("pol", &["political science", "social science"]),
        ("psy", &["psychology", "social science"]),
        ("soc", &["sociology", "social science"]),
        ("ant", &["anthropology", "social science"]),
        ("eco", &["economics"]),
        ("crj", &["criminal justice"]),
        ("swk", &["social work"]),
        ("pad", &["public administration"]),
        ("grg", &["geography"]),
        ("ges", &["geography"]),
        // Business (include generic "business" and "managerial" for RMP catch-alls)
        (
            "acc",
            &["accounting", "business", "managerial science", "managerial"],
        ),
        (
            "fin",
            &["finance", "business", "managerial science", "managerial"],
        ),
        (
            "mgt",
            &["management", "business", "managerial science", "managerial"],
        ),
        (
            "mkt",
            &["marketing", "business", "managerial science", "managerial"],
        ),
        (
            "ms",
            &["management science", "managerial science", "managerial"],
        ),
        ("is", &["information systems", "information science"]),
        (
            "gba",
            &[
                "general business",
                "business",
                "managerial science",
                "managerial",
            ],
        ),
        (
            "ent",
            &[
                "entrepreneurship",
                "business",
                "managerial science",
                "managerial",
            ],
        ),
        ("blw", &["business law", "law", "business"]),
        ("rfd", &["real estate"]),
        (
            "mot",
            &[
                "management of technology",
                "management",
                "business",
                "managerial science",
                "managerial",
            ],
        ),
        // Arts & Fine Arts (include generic "fine arts")
        ("art", &["art", "fine arts"]),
        ("mus", &["music", "fine arts"]),
        ("dan", &["dance", "fine arts"]),
        ("thr", &["theater", "fine arts"]),
        ("ahc", &["art history", "fine arts"]),
        // Architecture & Design
        ("arc", &["architecture"]),
        ("ide", &["interior design", "design"]),
        // Anthropology & Ethnic Studies
        ("aas", &["african american studies", "ethnic studies"]),
        ("mas", &["mexican american studies", "ethnic studies"]),
        ("regs", &["ethnic studies", "gender"]),
        // Languages
        ("lng", &["linguistics", "applied linguistics"]),
        ("spn", &["spanish"]),
        ("frn", &["french"]),
        ("ger", &["german"]),
        ("chn", &["chinese"]),
        ("jpn", &["japanese"]),
        ("kor", &["korean"]),
        ("itl", &["italian"]),
        ("rus", &["russian"]),
        ("lat", &["latin"]),
        ("grk", &["greek"]),
        ("asl", &["american sign language", "sign language"]),
        (
            "fl",
            &["foreign languages", "languages", "modern languages"],
        ),
        // Education
        ("edu", &["education"]),
        ("ci", &["curriculum", "education"]),
        ("edl", &["educational leadership", "education"]),
        ("edp", &["educational psychology", "education"]),
        ("bbl", &["bilingual education"]),
        ("spe", &["special education", "education"]),
        // Health & Kinesiology
        ("hth", &["health"]),
        ("hcp", &["health science", "health"]),
        ("ntr", &["nutrition"]),
        ("kin", &["kinesiology", "physical ed", "physical education"]),
        // Communication & Film
        ("com", &["communication", "film"]),
        // Military
        ("msc", &["military science"]),
        ("asc", &["aerospace"]),
        // Other
        ("cou", &["counseling"]),
        ("hon", &["honors"]),
        ("csm", &["construction"]),
        ("wrc", &["writing"]),
        ("set", &["tourism management", "tourism"]),
    ];

    for &(abbr, expansions) in MAPPINGS {
        if subject == abbr {
            return expansions
                .iter()
                .any(|expansion| department.contains(expansion));
        }
    }
    false
}

/// Compute match confidence score (0.0-1.0) for an instructor-RMP pair.
///
/// When `nickname_match` is true, the name score is reduced to 0.7 to reflect
/// the lower confidence of matching via common nickname expansion (e.g.,
/// "Christopher" ↔ "Chris"). Primary name matches score 1.0.
///
/// `rmp_review_subjects` contains subject prefixes extracted from the RMP
/// professor's review course codes (e.g., `["WRC", "HIS"]`). When available,
/// overlap with `instructor_subjects` provides strong matching evidence.
pub fn compute_match_score(
    instructor_subjects: &[String],
    rmp_department: Option<&str>,
    candidate_count: usize,
    rmp_num_ratings: i32,
    nickname_match: bool,
    rmp_review_subjects: &[String],
) -> MatchScore {
    let name_score = if nickname_match { 0.7 } else { 1.0 };

    let dept_score = department_similarity(instructor_subjects, rmp_department);

    let uniqueness_score = match candidate_count {
        0 | 1 => 1.0,
        2 => 0.5,
        _ => 0.2,
    };

    let volume_score = ((rmp_num_ratings as f32).ln_1p() / 5.0_f32.ln_1p()).clamp(0.0, 1.0);

    // Review course overlap: if the RMP professor's reviews mention courses
    // in the same subject(s) the instructor teaches, that's strong evidence.
    let review_courses_score = if rmp_review_subjects.is_empty() {
        // No review data — neutral (don't penalize professors without reviews).
        0.5
    } else if instructor_subjects.is_empty() {
        // Instructor has no courses — neutral.
        0.5
    } else {
        let instructor_lower: HashSet<String> = instructor_subjects
            .iter()
            .map(|s| s.to_lowercase())
            .collect();
        let overlap = rmp_review_subjects
            .iter()
            .any(|rs| instructor_lower.contains(&rs.to_lowercase()));
        if overlap { 1.0 } else { 0.2 }
    };

    let composite = name_score * WEIGHT_NAME
        + dept_score * WEIGHT_DEPARTMENT
        + uniqueness_score * WEIGHT_UNIQUENESS
        + volume_score * WEIGHT_VOLUME
        + review_courses_score * WEIGHT_REVIEW_COURSES;

    MatchScore {
        score: composite,
        breakdown: ScoreBreakdown {
            name: name_score,
            department: dept_score,
            uniqueness: uniqueness_score,
            volume: volume_score,
            review_courses: review_courses_score,
        },
    }
}

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

/// Raw row fetched from `rmp_professors` for the matching pipeline.
#[derive(sqlx::FromRow)]
struct RmpProfRow {
    legacy_id: i32,
    first_name: String,
    last_name: String,
    department: Option<String>,
    num_ratings: i32,
    course_codes: Option<serde_json::Value>,
}

/// Lightweight row for building the in-memory RMP name index.
struct RmpProfForMatching {
    legacy_id: i32,
    department: Option<String>,
    num_ratings: i32,
    /// Subject prefixes extracted from RMP review course codes.
    review_subjects: Vec<String>,
    /// The origin of the key that placed this professor in the index bucket.
    key_origin: KeyOrigin,
}

/// Extract unique subject prefixes from RMP `course_codes` JSONB.
///
/// Course codes are formatted as `"SPN1014"`, `"WRC1013"`, etc.
/// Extracts the alphabetic prefix (e.g., `"SPN"`, `"WRC"`).
fn extract_review_subjects(course_codes: Option<&serde_json::Value>) -> Vec<String> {
    let Some(arr) = course_codes.and_then(|v| v.as_array()) else {
        return Vec::new();
    };

    let mut subjects: HashSet<String> = HashSet::new();
    for entry in arr {
        if let Some(name) = entry.get("courseName").and_then(|v| v.as_str()) {
            // Extract alphabetic prefix: "WRC1013" -> "WRC"
            let prefix: String = name.chars().take_while(|c| c.is_alphabetic()).collect();
            if !prefix.is_empty() {
                subjects.insert(prefix.to_uppercase());
            }
        }
    }

    subjects.into_iter().collect()
}

/// Generate match candidates for all unmatched and pending instructors.
///
/// Runs entirely inside a transaction to prevent data loss if the process
/// crashes mid-way. The steps are:
///
/// 1. Delete all `pending` candidate rows (algorithm-generated, not yet resolved).
/// 2. Delete all `auto` links from `instructor_rmp_links`.
/// 3. Reset `instructors.rmp_match_status` from `'auto'` back to `'unmatched'`.
/// 4. Load all instructors where status ∉ `{confirmed, rejected}` — this now
///    includes the freshly-reset instructors from step 3.
/// 5. Build a name index from all RMP professors.
/// 6. Score every instructor-RMP pair and collect candidates above
///    [`MIN_CANDIDATE_THRESHOLD`]. Skip pairs with existing `accepted`/`rejected`
///    decisions (manual decisions are preserved).
/// 7. Batch-insert new candidate rows.
/// 8. Auto-link every candidate scoring >= [`AUTO_ACCEPT_THRESHOLD`]; set
///    instructor status to `'auto'`.
/// 9. Set remaining instructors that received at least one candidate (but no
///    auto-link) to `'pending'`.
pub async fn generate_candidates(db_pool: &PgPool) -> Result<MatchingStats> {
    let mut tx = db_pool.begin().await?;

    // Step 1: Delete pending candidates (algorithm-generated, not manually resolved).
    let deleted_candidates =
        sqlx::query("DELETE FROM rmp_match_candidates WHERE status = 'pending'")
            .execute(&mut *tx)
            .await?
            .rows_affected() as usize;

    // Step 2: Delete auto-generated links.
    let deleted_links = sqlx::query("DELETE FROM instructor_rmp_links WHERE source = 'auto'")
        .execute(&mut *tx)
        .await?
        .rows_affected() as usize;

    // Step 3: Reset auto-matched instructors back to unmatched so they are
    // included in this run.
    sqlx::query(
        "UPDATE instructors SET rmp_match_status = 'unmatched' WHERE rmp_match_status = 'auto'",
    )
    .execute(&mut *tx)
    .await?;

    // Step 4: Load all instructors eligible for matching.
    // 'confirmed' and 'rejected' are manual decisions — never touch them.
    let instructors: Vec<(i32, String)> = sqlx::query_as(
        "SELECT id, display_name FROM instructors \
         WHERE rmp_match_status NOT IN ('confirmed', 'rejected')",
    )
    .fetch_all(&mut *tx)
    .await?;

    if instructors.is_empty() {
        tx.commit().await?;
        info!("No eligible instructors to generate candidates for");
        return Ok(MatchingStats {
            total_processed: 0,
            deleted_pending_candidates: deleted_candidates,
            deleted_auto_links: deleted_links,
            candidates_created: 0,
            auto_matched: 0,
            pending_review: 0,
            skipped_unparseable: 0,
            skipped_no_candidates: 0,
        });
    }

    let instructor_ids: Vec<i32> = instructors.iter().map(|(id, _)| *id).collect();
    let total_processed = instructors.len();

    // Step 5a: Load instructor subjects (for department scoring).
    let subject_rows: Vec<(i32, String)> = sqlx::query_as(
        r#"
        SELECT DISTINCT ci.instructor_id, c.subject
        FROM course_instructors ci
        JOIN courses c ON c.id = ci.course_id
        WHERE ci.instructor_id = ANY($1)
        "#,
    )
    .bind(&instructor_ids)
    .fetch_all(&mut *tx)
    .await?;

    let mut subject_map: HashMap<i32, Vec<String>> = HashMap::new();
    for (iid, subject) in subject_rows {
        subject_map.entry(iid).or_default().push(subject);
    }

    // Step 5b: Load all RMP professors and build multi-key name index.
    // Each professor may appear under multiple keys (nicknames, token variants).
    // The key_origin on each entry tracks whether that index slot came from a
    // Primary name or a Nickname expansion on the RMP side.
    let prof_rows: Vec<RmpProfRow> = sqlx::query_as(
        "SELECT legacy_id, first_name, last_name, department, num_ratings, course_codes \
         FROM rmp_professors",
    )
    .fetch_all(&mut *tx)
    .await?;

    let mut name_index: HashMap<(String, String), Vec<RmpProfForMatching>> = HashMap::new();
    let mut rmp_parse_failures = 0usize;
    for row in &prof_rows {
        match parse_rmp_name(&row.first_name, &row.last_name) {
            Some(parts) => {
                let review_subjects = extract_review_subjects(row.course_codes.as_ref());
                let keys = matching_keys(&parts);
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
        debug!(
            count = rmp_parse_failures,
            "RMP professors with unparseable names"
        );
    }

    // Step 5c: Load only resolved (accepted/rejected) pairs so we don't
    // overwrite manual decisions. Pending rows were deleted in step 1.
    let resolved_rows: Vec<(i32, i32, String)> = sqlx::query_as(
        "SELECT instructor_id, rmp_legacy_id, status \
         FROM rmp_match_candidates \
         WHERE status IN ('accepted', 'rejected')",
    )
    .fetch_all(&mut *tx)
    .await?;

    let mut resolved_pairs: HashSet<(i32, i32)> = HashSet::new();
    let mut rejected_pairs: HashSet<(i32, i32)> = HashSet::new();
    for (iid, lid, status) in resolved_rows {
        resolved_pairs.insert((iid, lid));
        if status == "rejected" {
            rejected_pairs.insert((iid, lid));
        }
    }

    // Step 6: Score and collect candidates.
    let empty_subjects: Vec<String> = Vec::new();
    let mut new_candidates: Vec<(i32, i32, f32, serde_json::Value)> = Vec::new();
    // Track which instructors get any above-threshold candidate (for 'pending' status).
    let mut instructors_with_candidates: HashSet<i32> = HashSet::new();
    let mut auto_accept: Vec<(i32, i32)> = Vec::new();
    let mut skipped_unparseable = 0usize;
    let mut skipped_no_candidates = 0usize;

    for (instructor_id, display_name) in &instructors {
        let Some(instructor_parts) = parse_banner_name(display_name) else {
            skipped_unparseable += 1;
            debug!(
                instructor_id,
                display_name, "Unparseable display name, skipping"
            );
            continue;
        };

        let subjects = subject_map.get(instructor_id).unwrap_or(&empty_subjects);

        // Collect candidate RMP professors across all key variants (deduplicated
        // by legacy_id). Track the best key origin per professor: Primary if any
        // key pair is fully Primary on both sides, Nickname otherwise.
        let instructor_keys = matching_keys(&instructor_parts);
        let mut prof_best_origin: HashMap<i32, KeyOrigin> = HashMap::new();
        let mut matched_profs_map: HashMap<i32, &RmpProfForMatching> = HashMap::new();

        for ikey in &instructor_keys {
            let lookup = (ikey.last.clone(), ikey.first.clone());
            if let Some(profs) = name_index.get(&lookup) {
                for prof in profs {
                    let pair_origin = if ikey.origin == KeyOrigin::Primary
                        && prof.key_origin == KeyOrigin::Primary
                    {
                        KeyOrigin::Primary
                    } else {
                        KeyOrigin::Nickname
                    };

                    let entry = prof_best_origin
                        .entry(prof.legacy_id)
                        .or_insert(pair_origin);
                    if pair_origin == KeyOrigin::Primary {
                        *entry = KeyOrigin::Primary;
                    }

                    matched_profs_map.entry(prof.legacy_id).or_insert(prof);
                }
            }
        }

        if matched_profs_map.is_empty() {
            skipped_no_candidates += 1;
            continue;
        }

        let candidate_count = matched_profs_map.len();

        for (&legacy_id, &prof) in &matched_profs_map {
            let pair = (*instructor_id, legacy_id);
            if resolved_pairs.contains(&pair) {
                continue;
            }

            let nickname_match = prof_best_origin.get(&legacy_id) == Some(&KeyOrigin::Nickname);

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

            let breakdown_json =
                serde_json::to_value(&ms.breakdown).unwrap_or_else(|_| serde_json::json!({}));

            new_candidates.push((*instructor_id, prof.legacy_id, ms.score, breakdown_json));
            instructors_with_candidates.insert(*instructor_id);

            if ms.score >= AUTO_ACCEPT_THRESHOLD
                && !rejected_pairs.contains(&(*instructor_id, prof.legacy_id))
            {
                auto_accept.push((*instructor_id, prof.legacy_id));
            }
        }
    }

    // Step 7: Batch-insert all new candidates.
    let candidates_created = new_candidates.len();

    if !new_candidates.is_empty() {
        let c_instructor_ids: Vec<i32> = new_candidates.iter().map(|(iid, _, _, _)| *iid).collect();
        let c_legacy_ids: Vec<i32> = new_candidates.iter().map(|(_, lid, _, _)| *lid).collect();
        let c_scores: Vec<f32> = new_candidates.iter().map(|(_, _, s, _)| *s).collect();
        let c_breakdowns: Vec<serde_json::Value> =
            new_candidates.into_iter().map(|(_, _, _, b)| b).collect();

        sqlx::query(
            r#"
            INSERT INTO rmp_match_candidates (instructor_id, rmp_legacy_id, score, score_breakdown)
            SELECT v.instructor_id, v.rmp_legacy_id, v.score, v.score_breakdown
            FROM UNNEST($1::int4[], $2::int4[], $3::real[], $4::jsonb[])
                AS v(instructor_id, rmp_legacy_id, score, score_breakdown)
            ON CONFLICT (instructor_id, rmp_legacy_id) DO NOTHING
            "#,
        )
        .bind(&c_instructor_ids)
        .bind(&c_legacy_ids)
        .bind(&c_scores)
        .bind(&c_breakdowns)
        .execute(&mut *tx)
        .await?;
    }

    // Step 8: Auto-accept high-confidence candidates.
    let auto_matched = auto_accept.len();

    if !auto_accept.is_empty() {
        let aa_instructor_ids: Vec<i32> = auto_accept.iter().map(|(iid, _)| *iid).collect();
        let aa_legacy_ids: Vec<i32> = auto_accept.iter().map(|(_, lid)| *lid).collect();

        sqlx::query(
            r#"
            UPDATE rmp_match_candidates mc
            SET status = 'accepted', resolved_at = NOW()
            FROM UNNEST($1::int4[], $2::int4[]) AS v(instructor_id, rmp_legacy_id)
            WHERE mc.instructor_id = v.instructor_id
              AND mc.rmp_legacy_id = v.rmp_legacy_id
            "#,
        )
        .bind(&aa_instructor_ids)
        .bind(&aa_legacy_ids)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO instructor_rmp_links (instructor_id, rmp_legacy_id, source)
            SELECT v.instructor_id, v.rmp_legacy_id, 'auto'
            FROM UNNEST($1::int4[], $2::int4[]) AS v(instructor_id, rmp_legacy_id)
            ON CONFLICT (rmp_legacy_id) DO NOTHING
            "#,
        )
        .bind(&aa_instructor_ids)
        .bind(&aa_legacy_ids)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            UPDATE instructors i
            SET rmp_match_status = 'auto'
            FROM UNNEST($1::int4[]) AS v(instructor_id)
            WHERE i.id = v.instructor_id
            "#,
        )
        .bind(&aa_instructor_ids)
        .execute(&mut *tx)
        .await?;
    }

    // Step 9: Mark instructors that have candidates but no auto-link as 'pending'
    // so they appear in the review queue with a distinct status.
    let auto_instructor_ids: HashSet<i32> = auto_accept.iter().map(|(iid, _)| *iid).collect();
    let pending_instructor_ids: Vec<i32> = instructors_with_candidates
        .iter()
        .filter(|id| !auto_instructor_ids.contains(id))
        .copied()
        .collect();
    let pending_review = pending_instructor_ids.len();

    if !pending_instructor_ids.is_empty() {
        sqlx::query(
            r#"
            UPDATE instructors i
            SET rmp_match_status = 'pending'
            FROM UNNEST($1::int4[]) AS v(instructor_id)
            WHERE i.id = v.instructor_id
              AND i.rmp_match_status = 'unmatched'
            "#,
        )
        .bind(&pending_instructor_ids)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    let stats = MatchingStats {
        total_processed,
        deleted_pending_candidates: deleted_candidates,
        deleted_auto_links: deleted_links,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ideal_candidate_high_score() {
        let ms = compute_match_score(
            &["CS".to_string()],
            Some("Computer Science"),
            1,     // unique candidate
            50,    // decent ratings
            false, // primary name match
            &[],   // no review data
        );
        assert!(ms.score >= 0.85, "Expected score >= 0.85, got {}", ms.score);
        assert_eq!(ms.breakdown.name, 1.0);
        assert_eq!(ms.breakdown.uniqueness, 1.0);
        assert_eq!(ms.breakdown.department, 1.0);
    }

    #[test]
    fn test_ambiguous_candidates_lower_score() {
        let unique = compute_match_score(&[], None, 1, 10, false, &[]);
        let ambiguous = compute_match_score(&[], None, 3, 10, false, &[]);
        assert!(
            unique.score > ambiguous.score,
            "Unique ({}) should outscore ambiguous ({})",
            unique.score,
            ambiguous.score
        );
        assert_eq!(unique.breakdown.uniqueness, 1.0);
        assert_eq!(ambiguous.breakdown.uniqueness, 0.2);
    }

    #[test]
    fn test_no_department_neutral() {
        let ms = compute_match_score(&["CS".to_string()], None, 1, 10, false, &[]);
        assert_eq!(ms.breakdown.department, 0.5);
    }

    #[test]
    fn test_department_match() {
        let ms = compute_match_score(
            &["CS".to_string()],
            Some("Computer Science"),
            1,
            10,
            false,
            &[],
        );
        assert_eq!(ms.breakdown.department, 1.0);
    }

    #[test]
    fn test_department_mismatch() {
        let ms = compute_match_score(&["CS".to_string()], Some("History"), 1, 10, false, &[]);
        assert_eq!(ms.breakdown.department, 0.2);
    }

    #[test]
    fn test_department_match_outscores_mismatch() {
        let matched = compute_match_score(
            &["CS".to_string()],
            Some("Computer Science"),
            1,
            10,
            false,
            &[],
        );
        let mismatched =
            compute_match_score(&["CS".to_string()], Some("History"), 1, 10, false, &[]);
        assert!(
            matched.score > mismatched.score,
            "Department match ({}) should outscore mismatch ({})",
            matched.score,
            mismatched.score
        );
    }

    #[test]
    fn test_volume_scaling() {
        let zero = compute_match_score(&[], None, 1, 0, false, &[]);
        let many = compute_match_score(&[], None, 1, 100, false, &[]);
        assert!(
            many.breakdown.volume > zero.breakdown.volume,
            "100 ratings ({}) should outscore 0 ratings ({})",
            many.breakdown.volume,
            zero.breakdown.volume
        );
        assert_eq!(zero.breakdown.volume, 0.0);
        assert!(
            many.breakdown.volume > 0.9,
            "100 ratings should be near max"
        );
    }

    #[test]
    fn test_nickname_match_lowers_name_score() {
        let primary = compute_match_score(
            &["CS".to_string()],
            Some("Computer Science"),
            1,
            50,
            false, // primary
            &[],
        );
        let nickname = compute_match_score(
            &["CS".to_string()],
            Some("Computer Science"),
            1,
            50,
            true, // nickname
            &[],
        );
        assert_eq!(primary.breakdown.name, 1.0);
        assert_eq!(nickname.breakdown.name, 0.7);
        assert!(
            primary.score > nickname.score,
            "Primary ({}) should outscore nickname ({})",
            primary.score,
            nickname.score
        );
        assert!(
            nickname.score >= MIN_CANDIDATE_THRESHOLD,
            "Nickname match should still be above minimum threshold"
        );
    }

    #[test]
    fn test_review_courses_overlap_boosts_score() {
        let no_reviews = compute_match_score(
            &["CS".to_string()],
            Some("Computer Science"),
            1,
            50,
            false,
            &[], // no review data
        );
        let with_matching_reviews = compute_match_score(
            &["CS".to_string()],
            Some("Computer Science"),
            1,
            50,
            false,
            &["CS".to_string()], // matching review subject
        );
        let with_mismatched_reviews = compute_match_score(
            &["CS".to_string()],
            Some("Computer Science"),
            1,
            50,
            false,
            &["HIS".to_string()], // non-matching review subject
        );

        assert_eq!(no_reviews.breakdown.review_courses, 0.5);
        assert_eq!(with_matching_reviews.breakdown.review_courses, 1.0);
        assert_eq!(with_mismatched_reviews.breakdown.review_courses, 0.2);

        assert!(
            with_matching_reviews.score > no_reviews.score,
            "Matching reviews ({}) should outscore no reviews ({})",
            with_matching_reviews.score,
            no_reviews.score
        );
        assert!(
            no_reviews.score > with_mismatched_reviews.score,
            "No reviews ({}) should outscore mismatched reviews ({})",
            no_reviews.score,
            with_mismatched_reviews.score
        );
    }

    #[test]
    fn test_extract_review_subjects() {
        let json = serde_json::json!([
            {"courseName": "WRC1013", "courseCount": 230},
            {"courseName": "WRC2013", "courseCount": 50},
            {"courseName": "HIS1053", "courseCount": 10}
        ]);
        let subjects = extract_review_subjects(Some(&json));
        assert!(subjects.contains(&"WRC".to_string()));
        assert!(subjects.contains(&"HIS".to_string()));
        assert_eq!(subjects.len(), 2); // deduplicated WRC
    }

    #[test]
    fn test_extract_review_subjects_empty() {
        assert!(extract_review_subjects(None).is_empty());
        assert!(extract_review_subjects(Some(&serde_json::json!([]))).is_empty());
    }
}
