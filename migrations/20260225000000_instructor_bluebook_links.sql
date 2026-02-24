CREATE TABLE instructor_bluebook_links (
    id              SERIAL PRIMARY KEY,
    instructor_id   INTEGER REFERENCES instructors(id) ON DELETE CASCADE,
    instructor_name VARCHAR NOT NULL,
    subject         VARCHAR,
    match_method    VARCHAR NOT NULL DEFAULT 'manual',
    status          VARCHAR NOT NULL DEFAULT 'pending',
    confidence      REAL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_by      BIGINT REFERENCES users(discord_id)
);

CREATE UNIQUE INDEX idx_bluebook_links_name_subject
    ON instructor_bluebook_links (instructor_name, COALESCE(subject, ''));

CREATE INDEX idx_bluebook_links_instructor ON instructor_bluebook_links (instructor_id);
CREATE INDEX idx_bluebook_links_status ON instructor_bluebook_links (status);
