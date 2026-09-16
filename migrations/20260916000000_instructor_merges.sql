-- Records that two scraped instructor identities are one person.
--
-- Merging deletes the absorbed row, but the scrape rebuilds instructors from
-- Banner on every run and would recreate it. These rows outlive the deletion so
-- the next upsert resolves the absorbed identity to its survivor instead.

CREATE TABLE instructor_merges (
    id SERIAL PRIMARY KEY,
    survivor_id INTEGER NOT NULL REFERENCES instructors(id) ON DELETE CASCADE,
    absorbed_email VARCHAR,
    absorbed_display_name VARCHAR NOT NULL,
    absorbed_slug TEXT,
    tier VARCHAR NOT NULL,
    decided_by BIGINT REFERENCES users(discord_id),
    decided_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- An absorbed identity resolves to exactly one survivor. Records carrying an
-- address are keyed on it; those without one fall back to the display name,
-- matching how the instructor upsert itself dedupes each case.
CREATE UNIQUE INDEX idx_instructor_merges_email
    ON instructor_merges (absorbed_email) WHERE absorbed_email IS NOT NULL;

CREATE UNIQUE INDEX idx_instructor_merges_display_name
    ON instructor_merges (absorbed_display_name) WHERE absorbed_email IS NULL;

-- Slugs carry a random suffix and cannot be regenerated, so the absorbed one is
-- kept to redirect a URL that is already published in the sitemap.
CREATE UNIQUE INDEX idx_instructor_merges_slug
    ON instructor_merges (absorbed_slug) WHERE absorbed_slug IS NOT NULL;

CREATE INDEX idx_instructor_merges_survivor ON instructor_merges (survivor_id);
