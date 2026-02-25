CREATE TABLE rmp_reviews (
    id                   SERIAL PRIMARY KEY,
    rmp_legacy_id        INTEGER NOT NULL REFERENCES rmp_professors(legacy_id) ON DELETE CASCADE,
    comment              TEXT,
    class                VARCHAR,
    grade                VARCHAR,
    rating_tags          TEXT[] NOT NULL DEFAULT '{}',
    helpful_rating       REAL,
    clarity_rating       REAL,
    difficulty_rating    REAL,
    would_take_again     SMALLINT,
    is_for_credit        BOOLEAN,
    is_for_online_class  BOOLEAN,
    attendance_mandatory VARCHAR,
    flag_status          VARCHAR NOT NULL DEFAULT 'visible',
    textbook_use         INTEGER,
    thumbs_up_total      INTEGER NOT NULL DEFAULT 0,
    thumbs_down_total    INTEGER NOT NULL DEFAULT 0,
    posted_at            TIMESTAMPTZ,
    scraped_at           TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_rmp_reviews_legacy_id ON rmp_reviews (rmp_legacy_id);
CREATE INDEX idx_rmp_reviews_posted_at ON rmp_reviews (rmp_legacy_id, posted_at DESC NULLS LAST);
