-- Add CHECK constraint on status column to prevent invalid states.
ALTER TABLE instructor_bluebook_links
    ADD CONSTRAINT instructor_bluebook_links_status_check
    CHECK (status IN ('auto', 'pending', 'approved', 'rejected'));
