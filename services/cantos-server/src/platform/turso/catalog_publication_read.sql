-- Server.db version 12: the Catalog closure of the CMP business API slice (#333): works, units,
-- their immutable metadata revisions and the immutable content revisions/heads that a
-- production brief and a published release bind. Delegation grants, members, operations and the
-- PostgreSQL `catalog_clock` write fence are not part of this slice: the serialized writer
-- replaces the fence, and the Studio write path is ported by its own version.
--
-- Timestamps and the snapshots' own times are the domain's Unix milliseconds; this version stores
-- none of its own. Every regular-expression CHECK is `coalesce(.. REGEXP .., 0)` so a pattern that
-- stopped compiling would refuse rows instead of admitting them. `REPLACE` deletes a conflicting
-- row without firing delete triggers, so each guarded table also refuses an insert over any of
-- its unique keys. DDL only; queries live in the owning persistence adapters.

CREATE TABLE catalog_works (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        coalesce(id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$', 0)
    ),
    owner_id TEXT NOT NULL REFERENCES actors(id),
    kind TEXT NOT NULL CHECK (kind IN ('novel', 'story', 'podcast')),
    lifecycle TEXT NOT NULL CHECK (lifecycle IN ('draft', 'archived')),
    head_revision INTEGER NOT NULL CHECK (head_revision > 0),
    UNIQUE (id, kind),
    UNIQUE (id, owner_id),
    FOREIGN KEY (id, head_revision) REFERENCES catalog_work_revisions(work_id, revision)
        DEFERRABLE INITIALLY DEFERRED
) STRICT;
CREATE INDEX catalog_works_owner ON catalog_works(owner_id, id);

CREATE TABLE catalog_work_revisions (
    work_id TEXT NOT NULL REFERENCES catalog_works(id),
    revision INTEGER NOT NULL CHECK (revision > 0),
    snapshot BLOB NOT NULL CHECK (length(snapshot) BETWEEN 1 AND 131072),
    snapshot_digest TEXT NOT NULL CHECK (coalesce(snapshot_digest REGEXP '^[a-f0-9]{64}$', 0)),
    recorded_by TEXT NOT NULL REFERENCES actors(id),
    PRIMARY KEY (work_id, revision)
) STRICT;

CREATE TABLE catalog_units (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        coalesce(id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$', 0)
    ),
    work_id TEXT NOT NULL,
    work_kind TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('chapter', 'episode')),
    head_revision INTEGER NOT NULL CHECK (head_revision > 0),
    "position" INTEGER NOT NULL CHECK ("position" >= 0),
    lifecycle TEXT NOT NULL CHECK (lifecycle IN ('draft', 'archived')),
    CHECK (
        (work_kind IN ('novel', 'story') AND kind = 'chapter')
        OR (work_kind = 'podcast' AND kind = 'episode')
    ),
    -- PostgreSQL's UNIQUE (work_id, position) was DEFERRABLE, which this engine lacks: the
    -- Studio reorder port shifts positions out of range and renumbers inside one transaction.
    UNIQUE (work_id, "position"),
    FOREIGN KEY (work_id, work_kind) REFERENCES catalog_works(id, kind),
    FOREIGN KEY (id, head_revision) REFERENCES catalog_unit_revisions(unit_id, revision)
        DEFERRABLE INITIALLY DEFERRED
) STRICT;
CREATE INDEX catalog_units_order ON catalog_units(work_id, "position", id);

CREATE TABLE catalog_unit_revisions (
    unit_id TEXT NOT NULL REFERENCES catalog_units(id),
    revision INTEGER NOT NULL CHECK (revision > 0),
    snapshot BLOB NOT NULL CHECK (length(snapshot) BETWEEN 1 AND 131072),
    snapshot_digest TEXT NOT NULL CHECK (coalesce(snapshot_digest REGEXP '^[a-f0-9]{64}$', 0)),
    recorded_by TEXT NOT NULL REFERENCES actors(id),
    PRIMARY KEY (unit_id, revision)
) STRICT;

CREATE TABLE catalog_content_revisions (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        coalesce(id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$', 0)
    ),
    unit_id TEXT NOT NULL REFERENCES catalog_units(id),
    revision INTEGER NOT NULL CHECK (revision > 0),
    snapshot BLOB NOT NULL CHECK (length(snapshot) BETWEEN 1 AND 131072),
    revision_digest TEXT NOT NULL CHECK (
        coalesce(revision_digest REGEXP '^cc-r1:sha256:[a-f0-9]{64}$', 0)
    ),
    recorded_by TEXT NOT NULL REFERENCES actors(id),
    UNIQUE (unit_id, id, revision),
    UNIQUE (unit_id, revision)
) STRICT;

CREATE TABLE catalog_content_heads (
    unit_id TEXT PRIMARY KEY NOT NULL REFERENCES catalog_units(id),
    revision_id TEXT NOT NULL,
    revision INTEGER NOT NULL CHECK (revision > 0),
    FOREIGN KEY (unit_id, revision_id, revision)
        REFERENCES catalog_content_revisions(unit_id, id, revision)
) STRICT;

-- Identity never changes and a Work or Unit is never deleted. Only the head, lifecycle and (for a
-- Unit) position move. Stronger than PostgreSQL, whose runtime grants withheld DELETE.
CREATE TRIGGER catalog_works_identity BEFORE UPDATE ON catalog_works
WHEN NEW.id <> OLD.id OR NEW.owner_id <> OLD.owner_id OR NEW.kind <> OLD.kind
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.catalog_identity'); END;
CREATE TRIGGER catalog_works_no_delete BEFORE DELETE ON catalog_works
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.catalog_identity'); END;
CREATE TRIGGER catalog_works_no_replace BEFORE INSERT ON catalog_works
WHEN EXISTS (SELECT 1 FROM catalog_works WHERE id = NEW.id)
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.catalog_identity'); END;

CREATE TRIGGER catalog_units_identity BEFORE UPDATE ON catalog_units
WHEN NEW.id <> OLD.id OR NEW.work_id <> OLD.work_id OR NEW.work_kind <> OLD.work_kind
    OR NEW.kind <> OLD.kind
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.catalog_identity'); END;
CREATE TRIGGER catalog_units_no_delete BEFORE DELETE ON catalog_units
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.catalog_identity'); END;
CREATE TRIGGER catalog_units_no_replace BEFORE INSERT ON catalog_units
WHEN EXISTS (
    SELECT 1 FROM catalog_units
    WHERE id = NEW.id OR (work_id = NEW.work_id AND "position" = NEW.position)
)
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.catalog_identity'); END;

-- Revision history is append-only.
CREATE TRIGGER catalog_work_revisions_no_update BEFORE UPDATE ON catalog_work_revisions
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.catalog_history'); END;
CREATE TRIGGER catalog_work_revisions_no_delete BEFORE DELETE ON catalog_work_revisions
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.catalog_history'); END;
CREATE TRIGGER catalog_work_revisions_no_replace BEFORE INSERT ON catalog_work_revisions
WHEN EXISTS (SELECT 1 FROM catalog_work_revisions WHERE work_id = NEW.work_id AND revision = NEW.revision)
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.catalog_history'); END;

CREATE TRIGGER catalog_unit_revisions_no_update BEFORE UPDATE ON catalog_unit_revisions
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.catalog_history'); END;
CREATE TRIGGER catalog_unit_revisions_no_delete BEFORE DELETE ON catalog_unit_revisions
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.catalog_history'); END;
CREATE TRIGGER catalog_unit_revisions_no_replace BEFORE INSERT ON catalog_unit_revisions
WHEN EXISTS (SELECT 1 FROM catalog_unit_revisions WHERE unit_id = NEW.unit_id AND revision = NEW.revision)
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.catalog_history'); END;

CREATE TRIGGER catalog_content_revisions_no_update BEFORE UPDATE ON catalog_content_revisions
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.catalog_history'); END;
CREATE TRIGGER catalog_content_revisions_no_delete BEFORE DELETE ON catalog_content_revisions
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.catalog_history'); END;
CREATE TRIGGER catalog_content_revisions_no_replace BEFORE INSERT ON catalog_content_revisions
WHEN EXISTS (
    SELECT 1 FROM catalog_content_revisions
    WHERE id = NEW.id OR (unit_id = NEW.unit_id AND revision = NEW.revision)
)
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.catalog_history'); END;

-- A content head advances in place and never changes hands or disappears.
CREATE TRIGGER catalog_content_heads_identity BEFORE UPDATE ON catalog_content_heads
WHEN NEW.unit_id <> OLD.unit_id
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.catalog_identity'); END;
CREATE TRIGGER catalog_content_heads_no_delete BEFORE DELETE ON catalog_content_heads
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.catalog_identity'); END;
CREATE TRIGGER catalog_content_heads_no_replace BEFORE INSERT ON catalog_content_heads
WHEN EXISTS (SELECT 1 FROM catalog_content_heads WHERE unit_id = NEW.unit_id)
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.catalog_identity'); END;
