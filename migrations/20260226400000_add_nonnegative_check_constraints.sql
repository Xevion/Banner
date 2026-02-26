-- Add CHECK constraints for non-negative fields.
-- These constraints back the Rust unsigned newtype wrappers (Count, DurationMs)
-- which decode INTEGER -> u32 and would error on negative values.
--
-- Note: seats_available is intentionally excluded — overenrolled courses
-- produce legitimate negative values.

-- Fix historical data: courses_unchanged was computed as (fetched - changed) which
-- could go negative when all fetched courses had changes. Clamp to 0.
UPDATE scrape_job_results
SET courses_unchanged = 0
WHERE courses_unchanged < 0;

-- Validate remaining data before adding constraints
DO $$ BEGIN
    IF EXISTS (
        SELECT 1 FROM courses
        WHERE enrollment < 0 OR max_enrollment < 0 OR wait_count < 0 OR wait_capacity < 0
    ) THEN
        RAISE EXCEPTION 'courses contains negative enrollment/capacity values';
    END IF;

    IF EXISTS (
        SELECT 1 FROM course_metrics
        WHERE enrollment < 0 OR wait_count < 0
    ) THEN
        RAISE EXCEPTION 'course_metrics contains negative enrollment/wait_count values';
    END IF;

    IF EXISTS (
        SELECT 1 FROM scrape_job_results
        WHERE duration_ms < 0 OR retry_count < 0
           OR courses_fetched < 0 OR courses_changed < 0 OR courses_unchanged < 0
    ) THEN
        RAISE EXCEPTION 'scrape_job_results contains negative values';
    END IF;
END $$;

-- courses
ALTER TABLE courses ADD CONSTRAINT chk_courses_enrollment_nonneg CHECK (enrollment >= 0);
ALTER TABLE courses ADD CONSTRAINT chk_courses_max_enrollment_nonneg CHECK (max_enrollment >= 0);
ALTER TABLE courses ADD CONSTRAINT chk_courses_wait_count_nonneg CHECK (wait_count >= 0);
ALTER TABLE courses ADD CONSTRAINT chk_courses_wait_capacity_nonneg CHECK (wait_capacity >= 0);

-- course_metrics (seats_available excluded: legitimately negative for overenrolled courses)
ALTER TABLE course_metrics ADD CONSTRAINT chk_metrics_enrollment_nonneg CHECK (enrollment >= 0);
ALTER TABLE course_metrics ADD CONSTRAINT chk_metrics_wait_count_nonneg CHECK (wait_count >= 0);

-- scrape_job_results (retry_count/max_retries on scrape_jobs already have CHECK from migration 20251103093649)
ALTER TABLE scrape_job_results ADD CONSTRAINT chk_results_duration_ms_nonneg CHECK (duration_ms >= 0);
ALTER TABLE scrape_job_results ADD CONSTRAINT chk_results_retry_count_nonneg CHECK (retry_count >= 0);
ALTER TABLE scrape_job_results ADD CONSTRAINT chk_results_fetched_nonneg CHECK (courses_fetched >= 0);
ALTER TABLE scrape_job_results ADD CONSTRAINT chk_results_changed_nonneg CHECK (courses_changed >= 0);
ALTER TABLE scrape_job_results ADD CONSTRAINT chk_results_unchanged_nonneg CHECK (courses_unchanged >= 0);
