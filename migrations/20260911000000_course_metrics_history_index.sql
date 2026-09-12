-- Per-course history lookups filter on course_id and sort by timestamp desc;
-- the BRIN on timestamp alone can't serve that.

CREATE INDEX idx_course_metrics_course_timestamp
    ON course_metrics (course_id, "timestamp" DESC);

ANALYZE course_metrics;
