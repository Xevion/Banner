CREATE TABLE course_watches (
    id SERIAL PRIMARY KEY,
    discord_user_id BIGINT NOT NULL REFERENCES users(discord_id) ON DELETE CASCADE,
    course_id INT NOT NULL REFERENCES courses(id) ON DELETE CASCADE,
    watch_type TEXT NOT NULL CHECK (watch_type IN ('seats_available', 'waitlist_open', 'any_change')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    notified_at TIMESTAMPTZ,
    active BOOLEAN NOT NULL DEFAULT TRUE,
    UNIQUE (discord_user_id, course_id, watch_type)
);

CREATE INDEX idx_course_watches_user ON course_watches(discord_user_id);
CREATE INDEX idx_course_watches_active_course ON course_watches(course_id) WHERE active = TRUE;
