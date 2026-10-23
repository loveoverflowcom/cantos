-- #333: append after the registered placeholders and CMP closure. No applied bytes change.
-- All timestamps are trusted transaction-clock microseconds, formatted by the adapter.
CREATE TABLE scripts (
    id TEXT PRIMARY KEY NOT NULL CHECK (coalesce(id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$', 0)),
    owner_id TEXT NOT NULL REFERENCES actors(id),
    head_revision INTEGER NOT NULL DEFAULT 0 CHECK (head_revision >= 0),
    FOREIGN KEY (id, head_revision) REFERENCES script_revisions(script_id, revision)
        DEFERRABLE INITIALLY DEFERRED
) STRICT;

CREATE TABLE script_members (
    script_id TEXT NOT NULL REFERENCES scripts(id),
    actor_id TEXT NOT NULL REFERENCES actors(id),
    role TEXT NOT NULL CHECK (role IN ('reader','editor')),
    PRIMARY KEY (script_id, actor_id)
) STRICT;

CREATE TABLE script_evidence (
    owner_id TEXT NOT NULL REFERENCES actors(id),
    kind TEXT NOT NULL CHECK (kind IN ('source','rights','generation','asset')),
    id TEXT NOT NULL CHECK (coalesce(id REGEXP '^[a-z0-9_-]{1,64}$', 0)),
    description TEXT NOT NULL,
    PRIMARY KEY (owner_id, kind, id)
) STRICT;

CREATE TABLE script_revisions (
    script_id TEXT NOT NULL REFERENCES scripts(id),
    revision INTEGER NOT NULL CHECK (revision > 0),
    expected_revision INTEGER NOT NULL CHECK (expected_revision = revision - 1),
    operation_id TEXT NOT NULL CHECK (coalesce(operation_id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$', 0)),
    accepted_by TEXT NOT NULL REFERENCES actors(id),
    accepted_at INTEGER NOT NULL CHECK (accepted_at >= 0),
    schema_version TEXT NOT NULL CHECK (schema_version = '0.1.0'),
    canonical_export BLOB NOT NULL CHECK (length(canonical_export) BETWEEN 1 AND 2097152),
    content_digest TEXT NOT NULL CHECK (coalesce(content_digest REGEXP '^sir-c1:sha256:[a-f0-9]{64}$', 0)),
    export_digest TEXT NOT NULL CHECK (coalesce(export_digest REGEXP '^sir-e1:sha256:[a-f0-9]{64}$', 0)),
    PRIMARY KEY (script_id, revision),
    UNIQUE (script_id, accepted_by, operation_id)
) STRICT;

CREATE TABLE revision_evidence (
    script_id TEXT NOT NULL,
    revision INTEGER NOT NULL,
    owner_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    evidence_id TEXT NOT NULL,
    PRIMARY KEY (script_id, revision, kind, evidence_id),
    FOREIGN KEY (script_id, revision) REFERENCES script_revisions(script_id, revision),
    FOREIGN KEY (owner_id, kind, evidence_id) REFERENCES script_evidence(owner_id, kind, id)
) STRICT;

CREATE TABLE script_reviews (
    id TEXT UNIQUE NOT NULL CHECK (coalesce(id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$', 0)),
    script_id TEXT NOT NULL,
    revision INTEGER NOT NULL,
    operation_id TEXT NOT NULL CHECK (coalesce(operation_id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$', 0)),
    reviewed_by TEXT NOT NULL REFERENCES actors(id),
    reviewed_at INTEGER NOT NULL CHECK (reviewed_at >= 0),
    PRIMARY KEY (script_id, revision),
    FOREIGN KEY (script_id, revision) REFERENCES script_revisions(script_id, revision)
) STRICT;

CREATE TABLE script_review_operations (
    script_id TEXT NOT NULL,
    actor_id TEXT NOT NULL REFERENCES actors(id),
    operation_id TEXT NOT NULL CHECK (coalesce(operation_id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$', 0)),
    revision INTEGER NOT NULL,
    PRIMARY KEY (script_id, actor_id, operation_id),
    FOREIGN KEY (script_id, revision) REFERENCES script_reviews(script_id, revision)
) STRICT;

CREATE TABLE source_records (
    owner_id TEXT NOT NULL,
    kind TEXT NOT NULL DEFAULT 'source' CHECK (kind = 'source'),
    id TEXT NOT NULL,
    reference TEXT NOT NULL CHECK (octet_length(reference) BETWEEN 1 AND 2048),
    original_text TEXT CHECK (octet_length(original_text) BETWEEN 1 AND 1048576),
    sha256 TEXT NOT NULL CHECK (coalesce(sha256 REGEXP '^[a-f0-9]{64}$', 0)),
    recorded_at INTEGER NOT NULL CHECK (recorded_at >= 0),
    original_bytes BLOB,
    import_metadata TEXT,
    import_outcome TEXT,
    import_digest TEXT,
    imported_by TEXT REFERENCES actors(id),
    import_operation_id TEXT,
    CHECK (original_text IS NOT NULL OR original_bytes IS NOT NULL),
    CHECK (
        (original_bytes IS NULL AND import_metadata IS NULL AND import_outcome IS NULL
         AND import_digest IS NULL AND imported_by IS NULL AND import_operation_id IS NULL)
        OR
        (original_bytes IS NOT NULL AND import_metadata IS NOT NULL AND import_outcome IS NOT NULL
         AND import_digest IS NOT NULL AND imported_by IS NOT NULL AND import_operation_id IS NOT NULL
         AND coalesce(import_digest REGEXP '^[a-f0-9]{64}$', 0)
         AND imported_by = owner_id AND length(original_bytes) <= 1048576
         AND CASE WHEN json_valid(import_metadata) THEN json_type(import_metadata) = 'object' ELSE 0 END
         AND CASE WHEN json_valid(import_outcome) THEN json_type(import_outcome) = 'object' ELSE 0 END
         AND coalesce(import_operation_id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$', 0))
    ),
    PRIMARY KEY (owner_id, id),
    FOREIGN KEY (owner_id, kind, id) REFERENCES script_evidence(owner_id, kind, id)
) STRICT;
CREATE UNIQUE INDEX source_import_operations ON source_records(imported_by, import_operation_id)
    WHERE import_operation_id IS NOT NULL;

-- PG role restrictions become owner-process admission and DB barriers. REPLACE bypasses DELETE
-- triggers on this engine, so every settled unique identity also needs an INSERT guard.
CREATE TRIGGER scripts_identity BEFORE UPDATE ON scripts
WHEN NEW.id <> OLD.id OR NEW.owner_id <> OLD.owner_id
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.script_identity'); END;
CREATE TRIGGER scripts_no_delete BEFORE DELETE ON scripts
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.script_identity'); END;
CREATE TRIGGER scripts_no_replace BEFORE INSERT ON scripts
WHEN EXISTS (SELECT 1 FROM scripts WHERE id = NEW.id)
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.script_identity'); END;

CREATE TRIGGER script_revisions_no_update BEFORE UPDATE ON script_revisions
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.editorial_history'); END;
CREATE TRIGGER script_revisions_no_delete BEFORE DELETE ON script_revisions
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.editorial_history'); END;
CREATE TRIGGER script_revisions_no_replace BEFORE INSERT ON script_revisions
WHEN EXISTS (SELECT 1 FROM script_revisions WHERE script_id = NEW.script_id
    AND (revision = NEW.revision OR (accepted_by = NEW.accepted_by AND operation_id = NEW.operation_id)))
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.editorial_history'); END;

CREATE TRIGGER script_evidence_no_update BEFORE UPDATE ON script_evidence
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.editorial_history'); END;
CREATE TRIGGER script_evidence_no_delete BEFORE DELETE ON script_evidence
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.editorial_history'); END;
CREATE TRIGGER script_evidence_no_replace BEFORE INSERT ON script_evidence
WHEN EXISTS (SELECT 1 FROM script_evidence WHERE owner_id = NEW.owner_id AND kind = NEW.kind AND id = NEW.id)
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.editorial_history'); END;

CREATE TRIGGER revision_evidence_no_update BEFORE UPDATE ON revision_evidence
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.editorial_history'); END;
CREATE TRIGGER revision_evidence_no_delete BEFORE DELETE ON revision_evidence
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.editorial_history'); END;
CREATE TRIGGER revision_evidence_no_replace BEFORE INSERT ON revision_evidence
WHEN EXISTS (SELECT 1 FROM revision_evidence WHERE script_id = NEW.script_id AND revision = NEW.revision
    AND kind = NEW.kind AND evidence_id = NEW.evidence_id)
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.editorial_history'); END;

CREATE TRIGGER script_reviews_no_update BEFORE UPDATE ON script_reviews
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.editorial_history'); END;
CREATE TRIGGER script_reviews_no_delete BEFORE DELETE ON script_reviews
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.editorial_history'); END;
CREATE TRIGGER script_reviews_no_replace BEFORE INSERT ON script_reviews
WHEN EXISTS (SELECT 1 FROM script_reviews WHERE id = NEW.id OR (script_id = NEW.script_id AND revision = NEW.revision))
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.editorial_history'); END;

CREATE TRIGGER script_review_operations_no_update BEFORE UPDATE ON script_review_operations
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.editorial_history'); END;
CREATE TRIGGER script_review_operations_no_delete BEFORE DELETE ON script_review_operations
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.editorial_history'); END;
CREATE TRIGGER script_review_operations_no_replace BEFORE INSERT ON script_review_operations
WHEN EXISTS (SELECT 1 FROM script_review_operations WHERE script_id = NEW.script_id
    AND actor_id = NEW.actor_id AND operation_id = NEW.operation_id)
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.editorial_history'); END;

CREATE TRIGGER source_records_no_update BEFORE UPDATE ON source_records
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.ingestion_history'); END;
CREATE TRIGGER source_records_no_delete BEFORE DELETE ON source_records
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.ingestion_history'); END;
CREATE TRIGGER source_records_no_replace BEFORE INSERT ON source_records
WHEN EXISTS (SELECT 1 FROM source_records WHERE (owner_id = NEW.owner_id AND id = NEW.id)
    OR (imported_by = NEW.imported_by AND import_operation_id = NEW.import_operation_id))
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.ingestion_history'); END;
