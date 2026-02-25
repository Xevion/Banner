//! Bayesian instructor scoring combining RMP and BlueBook data.
//!
//! Pipeline: Raw BB -> Regression calibration -> Bayesian posterior -> CI lower bound as sort key.
//!
//! Parameters are locked from prototype validation (scripts/scoring-prototype.ts).

use anyhow::{Context, Result};
use sqlx::PgPool;
use tracing::{info, instrument};

use super::course_types::{InstructorRating, RatingSource};

/// Prior distribution: mean of all RMP professors.
const PRIOR_MEAN: f64 = 3.775;
/// Prior variance of true instructor quality.
const PRIOR_VAR: f64 = 1.045;

/// Regression calibration: `rmp_equivalent = REG_ALPHA + REG_BETA * bb_score`.
const REG_ALPHA: f64 = -2.58;
const REG_BETA: f64 = 1.45;

/// Per-observation noise variance for each source.
const RMP_NOISE_VAR: f64 = 1.5;
const BB_NOISE_VAR: f64 = 1.036;

/// Effective-n multipliers: `n_eff = sqrt(n) * factor`.
/// RMP observations are ~1.38x more informative per raw count.
const RMP_N_FACTOR: f64 = 2.0;
const BB_N_FACTOR: f64 = 1.0;

/// z-score for 80% credible interval.
const CI_Z: f64 = 1.2816;

/// CI lower bound of a zero-evidence posterior: `PRIOR_MEAN - CI_Z * sqrt(PRIOR_VAR)`.
///
/// With no observations the posterior equals the prior, so this is the worst
/// possible rank score an instructor can receive.  Used as the COALESCE sentinel
/// for the `AsPrior` unrated policy so that unrated instructors sort as if they
/// had exactly this score (≈ 3.775 − 1.2816 × 1.02225 ≈ 2.465).
pub const PRIOR_RANK_SENTINEL: f32 = 2.465;

/// How to handle instructors with no computed score when sorting by rating.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum UnratedPolicy {
    /// COALESCE with the prior rank sentinel so unrated instructors sort among rated ones.
    AsPrior,
    /// Push unrated instructors to the end (Infinity for ASC, -Infinity for DESC).
    Last,
    /// Filter out unrated instructors entirely (adds a WHERE clause).
    Exclude,
}

/// Build SQL fragments for rating-based sorting.
///
/// Returns `(ORDER BY clause, optional WHERE filter)`.
pub fn rating_sort_sql(ascending: bool, policy: UnratedPolicy) -> (String, Option<String>) {
    let dir = if ascending { "ASC" } else { "DESC" };

    match policy {
        UnratedPolicy::AsPrior => (
            format!("COALESCE(sc.sort_score, {PRIOR_RANK_SENTINEL}) {dir}, i.display_name ASC"),
            None,
        ),
        UnratedPolicy::Last => {
            let sentinel = if ascending {
                "'Infinity'"
            } else {
                "'-Infinity'"
            };
            (
                format!("COALESCE(sc.sort_score, {sentinel}) {dir}, i.display_name ASC"),
                None,
            )
        }
        UnratedPolicy::Exclude => (
            format!("sc.sort_score {dir}, i.display_name ASC"),
            Some("sc.sort_score IS NOT NULL".to_string()),
        ),
    }
}

/// Raw inputs for scoring a single instructor.
#[derive(Debug)]
struct RawInstructorData {
    instructor_id: i32,
    rmp_rating: Option<f32>,
    rmp_num_ratings: i32,
    bb_avg_instructor_rating: Option<f32>,
    bb_total_responses: i32,
}

/// Computed score for a single instructor, ready for DB insertion.
#[derive(Debug)]
struct ComputedScore {
    instructor_id: i32,
    score: f32,
    rank_score: f32,
    ci_lower: f32,
    ci_upper: f32,
    confidence: f32,
    source: RatingSource,
    rmp_rating: Option<f32>,
    rmp_count: i32,
    bb_rating: Option<f32>,
    calibrated_bb: Option<f32>,
    bb_count: i32,
}

/// Compute the Bayesian posterior score for a single instructor.
///
/// Each instructor has a "true quality" μ. We observe noisy measurements from
/// RMP and regression-calibrated BlueBook. The posterior combines the prior with
/// all available evidence, weighted by effective sample size.
fn compute_score(data: &RawInstructorData) -> ComputedScore {
    let has_rmp = data.rmp_rating.is_some() && data.rmp_num_ratings > 0;
    let has_bb = data.bb_avg_instructor_rating.is_some() && data.bb_total_responses > 0;

    // Calibrate BB to RMP scale, clamped to [1.0, 5.0]
    let calibrated_bb = data.bb_avg_instructor_rating.map(|bb| {
        let raw = REG_ALPHA + REG_BETA * bb as f64;
        raw.clamp(1.0, 5.0) as f32
    });

    // Effective sample sizes with diminishing returns
    let rmp_n_eff = if data.rmp_num_ratings > 0 {
        (data.rmp_num_ratings as f64).sqrt() * RMP_N_FACTOR
    } else {
        0.0
    };
    let bb_n_eff = if data.bb_total_responses > 0 {
        (data.bb_total_responses as f64).sqrt() * BB_N_FACTOR
    } else {
        0.0
    };

    // Bayesian posterior: conjugate normal-normal update
    let mut precision = 1.0 / PRIOR_VAR;
    let mut weighted_sum = PRIOR_MEAN / PRIOR_VAR;

    if let Some(rmp) = data.rmp_rating
        && rmp_n_eff > 0.0
    {
        let rmp_precision = rmp_n_eff / RMP_NOISE_VAR;
        precision += rmp_precision;
        weighted_sum += rmp as f64 * rmp_precision;
    }

    if let Some(cal_bb) = calibrated_bb
        && bb_n_eff > 0.0
    {
        let bb_precision = bb_n_eff / BB_NOISE_VAR;
        precision += bb_precision;
        weighted_sum += cal_bb as f64 * bb_precision;
    }

    let posterior_mean_raw = weighted_sum / precision;
    let posterior_stddev = (1.0 / precision).sqrt();

    let score = posterior_mean_raw.clamp(1.0, 5.0) as f32;
    let ci_lower = (posterior_mean_raw - CI_Z * posterior_stddev).max(1.0) as f32;
    let ci_upper = (posterior_mean_raw + CI_Z * posterior_stddev).min(5.0) as f32;
    let confidence = (1.0 - posterior_stddev / PRIOR_VAR.sqrt()).clamp(0.0, 1.0) as f32;

    let source = match (has_rmp, has_bb) {
        (true, true) => RatingSource::Both,
        (true, false) => RatingSource::Rmp,
        (false, true) => RatingSource::BlueBook,
        (false, false) => RatingSource::BlueBook, // unreachable in practice
    };

    ComputedScore {
        instructor_id: data.instructor_id,
        score,
        rank_score: ci_lower,
        ci_lower,
        ci_upper,
        confidence,
        source,
        rmp_rating: data.rmp_rating,
        rmp_count: data.rmp_num_ratings,
        bb_rating: data.bb_avg_instructor_rating,
        calibrated_bb,
        bb_count: data.bb_total_responses,
    }
}

/// Recompute all instructor scores from raw RMP and BlueBook data.
///
/// Truncates the `instructor_scores` table and bulk-inserts fresh scores.
/// Should be called on startup and after scrape completions.
#[instrument(skip(pool))]
pub async fn recompute_all_scores(pool: &PgPool) -> Result<usize> {
    let start = std::time::Instant::now();

    // Load all instructors that have at least one rating source
    let rows = sqlx::query!(
        r#"
        WITH bluebook_agg AS (
            SELECT
                ibl.instructor_id,
                AVG(be.instructor_rating)::REAL AS bb_avg,
                SUM(be.instructor_response_count)::INTEGER AS bb_responses
            FROM instructor_bluebook_links ibl
            JOIN bluebook_evaluations be ON be.instructor_name = ibl.instructor_name
            WHERE ibl.status IN ('approved', 'auto')
              AND ibl.instructor_id IS NOT NULL
              AND be.instructor_rating IS NOT NULL
            GROUP BY ibl.instructor_id
        ),
        rmp_data AS (
            SELECT DISTINCT ON (irl.instructor_id)
                irl.instructor_id,
                rp.avg_rating::REAL AS rmp_rating,
                rp.num_ratings AS rmp_num_ratings
            FROM instructor_rmp_links irl
            JOIN rmp_professors rp ON irl.rmp_legacy_id = rp.legacy_id
            WHERE rp.avg_rating IS NOT NULL
              AND rp.num_ratings > 0
            ORDER BY irl.instructor_id, rp.num_ratings DESC
        )
        SELECT
            i.id AS instructor_id,
            rd.rmp_rating AS "rmp_rating: f32",
            COALESCE(rd.rmp_num_ratings, 0) AS "rmp_num_ratings!: i32",
            bb.bb_avg AS "bb_avg: f32",
            COALESCE(bb.bb_responses, 0) AS "bb_responses!: i32"
        FROM instructors i
        LEFT JOIN rmp_data rd ON i.id = rd.instructor_id
        LEFT JOIN bluebook_agg bb ON i.id = bb.instructor_id
        WHERE rd.rmp_rating IS NOT NULL OR bb.bb_avg IS NOT NULL
        "#
    )
    .fetch_all(pool)
    .await
    .context("Failed to load instructor rating data")?;

    let scores: Vec<ComputedScore> = rows
        .iter()
        .map(|r| {
            compute_score(&RawInstructorData {
                instructor_id: r.instructor_id,
                rmp_rating: r.rmp_rating,
                rmp_num_ratings: r.rmp_num_ratings,
                bb_avg_instructor_rating: r.bb_avg,
                bb_total_responses: r.bb_responses,
            })
        })
        .collect();

    let count = scores.len();

    // Bulk insert using UNNEST
    let instructor_ids: Vec<i32> = scores.iter().map(|s| s.instructor_id).collect();
    let display_scores: Vec<f32> = scores.iter().map(|s| s.score).collect();
    let sort_scores: Vec<f32> = scores.iter().map(|s| s.rank_score).collect();
    let ci_lowers: Vec<f32> = scores.iter().map(|s| s.ci_lower).collect();
    let ci_uppers: Vec<f32> = scores.iter().map(|s| s.ci_upper).collect();
    let confidences: Vec<f32> = scores.iter().map(|s| s.confidence).collect();
    let sources: Vec<String> = scores
        .iter()
        .map(|s| s.source.as_str().to_owned())
        .collect();
    let rmp_ratings: Vec<Option<f32>> = scores.iter().map(|s| s.rmp_rating).collect();
    let rmp_counts: Vec<i32> = scores.iter().map(|s| s.rmp_count).collect();
    let bb_ratings: Vec<Option<f32>> = scores.iter().map(|s| s.bb_rating).collect();
    let calibrated_bbs: Vec<Option<f32>> = scores.iter().map(|s| s.calibrated_bb).collect();
    let bb_counts: Vec<i32> = scores.iter().map(|s| s.bb_count).collect();

    // Truncate + insert in a single transaction
    let mut tx = pool.begin().await.context("Failed to begin transaction")?;

    sqlx::query!("TRUNCATE instructor_scores")
        .execute(&mut *tx)
        .await
        .context("Failed to truncate instructor_scores")?;

    sqlx::query!(
        r#"
        INSERT INTO instructor_scores (
            instructor_id, display_score, sort_score, ci_lower, ci_upper,
            confidence, source, rmp_rating, rmp_count, bb_rating, calibrated_bb, bb_count
        )
        SELECT * FROM UNNEST(
            $1::int[], $2::real[], $3::real[], $4::real[], $5::real[],
            $6::real[], $7::text[], $8::real[], $9::int[], $10::real[], $11::real[], $12::int[]
        )
        "#,
        &instructor_ids,
        &display_scores,
        &sort_scores,
        &ci_lowers,
        &ci_uppers,
        &confidences,
        &sources,
        &rmp_ratings as &[Option<f32>],
        &rmp_counts,
        &bb_ratings as &[Option<f32>],
        &calibrated_bbs as &[Option<f32>],
        &bb_counts,
    )
    .execute(&mut *tx)
    .await
    .context("Failed to insert instructor scores")?;

    tx.commit().await.context("Failed to commit transaction")?;

    let elapsed = start.elapsed();
    info!(
        count,
        elapsed_ms = elapsed.as_millis() as u64,
        "Recomputed instructor scores"
    );

    Ok(count)
}

/// Pre-joined score row fields from `instructor_scores`.
pub struct ScoreRow {
    pub display_score: f32,
    pub sort_score: f32,
    pub ci_lower: f32,
    pub ci_upper: f32,
    pub confidence: f32,
    pub source: String,
    pub rmp_count: i32,
    pub bb_count: i32,
}

/// Load an instructor rating from a pre-joined instructor_scores row.
pub fn build_rating_from_score_row(row: &ScoreRow) -> InstructorRating {
    InstructorRating {
        score: row.display_score,
        rank_score: row.sort_score,
        ci_lower: row.ci_lower,
        ci_upper: row.ci_upper,
        confidence: row.confidence,
        source: RatingSource::parse(&row.source).unwrap_or(RatingSource::BlueBook),
        total_responses: row.rmp_count + row.bb_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_score_both_sources() {
        let data = RawInstructorData {
            instructor_id: 1,
            rmp_rating: Some(4.5),
            rmp_num_ratings: 25,
            bb_avg_instructor_rating: Some(4.8),
            bb_total_responses: 100,
        };
        let score = compute_score(&data);
        assert_eq!(score.source, RatingSource::Both);
        assert!(score.score > 1.0 && score.score < 5.0);
        assert!(score.ci_lower <= score.score);
        assert!(score.ci_upper >= score.score);
        assert!(score.confidence > 0.0);
        assert!(score.calibrated_bb.is_some());
    }

    #[test]
    fn test_compute_score_rmp_only() {
        let data = RawInstructorData {
            instructor_id: 2,
            rmp_rating: Some(3.0),
            rmp_num_ratings: 10,
            bb_avg_instructor_rating: None,
            bb_total_responses: 0,
        };
        let score = compute_score(&data);
        assert_eq!(score.source, RatingSource::Rmp);
        assert!(score.calibrated_bb.is_none());
    }

    #[test]
    fn test_compute_score_bb_only() {
        let data = RawInstructorData {
            instructor_id: 3,
            rmp_rating: None,
            rmp_num_ratings: 0,
            bb_avg_instructor_rating: Some(4.2),
            bb_total_responses: 50,
        };
        let score = compute_score(&data);
        assert_eq!(score.source, RatingSource::BlueBook);
        assert!(score.rmp_rating.is_none());
    }

    #[test]
    fn test_high_confidence_beats_low_confidence() {
        // A well-evidenced 3.9 should rank above a poorly-evidenced 4.5
        let high_evidence = compute_score(&RawInstructorData {
            instructor_id: 1,
            rmp_rating: Some(3.9),
            rmp_num_ratings: 100,
            bb_avg_instructor_rating: Some(4.5),
            bb_total_responses: 500,
        });
        let low_evidence = compute_score(&RawInstructorData {
            instructor_id: 2,
            rmp_rating: None,
            rmp_num_ratings: 0,
            bb_avg_instructor_rating: Some(4.5),
            bb_total_responses: 10,
        });
        assert!(
            high_evidence.rank_score > low_evidence.rank_score,
            "High-evidence 3.9 should have higher rank_score than low-evidence BB-only 4.5"
        );
    }

    #[test]
    fn test_regression_calibration_direction() {
        // Higher BB should produce higher calibrated score
        let low_bb = compute_score(&RawInstructorData {
            instructor_id: 1,
            rmp_rating: None,
            rmp_num_ratings: 0,
            bb_avg_instructor_rating: Some(3.0),
            bb_total_responses: 50,
        });
        let high_bb = compute_score(&RawInstructorData {
            instructor_id: 2,
            rmp_rating: None,
            rmp_num_ratings: 0,
            bb_avg_instructor_rating: Some(4.5),
            bb_total_responses: 50,
        });
        assert!(high_bb.calibrated_bb.unwrap() > low_bb.calibrated_bb.unwrap());
        assert!(high_bb.score > low_bb.score);
    }

    #[test]
    fn test_prior_rank_sentinel_matches_computation() {
        let computed = (PRIOR_MEAN - CI_Z * PRIOR_VAR.sqrt()) as f32;
        assert!(
            (PRIOR_RANK_SENTINEL - computed).abs() < 0.01,
            "PRIOR_RANK_SENTINEL ({PRIOR_RANK_SENTINEL}) should match computed value ({computed})"
        );
    }

    #[test]
    fn test_rating_source_serialization() {
        assert_eq!(RatingSource::Both.as_str(), "both");
        assert_eq!(RatingSource::Rmp.as_str(), "rmp");
        assert_eq!(RatingSource::BlueBook.as_str(), "bluebook");
        assert_eq!(RatingSource::parse("both"), Some(RatingSource::Both));
        assert_eq!(RatingSource::parse("rmp"), Some(RatingSource::Rmp));
        assert_eq!(
            RatingSource::parse("bluebook"),
            Some(RatingSource::BlueBook)
        );
        // Legacy "bb" still accepted
        assert_eq!(RatingSource::parse("bb"), Some(RatingSource::BlueBook));
        assert_eq!(RatingSource::parse("invalid"), None);
    }
}
