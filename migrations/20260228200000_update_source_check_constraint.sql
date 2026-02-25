-- Accept both legacy 'bb' and new 'bluebook' values in the source column.
ALTER TABLE instructor_scores
    DROP CONSTRAINT IF EXISTS instructor_scores_source_check;

ALTER TABLE instructor_scores
    ADD CONSTRAINT instructor_scores_source_check
    CHECK (source IN ('both', 'rmp', 'bb', 'bluebook'));
