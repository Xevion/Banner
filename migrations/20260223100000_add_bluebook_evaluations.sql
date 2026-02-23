CREATE TABLE bluebook_evaluations (
    id SERIAL PRIMARY KEY,
    -- Section identification (BlueBook keys)
    subject VARCHAR NOT NULL,
    course_number VARCHAR NOT NULL,
    section VARCHAR NOT NULL,
    term VARCHAR NOT NULL,

    -- Instructor (text from BlueBook, linked later)
    instructor_name VARCHAR NOT NULL,
    instructor_id INTEGER REFERENCES instructors(id) ON DELETE SET NULL,

    -- Evaluation data
    instructor_rating REAL,
    instructor_response_count INTEGER,
    course_rating REAL,
    course_response_count INTEGER,

    -- Metadata
    department VARCHAR,
    scraped_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- One evaluation per section per term per instructor
    CONSTRAINT uq_bluebook_eval UNIQUE (subject, course_number, section, term, instructor_name)
);

CREATE INDEX idx_bluebook_eval_instructor ON bluebook_evaluations(instructor_id) WHERE instructor_id IS NOT NULL;
CREATE INDEX idx_bluebook_eval_course ON bluebook_evaluations(subject, course_number);
