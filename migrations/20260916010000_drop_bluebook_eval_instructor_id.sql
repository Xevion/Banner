-- Evaluations resolve to an instructor through instructor_bluebook_links on
-- instructor_name; this column was never populated and nothing reads it.
-- Dropping it takes its partial index and foreign key with it.

ALTER TABLE bluebook_evaluations DROP COLUMN instructor_id;
