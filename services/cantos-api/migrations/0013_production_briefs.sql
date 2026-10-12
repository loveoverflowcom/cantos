-- External instruction snapshots only. No generation jobs, media or release authority.
CREATE TABLE production_briefs (
    id text PRIMARY KEY CHECK(id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'),
    unit_id text NOT NULL REFERENCES catalog_units(id),
    content_revision_id text NOT NULL,
    content_sequence bigint NOT NULL CHECK(content_sequence>0),
    FOREIGN KEY(unit_id,content_revision_id,content_sequence) REFERENCES catalog_content_revisions(unit_id,id,revision),
    owner_id text NOT NULL REFERENCES actors(id),
    version bigint NOT NULL CHECK(version>0),
    operation_id text NOT NULL,
    request_digest text NOT NULL CHECK(request_digest ~ '^pb-request1:sha256:[a-f0-9]{64}$'),
    digest text NOT NULL CHECK(digest ~ '^pb-r1:sha256:[a-f0-9]{64}$'),
    snapshot bytea NOT NULL CHECK(octet_length(snapshot) BETWEEN 1 AND 262144),
    UNIQUE(unit_id,version), UNIQUE(owner_id,operation_id),
    UNIQUE(unit_id,id,version), UNIQUE(id,unit_id,owner_id,digest)
);
CREATE TABLE production_brief_heads (
    unit_id text PRIMARY KEY REFERENCES catalog_units(id),
    brief_id text NOT NULL,
    version bigint NOT NULL CHECK(version>0),
    FOREIGN KEY(unit_id,brief_id,version) REFERENCES production_briefs(unit_id,id,version)
);
CREATE FUNCTION production_brief_head_identity() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.unit_id<>OLD.unit_id OR NEW.version<>OLD.version+1 THEN
        RAISE EXCEPTION 'brief head advances one immutable version' USING ERRCODE='55000';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER production_brief_head_identity BEFORE UPDATE ON production_brief_heads
    FOR EACH ROW EXECUTE FUNCTION production_brief_head_identity();
CREATE TABLE production_producer_grants (
    id text PRIMARY KEY CHECK(id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'),
    unit_id text NOT NULL,
    brief_id text NOT NULL,
    owner_id text NOT NULL,
    brief_digest text NOT NULL,
    token_hash bytea NOT NULL UNIQUE CHECK(octet_length(token_hash)=32),
    expires_at bigint NOT NULL CHECK(expires_at>=0),
    created_at bigint NOT NULL CHECK(created_at>=0 AND expires_at>created_at AND expires_at-created_at<=86400000),
    operation_id text NOT NULL,
    request_digest text NOT NULL CHECK(request_digest ~ '^pb-grant1:sha256:[a-f0-9]{64}$'),
    UNIQUE(owner_id,operation_id),
    FOREIGN KEY(brief_id,unit_id,owner_id,brief_digest) REFERENCES production_briefs(id,unit_id,owner_id,digest)
);
CREATE TABLE production_producer_revocations (
    grant_id text PRIMARY KEY REFERENCES production_producer_grants(id),
    revoked_by text NOT NULL REFERENCES actors(id),
    revoked_at bigint NOT NULL CHECK(revoked_at>=0)
);
CREATE TRIGGER production_briefs_immutable BEFORE UPDATE OR DELETE ON production_briefs
    FOR EACH ROW EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER production_briefs_no_truncate BEFORE TRUNCATE ON production_briefs
    FOR EACH STATEMENT EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER production_grants_immutable BEFORE UPDATE OR DELETE ON production_producer_grants
    FOR EACH ROW EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER production_grants_no_truncate BEFORE TRUNCATE ON production_producer_grants
    FOR EACH STATEMENT EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER production_revocations_immutable BEFORE UPDATE OR DELETE ON production_producer_revocations
    FOR EACH ROW EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER production_revocations_no_truncate BEFORE TRUNCATE ON production_producer_revocations
    FOR EACH STATEMENT EXECUTE FUNCTION reject_settled_change();
REVOKE ALL ON production_briefs,production_brief_heads,production_producer_grants,production_producer_revocations FROM PUBLIC;
REVOKE ALL ON FUNCTION production_brief_head_identity() FROM PUBLIC;
