-- BlueBook fills an unreleased evaluation with a 6 or 9 in both rating cells and
-- no response counts. The scraper now discards those; remove the ones already stored.
DELETE FROM bluebook_evaluations
WHERE (instructor_rating IS NULL OR instructor_rating NOT BETWEEN 1 AND 5)
  AND (course_rating IS NULL OR course_rating NOT BETWEEN 1 AND 5);

UPDATE bluebook_evaluations
SET instructor_rating = NULL, instructor_response_count = NULL
WHERE instructor_rating NOT BETWEEN 1 AND 5;

UPDATE bluebook_evaluations
SET course_rating = NULL, course_response_count = NULL
WHERE course_rating NOT BETWEEN 1 AND 5;

ALTER TABLE bluebook_evaluations
    ADD CONSTRAINT chk_bluebook_instructor_rating_scale CHECK (instructor_rating BETWEEN 1 AND 5),
    ADD CONSTRAINT chk_bluebook_course_rating_scale CHECK (course_rating BETWEEN 1 AND 5);

-- MTC and C&I are second listings of MAT and CI. Their rows duplicate the canonical
-- subject's, and the few that differ credit the wrong instructor.
DELETE FROM bluebook_evaluations WHERE subject IN ('MTC', 'C&I');
DELETE FROM bluebook_subject_scrapes WHERE subject IN ('MTC', 'C&I');

-- BlueBook repeats one evaluation under every CRN of a cross-listed group. This view
-- keeps one row per evaluation, for aggregating an instructor's ratings and responses.
-- Which member's course code survives is arbitrary, so per-course work must not read it.
-- The survivor is the twin with a rating, then the most responses, then the lowest id.
-- An anti-join rather than DISTINCT ON, so a filter on instructor_name reaches the index.
CREATE VIEW bluebook_instructor_evaluations AS
SELECT
    be.id,
    be.instructor_name,
    be.subject,
    be.term,
    be.crn,
    be.instructor_rating,
    be.instructor_response_count,
    be.course_rating,
    be.course_response_count
FROM bluebook_evaluations be
LEFT JOIN courses c ON c.crn = be.crn AND c.term_code = be.term
WHERE NOT EXISTS (
    SELECT 1
    FROM bluebook_evaluations twin
    LEFT JOIN courses tc ON tc.crn = twin.crn AND tc.term_code = twin.term
    WHERE twin.instructor_name = be.instructor_name
      AND twin.term = be.term
      AND COALESCE('xl:' || tc.cross_list, 'crn:' || twin.crn) = COALESCE('xl:' || c.cross_list, 'crn:' || be.crn)
      AND (twin.instructor_rating IS NULL, -COALESCE(twin.instructor_response_count, -1), twin.id)
        < (be.instructor_rating IS NULL, -COALESCE(be.instructor_response_count, -1), be.id)
);
