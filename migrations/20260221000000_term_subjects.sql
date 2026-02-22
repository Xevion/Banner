CREATE TABLE term_subjects (
    term_code VARCHAR NOT NULL,
    subject_code VARCHAR NOT NULL,
    fetched_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (term_code, subject_code)
);
