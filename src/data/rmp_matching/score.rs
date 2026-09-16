//! Confidence scoring for a single instructor-RMP candidate pair.
//!
//! Every signal is quantized, weighted, and merged into one composite score;
//! `MatchScore::auto_eligible` decides whether that score may link without review.

use super::abbreviations::ABBREVIATIONS;
use crate::rmp::RmpCourseCode;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use ts_rs::TS;

/// Breakdown of individual scoring signals.
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ScoreBreakdown {
    pub name: f32,
    /// Department string similarity (from RMP profile).
    pub department: f32,
    /// Review course code overlap (from RMP reviews).
    pub review_courses: f32,
    /// Merged subject evidence: `max(department, review_courses)`.
    /// This is the value actually used in the composite score - the stronger
    /// of the two subject-alignment signals.
    pub subject: f32,
    pub uniqueness: f32,
    pub volume: f32,
}

/// Result of scoring a single instructor-RMP candidate pair.
#[derive(Debug, Clone)]
pub struct MatchScore {
    pub score: f32,
    pub breakdown: ScoreBreakdown,
}

impl MatchScore {
    /// Whether this pair may be linked without human review. Requires confirmed
    /// subject evidence, so a name-only score cannot auto-link unverified.
    pub fn auto_eligible(&self, candidate_count: usize) -> bool {
        // Never link when both subject signals disagree. Either one alone is
        // noisy: RMP departments do not always map onto a Banner subject.
        if self.breakdown.subject + SCORE_EPSILON < 0.5 {
            return false;
        }
        // Confirmed subject evidence: clear the usual bar.
        if self.breakdown.subject + SCORE_EPSILON >= 1.0 {
            return self.score + SCORE_EPSILON >= AUTO_ACCEPT_THRESHOLD;
        }
        // Unconfirmed but uncontradicted. Safe only for a sole candidate on a
        // full-strength name: there is no rival profile to weigh it against,
        // and a low rating count is no reason to doubt the identity.
        candidate_count == 1 && self.breakdown.name + SCORE_EPSILON >= NAME_PRIMARY
    }
}

/// Minimum composite score to store a candidate row.
pub(super) const MIN_CANDIDATE_THRESHOLD: f32 = 0.40;

/// Score at or above which a candidate is auto-accepted.
///
/// Set just clear of 0.85: the quantized signals make that value a common exact
/// total, so a threshold sitting on it decides real cases by f32 rounding.
pub(super) const AUTO_ACCEPT_THRESHOLD: f32 = 0.86;

/// Slack for threshold comparisons; quantized signals land exactly on the threshold.
pub(super) const SCORE_EPSILON: f32 = 1e-4;

/// Matching reviews needed before review codes confirm the subject.
const REVIEWS_TO_CONFIRM: u32 = 3;

/// Matching reviews needed to overturn a contradicting department string.
const REVIEWS_TO_OVERRIDE_DEPARTMENT: u32 = 5;

/// Share of a profile's reviews that must sit in the instructor's subjects.
const MIN_REVIEW_SHARE: f32 = 0.25;

/// Share of an instructor's load a university-wide code must cover to identify them.
const GENERIC_SUBJECT_MIN_SHARE: f32 = 0.25;

const NAME_PRIMARY: f32 = 1.0;
const NAME_NICKNAME: f32 = 0.7;

/// Nickname match that both subject signals independently confirm.
const NAME_NICKNAME_CORROBORATED: f32 = 0.95;

const WEIGHT_NAME: f32 = 0.50;
/// Weight for merged subject evidence (max of department and review_courses).
const WEIGHT_SUBJECT: f32 = 0.30;
const WEIGHT_UNIQUENESS: f32 = 0.15;
const WEIGHT_VOLUME: f32 = 0.05;

/// Whether a department string stands in for a missing value.
fn is_placeholder_department(department: &str) -> bool {
    let trimmed = department.trim();
    trimmed.is_empty() || trimmed.eq_ignore_ascii_case("not specified")
}

/// Check if an instructor's subjects overlap with an RMP department.
///
/// Returns `1.0` for a match, `0.2` for a mismatch, `0.5` when the RMP
/// department is unknown.
fn department_similarity(subjects: &[(String, u32)], rmp_department: Option<&str>) -> f32 {
    let Some(dept) = rmp_department.filter(|d| !is_placeholder_department(d)) else {
        return 0.5;
    };
    // No courses on record: unverifiable, not a mismatch.
    if subjects.is_empty() {
        return 0.5;
    }
    let dept_lower = dept.to_lowercase();

    // Quick check: does any subject appear directly in the department string
    // or vice-versa?
    let mut recognized = false;
    for (subj, _) in subjects {
        let subj_lower = subj.to_lowercase();
        if dept_lower.contains(&subj_lower) || subj_lower.contains(&dept_lower) {
            return 1.0;
        }

        // Handle common UTSA abbreviation mappings.
        if matches_known_abbreviation(&subj_lower, &dept_lower) {
            return 1.0;
        }
        recognized |= is_known_abbreviation(&subj_lower);
    }

    // Only call it a mismatch when the subject is one we can actually place.
    // An unmapped code means we have no opinion, not a contradiction.
    if recognized { 0.2 } else { 0.5 }
}

/// University-wide course codes taught by faculty from every college, so an
/// overlap on one proves nothing about subject alignment.
fn is_generic_subject(subject: &str) -> bool {
    matches!(
        subject.to_ascii_uppercase().as_str(),
        "AIS" | "HON" | "UCS" | "CSS" | "IDS"
    )
}

/// Whether the abbreviation table can place this subject at all.
fn is_known_abbreviation(subject: &str) -> bool {
    ABBREVIATIONS.iter().any(|(abbr, _)| *abbr == subject)
}

/// Expand common subject abbreviations used at UTSA and check for overlap.
fn matches_known_abbreviation(subject: &str, department: &str) -> bool {
    for &(abbr, expansions) in ABBREVIATIONS {
        if subject == abbr {
            return expansions.iter().any(|expansion| department.contains(expansion));
        }
    }
    false
}

/// Compute match confidence score (0.0-1.0) for an instructor-RMP pair.
///
/// When `nickname_match` is true, the name score is reduced to 0.7 to reflect
/// the lower confidence of matching via common nickname expansion (e.g.,
/// "Christopher" vs "Chris"). Primary name matches score 1.0.
///
/// `rmp_review_subjects` contains subject prefixes extracted from the RMP
/// professor's review course codes (e.g., `["WRC", "HIS"]`). When available,
/// overlap with `instructor_subjects` provides strong matching evidence.
///
/// Department and review-course signals both measure subject alignment through
/// different lenses. They are merged via `max()` into a single subject evidence
/// score so that whichever signal is stronger dominates - review data overrides
/// a noisy department string, and department helps when reviews are absent.
pub fn compute_match_score(
    instructor_subjects: &[(String, u32)],
    rmp_department: Option<&str>,
    candidate_count: usize,
    rmp_num_ratings: i32,
    nickname_match: bool,
    rmp_review_subjects: &[(String, u32)],
) -> MatchScore {
    let dept_score = department_similarity(instructor_subjects, rmp_department);

    let uniqueness_score = match candidate_count {
        0 | 1 => 1.0,
        2 => 0.5,
        _ => 0.2,
    };

    let volume_score = ((rmp_num_ratings as f32).ln_1p() / 5.0_f32.ln_1p()).clamp(0.0, 1.0);

    // Review course overlap: if the RMP professor's reviews mention courses
    // in the same subject(s) the instructor teaches, that's strong evidence.
    let total_reviews: u32 = rmp_review_subjects.iter().map(|(_, n)| *n).sum();

    let review_courses_score = if total_reviews == 0 || instructor_subjects.is_empty() {
        // Nothing to compare against on one side or the other.
        0.5
    } else {
        let taught: HashMap<String, u32> = instructor_subjects
            .iter()
            .map(|(s, n)| (s.to_lowercase(), *n))
            .collect();
        let taught_total: u32 = taught.values().sum::<u32>().max(1);

        // A university-wide code only identifies someone who actually teaches
        // it in bulk; one stray seminar says nothing about who they are.
        let counts = |subject: &str| -> bool {
            let Some(load) = taught.get(&subject.to_lowercase()) else {
                return false;
            };
            !is_generic_subject(subject) || (*load as f32 / taught_total as f32) >= GENERIC_SUBJECT_MIN_SHARE
        };

        let matching: u32 = rmp_review_subjects
            .iter()
            .filter(|(subject, _)| counts(subject))
            .map(|(_, n)| *n)
            .sum();

        // A department that actively disagrees takes more evidence to overturn
        // than a missing one.
        let required = if dept_score <= 0.2 {
            REVIEWS_TO_OVERRIDE_DEPARTMENT
        } else {
            REVIEWS_TO_CONFIRM
        };
        let share = matching as f32 / total_reviews as f32;

        if matching >= required && share >= MIN_REVIEW_SHARE {
            1.0
        } else if matching > 0 {
            // Present but too thin to carry the subject on its own.
            0.5
        } else {
            0.2
        }
    };

    // Merge the two subject-alignment signals: the stronger one wins.
    let subject_score = dept_score.max(review_courses_score);

    // Both signals agreeing is independent corroboration, not a single lucky hit.
    let dual_confirmed = dept_score >= 1.0 && review_courses_score >= 1.0;
    let name_score = match (nickname_match, dual_confirmed) {
        (false, _) => NAME_PRIMARY,
        (true, true) => NAME_NICKNAME_CORROBORATED,
        (true, false) => NAME_NICKNAME,
    };

    let composite = name_score * WEIGHT_NAME
        + subject_score * WEIGHT_SUBJECT
        + uniqueness_score * WEIGHT_UNIQUENESS
        + volume_score * WEIGHT_VOLUME;

    MatchScore {
        score: composite,
        breakdown: ScoreBreakdown {
            name: name_score,
            department: dept_score,
            review_courses: review_courses_score,
            subject: subject_score,
            uniqueness: uniqueness_score,
            volume: volume_score,
        },
    }
}

/// Extract unique subject prefixes from RMP course codes.
///
/// Course codes are formatted as `"SPN1014"`, `"WRC1013"`, etc.
/// Extracts the alphabetic prefix (e.g., `"SPN"`, `"WRC"`).
pub(super) fn extract_review_subjects(course_codes: Option<&[RmpCourseCode]>) -> Vec<(String, u32)> {
    let Some(codes) = course_codes else {
        return Vec::new();
    };

    let mut subjects: HashMap<String, u32> = HashMap::new();
    for entry in codes {
        // Extract alphabetic prefix: "WRC1013" -> "WRC"
        let prefix: String = entry.course_name.chars().take_while(|c| c.is_alphabetic()).collect();
        if !prefix.is_empty() {
            *subjects.entry(prefix.to_uppercase()).or_default() += entry.course_count.get();
        }
    }

    subjects.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ideal_candidate_high_score() {
        let ms = compute_match_score(
            &[("CS".to_string(), 20)],
            Some("Computer Science"),
            1,     // unique candidate
            50,    // decent ratings
            false, // primary name match
            &[],   // no review data
        );
        // name=1.0*0.50 + subject=max(1.0,0.5)*0.30 + unique=1.0*0.15 + vol~1.0*0.05 ~= 1.0
        assert!(ms.score >= 0.85, "Expected score >= 0.85, got {}", ms.score);
        assert_eq!(ms.breakdown.name, 1.0);
        assert_eq!(ms.breakdown.uniqueness, 1.0);
        assert_eq!(ms.breakdown.department, 1.0);
        assert_eq!(ms.breakdown.subject, 1.0);
    }

    #[test]
    fn test_ideal_zero_volume_still_auto_accepts() {
        // Primary name + dept match + unique + zero volume + no reviews
        // should still reach auto-accept threshold.
        let ms = compute_match_score(
            &[("CS".to_string(), 20)],
            Some("Computer Science"),
            1,
            0, // zero ratings
            false,
            &[],
        );
        // name=1.0*0.50 + subject=max(1.0,0.5)*0.30 + unique=1.0*0.15 + vol=0.0*0.05 = 0.95
        assert!(
            ms.score >= AUTO_ACCEPT_THRESHOLD,
            "Zero-volume ideal match ({}) should still auto-accept (threshold {})",
            ms.score,
            AUTO_ACCEPT_THRESHOLD
        );
    }

    #[test]
    fn test_review_courses_override_dept_mismatch() {
        // Dept string doesn't match, but review courses confirm subject alignment.
        // Subject evidence should use review_courses (1.0) not department (0.2).
        let ms = compute_match_score(
            &[("EDU".to_string(), 20)],
            Some("First Year Experience"), // doesn't match EDU
            1,
            50,
            false,
            &[("EDU".to_string(), 8)], // reviews confirm subject
        );
        assert_eq!(ms.breakdown.department, 0.2);
        assert_eq!(ms.breakdown.review_courses, 1.0);
        assert_eq!(ms.breakdown.subject, 1.0); // max(0.2, 1.0)
        assert!(
            ms.score >= AUTO_ACCEPT_THRESHOLD,
            "Review-confirmed match ({}) should auto-accept despite dept mismatch",
            ms.score
        );
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
        let ms = compute_match_score(&[("CS".to_string(), 20)], None, 1, 10, false, &[]);
        assert_eq!(ms.breakdown.department, 0.5);
    }

    #[test]
    fn test_department_match() {
        let ms = compute_match_score(&[("CS".to_string(), 20)], Some("Computer Science"), 1, 10, false, &[]);
        assert_eq!(ms.breakdown.department, 1.0);
    }

    #[test]
    fn test_department_mismatch() {
        let ms = compute_match_score(&[("CS".to_string(), 20)], Some("History"), 1, 10, false, &[]);
        assert_eq!(ms.breakdown.department, 0.2);
    }

    #[test]
    fn test_department_match_outscores_mismatch() {
        let matched = compute_match_score(&[("CS".to_string(), 20)], Some("Computer Science"), 1, 10, false, &[]);
        let mismatched = compute_match_score(&[("CS".to_string(), 20)], Some("History"), 1, 10, false, &[]);
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
        assert!(many.breakdown.volume > 0.9, "100 ratings should be near max");
    }

    #[test]
    fn test_nickname_match_lowers_name_score() {
        let primary = compute_match_score(
            &[("CS".to_string(), 20)],
            Some("Computer Science"),
            1,
            50,
            false, // primary
            &[],
        );
        let nickname = compute_match_score(
            &[("CS".to_string(), 20)],
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
        // Use a dept MISMATCH so review_courses actually differentiates.
        // With dept match, subject=max(1.0, x) is always 1.0 regardless of reviews.
        let no_reviews = compute_match_score(
            &[("EDU".to_string(), 20)],
            Some("First Year Experience"), // dept mismatch -> 0.2
            1,
            50,
            false,
            &[], // no review data -> 0.5; subject = max(0.2, 0.5) = 0.5
        );
        let with_matching_reviews = compute_match_score(
            &[("EDU".to_string(), 20)],
            Some("First Year Experience"),
            1,
            50,
            false,
            &[("EDU".to_string(), 8)], // matching -> 1.0; subject = max(0.2, 1.0) = 1.0
        );
        let with_mismatched_reviews = compute_match_score(
            &[("EDU".to_string(), 20)],
            Some("First Year Experience"),
            1,
            50,
            false,
            &[("HIS".to_string(), 8)], // mismatched -> 0.2; subject = max(0.2, 0.2) = 0.2
        );

        assert_eq!(no_reviews.breakdown.review_courses, 0.5);
        assert_eq!(with_matching_reviews.breakdown.review_courses, 1.0);
        assert_eq!(with_mismatched_reviews.breakdown.review_courses, 0.2);

        // Subject evidence should reflect the merge
        assert_eq!(no_reviews.breakdown.subject, 0.5);
        assert_eq!(with_matching_reviews.breakdown.subject, 1.0);
        assert_eq!(with_mismatched_reviews.breakdown.subject, 0.2);

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
    fn test_nickname_dual_confirmed_auto_accepts() {
        // Nickname expansion with department and review courses both agreeing.
        let ms = compute_match_score(
            &[("EDU".to_string(), 20)],
            Some("Education"),
            1,
            13,
            true,
            &[("EDU".to_string(), 8)],
        );
        assert_eq!(ms.breakdown.department, 1.0);
        assert_eq!(ms.breakdown.review_courses, 1.0);
        assert_eq!(ms.breakdown.name, NAME_NICKNAME_CORROBORATED);
        assert!(
            ms.auto_eligible(1),
            "Dual-confirmed nickname ({}) should auto-accept",
            ms.score
        );
    }

    #[test]
    fn test_nickname_single_signal_stays_pending() {
        // Department agrees but no review data corroborates it.
        let ms = compute_match_score(&[("EDU".to_string(), 20)], Some("Education"), 1, 13, true, &[]);
        assert_eq!(ms.breakdown.name, NAME_NICKNAME);
        assert!(!ms.auto_eligible(1), "Single-signal nickname should not auto");
    }

    #[test]
    fn test_no_courses_is_neutral_not_mismatch() {
        let ms = compute_match_score(&[], Some("Spanish"), 1, 56, false, &[]);
        assert_eq!(ms.breakdown.department, 0.5);
        assert_eq!(ms.breakdown.subject, 0.5);
    }

    #[test]
    fn test_contradicted_subject_never_auto_accepts() {
        // The department places this person somewhere else entirely.
        let ms = compute_match_score(
            &[("CS".to_string(), 40)],
            Some("History"),
            1,
            56,
            false,
            &[("HIS".to_string(), 30)],
        );
        assert_eq!(ms.breakdown.subject, 0.2);
        assert!(!ms.auto_eligible(1), "A contradicted subject must not link");
    }

    #[test]
    fn test_sole_uncontradicted_candidate_auto_accepts() {
        // Nothing confirms the subject, but nothing disputes it and no other
        // profile carries the name, so there is no competing reading.
        let ms = compute_match_score(&[], Some("Spanish"), 1, 56, false, &[]);
        assert_eq!(ms.breakdown.subject, 0.5);
        assert!(ms.auto_eligible(1));
        assert!(
            !ms.auto_eligible(2),
            "With a rival profile the subject must actually be confirmed"
        );
    }

    #[test]
    fn test_ambiguity_lowers_score_but_subject_gates_auto() {
        // Competing profiles depress uniqueness; the subject gate is what
        // actually decides, since duplicate profiles of one person should link.
        let confirmed = compute_match_score(
            &[("CS".to_string(), 20)],
            Some("Computer Science"),
            3,
            50,
            false,
            &[("CS".to_string(), 8)],
        );
        assert!(confirmed.auto_eligible(3));

        let unconfirmed = compute_match_score(&[("CS".to_string(), 20)], Some("Political Science"), 3, 3, false, &[]);
        assert!(
            !unconfirmed.auto_eligible(3),
            "A namesake in another department must not auto-link"
        );
    }

    #[test]
    fn test_generic_subject_is_not_evidence_for_an_occasional_seminar() {
        // One university-wide seminar against a large unrelated load, with a
        // department that cannot vouch for the pair either.
        let ms = compute_match_score(
            &[("AIS".to_string(), 2), ("ENG".to_string(), 90)],
            Some("Business"),
            1,
            50,
            false,
            &[("AIS".to_string(), 8)],
        );
        assert_eq!(ms.breakdown.review_courses, 0.2);
        assert!(!ms.auto_eligible(1));
    }

    #[test]
    fn test_generic_subject_identifies_someone_who_teaches_it() {
        // The same code is real evidence when it is most of their teaching.
        let ms = compute_match_score(
            &[("AIS".to_string(), 60), ("DS".to_string(), 30)],
            Some("Health Science"),
            1,
            20,
            false,
            &[("AIS".to_string(), 19), ("DS".to_string(), 1)],
        );
        assert_eq!(ms.breakdown.review_courses, 1.0);
        assert!(ms.auto_eligible(1));
    }

    #[test]
    fn test_extract_review_subjects() {
        let codes: Vec<RmpCourseCode> = serde_json::from_value(serde_json::json!([
            {"courseName": "WRC1013", "courseCount": 230},
            {"courseName": "WRC2013", "courseCount": 50},
            {"courseName": "HIS1053", "courseCount": 10}
        ]))
        .unwrap();
        let subjects: HashMap<String, u32> = extract_review_subjects(Some(&codes)).into_iter().collect();
        assert_eq!(subjects.get("WRC"), Some(&280));
        assert_eq!(subjects.get("HIS"), Some(&10));
        assert_eq!(subjects.len(), 2);
    }

    #[test]
    fn test_extract_review_subjects_empty() {
        assert!(extract_review_subjects(None).is_empty());
        assert!(extract_review_subjects(Some(&[])).is_empty());
    }
}
