-- Precomputed Bayesian instructor scores combining RMP and BlueBook data.
-- Recomputed on startup and after scrape completions. See src/data/scoring.rs.
CREATE TABLE instructor_scores (
    instructor_id   INTEGER PRIMARY KEY REFERENCES instructors(id) ON DELETE CASCADE,
    display_score   REAL NOT NULL,
    sort_score      REAL NOT NULL,
    ci_lower        REAL NOT NULL,
    ci_upper        REAL NOT NULL,
    confidence      REAL NOT NULL,
    source          TEXT NOT NULL CHECK (source IN ('both', 'rmp', 'bb')),
    rmp_rating      REAL,
    rmp_count       INTEGER NOT NULL DEFAULT 0,
    bb_rating       REAL,
    calibrated_bb   REAL,
    bb_count        INTEGER NOT NULL DEFAULT 0,
    computed_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_instructor_scores_sort ON instructor_scores(sort_score DESC);
CREATE INDEX idx_instructor_scores_display ON instructor_scores(display_score DESC);
