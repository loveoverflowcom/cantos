-- Server.db version 13: the production-brief relation that an external audio submission binds
-- (#333 CMP business API slice). Only `production_briefs` is here: it is the referenced parent of
-- `audio_submissions` and must exist for a released audio chain to be representable. Brief heads,
-- producer grants, settings, snapshots, approvals and rights claims belong to the Production
-- context's own port and are not required by any CMP route.
--
-- A brief is immutable. `REPLACE` deletes a conflicting row without firing delete triggers, so
-- an insert over any unique key is refused first. DDL only.

CREATE TABLE production_briefs (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        coalesce(id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$', 0)
    ),
    unit_id TEXT NOT NULL REFERENCES catalog_units(id),
    content_revision_id TEXT NOT NULL,
    content_sequence INTEGER NOT NULL CHECK (content_sequence > 0),
    owner_id TEXT NOT NULL REFERENCES actors(id),
    version INTEGER NOT NULL CHECK (version > 0),
    operation_id TEXT NOT NULL,
    request_digest TEXT NOT NULL CHECK (
        coalesce(request_digest REGEXP '^pb-request1:sha256:[a-f0-9]{64}$', 0)
    ),
    digest TEXT NOT NULL CHECK (coalesce(digest REGEXP '^pb-r1:sha256:[a-f0-9]{64}$', 0)),
    snapshot BLOB NOT NULL CHECK (length(snapshot) BETWEEN 1 AND 262144),
    UNIQUE (id, unit_id, owner_id, digest),
    UNIQUE (owner_id, operation_id),
    UNIQUE (unit_id, id, version),
    UNIQUE (unit_id, version),
    FOREIGN KEY (unit_id, content_revision_id, content_sequence)
        REFERENCES catalog_content_revisions(unit_id, id, revision)
) STRICT;

CREATE TRIGGER production_briefs_no_update BEFORE UPDATE ON production_briefs
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.production_briefs'); END;
CREATE TRIGGER production_briefs_no_delete BEFORE DELETE ON production_briefs
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.production_briefs'); END;
CREATE TRIGGER production_briefs_no_replace BEFORE INSERT ON production_briefs
WHEN EXISTS (
    SELECT 1 FROM production_briefs
    WHERE id = NEW.id
        OR (owner_id = NEW.owner_id AND operation_id = NEW.operation_id)
        OR (unit_id = NEW.unit_id AND version = NEW.version)
)
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.production_briefs'); END;
