-- Ephemeral key-value store for application state across restarts.
-- Used by the scheduler (last sync timestamps) and bot (command fingerprint).
-- UNLOGGED: no WAL overhead; truncated on DB crash recovery, which is fine —
-- losing this data just means periodic tasks re-run on next startup.
CREATE UNLOGGED TABLE app_kv (
    key        TEXT        PRIMARY KEY,
    value      TEXT        NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
