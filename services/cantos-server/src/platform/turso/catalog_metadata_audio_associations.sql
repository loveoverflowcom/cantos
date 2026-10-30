-- Server.db v32: exact audio association on metadata publication generations (ADR 0054 D06,
-- #649). A generation entry may pin one settled audio release by ID and release-document
-- digest. The pin is written with its generation, before the sealing event, so it is as
-- immutable as the entry: changing an association writes generation N+1. A pin is a fact, not a
-- playback right: readers still apply the live Media gates (active pointer, withdrawal,
-- revocation, current rights and approval, access window).
--
-- The event table is rebuilt (copy, drop, recreate, copy back, as in v29) only to admit the two
-- association actions; every existing row, version, receipt and pointer is preserved as is.
-- DDL only; queries live in the Catalog adapter.

CREATE TABLE catalog_v32_events (
    work_id TEXT NOT NULL, version INTEGER NOT NULL, owner_id TEXT NOT NULL, action TEXT NOT NULL,
    lifecycle TEXT NOT NULL, generation INTEGER NOT NULL, operation_id TEXT NOT NULL,
    fingerprint TEXT NOT NULL, recorded_by TEXT NOT NULL, recorded_at_ms INTEGER NOT NULL
) STRICT;
CREATE TABLE catalog_v32_pointers (
    work_id TEXT NOT NULL, owner_id TEXT NOT NULL, version INTEGER NOT NULL, lifecycle TEXT NOT NULL,
    generation INTEGER NOT NULL, first_visible_at_ms INTEGER NOT NULL
) STRICT;

INSERT INTO catalog_v32_events SELECT * FROM catalog_metadata_publication_events;
INSERT INTO catalog_v32_pointers SELECT * FROM catalog_metadata_publication_pointers;

DROP TRIGGER catalog_metadata_pointers_no_delete;
DROP TRIGGER catalog_metadata_pointers_no_replace;
DROP TRIGGER catalog_metadata_pointers_first;
DROP TRIGGER catalog_metadata_pointers_identity;
DROP TRIGGER catalog_metadata_pointers_advance;
DROP TRIGGER catalog_metadata_events_no_update;
DROP TRIGGER catalog_metadata_events_no_delete;
DROP TRIGGER catalog_metadata_events_sequence;
DROP TRIGGER catalog_metadata_events_generation;
DROP INDEX catalog_metadata_publication_recent;

DROP TABLE catalog_metadata_publication_pointers;
DROP TABLE catalog_metadata_publication_events;

CREATE TABLE catalog_metadata_publication_events (
    work_id TEXT NOT NULL,
    version INTEGER NOT NULL CHECK (version > 0),
    owner_id TEXT NOT NULL,
    -- `associate_audio`: the owner's explicit association (D06b); `activate_audio`: the same
    -- transaction as an audio activation (D06c).
    action TEXT NOT NULL CHECK (action IN (
        'publish', 'publish_unit', 'withdraw_unit', 'withdraw', 'associate_audio', 'activate_audio'
    )),
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

INSERT INTO catalog_metadata_publication_events SELECT * FROM catalog_v32_events ORDER BY work_id, version;
INSERT INTO catalog_metadata_publication_pointers SELECT * FROM catalog_v32_pointers;
DROP TABLE catalog_v32_pointers;
DROP TABLE catalog_v32_events;

CREATE INDEX catalog_metadata_publication_recent
    ON catalog_metadata_publication_pointers(lifecycle, first_visible_at_ms, work_id);

CREATE TRIGGER catalog_metadata_events_no_update BEFORE UPDATE ON catalog_metadata_publication_events
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.metadata_publication'); END;
CREATE TRIGGER catalog_metadata_events_no_delete BEFORE DELETE ON catalog_metadata_publication_events
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.metadata_publication'); END;
CREATE TRIGGER catalog_metadata_pointers_no_delete BEFORE DELETE ON catalog_metadata_publication_pointers
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.metadata_publication'); END;

-- Versions are consecutive and operation IDs unique per owner. A generation-writing action
-- names the newest generation, which no earlier event named; a per-Unit action, an
-- association or a withdrawal follows an ACTIVE version, and a withdrawal keeps naming that
-- version's generation.
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
    OR (NEW.action IN ('publish_unit', 'withdraw_unit', 'withdraw', 'associate_audio', 'activate_audio')
        AND NOT EXISTS (SELECT 1 FROM catalog_metadata_publication_events
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

-- One optional audio pin per entry. `exact`: the release pins the entry's Unit revision;
-- `display_only`: the owner confirmed a difference limited to display text (D06a/b).
CREATE TABLE catalog_metadata_generation_audio (
    work_id TEXT NOT NULL,
    generation INTEGER NOT NULL,
    unit_id TEXT NOT NULL,
    release_id TEXT NOT NULL CHECK (
        coalesce(release_id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$', 0)
    ),
    release_digest TEXT NOT NULL CHECK (
        coalesce(release_digest REGEXP '^rl-r1:sha256:[a-f0-9]{64}$', 0)
    ),
    basis TEXT NOT NULL CHECK (basis IN ('exact', 'display_only')),
    PRIMARY KEY (work_id, generation, unit_id),
    FOREIGN KEY (work_id, generation, unit_id)
        REFERENCES catalog_metadata_generation_entries(work_id, generation, unit_id)
) STRICT;

CREATE TRIGGER catalog_metadata_audio_no_update BEFORE UPDATE ON catalog_metadata_generation_audio
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.metadata_publication'); END;
CREATE TRIGGER catalog_metadata_audio_no_delete BEFORE DELETE ON catalog_metadata_generation_audio
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.metadata_publication'); END;

-- The pin names an existing release of the same Unit by the same owner, with its exact
-- document digest, and is written only before an event seals the generation.
CREATE TRIGGER catalog_metadata_audio_pin BEFORE INSERT ON catalog_metadata_generation_audio
WHEN NOT EXISTS (SELECT 1 FROM audio_releases r
        JOIN catalog_metadata_generations g ON g.work_id = NEW.work_id AND g.generation = NEW.generation
        WHERE r.id = NEW.release_id AND r.unit_id = NEW.unit_id AND r.owner_id = g.owner_id
            AND r.digest = NEW.release_digest)
    OR EXISTS (SELECT 1 FROM catalog_metadata_publication_events e
        WHERE e.work_id = NEW.work_id AND e.generation = NEW.generation)
    OR EXISTS (SELECT 1 FROM catalog_metadata_generation_audio x
        WHERE x.work_id = NEW.work_id AND x.generation = NEW.generation AND x.unit_id = NEW.unit_id)
BEGIN SELECT RAISE(ABORT, 'cantos:binding.metadata_generation_audio'); END;
