ALTER TABLE bluebook_evaluations ADD COLUMN crn VARCHAR NOT NULL;
CREATE INDEX idx_bluebook_eval_crn_term ON bluebook_evaluations(crn, term);
