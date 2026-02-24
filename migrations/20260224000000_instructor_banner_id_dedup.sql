-- Allow instructors without email addresses to be stored.
-- Previously, instructors missing an email were silently skipped during scraping.
--
-- Dedup strategy:
--   - Instructors WITH email: dedup by email (existing behavior, partial unique)
--   - Instructors WITHOUT email: dedup by display_name (partial unique)

-- 1. Make email nullable
ALTER TABLE instructors ALTER COLUMN email DROP NOT NULL;

-- 2. Replace absolute email uniqueness with partial (only for non-null emails)
ALTER TABLE instructors DROP CONSTRAINT instructors_email_unique;
CREATE UNIQUE INDEX idx_instructors_email_unique ON instructors (email) WHERE email IS NOT NULL;

-- 3. Add partial unique index on display_name for no-email instructors
CREATE UNIQUE INDEX idx_instructors_no_email_display_name
    ON instructors (display_name) WHERE email IS NULL;
