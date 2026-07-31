-- Leave free space on each courses page for in-place row updates.
--
-- The scraper rewrites enrollment and wait_count on every pass. Reserving space
-- lets a new row version stay on its original page where possible, which limits
-- how quickly the physical ordering by term_code decays between re-clusters.
--
-- Metadata-only: this takes effect for pages written after the change, so it
-- pairs with the CLUSTER run in maintenance rather than acting on its own.

ALTER TABLE courses SET (fillfactor = 85);
