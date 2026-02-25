-- Extended profile data from per-teacher GraphQL node query
ALTER TABLE rmp_professors
    ADD COLUMN ratings_r1  INTEGER,
    ADD COLUMN ratings_r2  INTEGER,
    ADD COLUMN ratings_r3  INTEGER,
    ADD COLUMN ratings_r4  INTEGER,
    ADD COLUMN ratings_r5  INTEGER,
    ADD COLUMN course_codes JSONB;

-- Review scrape scheduling
ALTER TABLE rmp_professors
    ADD COLUMN reviews_last_scraped_at TIMESTAMPTZ,
    ADD COLUMN review_scrape_interval  INTERVAL NOT NULL DEFAULT INTERVAL '14 days';

CREATE INDEX idx_rmp_professors_reviews_last_scraped
    ON rmp_professors (reviews_last_scraped_at ASC NULLS FIRST);
