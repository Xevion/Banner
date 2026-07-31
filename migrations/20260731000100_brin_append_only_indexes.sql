-- BRIN indexes for the append-only audit and metric tables.
--
-- Both tables are written in timestamp order and never updated, giving a
-- physical correlation above 0.9999. BRIN stores one summary per block range
-- instead of one entry per row, replacing an 18MB btree with a few hundred
-- kilobytes while still pruning effectively for time-bounded scans.

CREATE INDEX idx_course_audits_timestamp_brin
    ON course_audits USING brin ("timestamp") WITH (pages_per_range = 32);

-- Superseded by the BRIN index above; the btree served 25 scans for 18MB.
DROP INDEX idx_course_audits_timestamp;

-- course_metrics previously had no timestamp index at all.
CREATE INDEX idx_course_metrics_timestamp_brin
    ON course_metrics USING brin ("timestamp") WITH (pages_per_range = 32);
