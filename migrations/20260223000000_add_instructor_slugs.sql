ALTER TABLE instructors ADD COLUMN slug TEXT;

CREATE UNIQUE INDEX idx_instructors_slug ON instructors (slug) WHERE slug IS NOT NULL;
