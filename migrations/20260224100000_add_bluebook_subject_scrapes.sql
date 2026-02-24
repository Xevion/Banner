CREATE TABLE bluebook_subject_scrapes (
    subject VARCHAR NOT NULL PRIMARY KEY,
    last_scraped_at TIMESTAMPTZ NOT NULL
);
