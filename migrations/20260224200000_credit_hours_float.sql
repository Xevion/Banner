-- Credit hours from the Banner API can be fractional (e.g. 1.25), so INTEGER
-- is too narrow. Widen all three columns to DOUBLE PRECISION.
ALTER TABLE courses
    ALTER COLUMN credit_hours     TYPE DOUBLE PRECISION,
    ALTER COLUMN credit_hour_low  TYPE DOUBLE PRECISION,
    ALTER COLUMN credit_hour_high TYPE DOUBLE PRECISION;
