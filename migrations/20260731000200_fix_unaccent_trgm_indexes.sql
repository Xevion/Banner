-- Rebuild trigram indexes on the expression the queries actually use.
--
-- The previous indexes were built on the bare columns while every search and
-- suggest query matches against immutable_unaccent(...). An expression index
-- only applies to that exact expression, so neither index was ever usable and
-- both showed zero scans. Because the ILIKE half of the search predicate had no
-- usable index, the planner could not form a BitmapOr and fell back to a
-- sequential scan over the whole courses heap.

CREATE INDEX idx_courses_title_unaccent_trgm
    ON courses USING gin (immutable_unaccent(title) gin_trgm_ops);

DROP INDEX idx_courses_title_trgm;

CREATE INDEX idx_instructors_display_name_unaccent_trgm
    ON instructors USING gin (immutable_unaccent(display_name) gin_trgm_ops);

DROP INDEX idx_instructors_display_name_trgm;

-- Never scanned, and low-cardinality enough that a scan is cheaper anyway.
DROP INDEX idx_courses_instructional_method;
