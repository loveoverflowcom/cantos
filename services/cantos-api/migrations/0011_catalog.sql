-- Catalog foundation. Version 0010 is reserved by the independent audio-intake change.
-- This migration is independently applicable after 0009; no audio table is assumed.
CREATE TABLE catalog_clock (
    singleton boolean PRIMARY KEY DEFAULT true CHECK (singleton),
    revision bigint NOT NULL CHECK (revision >= 0)
);
INSERT INTO catalog_clock VALUES(true,0);
CREATE TABLE catalog_works (
    id text PRIMARY KEY CHECK (id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'),
    owner_id text NOT NULL REFERENCES actors(id),
    kind text NOT NULL CHECK (kind IN ('novel','story','podcast')),
    lifecycle text NOT NULL CHECK (lifecycle IN ('draft','archived')),
    head_revision bigint NOT NULL CHECK (head_revision > 0),
    UNIQUE(id,kind)
);
CREATE TABLE catalog_work_revisions (
    work_id text NOT NULL REFERENCES catalog_works(id),
    revision bigint NOT NULL CHECK (revision > 0),
    snapshot bytea NOT NULL CHECK (octet_length(snapshot) BETWEEN 1 AND 131072),
    snapshot_digest text NOT NULL CHECK (snapshot_digest ~ '^[a-f0-9]{64}$'),
    recorded_by text NOT NULL REFERENCES actors(id),
    PRIMARY KEY(work_id,revision)
);
ALTER TABLE catalog_works ADD CONSTRAINT catalog_work_head_exists
    FOREIGN KEY(id,head_revision) REFERENCES catalog_work_revisions(work_id,revision)
    DEFERRABLE INITIALLY DEFERRED;
CREATE TABLE catalog_units (
    id text PRIMARY KEY CHECK (id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'),
    work_id text NOT NULL,
    work_kind text NOT NULL,
    kind text NOT NULL CHECK (kind IN ('chapter','episode')),
    head_revision bigint NOT NULL CHECK (head_revision > 0),
    position integer NOT NULL CHECK (position >= 0),
    lifecycle text NOT NULL CHECK (lifecycle IN ('draft','archived')),
    FOREIGN KEY(work_id,work_kind) REFERENCES catalog_works(id,kind),
    CHECK ((work_kind IN ('novel','story') AND kind='chapter') OR (work_kind='podcast' AND kind='episode')),
    UNIQUE(work_id,position) DEFERRABLE INITIALLY DEFERRED
);
CREATE TABLE catalog_unit_revisions (
    unit_id text NOT NULL REFERENCES catalog_units(id),
    revision bigint NOT NULL CHECK (revision > 0),
    snapshot bytea NOT NULL CHECK (octet_length(snapshot) BETWEEN 1 AND 131072),
    snapshot_digest text NOT NULL CHECK (snapshot_digest ~ '^[a-f0-9]{64}$'),
    recorded_by text NOT NULL REFERENCES actors(id),
    PRIMARY KEY(unit_id,revision)
);
ALTER TABLE catalog_units ADD CONSTRAINT catalog_unit_head_exists
    FOREIGN KEY(id,head_revision) REFERENCES catalog_unit_revisions(unit_id,revision)
    DEFERRABLE INITIALLY DEFERRED;
CREATE TABLE catalog_members (
    work_id text NOT NULL REFERENCES catalog_works(id),
    actor_id text NOT NULL REFERENCES actors(id),
    role text NOT NULL CHECK(role IN ('reader','editor')),
    PRIMARY KEY(work_id,actor_id)
);
CREATE TABLE catalog_operations (
    actor_id text NOT NULL REFERENCES actors(id),
    operation_id text NOT NULL CHECK (operation_id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'),
    fingerprint text NOT NULL CHECK (fingerprint ~ '^[a-f0-9]{64}$'),
    result_digest text NOT NULL CHECK (result_digest ~ '^[a-f0-9]{64}$'),
    result_bytes bytea NOT NULL CHECK (octet_length(result_bytes) BETWEEN 1 AND 4194304),
    PRIMARY KEY(actor_id,operation_id)
);
CREATE INDEX catalog_works_owner ON catalog_works(owner_id,id);
CREATE INDEX catalog_members_actor ON catalog_members(actor_id,work_id);
CREATE INDEX catalog_units_order ON catalog_units(work_id,position,id);
CREATE FUNCTION catalog_lock_actor(actor text) RETURNS boolean LANGUAGE sql SECURITY DEFINER
    SET search_path=pg_catalog AS $$ SELECT active FROM public.actors WHERE id=actor FOR SHARE $$;
CREATE FUNCTION catalog_lock_member(work text,actor text) RETURNS text LANGUAGE sql SECURITY DEFINER
    SET search_path=pg_catalog AS $$ SELECT role FROM public.catalog_members WHERE work_id=work AND actor_id=actor FOR SHARE $$;
CREATE FUNCTION catalog_identity_immutable() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_TABLE_NAME='catalog_works' THEN
        IF NEW.id<>OLD.id OR NEW.owner_id<>OLD.owner_id OR NEW.kind<>OLD.kind THEN
            RAISE EXCEPTION 'catalog identity is immutable' USING ERRCODE='55000';
        END IF;
    ELSE
        IF NEW.id<>OLD.id OR NEW.work_id<>OLD.work_id OR NEW.work_kind<>OLD.work_kind OR NEW.kind<>OLD.kind THEN
            RAISE EXCEPTION 'catalog identity is immutable' USING ERRCODE='55000';
        END IF;
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER catalog_work_identity BEFORE UPDATE ON catalog_works FOR EACH ROW EXECUTE FUNCTION catalog_identity_immutable();
CREATE TRIGGER catalog_unit_identity BEFORE UPDATE ON catalog_units FOR EACH ROW EXECUTE FUNCTION catalog_identity_immutable();
CREATE TRIGGER catalog_work_history_immutable BEFORE UPDATE OR DELETE ON catalog_work_revisions FOR EACH ROW EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER catalog_work_history_no_truncate BEFORE TRUNCATE ON catalog_work_revisions FOR EACH STATEMENT EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER catalog_unit_history_immutable BEFORE UPDATE OR DELETE ON catalog_unit_revisions FOR EACH ROW EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER catalog_unit_history_no_truncate BEFORE TRUNCATE ON catalog_unit_revisions FOR EACH STATEMENT EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER catalog_operations_immutable BEFORE UPDATE OR DELETE ON catalog_operations FOR EACH ROW EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER catalog_operations_no_truncate BEFORE TRUNCATE ON catalog_operations FOR EACH STATEMENT EXECUTE FUNCTION reject_settled_change();
REVOKE ALL ON catalog_clock,catalog_works,catalog_work_revisions,catalog_units,catalog_unit_revisions,catalog_members,catalog_operations FROM PUBLIC;
REVOKE ALL ON FUNCTION catalog_lock_actor(text),catalog_lock_member(text,text),catalog_identity_immutable() FROM PUBLIC;
