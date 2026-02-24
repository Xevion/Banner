-- Fix year column for Fall terms.
--
-- Banner encodes Fall semesters as (display_year + 1)10: "Fall 2025" has code "202610",
-- so the code prefix is 2026. Previously parse_term_code stored the raw code prefix,
-- making every Fall term's year column one too high (e.g., 2026 instead of 2025).
--
-- This migration corrects all existing Fall rows to store the display year.

UPDATE terms SET year = year - 1, updated_at = now() WHERE season = 'Fall';
