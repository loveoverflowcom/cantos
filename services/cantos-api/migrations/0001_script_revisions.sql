-- First schema. Applied transactionally by cantos-migrate; never edits accepted rows.
CREATE TABLE actors (
    id text PRIMARY KEY CHECK (id ~ '^[a-z0-9_-]{1,64}$'),
    active boolean NOT NULL DEFAULT true
);
CREATE TABLE sessions (
    token_hash bytea PRIMARY KEY CHECK (octet_length(token_hash) = 32),
    actor_id text NOT NULL REFERENCES actors(id),
    expires_at timestamptz NOT NULL,
    revoked boolean NOT NULL DEFAULT false
);
CREATE TABLE scripts (
    id text PRIMARY KEY,
    owner_id text NOT NULL REFERENCES actors(id),
    head_revision bigint NOT NULL DEFAULT 0 CHECK (head_revision >= 0)
);
CREATE TABLE script_members (
    script_id text NOT NULL REFERENCES scripts(id),
    actor_id text NOT NULL REFERENCES actors(id),
    role text NOT NULL CHECK (role IN ('reader', 'editor')),
    PRIMARY KEY (script_id, actor_id)
);
-- Registry binds opaque external record references to their owning account.
-- Existence/ownership does not establish production or publication rights eligibility.
CREATE TABLE script_evidence (
    owner_id text NOT NULL REFERENCES actors(id),
    kind text NOT NULL CHECK (kind IN ('source', 'rights', 'generation', 'asset')),
    id text NOT NULL CHECK (id ~ '^[a-z0-9_-]{1,64}$'),
    description text NOT NULL,
    PRIMARY KEY (owner_id, kind, id)
);
CREATE TABLE script_revisions (
    script_id text NOT NULL REFERENCES scripts(id),
    revision bigint NOT NULL CHECK (revision > 0),
    expected_revision bigint NOT NULL CHECK (expected_revision = revision - 1),
    operation_id text NOT NULL,
    accepted_by text NOT NULL REFERENCES actors(id),
    accepted_at timestamptz NOT NULL DEFAULT CURRENT_TIMESTAMP,
    schema_version text NOT NULL CHECK (schema_version = '0.1.0'),
    canonical_export bytea NOT NULL CHECK (octet_length(canonical_export) BETWEEN 1 AND 2097152),
    content_digest text NOT NULL CHECK (content_digest ~ '^sir-c1:sha256:[a-f0-9]{64}$'),
    export_digest text NOT NULL CHECK (export_digest ~ '^sir-e1:sha256:[a-f0-9]{64}$'),
    PRIMARY KEY (script_id, revision),
    UNIQUE (script_id, accepted_by, operation_id)
);
ALTER TABLE scripts ADD CONSTRAINT script_head_exists
    FOREIGN KEY (id, head_revision) REFERENCES script_revisions(script_id, revision)
    DEFERRABLE INITIALLY DEFERRED;
-- A head of zero exists only transiently during first-save transaction.
CREATE TABLE revision_evidence (
    script_id text NOT NULL,
    revision bigint NOT NULL,
    owner_id text NOT NULL,
    kind text NOT NULL,
    evidence_id text NOT NULL,
    PRIMARY KEY (script_id, revision, kind, evidence_id),
    FOREIGN KEY (script_id, revision) REFERENCES script_revisions(script_id, revision),
    FOREIGN KEY (owner_id, kind, evidence_id) REFERENCES script_evidence(owner_id, kind, id)
);
CREATE FUNCTION reject_settled_change() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'settled records are immutable' USING ERRCODE = '55000';
END;
$$;
CREATE TRIGGER revisions_immutable BEFORE UPDATE OR DELETE ON script_revisions
    FOR EACH ROW EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER revisions_no_truncate BEFORE TRUNCATE ON script_revisions
    FOR EACH STATEMENT EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER evidence_immutable BEFORE UPDATE OR DELETE ON revision_evidence
    FOR EACH ROW EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER evidence_no_truncate BEFORE TRUNCATE ON revision_evidence
    FOR EACH STATEMENT EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER registry_immutable BEFORE UPDATE OR DELETE ON script_evidence
    FOR EACH ROW EXECUTE FUNCTION reject_settled_change();
REVOKE ALL ON ALL TABLES IN SCHEMA public FROM PUBLIC;
REVOKE ALL ON FUNCTION reject_settled_change() FROM PUBLIC;
