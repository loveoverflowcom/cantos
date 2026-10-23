-- Version 21: versioned brief pointers and scoped producer instructions, separate from sessions.
CREATE TABLE production_brief_heads (
 unit_id TEXT PRIMARY KEY NOT NULL REFERENCES catalog_units(id), brief_id TEXT NOT NULL, version INTEGER NOT NULL CHECK(version>0),
 FOREIGN KEY(unit_id,brief_id,version) REFERENCES production_briefs(unit_id,id,version)
) STRICT;
CREATE TRIGGER production_brief_heads_identity BEFORE UPDATE ON production_brief_heads
WHEN NEW.unit_id<>OLD.unit_id OR NEW.version<>OLD.version+1
BEGIN SELECT RAISE(ABORT,'cantos:immutable.brief_head'); END;
CREATE TABLE production_producer_grants (
 id TEXT NOT NULL CHECK(coalesce(id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$',0)) PRIMARY KEY, unit_id TEXT NOT NULL, brief_id TEXT NOT NULL, owner_id TEXT NOT NULL,
 brief_digest TEXT NOT NULL CHECK(coalesce(brief_digest REGEXP '^pb-r1:sha256:[a-f0-9]{64}$',0)), token_hash BLOB NOT NULL UNIQUE CHECK(length(token_hash)=32),
 expires_at INTEGER NOT NULL CHECK(expires_at>=0), created_at INTEGER NOT NULL CHECK(created_at>=0),
 operation_id TEXT NOT NULL CHECK(coalesce(operation_id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$',0)), request_digest TEXT NOT NULL CHECK(coalesce(request_digest REGEXP '^pb-grant1:sha256:[a-f0-9]{64}$',0)),
 CHECK(expires_at>created_at AND expires_at-created_at<=86400000), UNIQUE(owner_id,operation_id),
 FOREIGN KEY(brief_id,unit_id,owner_id,brief_digest) REFERENCES production_briefs(id,unit_id,owner_id,digest)
) STRICT;
CREATE TABLE production_producer_revocations (
 grant_id TEXT PRIMARY KEY NOT NULL REFERENCES production_producer_grants(id), revoked_by TEXT NOT NULL REFERENCES actors(id), revoked_at INTEGER NOT NULL CHECK(revoked_at>=0)
) STRICT;
CREATE TRIGGER production_brief_heads_no_delete BEFORE DELETE ON production_brief_heads
BEGIN SELECT RAISE(ABORT,'cantos:immutable.producer_grant'); END;
CREATE TRIGGER production_brief_heads_no_replace BEFORE INSERT ON production_brief_heads
WHEN EXISTS(SELECT 1 FROM production_brief_heads WHERE (unit_id=NEW.unit_id))
BEGIN SELECT RAISE(ABORT,'cantos:immutable.producer_grant'); END;
CREATE TRIGGER production_producer_grants_no_update BEFORE UPDATE ON production_producer_grants
BEGIN SELECT RAISE(ABORT,'cantos:immutable.producer_grant'); END;
CREATE TRIGGER production_producer_grants_no_delete BEFORE DELETE ON production_producer_grants
BEGIN SELECT RAISE(ABORT,'cantos:immutable.producer_grant'); END;
CREATE TRIGGER production_producer_grants_no_replace BEFORE INSERT ON production_producer_grants
WHEN EXISTS(SELECT 1 FROM production_producer_grants WHERE (id=NEW.id) OR (token_hash=NEW.token_hash) OR (owner_id=NEW.owner_id AND operation_id=NEW.operation_id))
BEGIN SELECT RAISE(ABORT,'cantos:immutable.producer_grant'); END;
CREATE TRIGGER production_producer_revocations_no_update BEFORE UPDATE ON production_producer_revocations
BEGIN SELECT RAISE(ABORT,'cantos:immutable.producer_grant'); END;
CREATE TRIGGER production_producer_revocations_no_delete BEFORE DELETE ON production_producer_revocations
BEGIN SELECT RAISE(ABORT,'cantos:immutable.producer_grant'); END;
CREATE TRIGGER production_producer_revocations_no_replace BEFORE INSERT ON production_producer_revocations
WHEN EXISTS(SELECT 1 FROM production_producer_revocations WHERE (grant_id=NEW.grant_id))
BEGIN SELECT RAISE(ABORT,'cantos:immutable.producer_grant'); END;
