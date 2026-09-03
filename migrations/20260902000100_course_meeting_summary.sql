-- Sorting reads these instead of parsing meeting_times: a text comparison on a
-- JSONB path cannot use an index, and ordering by array position disagrees with
-- the earliest meeting the row actually displays.
ALTER TABLE courses
    ADD COLUMN first_begin_minutes smallint,
    ADD COLUMN first_end_minutes   smallint,
    ADD COLUMN meeting_minutes     smallint,
    ADD COLUMN weekly_minutes      smallint,
    ADD COLUMN day_mask            smallint;

-- The earliest meeting defines the start, end and length a row shows; the mask
-- and weekly total aggregate every meeting. Backfilled from course_meetings
-- rather than the JSONB, that table already being the parsed form of it.
UPDATE courses c
SET first_begin_minutes = a.first_begin,
    first_end_minutes   = a.first_end,
    meeting_minutes     = a.first_end - a.first_begin,
    weekly_minutes      = a.weekly,
    day_mask            = a.mask
FROM (
    SELECT m.course_id,
           (array_agg(m.begin_minutes ORDER BY m.begin_minutes))[1] AS first_begin,
           (array_agg(m.end_minutes   ORDER BY m.begin_minutes))[1] AS first_end,
           sum(m.end_minutes - m.begin_minutes)::smallint           AS weekly,
           bit_or(m.day_bits)                                       AS mask
    FROM course_meetings m
    GROUP BY m.course_id
) a
WHERE c.id = a.course_id;

CREATE INDEX idx_courses_term_start ON courses (term_code, first_begin_minutes);
CREATE INDEX idx_courses_term_day_mask ON courses (term_code, day_mask);
