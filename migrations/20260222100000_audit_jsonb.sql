-- Convert course_audits old_value/new_value from TEXT to JSONB.
-- Existing data is a mix of valid JSON and plain scalars (e.g. "42", "CS").
-- The helper tries a direct cast first, falling back to to_jsonb()
-- which wraps plain strings as JSON strings.

-- old_value: empty string → SQL NULL (initial inserts have no previous value)
CREATE FUNCTION _old_val_to_jsonb(val text) RETURNS jsonb
    LANGUAGE plpgsql IMMUTABLE AS $$
BEGIN
    IF val = '' THEN RETURN NULL; END IF;
    RETURN val::jsonb;
EXCEPTION WHEN OTHERS THEN
    RETURN to_jsonb(val);
END;
$$;

-- new_value: empty string → JSONB null literal (column stays NOT NULL)
CREATE FUNCTION _new_val_to_jsonb(val text) RETURNS jsonb
    LANGUAGE plpgsql IMMUTABLE AS $$
BEGIN
    IF val = '' THEN RETURN 'null'::jsonb; END IF;
    RETURN val::jsonb;
EXCEPTION WHEN OTHERS THEN
    RETURN to_jsonb(val);
END;
$$;

ALTER TABLE course_audits
    ALTER COLUMN old_value TYPE jsonb USING _old_val_to_jsonb(old_value),
    ALTER COLUMN old_value DROP NOT NULL,
    ALTER COLUMN new_value TYPE jsonb USING _new_val_to_jsonb(new_value);

DROP FUNCTION _old_val_to_jsonb;
DROP FUNCTION _new_val_to_jsonb;
