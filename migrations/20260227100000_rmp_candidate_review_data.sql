-- Add review-derived data to match candidates for display and matching context.
-- Populated during rescore from aggregated rmp_reviews data.
ALTER TABLE rmp_match_candidates
ADD COLUMN review_subjects TEXT[] NOT NULL DEFAULT '{}',
ADD COLUMN review_years SMALLINT[] NOT NULL DEFAULT '{}';
