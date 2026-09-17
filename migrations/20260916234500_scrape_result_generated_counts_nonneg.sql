-- The other three per-result counters already carry a non-negative CHECK. These
-- two are written from the same Count-typed values, so the omission was an
-- oversight, and the constraint is what lets the read type be Count as well.
ALTER TABLE scrape_job_results
    ADD CONSTRAINT chk_results_audits_generated_nonneg
    CHECK (audits_generated >= 0);

ALTER TABLE scrape_job_results
    ADD CONSTRAINT chk_results_metrics_generated_nonneg
    CHECK (metrics_generated >= 0);
