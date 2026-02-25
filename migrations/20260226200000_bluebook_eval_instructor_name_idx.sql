-- Index for joining bluebook_evaluations to instructor_bluebook_links by instructor_name.
-- Without this, LATERAL joins and subqueries that aggregate per-instructor do sequential scans.
CREATE INDEX IF NOT EXISTS idx_bluebook_eval_instructor_name ON bluebook_evaluations (instructor_name);
