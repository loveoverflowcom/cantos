-- Version 22: optional Admin policy/audit. No production verifier is composed.
CREATE TABLE admin_access_revocations (
 actor_id TEXT PRIMARY KEY NOT NULL REFERENCES actors(id), revoked_at INTEGER NOT NULL CHECK(revoked_at>=0)
) STRICT;
CREATE TABLE admin_audit (
 id TEXT NOT NULL CHECK(coalesce(id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$',0)) PRIMARY KEY, actor_id TEXT REFERENCES actors(id), surface TEXT NOT NULL CHECK(surface='admin'),
 action TEXT NOT NULL CHECK(action IN ('capabilities','revoke_admin_access')), target_actor_id TEXT,
 outcome TEXT NOT NULL CHECK(outcome IN ('allowed','denied','replayed','conflict','unavailable')),
 occurred_at INTEGER NOT NULL CHECK(occurred_at>=0), correlation_id TEXT NOT NULL CHECK(coalesce(correlation_id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$',0)) UNIQUE
) STRICT;
CREATE TABLE admin_operations (
 actor_id TEXT NOT NULL REFERENCES actors(id), operation_id TEXT NOT NULL CHECK(coalesce(operation_id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$',0)), target_actor_id TEXT NOT NULL REFERENCES actors(id),
 audit_id TEXT NOT NULL REFERENCES admin_audit(id), PRIMARY KEY(actor_id,operation_id)
) STRICT;
CREATE TRIGGER admin_access_revocations_no_update BEFORE UPDATE ON admin_access_revocations
BEGIN SELECT RAISE(ABORT,'cantos:immutable.admin_receipt'); END;
CREATE TRIGGER admin_access_revocations_no_delete BEFORE DELETE ON admin_access_revocations
BEGIN SELECT RAISE(ABORT,'cantos:immutable.admin_receipt'); END;
CREATE TRIGGER admin_access_revocations_no_replace BEFORE INSERT ON admin_access_revocations
WHEN EXISTS(SELECT 1 FROM admin_access_revocations WHERE (actor_id=NEW.actor_id))
BEGIN SELECT RAISE(ABORT,'cantos:immutable.admin_receipt'); END;
CREATE TRIGGER admin_audit_no_update BEFORE UPDATE ON admin_audit
BEGIN SELECT RAISE(ABORT,'cantos:immutable.admin_receipt'); END;
CREATE TRIGGER admin_audit_no_delete BEFORE DELETE ON admin_audit
BEGIN SELECT RAISE(ABORT,'cantos:immutable.admin_receipt'); END;
CREATE TRIGGER admin_audit_no_replace BEFORE INSERT ON admin_audit
WHEN EXISTS(SELECT 1 FROM admin_audit WHERE (id=NEW.id) OR (correlation_id=NEW.correlation_id))
BEGIN SELECT RAISE(ABORT,'cantos:immutable.admin_receipt'); END;
CREATE TRIGGER admin_operations_no_update BEFORE UPDATE ON admin_operations
BEGIN SELECT RAISE(ABORT,'cantos:immutable.admin_receipt'); END;
CREATE TRIGGER admin_operations_no_delete BEFORE DELETE ON admin_operations
BEGIN SELECT RAISE(ABORT,'cantos:immutable.admin_receipt'); END;
CREATE TRIGGER admin_operations_no_replace BEFORE INSERT ON admin_operations
WHEN EXISTS(SELECT 1 FROM admin_operations WHERE (actor_id=NEW.actor_id AND operation_id=NEW.operation_id))
BEGIN SELECT RAISE(ABORT,'cantos:immutable.admin_receipt'); END;
