-- Additive after Catalog 0011, independent of the separately delivered audio intake 0010.
-- Existing metadata snapshots, source bytes and Script IR exports are never rewritten.
CREATE TABLE catalog_content_revisions (
    id text PRIMARY KEY CHECK (id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'),
    unit_id text NOT NULL REFERENCES catalog_units(id),
    revision bigint NOT NULL CHECK (revision > 0),
    snapshot bytea NOT NULL CHECK (octet_length(snapshot) BETWEEN 1 AND 131072),
    revision_digest text NOT NULL CHECK (revision_digest ~ '^cc-r1:sha256:[a-f0-9]{64}$'),
    recorded_by text NOT NULL REFERENCES actors(id),
    UNIQUE(unit_id,revision),
    UNIQUE(unit_id,id,revision)
);
CREATE TABLE catalog_content_heads (
    unit_id text PRIMARY KEY REFERENCES catalog_units(id),
    revision_id text NOT NULL,
    revision bigint NOT NULL CHECK (revision > 0),
    FOREIGN KEY(unit_id,revision_id,revision) REFERENCES catalog_content_revisions(unit_id,id,revision)
);
CREATE FUNCTION catalog_content_head_identity() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.unit_id<>OLD.unit_id THEN
        RAISE EXCEPTION 'content head identity is immutable' USING ERRCODE='55000';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER catalog_content_head_identity BEFORE UPDATE ON catalog_content_heads
    FOR EACH ROW EXECUTE FUNCTION catalog_content_head_identity();
CREATE TRIGGER catalog_content_history_immutable BEFORE UPDATE OR DELETE ON catalog_content_revisions
    FOR EACH ROW EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER catalog_content_history_no_truncate BEFORE TRUNCATE ON catalog_content_revisions
    FOR EACH STATEMENT EXECUTE FUNCTION reject_settled_change();
REVOKE ALL ON catalog_content_revisions,catalog_content_heads FROM PUBLIC;
REVOKE ALL ON FUNCTION catalog_content_head_identity() FROM PUBLIC;
