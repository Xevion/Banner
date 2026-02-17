//! Confidence scoring and candidate generation for RMP instructor matching.

use crate::data::names::{matching_keys, parse_banner_name, parse_rmp_name};
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
}

/// Result of scoring a single instructor–RMP candidate pair.
#[derive(Debug, Clone)]
pub struct MatchScore {
    pub score: f32,
    pub breakdown: ScoreBreakdown,
}

/// Minimum composite score to store a candidate row.
const MIN_CANDIDATE_THRESHOLD: f32 = 0.40;

/// Score at or above which a candidate is auto-accepted.
const AUTO_ACCEPT_THRESHOLD: f32 = 0.85;

const WEIGHT_NAME: f32 = 0.50;
const WEIGHT_DEPARTMENT: f32 = 0.25;
const WEIGHT_UNIQUENESS: f32 = 0.15;
const WEIGHT_VOLUME: f32 = 0.10;

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
        // Core subjects (original mappings, corrected)
        ("cs", &["computer science"]),
        ("ece", &["early childhood education", "early childhood"]),
        ("ee", &["electrical engineering", "electrical"]),
        ("me", &["mechanical engineering", "mechanical"]),
        ("ce", &["civil engineering", "civil"]),
        ("bio", &["biology", "biological"]),
        ("chem", &["chemistry"]),
        ("phys", &["physics"]),
        ("math", &["mathematics"]),
        ("sta", &["statistics"]),
        ("eng", &["english"]),
        ("his", &["history"]),
        ("pol", &["political science"]),
        ("psy", &["psychology"]),
        ("soc", &["sociology"]),
        ("mus", &["music"]),
        ("art", &["art"]),
        ("phi", &["philosophy"]),
        ("eco", &["economics"]),
        ("acc", &["accounting"]),
        ("fin", &["finance"]),
        ("mgt", &["management"]),
        ("mkt", &["marketing"]),
        ("is", &["information systems"]),
        ("ms", &["management science"]),
        ("kin", &["kinesiology"]),
        ("com", &["communication"]),
        // Architecture & Design
        ("arc", &["architecture"]),
        ("ide", &["interior design", "design"]),
        // Anthropology & Ethnic Studies
        ("ant", &["anthropology"]),
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
        // Business
        ("ent", &["entrepreneurship"]),
        ("gba", &["general business", "business"]),
        ("blw", &["business law", "law"]),
        ("rfd", &["real estate"]),
        ("mot", &["management of technology", "management"]),
        // Engineering
        ("egr", &["engineering"]),
        ("bme", &["biomedical engineering", "engineering"]),
        ("cme", &["chemical engineering", "engineering"]),
        ("cpe", &["computer engineering", "engineering"]),
        ("ise", &["industrial", "systems engineering", "engineering"]),
        ("mate", &["materials engineering", "engineering"]),
        // Sciences
        ("che", &["chemistry"]),
        ("bch", &["biochemistry", "chemistry"]),
        ("geo", &["geology"]),
        ("phy", &["physics"]),
        ("ast", &["astronomy"]),
        ("es", &["environmental science"]),
        // Social Sciences
        ("crj", &["criminal justice"]),
        ("swk", &["social work"]),
        ("pad", &["public administration"]),
        ("grg", &["geography"]),
        ("ges", &["geography"]),
        // Humanities
        ("cla", &["classics"]),
        ("hum", &["humanities"]),
        ("wgss", &["women's studies"]),
        // Health
        ("hth", &["health"]),
        ("hcp", &["health science", "health"]),
        ("ntr", &["nutrition"]),
        // Military
        ("msc", &["military science"]),
        ("asc", &["aerospace"]),
        // Arts
        ("dan", &["dance"]),
        ("thr", &["theater"]),
        ("ahc", &["art history"]),
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

/// Compute match confidence score (0.0–1.0) for an instructor–RMP pair.
///
/// The name signal is always 1.0 since candidates are only generated for
/// exact normalized name matches. The effective score range is 0.50–1.0.
pub fn compute_match_score(
    instructor_subjects: &[String],
    rmp_department: Option<&str>,
    candidate_count: usize,
    rmp_num_ratings: i32,
) -> MatchScore {
    // --- Name (0.50) — always 1.0, candidates only exist for exact matches ---
    let name_score = 1.0;

    // --- Department (0.25) ---
    let dept_score = department_similarity(instructor_subjects, rmp_department);

    // --- Uniqueness (0.15) ---
    let uniqueness_score = match candidate_count {
        0 | 1 => 1.0,
        2 => 0.5,
        _ => 0.2,
    };

    // --- Volume (0.10) ---
    let volume_score = ((rmp_num_ratings as f32).ln_1p() / 5.0_f32.ln_1p()).clamp(0.0, 1.0);

    let composite = name_score * WEIGHT_NAME
        + dept_score * WEIGHT_DEPARTMENT
        + uniqueness_score * WEIGHT_UNIQUENESS
        + volume_score * WEIGHT_VOLUME;

    MatchScore {
        score: composite,
        breakdown: ScoreBreakdown {
            name: name_score,
            department: dept_score,
            uniqueness: uniqueness_score,
            volume: volume_score,
        },
    }
}

/// Statistics returned from candidate generation.
#[derive(Debug)]
pub struct MatchingStats {
    pub total_unmatched: usize,
    pub candidates_created: usize,
    pub candidates_rescored: usize,
    pub auto_matched: usize,
    pub skipped_unparseable: usize,
    pub skipped_no_candidates: usize,
}

/// Lightweight row for building the in-memory RMP name index.
struct RmpProfForMatching {
    legacy_id: i32,
    department: Option<String>,
    num_ratings: i32,
}

/// Generate match candidates for all unmatched instructors.
///
/// For each unmatched instructor:
/// 1. Parse `display_name` into [`NameParts`] and generate matching keys.
/// 2. Find RMP professors with matching normalized name keys.
/// 3. Score each candidate.
/// 4. Store candidates scoring above [`MIN_CANDIDATE_THRESHOLD`].
/// 5. Auto-accept if the top candidate scores ≥ [`AUTO_ACCEPT_THRESHOLD`]
///    and no existing rejected candidate exists for that pair.
///
/// Already-evaluated instructor–RMP pairs (any status) are skipped.
pub async fn generate_candidates(db_pool: &PgPool) -> Result<MatchingStats> {
    // 1. Load unmatched instructors
    let instructors: Vec<(i32, String)> = sqlx::query_as(
        "SELECT id, display_name FROM instructors WHERE rmp_match_status = 'unmatched'",
    )
    .fetch_all(db_pool)
    .await?;

    if instructors.is_empty() {
        info!("No unmatched instructors to generate candidates for");
        return Ok(MatchingStats {
            total_unmatched: 0,
            candidates_created: 0,
            candidates_rescored: 0,
            auto_matched: 0,
            skipped_unparseable: 0,
            skipped_no_candidates: 0,
        });
    }

    let instructor_ids: Vec<i32> = instructors.iter().map(|(id, _)| *id).collect();
    let total_unmatched = instructors.len();

    // 2. Load instructor subjects
    let subject_rows: Vec<(i32, String)> = sqlx::query_as(
        r#"
        SELECT DISTINCT ci.instructor_id, c.subject
        FROM course_instructors ci
        JOIN courses c ON c.id = ci.course_id
        WHERE ci.instructor_id = ANY($1)
        "#,
    )
    .bind(&instructor_ids)
    .fetch_all(db_pool)
    .await?;

    let mut subject_map: HashMap<i32, Vec<String>> = HashMap::new();
    for (iid, subject) in subject_rows {
        subject_map.entry(iid).or_default().push(subject);
    }

    // 3. Load all RMP professors and build multi-key name index
    let prof_rows: Vec<(i32, String, String, Option<String>, i32)> = sqlx::query_as(
        "SELECT legacy_id, first_name, last_name, department, num_ratings FROM rmp_professors",
    )
    .fetch_all(db_pool)
    .await?;

    // Build name index: (normalized_last, normalized_first) -> Vec<RmpProfForMatching>
    // Each professor may appear under multiple keys (nicknames, token variants).
    let mut name_index: HashMap<(String, String), Vec<RmpProfForMatching>> = HashMap::new();
    let mut rmp_parse_failures = 0usize;
    for (legacy_id, first_name, last_name, department, num_ratings) in &prof_rows {
        match parse_rmp_name(first_name, last_name) {
            Some(parts) => {
                let keys = matching_keys(&parts);
                for key in keys {
                    name_index.entry(key).or_default().push(RmpProfForMatching {
                        legacy_id: *legacy_id,
                        department: department.clone(),
                        num_ratings: *num_ratings,
                    });
                }
            }
            None => {
                rmp_parse_failures += 1;
                debug!(
                    legacy_id,
                    first_name, last_name, "Unparseable RMP professor name, skipping"
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

    // 4. Load existing candidate pairs — only skip resolved (accepted/rejected) pairs.
    //    Pending candidates are rescored so updated mappings take effect.
    let candidate_rows: Vec<(i32, i32, String)> =
        sqlx::query_as("SELECT instructor_id, rmp_legacy_id, status FROM rmp_match_candidates")
            .fetch_all(db_pool)
            .await?;

    let mut resolved_pairs: HashSet<(i32, i32)> = HashSet::new();
    let mut pending_pairs: HashSet<(i32, i32)> = HashSet::new();
    let mut rejected_pairs: HashSet<(i32, i32)> = HashSet::new();
    for (iid, lid, status) in candidate_rows {
        match status.as_str() {
            "accepted" | "rejected" => {
                resolved_pairs.insert((iid, lid));
                if status == "rejected" {
                    rejected_pairs.insert((iid, lid));
                }
            }
            _ => {
                pending_pairs.insert((iid, lid));
            }
        }
    }

    // 5. Score and collect candidates (new + rescored pending)
    let empty_subjects: Vec<String> = Vec::new();
    let mut new_candidates: Vec<(i32, i32, f32, serde_json::Value)> = Vec::new();
    let mut rescored_candidates: Vec<(i32, i32, f32, serde_json::Value)> = Vec::new();
    let mut auto_accept: Vec<(i32, i32)> = Vec::new(); // (instructor_id, legacy_id)
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

        // Generate all matching keys for this instructor and collect candidate
        // RMP professors across all key variants (deduplicated by legacy_id).
        let instructor_keys = matching_keys(&instructor_parts);
        let mut seen_profs: HashSet<i32> = HashSet::new();
        let mut matched_profs: Vec<&RmpProfForMatching> = Vec::new();

        for key in &instructor_keys {
            if let Some(profs) = name_index.get(key) {
                for prof in profs {
                    if seen_profs.insert(prof.legacy_id) {
                        matched_profs.push(prof);
                    }
                }
            }
        }

        if matched_profs.is_empty() {
            skipped_no_candidates += 1;
            continue;
        }

        let candidate_count = matched_profs.len();
        let mut best: Option<(f32, i32)> = None;

        for prof in &matched_profs {
            let pair = (*instructor_id, prof.legacy_id);
            if resolved_pairs.contains(&pair) {
                continue;
            }

            let ms = compute_match_score(
                subjects,
                prof.department.as_deref(),
                candidate_count,
                prof.num_ratings,
            );

            if ms.score < MIN_CANDIDATE_THRESHOLD {
                continue;
            }

            let breakdown_json =
                serde_json::to_value(&ms.breakdown).unwrap_or_else(|_| serde_json::json!({}));

            if pending_pairs.contains(&pair) {
                rescored_candidates.push((
                    *instructor_id,
                    prof.legacy_id,
                    ms.score,
                    breakdown_json,
                ));
            } else {
                new_candidates.push((*instructor_id, prof.legacy_id, ms.score, breakdown_json));
            }

            match best {
                Some((s, _)) if ms.score > s => best = Some((ms.score, prof.legacy_id)),
                None => best = Some((ms.score, prof.legacy_id)),
                _ => {}
            }
        }

        // Auto-accept the top candidate if it meets the threshold and is not
        // previously rejected.
        if let Some((score, legacy_id)) = best
            && score >= AUTO_ACCEPT_THRESHOLD
            && !rejected_pairs.contains(&(*instructor_id, legacy_id))
        {
            auto_accept.push((*instructor_id, legacy_id));
        }
    }

    // 6–7. Write candidates, rescore, and auto-accept within a single transaction
    let candidates_created = new_candidates.len();
    let candidates_rescored = rescored_candidates.len();
    let auto_matched = auto_accept.len();

    let mut tx = db_pool.begin().await?;

    // 6a. Batch-insert new candidates
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

    // 6b. Batch-update rescored pending candidates
    if !rescored_candidates.is_empty() {
        let r_instructor_ids: Vec<i32> = rescored_candidates
            .iter()
            .map(|(iid, _, _, _)| *iid)
            .collect();
        let r_legacy_ids: Vec<i32> = rescored_candidates
            .iter()
            .map(|(_, lid, _, _)| *lid)
            .collect();
        let r_scores: Vec<f32> = rescored_candidates.iter().map(|(_, _, s, _)| *s).collect();
        let r_breakdowns: Vec<serde_json::Value> = rescored_candidates
            .into_iter()
            .map(|(_, _, _, b)| b)
            .collect();

        sqlx::query(
            r#"
            UPDATE rmp_match_candidates mc
            SET score = v.score, score_breakdown = v.score_breakdown
            FROM UNNEST($1::int4[], $2::int4[], $3::real[], $4::jsonb[])
                AS v(instructor_id, rmp_legacy_id, score, score_breakdown)
            WHERE mc.instructor_id = v.instructor_id
              AND mc.rmp_legacy_id = v.rmp_legacy_id
            "#,
        )
        .bind(&r_instructor_ids)
        .bind(&r_legacy_ids)
        .bind(&r_scores)
        .bind(&r_breakdowns)
        .execute(&mut *tx)
        .await?;
    }

    // 7. Auto-accept top candidates
    if !auto_accept.is_empty() {
        let aa_instructor_ids: Vec<i32> = auto_accept.iter().map(|(iid, _)| *iid).collect();
        let aa_legacy_ids: Vec<i32> = auto_accept.iter().map(|(_, lid)| *lid).collect();

        // Mark the candidate row as accepted
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

        // Insert links into instructor_rmp_links
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

        // Update instructor match status
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

    tx.commit().await?;

    let stats = MatchingStats {
        total_unmatched,
        candidates_created,
        candidates_rescored,
        auto_matched,
        skipped_unparseable,
        skipped_no_candidates,
    };

    info!(
        total_unmatched = stats.total_unmatched,
        candidates_created = stats.candidates_created,
        candidates_rescored = stats.candidates_rescored,
        auto_matched = stats.auto_matched,
        skipped_unparseable = stats.skipped_unparseable,
        skipped_no_candidates = stats.skipped_no_candidates,
        "Candidate generation complete"
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
            1,  // unique candidate
            50, // decent ratings
        );
        // name 1.0*0.50 + dept 1.0*0.25 + unique 1.0*0.15 + volume ~0.97*0.10 ≈ 0.997
        assert!(ms.score >= 0.85, "Expected score >= 0.85, got {}", ms.score);
        assert_eq!(ms.breakdown.name, 1.0);
        assert_eq!(ms.breakdown.uniqueness, 1.0);
        assert_eq!(ms.breakdown.department, 1.0);
    }

    #[test]
    fn test_ambiguous_candidates_lower_score() {
        let unique = compute_match_score(&[], None, 1, 10);
        let ambiguous = compute_match_score(&[], None, 3, 10);
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
        let ms = compute_match_score(&["CS".to_string()], None, 1, 10);
        assert_eq!(ms.breakdown.department, 0.5);
    }

    #[test]
    fn test_department_match() {
        let ms = compute_match_score(&["CS".to_string()], Some("Computer Science"), 1, 10);
        assert_eq!(ms.breakdown.department, 1.0);
    }

    #[test]
    fn test_department_mismatch() {
        let ms = compute_match_score(&["CS".to_string()], Some("History"), 1, 10);
        assert_eq!(ms.breakdown.department, 0.2);
    }

    #[test]
    fn test_department_match_outscores_mismatch() {
        let matched = compute_match_score(&["CS".to_string()], Some("Computer Science"), 1, 10);
        let mismatched = compute_match_score(&["CS".to_string()], Some("History"), 1, 10);
        assert!(
            matched.score > mismatched.score,
            "Department match ({}) should outscore mismatch ({})",
            matched.score,
            mismatched.score
        );
    }

    #[test]
    fn test_volume_scaling() {
        let zero = compute_match_score(&[], None, 1, 0);
        let many = compute_match_score(&[], None, 1, 100);
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
}
