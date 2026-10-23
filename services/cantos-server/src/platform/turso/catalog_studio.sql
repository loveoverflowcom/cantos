-- Version 17: Studio Catalog decisions share the owner's serialized write unit.
-- Domain time remains Unix milliseconds. Settled receipts also guard REPLACE/upsert.
CREATE TABLE catalog_clock (
 singleton INTEGER PRIMARY KEY CHECK(singleton=1),
 revision INTEGER NOT NULL CHECK(revision>=0)
) STRICT;
INSERT INTO catalog_clock VALUES(1,0);
CREATE TRIGGER catalog_clock_no_delete BEFORE DELETE ON catalog_clock
BEGIN SELECT RAISE(ABORT,'cantos:immutable.catalog_clock'); END;
CREATE TRIGGER catalog_clock_no_replace BEFORE INSERT ON catalog_clock
WHEN EXISTS(SELECT 1 FROM catalog_clock WHERE singleton=NEW.singleton)
BEGIN SELECT RAISE(ABORT,'cantos:immutable.catalog_clock'); END;
CREATE TABLE catalog_members (
 work_id TEXT NOT NULL REFERENCES catalog_works(id),
 actor_id TEXT NOT NULL REFERENCES actors(id),
 role TEXT NOT NULL CHECK(role IN ('reader','editor')),
 PRIMARY KEY(work_id,actor_id)
) STRICT;
CREATE INDEX catalog_members_actor ON catalog_members(actor_id,work_id);
CREATE TABLE catalog_operations (
 actor_id TEXT NOT NULL REFERENCES actors(id),
 operation_id TEXT NOT NULL CHECK(coalesce(operation_id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$',0)),
 fingerprint TEXT NOT NULL CHECK(coalesce(fingerprint REGEXP '^[a-f0-9]{64}$',0)),
 result_digest TEXT NOT NULL CHECK(coalesce(result_digest REGEXP '^[a-f0-9]{64}$',0)),
 result_bytes BLOB NOT NULL CHECK(length(result_bytes) BETWEEN 1 AND 4194304),
 PRIMARY KEY(actor_id,operation_id)
) STRICT;
CREATE TABLE catalog_delegation_grants (
 id TEXT NOT NULL CHECK(coalesce(id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$',0)) PRIMARY KEY,
 work_id TEXT NOT NULL,
 grantor_id TEXT NOT NULL,
 delegate_id TEXT NOT NULL REFERENCES actors(id),
 work_read INTEGER NOT NULL CHECK(work_read IN (0,1)),
 content_read INTEGER NOT NULL CHECK(content_read IN (0,1)),
 content_write INTEGER NOT NULL CHECK(content_write IN (0,1)),
 token_hash BLOB NOT NULL UNIQUE CHECK(length(token_hash)=32),
 issued_at INTEGER NOT NULL CHECK(issued_at>=0),
 expires_at INTEGER NOT NULL,
 operation_id TEXT NOT NULL CHECK(coalesce(operation_id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$',0)),
 request_digest TEXT NOT NULL CHECK(coalesce(request_digest REGEXP '^dg-request1:sha256:[a-f0-9]{64}$',0)),
 UNIQUE(grantor_id,operation_id), UNIQUE(id,delegate_id),
 FOREIGN KEY(work_id,grantor_id) REFERENCES catalog_works(id,owner_id),
 CHECK(delegate_id<>grantor_id),
 CHECK(work_read=1 OR content_read=1 OR content_write=1),
 CHECK(expires_at>issued_at AND expires_at-issued_at<=2592000000)
) STRICT;
CREATE INDEX catalog_delegation_grants_work ON catalog_delegation_grants(work_id,id);
CREATE TABLE catalog_delegation_revocations (
 grant_id TEXT PRIMARY KEY NOT NULL REFERENCES catalog_delegation_grants(id),
 revoked_by TEXT NOT NULL REFERENCES actors(id),
 revoked_at INTEGER NOT NULL CHECK(revoked_at>=0)
) STRICT;
CREATE TABLE catalog_delegated_operations (
 grant_id TEXT NOT NULL,
 actor_id TEXT NOT NULL,
 operation_id TEXT NOT NULL,
 PRIMARY KEY(actor_id,operation_id),
 FOREIGN KEY(actor_id,operation_id) REFERENCES catalog_operations(actor_id,operation_id),
 FOREIGN KEY(grant_id,actor_id) REFERENCES catalog_delegation_grants(id,delegate_id)
) STRICT;
CREATE TRIGGER catalog_operations_no_update BEFORE UPDATE ON catalog_operations
BEGIN SELECT RAISE(ABORT,'cantos:immutable.catalog_receipt'); END;
CREATE TRIGGER catalog_operations_no_delete BEFORE DELETE ON catalog_operations
BEGIN SELECT RAISE(ABORT,'cantos:immutable.catalog_receipt'); END;
CREATE TRIGGER catalog_operations_no_replace BEFORE INSERT ON catalog_operations
WHEN EXISTS(SELECT 1 FROM catalog_operations WHERE (actor_id=NEW.actor_id AND operation_id=NEW.operation_id))
BEGIN SELECT RAISE(ABORT,'cantos:immutable.catalog_receipt'); END;
CREATE TRIGGER catalog_delegation_grants_no_update BEFORE UPDATE ON catalog_delegation_grants
BEGIN SELECT RAISE(ABORT,'cantos:immutable.catalog_receipt'); END;
CREATE TRIGGER catalog_delegation_grants_no_delete BEFORE DELETE ON catalog_delegation_grants
BEGIN SELECT RAISE(ABORT,'cantos:immutable.catalog_receipt'); END;
CREATE TRIGGER catalog_delegation_grants_no_replace BEFORE INSERT ON catalog_delegation_grants
WHEN EXISTS(SELECT 1 FROM catalog_delegation_grants WHERE (id=NEW.id) OR (token_hash=NEW.token_hash) OR (grantor_id=NEW.grantor_id AND operation_id=NEW.operation_id))
BEGIN SELECT RAISE(ABORT,'cantos:immutable.catalog_receipt'); END;
CREATE TRIGGER catalog_delegation_revocations_no_update BEFORE UPDATE ON catalog_delegation_revocations
BEGIN SELECT RAISE(ABORT,'cantos:immutable.catalog_receipt'); END;
CREATE TRIGGER catalog_delegation_revocations_no_delete BEFORE DELETE ON catalog_delegation_revocations
BEGIN SELECT RAISE(ABORT,'cantos:immutable.catalog_receipt'); END;
CREATE TRIGGER catalog_delegation_revocations_no_replace BEFORE INSERT ON catalog_delegation_revocations
WHEN EXISTS(SELECT 1 FROM catalog_delegation_revocations WHERE (grant_id=NEW.grant_id))
BEGIN SELECT RAISE(ABORT,'cantos:immutable.catalog_receipt'); END;
CREATE TRIGGER catalog_delegated_operations_no_update BEFORE UPDATE ON catalog_delegated_operations
BEGIN SELECT RAISE(ABORT,'cantos:immutable.catalog_receipt'); END;
CREATE TRIGGER catalog_delegated_operations_no_delete BEFORE DELETE ON catalog_delegated_operations
BEGIN SELECT RAISE(ABORT,'cantos:immutable.catalog_receipt'); END;
CREATE TRIGGER catalog_delegated_operations_no_replace BEFORE INSERT ON catalog_delegated_operations
WHEN EXISTS(SELECT 1 FROM catalog_delegated_operations WHERE (actor_id=NEW.actor_id AND operation_id=NEW.operation_id))
BEGIN SELECT RAISE(ABORT,'cantos:immutable.catalog_receipt'); END;
