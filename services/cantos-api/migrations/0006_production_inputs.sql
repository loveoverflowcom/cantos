-- Casting revisions and input authorization only: no dispatch, reservation or charge table.
-- SELECT ... FOR SHARE requires UPDATE privileges. These two fixed-query capabilities
-- acquire the needed locks without granting the application any authority-editing permission.
CREATE FUNCTION production_lock_actor(text) RETURNS boolean LANGUAGE sql
    SECURITY DEFINER SET search_path = pg_catalog AS $$
    SELECT a.active FROM public.actors a WHERE a.id=$1 FOR SHARE;
$$;
CREATE FUNCTION production_lock_member(text,text) RETURNS text LANGUAGE sql
    SECURITY DEFINER SET search_path = pg_catalog AS $$
    SELECT m.role FROM public.script_members m WHERE m.script_id=$1 AND m.actor_id=$2 FOR SHARE;
$$;
REVOKE ALL ON FUNCTION production_lock_actor(text),production_lock_member(text,text) FROM PUBLIC;
CREATE TABLE production_settings (
    script_id text NOT NULL REFERENCES scripts(id),
    version bigint NOT NULL CHECK (version > 0),
    script_revision bigint NOT NULL,
    operation_id text NOT NULL,
    recorded_by text NOT NULL REFERENCES actors(id),
    recorded_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    request_bytes bytea NOT NULL CHECK (octet_length(request_bytes) BETWEEN 1 AND 262144),
    settings_bytes bytea NOT NULL CHECK (octet_length(settings_bytes) BETWEEN 1 AND 262144),
    PRIMARY KEY (script_id, version),
    UNIQUE (script_id, recorded_by, operation_id),
    FOREIGN KEY (script_id, script_revision) REFERENCES script_revisions(script_id, revision)
);
CREATE TABLE production_rights_claims (
    script_id text NOT NULL REFERENCES scripts(id),
    owner_id text NOT NULL,
    kind text NOT NULL DEFAULT 'rights' CHECK (kind = 'rights'),
    record_id text NOT NULL,
    version bigint NOT NULL CHECK (version > 0),
    operation_id text NOT NULL,
    recorded_by text NOT NULL REFERENCES actors(id),
    recorded_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    request_bytes bytea NOT NULL CHECK (octet_length(request_bytes) BETWEEN 1 AND 65536),
    claim_bytes bytea NOT NULL CHECK (octet_length(claim_bytes) BETWEEN 1 AND 65536),
    PRIMARY KEY (script_id, record_id, version),
    UNIQUE (script_id, recorded_by, operation_id),
    FOREIGN KEY (owner_id, kind, record_id) REFERENCES script_evidence(owner_id, kind, id)
);
CREATE TABLE production_snapshots (
    id text PRIMARY KEY,
    script_id text NOT NULL REFERENCES scripts(id),
    owner_id text NOT NULL REFERENCES actors(id),
    script_revision bigint NOT NULL,
    settings_version bigint NOT NULL,
    operation_id text NOT NULL,
    created_by text NOT NULL REFERENCES actors(id),
    recorded_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    input_digest text NOT NULL CHECK (input_digest ~ '^production-p1:sha256:[a-f0-9]{64}$'),
    request_bytes bytea NOT NULL CHECK (octet_length(request_bytes) BETWEEN 1 AND 65536),
    document_bytes bytea NOT NULL CHECK (octet_length(document_bytes) BETWEEN 1 AND 4194304),
    UNIQUE (script_id, created_by, operation_id),
    FOREIGN KEY (script_id, script_revision) REFERENCES script_revisions(script_id, revision),
    FOREIGN KEY (script_id, settings_version) REFERENCES production_settings(script_id, version)
);
CREATE INDEX production_snapshot_history ON production_snapshots(script_id, recorded_at DESC, id DESC);
CREATE TABLE production_approvals (
    id text PRIMARY KEY,
    snapshot_id text NOT NULL UNIQUE REFERENCES production_snapshots(id),
    script_id text NOT NULL REFERENCES scripts(id),
    operation_id text NOT NULL,
    approved_by text NOT NULL REFERENCES actors(id),
    approved_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    input_digest text NOT NULL CHECK (input_digest ~ '^production-p1:sha256:[a-f0-9]{64}$'),
    request_bytes bytea NOT NULL CHECK (octet_length(request_bytes) BETWEEN 1 AND 65536),
    UNIQUE (script_id, approved_by, operation_id)
);
CREATE TRIGGER production_settings_immutable BEFORE UPDATE OR DELETE ON production_settings
    FOR EACH ROW EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER production_settings_no_truncate BEFORE TRUNCATE ON production_settings
    FOR EACH STATEMENT EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER production_rights_immutable BEFORE UPDATE OR DELETE ON production_rights_claims
    FOR EACH ROW EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER production_rights_no_truncate BEFORE TRUNCATE ON production_rights_claims
    FOR EACH STATEMENT EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER production_snapshots_immutable BEFORE UPDATE OR DELETE ON production_snapshots
    FOR EACH ROW EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER production_snapshots_no_truncate BEFORE TRUNCATE ON production_snapshots
    FOR EACH STATEMENT EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER production_approvals_immutable BEFORE UPDATE OR DELETE ON production_approvals
    FOR EACH ROW EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER production_approvals_no_truncate BEFORE TRUNCATE ON production_approvals
    FOR EACH STATEMENT EXECUTE FUNCTION reject_settled_change();
REVOKE ALL ON production_settings,production_rights_claims,production_snapshots,production_approvals FROM PUBLIC;
