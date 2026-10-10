-- Initial schema: no prior deployed data. Forward-only; never rewrite applied migrations.
CREATE TABLE creators (
    id uuid PRIMARY KEY,
    disabled boolean NOT NULL DEFAULT false
);
CREATE TABLE creator_tokens (
    token_hash bytea PRIMARY KEY CHECK (octet_length(token_hash) = 32),
    creator_id uuid NOT NULL REFERENCES creators(id),
    expires_at timestamptz NOT NULL,
    revoked_at timestamptz
);
CREATE TABLE scripts (
    id uuid PRIMARY KEY,
    owner_id uuid NOT NULL REFERENCES creators(id),
    work_id text NOT NULL,
    episode_id text NOT NULL,
    head_version bigint NOT NULL CHECK (head_version > 0),
    create_operation_id uuid NOT NULL,
    create_request bytea NOT NULL,
    UNIQUE (owner_id, create_operation_id)
);
CREATE TABLE source_records (
    script_id uuid PRIMARY KEY REFERENCES scripts(id),
    id text NOT NULL,
    kind text NOT NULL CHECK (kind IN ('original', 'imported')),
    reference text NOT NULL CHECK (length(btrim(reference)) > 0),
    original_text text NOT NULL,
    sha256 text NOT NULL,
    actor_id uuid NOT NULL REFERENCES creators(id),
    created_at timestamptz NOT NULL DEFAULT clock_timestamp()
);
CREATE TABLE rights_records (
    script_id uuid PRIMARY KEY REFERENCES scripts(id),
    id text NOT NULL,
    holder text NOT NULL CHECK (length(btrim(holder)) > 0),
    evidence_reference text NOT NULL CHECK (length(btrim(evidence_reference)) > 0),
    status text NOT NULL DEFAULT 'pending_review' CHECK (status = 'pending_review'),
    actor_id uuid NOT NULL REFERENCES creators(id),
    created_at timestamptz NOT NULL DEFAULT clock_timestamp()
);
CREATE TABLE script_versions (
    script_id uuid NOT NULL REFERENCES scripts(id),
    version bigint NOT NULL CHECK (version > 0),
    base_version bigint NOT NULL CHECK (base_version = version - 1),
    operation_id uuid NOT NULL,
    actor_id uuid NOT NULL REFERENCES creators(id),
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    -- Exact canonical Script IR export bytes; jsonb would reorder keys and lose canonical form.
    document bytea NOT NULL,
    canonical bytea NOT NULL,
    document_digest bytea NOT NULL CHECK (octet_length(document_digest) = 32),
    digest text NOT NULL CHECK (digest ~ '^sir-c1:sha256:[a-f0-9]{64}$'),
    request bytea NOT NULL,
    PRIMARY KEY (script_id, version),
    UNIQUE (script_id, operation_id)
);
ALTER TABLE scripts ADD CONSTRAINT script_head_exists
    FOREIGN KEY (id, head_version) REFERENCES script_versions(script_id, version)
    DEFERRABLE INITIALLY DEFERRED;
CREATE TABLE script_revisions (
    id uuid PRIMARY KEY,
    script_id uuid NOT NULL,
    version bigint NOT NULL,
    operation_id uuid NOT NULL,
    actor_id uuid NOT NULL REFERENCES creators(id),
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    FOREIGN KEY (script_id, version) REFERENCES script_versions(script_id, version),
    UNIQUE (script_id, version)
);
-- Retrying an acceptance uses its original operation even after the head advances.
CREATE TABLE revision_operations (
    script_id uuid NOT NULL REFERENCES scripts(id),
    operation_id uuid NOT NULL,
    version bigint NOT NULL,
    revision_id uuid NOT NULL REFERENCES script_revisions(id),
    actor_id uuid NOT NULL REFERENCES creators(id),
    PRIMARY KEY (script_id, operation_id),
    FOREIGN KEY (script_id, version) REFERENCES script_versions(script_id, version)
);
CREATE FUNCTION reject_settled_change() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'settled record is immutable' USING ERRCODE = '23514';
END;
$$;
CREATE TRIGGER immutable_versions BEFORE UPDATE OR DELETE OR TRUNCATE ON script_versions
    FOR EACH STATEMENT EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER immutable_revisions BEFORE UPDATE OR DELETE OR TRUNCATE ON script_revisions
    FOR EACH STATEMENT EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER immutable_sources BEFORE UPDATE OR DELETE OR TRUNCATE ON source_records
    FOR EACH STATEMENT EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER immutable_rights BEFORE UPDATE OR DELETE OR TRUNCATE ON rights_records
    FOR EACH STATEMENT EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER immutable_revision_operations BEFORE UPDATE OR DELETE OR TRUNCATE ON revision_operations
    FOR EACH STATEMENT EXECUTE FUNCTION reject_settled_change();
-- The draft's head is its only mutable field.
CREATE FUNCTION guard_script_head() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF (NEW.id, NEW.owner_id, NEW.work_id, NEW.episode_id, NEW.create_operation_id, NEW.create_request)
       IS DISTINCT FROM
       (OLD.id, OLD.owner_id, OLD.work_id, OLD.episode_id, OLD.create_operation_id, OLD.create_request)
       OR NEW.head_version <> OLD.head_version + 1 THEN
        RAISE EXCEPTION 'only the next script head may be written' USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER guarded_head BEFORE UPDATE ON scripts
    FOR EACH ROW EXECUTE FUNCTION guard_script_head();
