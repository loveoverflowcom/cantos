-- Version 19: Script production settings/rights, immutable frozen pins and approvals.
-- Process/file ownership replaces PG roles; serialized writes replace actor/member locks.
CREATE TABLE production_settings (
 script_id TEXT NOT NULL REFERENCES scripts(id), version INTEGER NOT NULL CHECK(version>0), script_revision INTEGER NOT NULL CHECK(script_revision>0),
 operation_id TEXT NOT NULL CHECK(coalesce(operation_id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$',0)), recorded_by TEXT NOT NULL REFERENCES actors(id), recorded_at INTEGER NOT NULL CHECK(recorded_at>=0),
 request_bytes BLOB NOT NULL CHECK(length(request_bytes) BETWEEN 1 AND 262144), settings_bytes BLOB NOT NULL CHECK(length(settings_bytes) BETWEEN 1 AND 262144), PRIMARY KEY(script_id,version),
 UNIQUE(script_id,recorded_by,operation_id), FOREIGN KEY(script_id,script_revision) REFERENCES script_revisions(script_id,revision)
) STRICT;
CREATE TABLE production_rights_claims (
 script_id TEXT NOT NULL REFERENCES scripts(id), owner_id TEXT NOT NULL, kind TEXT NOT NULL DEFAULT 'rights' CHECK(kind='rights'),
 record_id TEXT NOT NULL, version INTEGER NOT NULL CHECK(version>0), operation_id TEXT NOT NULL CHECK(coalesce(operation_id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$',0)),
 recorded_by TEXT NOT NULL REFERENCES actors(id), recorded_at INTEGER NOT NULL CHECK(recorded_at>=0), request_bytes BLOB NOT NULL CHECK(length(request_bytes) BETWEEN 1 AND 65536), claim_bytes BLOB NOT NULL CHECK(length(claim_bytes) BETWEEN 1 AND 65536),
 PRIMARY KEY(script_id,record_id,version), UNIQUE(script_id,recorded_by,operation_id),
 FOREIGN KEY(owner_id,kind,record_id) REFERENCES script_evidence(owner_id,kind,id)
) STRICT;
CREATE TABLE production_snapshots (
 id TEXT NOT NULL CHECK(coalesce(id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$',0)) PRIMARY KEY, script_id TEXT NOT NULL REFERENCES scripts(id), owner_id TEXT NOT NULL REFERENCES actors(id),
 script_revision INTEGER NOT NULL CHECK(script_revision>0), settings_version INTEGER NOT NULL CHECK(settings_version>0), operation_id TEXT NOT NULL CHECK(coalesce(operation_id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$',0)),
 created_by TEXT NOT NULL REFERENCES actors(id), recorded_at INTEGER NOT NULL CHECK(recorded_at>=0), input_digest TEXT NOT NULL CHECK(coalesce(input_digest REGEXP '^production-p1:sha256:[a-f0-9]{64}$',0)), request_bytes BLOB NOT NULL CHECK(length(request_bytes) BETWEEN 1 AND 65536), document_bytes BLOB NOT NULL CHECK(length(document_bytes) BETWEEN 1 AND 4194304),
 UNIQUE(script_id,created_by,operation_id), FOREIGN KEY(script_id,script_revision) REFERENCES script_revisions(script_id,revision),
 FOREIGN KEY(script_id,settings_version) REFERENCES production_settings(script_id,version)
) STRICT;
CREATE INDEX production_snapshot_history ON production_snapshots(script_id,recorded_at DESC,id DESC);
CREATE TABLE production_approvals (
 id TEXT NOT NULL CHECK(coalesce(id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$',0)) PRIMARY KEY, snapshot_id TEXT NOT NULL UNIQUE REFERENCES production_snapshots(id), script_id TEXT NOT NULL REFERENCES scripts(id),
 operation_id TEXT NOT NULL CHECK(coalesce(operation_id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$',0)), approved_by TEXT NOT NULL REFERENCES actors(id), approved_at INTEGER NOT NULL CHECK(approved_at>=0), input_digest TEXT NOT NULL CHECK(coalesce(input_digest REGEXP '^production-p1:sha256:[a-f0-9]{64}$',0)), request_bytes BLOB NOT NULL CHECK(length(request_bytes) BETWEEN 1 AND 65536),
 UNIQUE(script_id,approved_by,operation_id)
) STRICT;
CREATE TRIGGER production_settings_no_update BEFORE UPDATE ON production_settings
BEGIN SELECT RAISE(ABORT,'cantos:immutable.production_receipt'); END;
CREATE TRIGGER production_settings_no_delete BEFORE DELETE ON production_settings
BEGIN SELECT RAISE(ABORT,'cantos:immutable.production_receipt'); END;
CREATE TRIGGER production_settings_no_replace BEFORE INSERT ON production_settings
WHEN EXISTS(SELECT 1 FROM production_settings WHERE (script_id=NEW.script_id AND version=NEW.version) OR (script_id=NEW.script_id AND recorded_by=NEW.recorded_by AND operation_id=NEW.operation_id))
BEGIN SELECT RAISE(ABORT,'cantos:immutable.production_receipt'); END;
CREATE TRIGGER production_rights_claims_no_update BEFORE UPDATE ON production_rights_claims
BEGIN SELECT RAISE(ABORT,'cantos:immutable.production_receipt'); END;
CREATE TRIGGER production_rights_claims_no_delete BEFORE DELETE ON production_rights_claims
BEGIN SELECT RAISE(ABORT,'cantos:immutable.production_receipt'); END;
CREATE TRIGGER production_rights_claims_no_replace BEFORE INSERT ON production_rights_claims
WHEN EXISTS(SELECT 1 FROM production_rights_claims WHERE (script_id=NEW.script_id AND record_id=NEW.record_id AND version=NEW.version) OR (script_id=NEW.script_id AND recorded_by=NEW.recorded_by AND operation_id=NEW.operation_id))
BEGIN SELECT RAISE(ABORT,'cantos:immutable.production_receipt'); END;
CREATE TRIGGER production_snapshots_no_update BEFORE UPDATE ON production_snapshots
BEGIN SELECT RAISE(ABORT,'cantos:immutable.production_receipt'); END;
CREATE TRIGGER production_snapshots_no_delete BEFORE DELETE ON production_snapshots
BEGIN SELECT RAISE(ABORT,'cantos:immutable.production_receipt'); END;
CREATE TRIGGER production_snapshots_no_replace BEFORE INSERT ON production_snapshots
WHEN EXISTS(SELECT 1 FROM production_snapshots WHERE (id=NEW.id) OR (script_id=NEW.script_id AND created_by=NEW.created_by AND operation_id=NEW.operation_id))
BEGIN SELECT RAISE(ABORT,'cantos:immutable.production_receipt'); END;
CREATE TRIGGER production_approvals_no_update BEFORE UPDATE ON production_approvals
BEGIN SELECT RAISE(ABORT,'cantos:immutable.production_receipt'); END;
CREATE TRIGGER production_approvals_no_delete BEFORE DELETE ON production_approvals
BEGIN SELECT RAISE(ABORT,'cantos:immutable.production_receipt'); END;
CREATE TRIGGER production_approvals_no_replace BEFORE INSERT ON production_approvals
WHEN EXISTS(SELECT 1 FROM production_approvals WHERE (id=NEW.id) OR (snapshot_id=NEW.snapshot_id) OR (script_id=NEW.script_id AND approved_by=NEW.approved_by AND operation_id=NEW.operation_id))
BEGIN SELECT RAISE(ABORT,'cantos:immutable.production_receipt'); END;
