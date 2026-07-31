-- Precomputed per-term aggregates for the search-options endpoint.
--
-- The filter-range and subject-enrollment aggregates were recomputed from the
-- full courses heap on every cache miss, reading ~4.6MB per call and accounting
-- for roughly 74% of all database block reads. Past terms never change, so their
-- summaries are computed once and never revisited; only terms touched by a scrape
-- are refreshed.

CREATE TABLE term_summary (
    term_code         TEXT PRIMARY KEY,
    course_number_min INTEGER,
    course_number_max INTEGER,
    credit_hour_min   DOUBLE PRECISION,
    credit_hour_max   DOUBLE PRECISION,
    wait_count_max    INTEGER,
    course_count      INTEGER NOT NULL,
    computed_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE term_subject_summary (
    term_code        TEXT NOT NULL REFERENCES term_summary(term_code) ON DELETE CASCADE,
    subject          TEXT NOT NULL,
    total_enrollment BIGINT NOT NULL,
    PRIMARY KEY (term_code, subject)
);

-- Recompute both summaries for a single term.
--
-- The range aggregates cover only numeric course numbers, while course_count
-- covers every row in the term. These are separate CTEs rather than one pass
-- with FILTER because course_number::int must never be evaluated on a row that
-- fails the numeric test.
CREATE OR REPLACE FUNCTION refresh_term_summary(p_term_code TEXT) RETURNS void
LANGUAGE plpgsql AS $$
BEGIN
    WITH numeric_rows AS (
        SELECT MIN(course_number::int)                          AS cn_min,
               MAX(course_number::int)                          AS cn_max,
               MIN(COALESCE(credit_hours, credit_hour_low, 0))  AS ch_min,
               MAX(COALESCE(credit_hours, credit_hour_high, 0)) AS ch_max,
               MAX(wait_count)                                  AS wc_max
        FROM courses
        WHERE term_code = p_term_code
          AND course_number ~ '^\d+$'
    ), all_rows AS (
        SELECT COUNT(*)::int AS course_count
        FROM courses
        WHERE term_code = p_term_code
    )
    INSERT INTO term_summary (
        term_code, course_number_min, course_number_max,
        credit_hour_min, credit_hour_max, wait_count_max,
        course_count, computed_at
    )
    SELECT p_term_code, n.cn_min, n.cn_max, n.ch_min, n.ch_max, n.wc_max,
           a.course_count, now()
    FROM all_rows a CROSS JOIN numeric_rows n
    ON CONFLICT (term_code) DO UPDATE SET
        course_number_min = EXCLUDED.course_number_min,
        course_number_max = EXCLUDED.course_number_max,
        credit_hour_min   = EXCLUDED.credit_hour_min,
        credit_hour_max   = EXCLUDED.credit_hour_max,
        wait_count_max    = EXCLUDED.wait_count_max,
        course_count      = EXCLUDED.course_count,
        computed_at       = EXCLUDED.computed_at;

    DELETE FROM term_subject_summary WHERE term_code = p_term_code;

    INSERT INTO term_subject_summary (term_code, subject, total_enrollment)
    SELECT p_term_code, subject, COALESCE(SUM(enrollment), 0)
    FROM courses
    WHERE term_code = p_term_code
    GROUP BY subject;
END;
$$;

-- Backfill every term present at migration time.
SELECT refresh_term_summary(term_code)
FROM (SELECT DISTINCT term_code FROM courses) AS t;
