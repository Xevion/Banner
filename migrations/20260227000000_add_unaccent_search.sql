-- Enable accent-insensitive search across all text search paths.
--
-- PostgreSQL's unaccent() is STABLE, not IMMUTABLE, which prevents its use
-- in indexes and generated columns. We create an immutable wrapper and a
-- custom text search configuration that uses unaccent as a dictionary.

CREATE EXTENSION IF NOT EXISTS unaccent;

-- Immutable wrapper around unaccent() for use in indexes and generated columns.
CREATE OR REPLACE FUNCTION immutable_unaccent(text) RETURNS text AS $$
    SELECT public.unaccent('public.unaccent', $1)
$$ LANGUAGE sql IMMUTABLE PARALLEL SAFE STRICT;

-- Custom text search configuration: simple tokenizer + unaccent dictionary.
-- This folds diacritics during both indexing and querying.
CREATE TEXT SEARCH CONFIGURATION simple_unaccent (COPY = simple);
ALTER TEXT SEARCH CONFIGURATION simple_unaccent
    ALTER MAPPING FOR hword, hword_part, word
    WITH unaccent, simple;

-- Rebuild the title_search generated column to use the unaccent-aware config.
ALTER TABLE courses DROP COLUMN title_search;
ALTER TABLE courses ADD COLUMN title_search tsvector
    GENERATED ALWAYS AS (to_tsvector('simple_unaccent', coalesce(title, ''))) STORED;

CREATE INDEX idx_courses_title_search ON courses USING GIN (title_search);
