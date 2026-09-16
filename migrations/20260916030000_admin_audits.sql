-- Append-only trail of admin actions. Targets live in several tables, so an
-- entry names its entity by a type tag and a text id rather than a foreign key.

CREATE TABLE admin_audits (
    id BIGSERIAL PRIMARY KEY,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    -- Deliberately not a foreign key: the trail must outlive the acting user.
    actor_discord_id BIGINT NOT NULL,
    actor_username TEXT NOT NULL,
    action TEXT NOT NULL,
    entity_type TEXT NOT NULL,
    -- Null when the action swept a whole class of entities instead of one.
    entity_id TEXT,
    -- Other entities of the same type the action touched, such as a merge loser.
    related_ids TEXT[] NOT NULL DEFAULT '{}',
    detail JSONB NOT NULL DEFAULT '{}'::jsonb
);

CREATE INDEX idx_admin_audits_created_at ON admin_audits (created_at DESC);
CREATE INDEX idx_admin_audits_entity ON admin_audits (entity_type, entity_id);
CREATE INDEX idx_admin_audits_actor ON admin_audits (actor_discord_id);
CREATE INDEX idx_admin_audits_action ON admin_audits (action);
CREATE INDEX idx_admin_audits_related ON admin_audits USING gin (related_ids);
