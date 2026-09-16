-- 'bb' was a legacy spelling of 'bluebook' accepted on read only. No code path
-- has ever written it and the column holds no such rows, so the constraint was
-- the last thing keeping the value reachable.
ALTER TABLE instructor_scores
    DROP CONSTRAINT IF EXISTS instructor_scores_source_check;

ALTER TABLE instructor_scores
    ADD CONSTRAINT instructor_scores_source_check
    CHECK (source IN ('both', 'rmp', 'bluebook'));
