-- Private, script-revision-bound Studio development intake only.
-- Media bytes live in explicitly configured private object storage, never PostgreSQL.
-- This is not a catalog, external producer credential, review or publication authority.
CREATE TABLE audio_upload_intents (
    id text PRIMARY KEY CHECK (id ~ '^[a-f0-9-]{36}$'),
    script_id text NOT NULL REFERENCES scripts(id),
    script_revision bigint NOT NULL CHECK (script_revision > 0),
    content_digest text NOT NULL CHECK (content_digest ~ '^sir-c1:sha256:[a-f0-9]{64}$'),
    profile_id text NOT NULL CHECK (profile_id IN ('opus-mono-20-v1','opus-mono-24-v1','opus-mono-32-v1','opus-mono-40-v1')),
    operation_id text NOT NULL CHECK (operation_id ~ '^[a-f0-9-]{36}$'),
    created_by text NOT NULL REFERENCES actors(id),
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    UNIQUE (created_by, operation_id),
    UNIQUE (id, script_id, script_revision, content_digest, profile_id, operation_id, created_by),
    FOREIGN KEY (script_id, script_revision) REFERENCES script_revisions(script_id, revision)
);
-- Second barrier: an intent cannot pin a caller-invented digest or another owner's script.
CREATE FUNCTION check_audio_intent_binding() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM script_revisions r JOIN scripts s ON s.id=r.script_id
        WHERE r.script_id=NEW.script_id AND r.revision=NEW.script_revision
          AND r.content_digest=NEW.content_digest AND s.owner_id=NEW.created_by
    ) THEN
        RAISE EXCEPTION 'invalid audio intent binding' USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER audio_intent_binding BEFORE INSERT ON audio_upload_intents
    FOR EACH ROW EXECUTE FUNCTION check_audio_intent_binding();
CREATE TABLE audio_intake_receipts (
    id text PRIMARY KEY CHECK (id ~ '^[a-f0-9-]{36}$'),
    intent_id text NOT NULL UNIQUE,
    script_id text NOT NULL,
    script_revision bigint NOT NULL,
    content_digest text NOT NULL,
    profile_id text NOT NULL,
    operation_id text NOT NULL,
    accepted_by text NOT NULL,
    accepted_at timestamptz NOT NULL,
    object_key text NOT NULL CHECK (object_key ~ '^[a-f0-9]{64}$'),
    receipt_bytes bytea NOT NULL CHECK (octet_length(receipt_bytes) BETWEEN 1 AND 8192),
    receipt_digest text NOT NULL CHECK (receipt_digest ~ '^audio-intake-r1:sha256:[a-f0-9]{64}$'),
    FOREIGN KEY (intent_id, script_id, script_revision, content_digest, profile_id, operation_id, accepted_by)
      REFERENCES audio_upload_intents(id, script_id, script_revision, content_digest, profile_id, operation_id, created_by)
);
CREATE TRIGGER audio_intents_immutable BEFORE UPDATE OR DELETE ON audio_upload_intents
    FOR EACH ROW EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER audio_intents_no_truncate BEFORE TRUNCATE ON audio_upload_intents
    FOR EACH STATEMENT EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER audio_receipts_immutable BEFORE UPDATE OR DELETE ON audio_intake_receipts
    FOR EACH ROW EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER audio_receipts_no_truncate BEFORE TRUNCATE ON audio_intake_receipts
    FOR EACH STATEMENT EXECUTE FUNCTION reject_settled_change();
REVOKE ALL ON audio_upload_intents,audio_intake_receipts FROM PUBLIC;
REVOKE ALL ON FUNCTION check_audio_intent_binding() FROM PUBLIC;
