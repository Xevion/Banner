//! An instructor's standing among everyone who teaches the same course.

use anyhow::{Context, Result};
use serde::Serialize;
use sqlx::PgPool;
use ts_rs::TS;

use crate::data::instructors::narrow_rating;
use crate::data::unsigned::Count;

/// Variance between one section's class and the next, however many of them responded.
/// Fitted with the per-response term below on per-section residuals, and calibrated by
/// split-half: the interval covers 95% of held-out half differences.
const SECTION_VARIANCE: f64 = 0.0624;

/// Variance of a single student's rating, shrinking with the responses averaged together.
const RESPONSE_VARIANCE: f64 = 0.392;

/// Roughly one population SD of instructor deltas: past this the interval spans the
/// difference between a strong and a weak instructor.
const INSUFFICIENT_ABOVE: f64 = 0.34;

/// Half the above, about the gap between the median and upper-quartile instructor.
const LIMITED_ABOVE: f64 = 0.17;

/// How far the 95% interval on a course rating leaves it from meaning anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum Evidence {
    /// Too wide to tell this instructor apart from the cohort; hide the number.
    Insufficient,
    /// Readable, but worth a caveat.
    Limited,
    Solid,
}

impl Evidence {
    #[must_use]
    pub fn from_half_width(half_width: f64) -> Self {
        if half_width > INSUFFICIENT_ABOVE {
            Self::Insufficient
        } else if half_width > LIMITED_ABOVE {
            Self::Limited
        } else {
            Self::Solid
        }
    }
}

/// Half-width of the 95% interval on an instructor's rating for one course.
#[must_use]
pub fn half_width(sections: u32, responses: u32) -> f64 {
    1.96 * (SECTION_VARIANCE / f64::from(sections) + RESPONSE_VARIANCE / f64::from(responses)).sqrt()
}

/// Half-width of the 95% interval on a response-weighted mean over several courses,
/// each given as `(sections, responses)`.
///
/// Cohort means are treated as exact, so for small cohorts this runs slightly narrow.
#[must_use]
pub fn pooled_half_width(courses: &[(u32, u32)]) -> f64 {
    let total: f64 = courses.iter().map(|&(_, responses)| f64::from(responses)).sum();
    let variance: f64 = courses
        .iter()
        .map(|&(sections, responses)| {
            let weight = f64::from(responses) / total;
            let se = half_width(sections, responses) / 1.96;
            weight * weight * se * se
        })
        .sum();
    1.96 * variance.sqrt()
}

/// Another instructor who teaches the same course.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CohortPeer {
    pub slug: Option<String>,
    pub display_name: String,
    pub rating: f32,
    pub responses: Count,
}

/// An instructor's rating on one course, set against everyone else who teaches it.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CohortCourse {
    pub subject: String,
    pub course_number: String,
    /// Response-weighted mean `BlueBook` instructor rating across this course's sections.
    pub rating: f32,
    pub responses: Count,
    pub sections: Count,
    pub cohort_mean: f32,
    /// Absent for a cohort of one.
    pub cohort_sd: Option<f32>,
    pub cohort_size: Count,
    /// 1 is the highest-rated; absent when nobody else teaches the course.
    pub rank: Option<Count>,
    /// `rating - cohort_mean`; absent when nobody else teaches the course.
    pub delta: Option<f32>,
    /// Half-width of the 95% interval on `rating`.
    pub half_width: f32,
    pub evidence: Evidence,
    /// The rest of the cohort, highest-rated first.
    pub peers: Vec<CohortPeer>,
}

/// Where an instructor sits among all instructors, by mean distance from their cohorts.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CohortStanding {
    /// Response-weighted mean of the per-course deltas.
    pub delta: f32,
    /// Share of instructors with a lower delta, 0 to 100.
    pub percentile: u8,
    /// Courses with a cohort to compare against.
    pub compared_courses: Count,
    /// Half-width of the 95% interval on `delta`.
    pub half_width: f32,
    pub evidence: Evidence,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct InstructorCohort {
    /// Absent when no course has anyone to compare against.
    pub standing: Option<CohortStanding>,
    pub courses: Vec<CohortCourse>,
}

/// Load an instructor's per-course cohort standing.
pub async fn fetch_instructor_cohort(pool: &PgPool, instructor_id: i32) -> Result<InstructorCohort> {
    let standing_row = sqlx::query!(
        r#"
        SELECT delta AS "delta!", compared_courses AS "compared_courses!: Count",
               percentile AS "percentile!"
        FROM instructor_cohort_standing
        WHERE instructor_id = $1
        "#,
        instructor_id,
    )
    .fetch_optional(pool)
    .await
    .context("failed to fetch cohort standing")?;

    let courses = sqlx::query!(
        r#"
        SELECT subject AS "subject!", course_number AS "course_number!",
               rating AS "rating!", responses AS "responses!: Count", sections AS "sections!: Count",
               cohort_mean AS "cohort_mean!", cohort_sd,
               cohort_size::int AS "cohort_size!: Count", cohort_rank::int AS "cohort_rank!: Count"
        FROM instructor_course_cohorts
        WHERE instructor_id = $1
        ORDER BY subject, course_number
        "#,
        instructor_id,
    )
    .fetch_all(pool)
    .await
    .context("failed to fetch cohort courses")?;

    let mut peers = sqlx::query!(
        r#"
        SELECT peer.subject AS "subject!", peer.course_number AS "course_number!",
               i.slug, i.display_name, peer.rating AS "rating!", peer.responses AS "responses!: Count"
        FROM instructor_course_cohorts me
        JOIN instructor_course_cohorts peer
            ON peer.subject = me.subject
           AND peer.course_number = me.course_number
           AND peer.instructor_id <> me.instructor_id
        JOIN instructors i ON i.id = peer.instructor_id
        WHERE me.instructor_id = $1
        ORDER BY peer.rating DESC, i.display_name
        "#,
        instructor_id,
    )
    .fetch_all(pool)
    .await
    .context("failed to fetch cohort peers")?;

    let courses = courses
        .into_iter()
        .map(|r| {
            let compared = r.cohort_size.get() > 1;
            let course_peers = peers
                .extract_if(.., |p| p.subject == r.subject && p.course_number == r.course_number)
                .map(|p| CohortPeer {
                    slug: p.slug,
                    display_name: p.display_name,
                    rating: narrow_rating(p.rating),
                    responses: p.responses,
                })
                .collect();
            let half_width = half_width(r.sections.get(), r.responses.get());
            CohortCourse {
                subject: r.subject,
                course_number: r.course_number,
                rating: narrow_rating(r.rating),
                responses: r.responses,
                sections: r.sections,
                cohort_mean: narrow_rating(r.cohort_mean),
                cohort_sd: r.cohort_sd.map(narrow_rating),
                cohort_size: r.cohort_size,
                rank: compared.then_some(r.cohort_rank),
                delta: compared.then(|| narrow_rating(r.rating - r.cohort_mean)),
                half_width: narrow_rating(half_width),
                evidence: Evidence::from_half_width(half_width),
                peers: course_peers,
            }
        })
        .collect::<Vec<_>>();

    let compared: Vec<(u32, u32)> = courses
        .iter()
        .filter(|c| c.delta.is_some())
        .map(|c| (c.sections.get(), c.responses.get()))
        .collect();
    let standing = standing_row.map(|r| {
        let half_width = pooled_half_width(&compared);
        CohortStanding {
            delta: narrow_rating(r.delta),
            percentile: percent(r.percentile),
            compared_courses: r.compared_courses,
            half_width: narrow_rating(half_width),
            evidence: Evidence::from_half_width(half_width),
        }
    });

    Ok(InstructorCohort { standing, courses })
}

/// Rebuild both cohort views from the current evaluations and links.
pub async fn refresh_cohorts(pool: &PgPool) -> Result<()> {
    sqlx::query!("REFRESH MATERIALIZED VIEW CONCURRENTLY instructor_course_cohorts")
        .execute(pool)
        .await
        .context("failed to refresh instructor_course_cohorts")?;
    sqlx::query!("REFRESH MATERIALIZED VIEW CONCURRENTLY instructor_cohort_standing")
        .execute(pool)
        .await
        .context("failed to refresh instructor_cohort_standing")?;
    Ok(())
}

/// A `PERCENT_RANK` fraction as a whole percentage.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "PERCENT_RANK lies in 0..=1, so the rounded percentage fits a u8"
)]
fn percent(fraction: f64) -> u8 {
    (fraction * 100.0).round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[rstest::rstest]
    #[case(1, 150, 0.50)]
    #[case(3, 135, 0.30)]
    #[case(86, 1354, 0.06)]
    fn test_half_width_matches_fitted_model(#[case] sections: u32, #[case] responses: u32, #[case] expected: f64) {
        assert!((half_width(sections, responses) - expected).abs() < 0.005);
    }

    #[test]
    fn test_pooled_half_width_of_one_course_is_that_course() {
        assert!((pooled_half_width(&[(3, 135)]) - half_width(3, 135)).abs() < 1e-12);
    }

    #[test]
    fn test_pooled_half_width_of_two_equal_courses_shrinks_by_root_two() {
        let pooled = pooled_half_width(&[(3, 135), (3, 135)]);
        assert!((pooled - half_width(3, 135) / 2f64.sqrt()).abs() < 1e-12);
    }

    #[test]
    fn test_pooled_half_width_leans_on_the_heavier_course() {
        let pooled = pooled_half_width(&[(1, 20), (40, 2000)]);
        assert!(pooled < half_width(40, 2000) * 1.05);
    }

    #[rstest::rstest]
    #[case(1, 5000, Evidence::Insufficient)]
    #[case(2, 400, Evidence::Insufficient)]
    #[case(3, 135, Evidence::Limited)]
    #[case(86, 1354, Evidence::Solid)]
    fn test_evidence_follows_interval_width(#[case] sections: u32, #[case] responses: u32, #[case] expected: Evidence) {
        assert_eq!(Evidence::from_half_width(half_width(sections, responses)), expected);
    }
}
