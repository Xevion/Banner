-- Match status derived from the decisions that actually survive a rescore.
--
-- `clear_previous_run` keeps exactly two things: links a human made by hand, and
-- candidates a human rejected. Everything else is regenerated, so a stored status
-- column was a cache of these rows that could disagree with them.
CREATE VIEW instructor_rmp_match_status AS
SELECT
    i.id AS instructor_id,
    CASE
        WHEN COALESCE(l.manual_links, 0) > 0 THEN 'confirmed'
        WHEN COALESCE(l.total_links, 0) > 0 THEN 'auto'
        WHEN COALESCE(c.open_candidates, 0) > 0 THEN 'pending'
        WHEN COALESCE(c.total_candidates, 0) > 0 THEN 'rejected'
        ELSE 'unmatched'
    END AS status
FROM instructors i
LEFT JOIN (
    SELECT
        instructor_id,
        COUNT(*) AS total_links,
        COUNT(*) FILTER (WHERE source = 'manual') AS manual_links
    FROM instructor_rmp_links
    GROUP BY instructor_id
) l ON l.instructor_id = i.id
LEFT JOIN (
    SELECT
        instructor_id,
        COUNT(*) AS total_candidates,
        COUNT(*) FILTER (WHERE status <> 'rejected') AS open_candidates
    FROM rmp_match_candidates
    GROUP BY instructor_id
) c ON c.instructor_id = i.id;

ALTER TABLE instructors DROP COLUMN rmp_match_status;
