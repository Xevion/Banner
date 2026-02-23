CREATE INDEX idx_instructors_display_name_trgm ON instructors USING GIN (display_name gin_trgm_ops);
