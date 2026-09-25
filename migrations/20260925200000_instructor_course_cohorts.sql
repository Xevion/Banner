-- Each instructor's BlueBook rating on each course they teach, against everyone else who
-- teaches it. Cohort-relative standing controls for department and course difficulty,
-- which a raw 1-5 rating cannot: most instructors score above 4 regardless.
--
-- A cross-listed evaluation counts once per course code it is listed under, since it is a
-- real section of each; twins under the same course code collapse to one. Pairs under 15
-- responses are left out as too thin to rank.
CREATE MATERIALIZED VIEW instructor_course_cohorts AS
WITH evaluations AS (
    SELECT DISTINCT ON (ibl.instructor_id, be.term, be.subject, be.course_number,
                        COALESCE('xl:' || c.cross_list, 'crn:' || be.crn))
        ibl.instructor_id,
        be.subject,
        be.course_number,
        be.instructor_rating::float8 AS rating,
        be.instructor_response_count AS responses
    FROM bluebook_evaluations be
    JOIN instructor_bluebook_links ibl ON ibl.instructor_name = be.instructor_name
        AND (ibl.subject IS NULL OR ibl.subject = be.subject)
    LEFT JOIN courses c ON c.crn = be.crn AND c.term_code = be.term
    WHERE ibl.status IN ('approved', 'auto')
      AND ibl.instructor_id IS NOT NULL
      AND be.instructor_rating IS NOT NULL
      AND be.instructor_response_count > 0
    ORDER BY ibl.instructor_id, be.term, be.subject, be.course_number,
             COALESCE('xl:' || c.cross_list, 'crn:' || be.crn),
             be.instructor_response_count DESC, be.id
),
pairs AS (
    SELECT instructor_id, subject, course_number,
           SUM(rating * responses) / SUM(responses) AS rating,
           SUM(responses)::int AS responses,
           COUNT(*)::int AS sections
    FROM evaluations
    GROUP BY instructor_id, subject, course_number
    HAVING SUM(responses) >= 15
)
SELECT
    instructor_id,
    subject,
    course_number,
    rating,
    responses,
    sections,
    AVG(rating) OVER course AS cohort_mean,
    STDDEV_SAMP(rating) OVER course AS cohort_sd,
    COUNT(*) OVER course AS cohort_size,
    RANK() OVER (course ORDER BY rating DESC) AS cohort_rank
FROM pairs
WINDOW course AS (PARTITION BY subject, course_number);

CREATE UNIQUE INDEX idx_instructor_course_cohorts_pair
    ON instructor_course_cohorts (instructor_id, subject, course_number);
CREATE INDEX idx_instructor_course_cohorts_course
    ON instructor_course_cohorts (subject, course_number);

-- Each instructor's response-weighted mean distance from their cohorts, over the courses
-- that have a cohort to compare against, and where that falls among all instructors.
CREATE MATERIALIZED VIEW instructor_cohort_standing AS
WITH deltas AS (
    SELECT instructor_id,
           SUM((rating - cohort_mean) * responses) / SUM(responses) AS delta,
           COUNT(*)::int AS compared_courses
    FROM instructor_course_cohorts
    WHERE cohort_size > 1
    GROUP BY instructor_id
)
SELECT
    instructor_id,
    delta,
    compared_courses,
    PERCENT_RANK() OVER (ORDER BY delta) AS percentile
FROM deltas;

CREATE UNIQUE INDEX idx_instructor_cohort_standing_instructor
    ON instructor_cohort_standing (instructor_id);
