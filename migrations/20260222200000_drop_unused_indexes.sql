-- Drop unused/redundant indexes identified via pg_stat_user_indexes analysis.
-- All targets have 0 index scans since last stats reset.

-- Redundant with instructors_email_unique (same column, 672K scans)
DROP INDEX IF EXISTS idx_instructors_email;

-- course_metrics: 315K rows, 8.3K inserts, zero reads on any index.
-- PK is kept; these two are pure write overhead.
DROP INDEX IF EXISTS idx_course_metrics_course_timestamp;
DROP INDEX IF EXISTS idx_course_metrics_timestamp;

-- Superseded by idx_scrape_job_results_completed (184K scans)
DROP INDEX IF EXISTS idx_scrape_job_results_target_time;

-- scrape_jobs: subset of idx_scrape_jobs_priority_pending (which is used)
DROP INDEX IF EXISTS idx_scrape_jobs_pending;
-- scheduler_lookup: 0 scans, created in optimize_indexes migration but never hit
DROP INDEX IF EXISTS idx_scrape_jobs_scheduler_lookup;

-- The lock_next() query uses OR (locked_at IS NULL OR locked_at < ...)
-- which defeats all partial indexes filtered on locked_at IS NULL.
-- A plain index lets Postgres use a BitmapOr of two index scans instead of seq scan.
CREATE INDEX IF NOT EXISTS idx_scrape_jobs_locked_at ON scrape_jobs (locked_at);
