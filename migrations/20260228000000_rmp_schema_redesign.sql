-- Add composite unique on instructor_rmp_links to prevent duplicate pairs.
-- The existing UNIQUE(rmp_legacy_id) prevents one RMP prof linking to multiple
-- instructors; this prevents the same (instructor, rmp_prof) pair twice.
ALTER TABLE instructor_rmp_links
  ADD CONSTRAINT uq_instructor_rmp_link UNIQUE (instructor_id, rmp_legacy_id);

-- Materialized view for aggregated RMP data across all linked profiles.
-- Public queries join against this instead of per-row LATERAL subqueries.
-- Refresh explicitly after link mutations and professor rating syncs.
CREATE MATERIALIZED VIEW instructor_rmp_summary AS
SELECT
    l.instructor_id,
    CASE WHEN SUM(rp.num_ratings) > 0
         THEN SUM(rp.avg_rating * rp.num_ratings) / SUM(rp.num_ratings)
         ELSE NULL
    END AS avg_rating,
    CASE WHEN SUM(rp.num_ratings) > 0
         THEN SUM(rp.avg_difficulty * rp.num_ratings) / SUM(rp.num_ratings)
         ELSE NULL
    END AS avg_difficulty,
    CASE WHEN SUM(rp.num_ratings) FILTER (WHERE rp.would_take_again_pct IS NOT NULL) > 0
         THEN SUM(rp.would_take_again_pct * rp.num_ratings)
              FILTER (WHERE rp.would_take_again_pct IS NOT NULL)
              / SUM(rp.num_ratings) FILTER (WHERE rp.would_take_again_pct IS NOT NULL)
         ELSE NULL
    END AS would_take_again_pct,
    SUM(rp.num_ratings)::integer AS num_ratings,
    (ARRAY_AGG(rp.legacy_id ORDER BY rp.num_ratings DESC NULLS LAST, rp.legacy_id ASC))[1]
        AS primary_legacy_id,
    COUNT(*)::integer AS profile_count
FROM instructor_rmp_links l
JOIN rmp_professors rp ON rp.legacy_id = l.rmp_legacy_id
GROUP BY l.instructor_id;

CREATE UNIQUE INDEX idx_rmp_summary_instructor ON instructor_rmp_summary (instructor_id);
