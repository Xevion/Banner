-- Make BlueBook instructor links idempotent: drop match_method, add 'auto' status.
--
-- Status values:
--   'auto'     — algorithm-generated high-confidence match (regenerated on refresh)
--   'pending'  — algorithm-generated low-confidence match (regenerated on refresh)
--   'approved' — manually confirmed (preserved across refreshes)
--   'rejected' — manually rejected (preserved across refreshes)
--
-- The match_method column is now redundant: status encodes the workflow state,
-- and manual vs auto provenance is captured by the status transitions themselves.

-- Wipe all existing links so we start fresh with the new model.
TRUNCATE instructor_bluebook_links RESTART IDENTITY;

-- Drop the match_method column.
ALTER TABLE instructor_bluebook_links DROP COLUMN match_method;
