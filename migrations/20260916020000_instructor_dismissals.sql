-- Records that two instructor records sharing a display name are different
-- people, so duplicate review stops offering the pair on every visit.

CREATE TABLE instructor_dismissals (
    id SERIAL PRIMARY KEY,
    -- Ordering the ids makes one row per pair whichever way it is submitted.
    lesser_id INTEGER NOT NULL REFERENCES instructors(id) ON DELETE CASCADE,
    greater_id INTEGER NOT NULL REFERENCES instructors(id) ON DELETE CASCADE,
    decided_by BIGINT REFERENCES users(discord_id),
    decided_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (lesser_id < greater_id),
    UNIQUE (lesser_id, greater_id)
);

-- The unique index covers lesser_id only, so a cascade from the other side
-- would seq-scan without this.
CREATE INDEX idx_instructor_dismissals_greater ON instructor_dismissals (greater_id);
