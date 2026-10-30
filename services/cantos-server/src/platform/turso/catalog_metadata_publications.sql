-- Server.db v31: metadata publication generations (ADR 0054 D01/D02/D05, #648). Catalog keeps
-- the only editing head; a generation pins one exact saved Work revision and an explicit ordered
-- list of exact Unit revisions, never `catalog_units.position` or an unselected head. One pointer
-- per Work carries the lifecycle and a version for compare-and-set; every version is one
-- immutable event that is also the owner's operation receipt. These rows alone grant nothing:
-- readers still apply the live owner/archive gates, and no Theatre route reads them yet (S05).
--
-- Each table refuses an insert over any of its unique keys, because `REPLACE` deletes a
-- conflicting row without firing delete triggers. DDL only; queries live in the Catalog adapter.

CREATE TABLE catalog_metadata_generations (
    work_id TEXT NOT NULL,
    generation INTEGER NOT NULL CHECK (generation > 0),
    owner_id TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('novel', 'story', 'podcast')),
    work_revision INTEGER NOT NULL CHECK (work_revision > 0),
    work_digest TEXT NOT NULL CHECK (coalesce(work_digest REGEXP '^[a-f0-9]{64}$', 0)),
    entry_count INTEGER NOT NULL CHECK (entry_count BETWEEN 0 AND 1000),
    -- Derived once from the pinned revision for public filtering and search.
    language TEXT NOT NULL CHECK (length(language) BETWEEN 1 AND 63),
    search_text TEXT NOT NULL CHECK (octet_length(search_text) <= 32768),
    recorded_by TEXT NOT NULL REFERENCES actors(id) CHECK (recorded_by = owner_id),
    recorded_at_ms INTEGER NOT NULL CHECK (recorded_at_ms BETWEEN 0 AND 9007199254740991),
    PRIMARY KEY (work_id, generation),
    FOREIGN KEY (work_id, owner_id) REFERENCES catalog_works(id, owner_id),
    FOREIGN KEY (work_id, kind) REFERENCES catalog_works(id, kind),
    FOREIGN KEY (work_id, work_revision) REFERENCES catalog_work_revisions(work_id, revision)
) STRICT;

CREATE TABLE catalog_metadata_generation_entries (
    work_id TEXT NOT NULL,
    generation INTEGER NOT NULL,
    -- One-based published position, chosen by the publish command (D02b).
    position INTEGER NOT NULL CHECK (position BETWEEN 1 AND 1000),
    unit_id TEXT NOT NULL,
    unit_revision INTEGER NOT NULL CHECK (unit_revision > 0),
    unit_digest TEXT NOT NULL CHECK (coalesce(unit_digest REGEXP '^[a-f0-9]{64}$', 0)),
    PRIMARY KEY (work_id, generation, position),
    UNIQUE (work_id, generation, unit_id),
    FOREIGN KEY (work_id, generation) REFERENCES catalog_metadata_generations(work_id, generation),
    FOREIGN KEY (unit_id, unit_revision) REFERENCES catalog_unit_revisions(unit_id, revision)
) STRICT;

CREATE TABLE catalog_metadata_publication_events (
    work_id TEXT NOT NULL,
    version INTEGER NOT NULL CHECK (version > 0),
    owner_id TEXT NOT NULL,
    action TEXT NOT NULL CHECK (action IN ('publish', 'publish_unit', 'withdraw_unit', 'withdraw')),
    lifecycle TEXT NOT NULL CHECK (lifecycle IN ('active', 'withdrawn')),
    generation INTEGER NOT NULL,
    operation_id TEXT NOT NULL CHECK (
        coalesce(operation_id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$', 0)
    ),
    -- SHA-256 of the canonical admitted request: same ID and digest replays, a new digest refuses.
    fingerprint TEXT NOT NULL CHECK (coalesce(fingerprint REGEXP '^[a-f0-9]{64}$', 0)),
    recorded_by TEXT NOT NULL REFERENCES actors(id) CHECK (recorded_by = owner_id),
    recorded_at_ms INTEGER NOT NULL CHECK (recorded_at_ms BETWEEN 0 AND 9007199254740991),
    CHECK ((action = 'withdraw') = (lifecycle = 'withdrawn')),
    PRIMARY KEY (work_id, version),
    UNIQUE (owner_id, operation_id),
    FOREIGN KEY (work_id, owner_id) REFERENCES catalog_works(id, owner_id),
    FOREIGN KEY (work_id, generation) REFERENCES catalog_metadata_generations(work_id, generation)
) STRICT;

CREATE TABLE catalog_metadata_publication_pointers (
    work_id TEXT PRIMARY KEY NOT NULL,
    owner_id TEXT NOT NULL,
    version INTEGER NOT NULL CHECK (version > 0),
    lifecycle TEXT NOT NULL CHECK (lifecycle IN ('active', 'withdrawn')),
    generation INTEGER NOT NULL,
    -- The first ACTIVE generation's time; later generations and withdrawals never move it (D02c).
    first_visible_at_ms INTEGER NOT NULL CHECK (first_visible_at_ms BETWEEN 0 AND 9007199254740991),
    FOREIGN KEY (work_id, owner_id) REFERENCES catalog_works(id, owner_id),
    FOREIGN KEY (work_id, version) REFERENCES catalog_metadata_publication_events(work_id, version),
    FOREIGN KEY (work_id, generation) REFERENCES catalog_metadata_generations(work_id, generation)
) STRICT;
CREATE INDEX catalog_metadata_publication_recent
    ON catalog_metadata_publication_pointers(lifecycle, first_visible_at_ms, work_id);

-- Generations, entries and events are append-only.
CREATE TRIGGER catalog_metadata_generations_no_update BEFORE UPDATE ON catalog_metadata_generations
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.metadata_publication'); END;
CREATE TRIGGER catalog_metadata_generations_no_delete BEFORE DELETE ON catalog_metadata_generations
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.metadata_publication'); END;
CREATE TRIGGER catalog_metadata_entries_no_update BEFORE UPDATE ON catalog_metadata_generation_entries
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.metadata_publication'); END;
CREATE TRIGGER catalog_metadata_entries_no_delete BEFORE DELETE ON catalog_metadata_generation_entries
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.metadata_publication'); END;
CREATE TRIGGER catalog_metadata_events_no_update BEFORE UPDATE ON catalog_metadata_publication_events
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.metadata_publication'); END;
CREATE TRIGGER catalog_metadata_events_no_delete BEFORE DELETE ON catalog_metadata_publication_events
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.metadata_publication'); END;
CREATE TRIGGER catalog_metadata_pointers_no_delete BEFORE DELETE ON catalog_metadata_publication_pointers
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.metadata_publication'); END;

-- Generation numbers are consecutive per Work, which also refuses an insert over an existing
-- key. The Work pin must name the exact stored revision bytes.
CREATE TRIGGER catalog_metadata_generations_sequence BEFORE INSERT ON catalog_metadata_generations
WHEN NEW.generation <> coalesce((SELECT max(generation) FROM catalog_metadata_generations
    WHERE work_id = NEW.work_id), 0) + 1
BEGIN SELECT RAISE(ABORT, 'cantos:binding.metadata_generation_sequence'); END;
CREATE TRIGGER catalog_metadata_generations_pin BEFORE INSERT ON catalog_metadata_generations
WHEN NOT EXISTS (SELECT 1 FROM catalog_work_revisions
    WHERE work_id = NEW.work_id AND revision = NEW.work_revision AND snapshot_digest = NEW.work_digest)
BEGIN SELECT RAISE(ABORT, 'cantos:binding.metadata_generation_pin'); END;

-- An entry pins an exact revision of a Unit of the same Work, inside the declared count, and
-- only before an event seals its generation.
CREATE TRIGGER catalog_metadata_entries_pin BEFORE INSERT ON catalog_metadata_generation_entries
WHEN NOT EXISTS (SELECT 1 FROM catalog_units u
        JOIN catalog_unit_revisions r ON r.unit_id = u.id
        WHERE u.id = NEW.unit_id AND u.work_id = NEW.work_id
            AND r.revision = NEW.unit_revision AND r.snapshot_digest = NEW.unit_digest)
    OR NEW.position > coalesce((SELECT entry_count FROM catalog_metadata_generations g
        WHERE g.work_id = NEW.work_id AND g.generation = NEW.generation), 0)
    OR EXISTS (SELECT 1 FROM catalog_metadata_publication_events e
        WHERE e.work_id = NEW.work_id AND e.generation = NEW.generation)
    OR EXISTS (SELECT 1 FROM catalog_metadata_generation_entries x
        WHERE x.work_id = NEW.work_id AND x.generation = NEW.generation
            AND (x.position = NEW.position OR x.unit_id = NEW.unit_id))
BEGIN SELECT RAISE(ABORT, 'cantos:binding.metadata_generation_entry'); END;

-- Versions are consecutive and operation IDs unique per owner. A generation-writing action
-- names the newest generation, which no earlier event named; a per-Unit action or withdrawal
-- follows an ACTIVE version, and a withdrawal keeps naming that version's generation.
CREATE TRIGGER catalog_metadata_events_sequence BEFORE INSERT ON catalog_metadata_publication_events
WHEN NEW.version <> coalesce((SELECT max(version) FROM catalog_metadata_publication_events
        WHERE work_id = NEW.work_id), 0) + 1
    OR EXISTS (SELECT 1 FROM catalog_metadata_publication_events
        WHERE owner_id = NEW.owner_id AND operation_id = NEW.operation_id)
BEGIN SELECT RAISE(ABORT, 'cantos:binding.metadata_publication_sequence'); END;
CREATE TRIGGER catalog_metadata_events_generation BEFORE INSERT ON catalog_metadata_publication_events
WHEN (NEW.action <> 'withdraw' AND (
        NEW.generation <> (SELECT max(generation) FROM catalog_metadata_generations
            WHERE work_id = NEW.work_id)
        OR EXISTS (SELECT 1 FROM catalog_metadata_publication_events
            WHERE work_id = NEW.work_id AND generation = NEW.generation)))
    OR (NEW.action IN ('publish_unit', 'withdraw_unit', 'withdraw') AND NOT EXISTS (
        SELECT 1 FROM catalog_metadata_publication_events
        WHERE work_id = NEW.work_id AND version = NEW.version - 1 AND lifecycle = 'active'))
    OR (NEW.action = 'withdraw' AND NOT EXISTS (
        SELECT 1 FROM catalog_metadata_publication_events
        WHERE work_id = NEW.work_id AND version = NEW.version - 1 AND generation = NEW.generation))
BEGIN SELECT RAISE(ABORT, 'cantos:binding.metadata_publication_generation'); END;

-- The pointer only ever names its newest event, and a generation whose entries are complete.
-- Identity, owner and the recency anchor never change after the first publication.
CREATE TRIGGER catalog_metadata_pointers_no_replace BEFORE INSERT ON catalog_metadata_publication_pointers
WHEN EXISTS (SELECT 1 FROM catalog_metadata_publication_pointers WHERE work_id = NEW.work_id)
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.metadata_publication'); END;
CREATE TRIGGER catalog_metadata_pointers_first BEFORE INSERT ON catalog_metadata_publication_pointers
WHEN NEW.version <> 1
    OR NOT EXISTS (SELECT 1 FROM catalog_metadata_publication_events e
        WHERE e.work_id = NEW.work_id AND e.version = NEW.version
            AND e.lifecycle = NEW.lifecycle AND e.generation = NEW.generation)
    OR coalesce((SELECT entry_count FROM catalog_metadata_generations g
        WHERE g.work_id = NEW.work_id AND g.generation = NEW.generation), -1)
        <> (SELECT count(*) FROM catalog_metadata_generation_entries x
            WHERE x.work_id = NEW.work_id AND x.generation = NEW.generation)
BEGIN SELECT RAISE(ABORT, 'cantos:binding.metadata_publication_pointer'); END;
CREATE TRIGGER catalog_metadata_pointers_identity BEFORE UPDATE ON catalog_metadata_publication_pointers
WHEN NEW.work_id <> OLD.work_id OR NEW.owner_id <> OLD.owner_id
    OR NEW.first_visible_at_ms <> OLD.first_visible_at_ms
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.metadata_publication'); END;
CREATE TRIGGER catalog_metadata_pointers_advance BEFORE UPDATE ON catalog_metadata_publication_pointers
WHEN NEW.version <> OLD.version + 1
    OR NOT EXISTS (SELECT 1 FROM catalog_metadata_publication_events e
        WHERE e.work_id = NEW.work_id AND e.version = NEW.version
            AND e.lifecycle = NEW.lifecycle AND e.generation = NEW.generation)
    OR coalesce((SELECT entry_count FROM catalog_metadata_generations g
        WHERE g.work_id = NEW.work_id AND g.generation = NEW.generation), -1)
        <> (SELECT count(*) FROM catalog_metadata_generation_entries x
            WHERE x.work_id = NEW.work_id AND x.generation = NEW.generation)
BEGIN SELECT RAISE(ABORT, 'cantos:binding.metadata_publication_pointer'); END;
