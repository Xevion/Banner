-- Denormalized meeting time scalars for fast schedule cache queries.
-- Eliminates the expensive COALESCE + lateral-join over dual-format JSONB
-- that previously took ~1.6s across 332K+ courses.

CREATE TABLE course_meetings (
    id SERIAL PRIMARY KEY,
    course_id INTEGER NOT NULL REFERENCES courses(id) ON DELETE CASCADE,
    day_bits SMALLINT NOT NULL CHECK (day_bits > 0),
    begin_minutes SMALLINT NOT NULL CHECK (begin_minutes >= 0 AND begin_minutes < 1440),
    end_minutes SMALLINT NOT NULL CHECK (end_minutes > begin_minutes AND end_minutes <= 1440),
    start_date DATE NOT NULL,
    end_date DATE NOT NULL CHECK (end_date >= start_date)
);

CREATE INDEX idx_course_meetings_course_id ON course_meetings(course_id);

-- Backfill from existing JSONB meeting_times, handling both legacy and current formats.
INSERT INTO course_meetings (course_id, day_bits, begin_minutes, end_minutes, start_date, end_date)
SELECT
    c.id,
    COALESCE(
        (SELECT bit_or(
            CASE d
                WHEN 'monday' THEN 1 WHEN 'tuesday' THEN 2 WHEN 'wednesday' THEN 4
                WHEN 'thursday' THEN 8 WHEN 'friday' THEN 16 WHEN 'saturday' THEN 32
                WHEN 'sunday' THEN 64
            END
        ) FROM jsonb_array_elements_text(mt.val->'days') AS d),
        (CASE WHEN (mt.val->>'monday')::boolean THEN 1 ELSE 0 END |
         CASE WHEN (mt.val->>'tuesday')::boolean THEN 2 ELSE 0 END |
         CASE WHEN (mt.val->>'wednesday')::boolean THEN 4 ELSE 0 END |
         CASE WHEN (mt.val->>'thursday')::boolean THEN 8 ELSE 0 END |
         CASE WHEN (mt.val->>'friday')::boolean THEN 16 ELSE 0 END |
         CASE WHEN (mt.val->>'saturday')::boolean THEN 32 ELSE 0 END |
         CASE WHEN (mt.val->>'sunday')::boolean THEN 64 ELSE 0 END)
    )::smallint,
    CASE
        WHEN mt.val->>'begin_time' IS NOT NULL THEN
            (LEFT(mt.val->>'begin_time', 2)::int * 60 + RIGHT(mt.val->>'begin_time', 2)::int)::smallint
        ELSE
            (SPLIT_PART(mt.val->'timeRange'->>'start', ':', 1)::int * 60 +
             SPLIT_PART(mt.val->'timeRange'->>'start', ':', 2)::int)::smallint
    END,
    CASE
        WHEN mt.val->>'end_time' IS NOT NULL THEN
            (LEFT(mt.val->>'end_time', 2)::int * 60 + RIGHT(mt.val->>'end_time', 2)::int)::smallint
        ELSE
            (SPLIT_PART(mt.val->'timeRange'->>'end', ':', 1)::int * 60 +
             SPLIT_PART(mt.val->'timeRange'->>'end', ':', 2)::int)::smallint
    END,
    CASE
        WHEN mt.val->>'start_date' IS NOT NULL THEN TO_DATE(mt.val->>'start_date', 'MM/DD/YYYY')
        ELSE (mt.val->'dateRange'->>'start')::date
    END,
    CASE
        WHEN mt.val->>'end_date' IS NOT NULL THEN TO_DATE(mt.val->>'end_date', 'MM/DD/YYYY')
        ELSE (mt.val->'dateRange'->>'end')::date
    END
FROM courses c
CROSS JOIN LATERAL jsonb_array_elements(c.meeting_times) AS mt(val)
WHERE
    -- Must have time data
    COALESCE(mt.val->>'begin_time', mt.val->'timeRange'->>'start') IS NOT NULL
    AND COALESCE(mt.val->>'end_time', mt.val->'timeRange'->>'end') IS NOT NULL
    -- Must have at least one day
    AND COALESCE(
        (SELECT bit_or(
            CASE d
                WHEN 'monday' THEN 1 WHEN 'tuesday' THEN 2 WHEN 'wednesday' THEN 4
                WHEN 'thursday' THEN 8 WHEN 'friday' THEN 16 WHEN 'saturday' THEN 32
                WHEN 'sunday' THEN 64
            END
        ) FROM jsonb_array_elements_text(mt.val->'days') AS d),
        (CASE WHEN (mt.val->>'monday')::boolean THEN 1 ELSE 0 END |
         CASE WHEN (mt.val->>'tuesday')::boolean THEN 2 ELSE 0 END |
         CASE WHEN (mt.val->>'wednesday')::boolean THEN 4 ELSE 0 END |
         CASE WHEN (mt.val->>'thursday')::boolean THEN 8 ELSE 0 END |
         CASE WHEN (mt.val->>'friday')::boolean THEN 16 ELSE 0 END |
         CASE WHEN (mt.val->>'saturday')::boolean THEN 32 ELSE 0 END |
         CASE WHEN (mt.val->>'sunday')::boolean THEN 64 ELSE 0 END)
    ) > 0
    -- Time must be valid (end > begin)
    AND CASE
        WHEN mt.val->>'begin_time' IS NOT NULL THEN
            (LEFT(mt.val->>'end_time', 2)::int * 60 + RIGHT(mt.val->>'end_time', 2)::int) >
            (LEFT(mt.val->>'begin_time', 2)::int * 60 + RIGHT(mt.val->>'begin_time', 2)::int)
        ELSE
            (SPLIT_PART(mt.val->'timeRange'->>'end', ':', 1)::int * 60 +
             SPLIT_PART(mt.val->'timeRange'->>'end', ':', 2)::int) >
            (SPLIT_PART(mt.val->'timeRange'->>'start', ':', 1)::int * 60 +
             SPLIT_PART(mt.val->'timeRange'->>'start', ':', 2)::int)
    END;
