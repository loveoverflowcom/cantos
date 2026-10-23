-- Server.db version 15: the account-owned Listener Library (#333 CMP business API slice):
-- account state, progress, saved Works, playback history, release-bound bookmarks, per-device
-- sequences, playback sessions and idempotent operation receipts. The account is an `actors` row;
-- PostgreSQL's `listener_lock_actor` row lock is replaced by the serialized writer.
--
-- Timestamps and `listener_order` are the domain's Unix milliseconds and per-account counter. A
-- record's JSON snapshot is checked against its own columns, which replaces PostgreSQL's
-- `jsonb @>` containment. The snapshot digest is SHA-256 of the exact snapshot bytes and is
-- verified by the adapter's decoder. `REPLACE` deletes a conflicting row without firing delete
-- triggers, so each guarded table also refuses an insert over its primary key. DDL only.

CREATE TABLE listener_state (
    listener_id TEXT PRIMARY KEY NOT NULL REFERENCES actors(id),
    revision INTEGER NOT NULL DEFAULT 0 CHECK (revision >= 0),
    history_revision INTEGER NOT NULL DEFAULT 0 CHECK (history_revision >= 0),
    history_epoch INTEGER NOT NULL DEFAULT 0 CHECK (history_epoch >= 0),
    CHECK (history_epoch <= history_revision)
) STRICT;

CREATE TABLE listener_progress (
    listener_id TEXT NOT NULL REFERENCES listener_state(listener_id),
    publication_id TEXT NOT NULL REFERENCES audio_releases(id),
    work_id TEXT NOT NULL REFERENCES catalog_works(id),
    unit_id TEXT NOT NULL REFERENCES catalog_units(id),
    rendition_id TEXT NOT NULL REFERENCES audio_renditions(id),
    sha256 TEXT NOT NULL CHECK (coalesce(sha256 REGEXP '^[a-f0-9]{64}$', 0)),
    duration_ms INTEGER NOT NULL CHECK (duration_ms > 0),
    position_ms INTEGER NOT NULL,
    completion TEXT NOT NULL CHECK (completion IN ('in_progress', 'completed')),
    revision INTEGER NOT NULL CHECK (revision > 0),
    listener_order INTEGER NOT NULL CHECK (listener_order > 0),
    accepted_at INTEGER NOT NULL CHECK (accepted_at >= 0),
    snapshot BLOB NOT NULL CHECK (length(snapshot) BETWEEN 1 AND 8192),
    snapshot_digest BLOB NOT NULL CHECK (length(snapshot_digest) = 32),
    PRIMARY KEY (listener_id, publication_id),
    CHECK (position_ms >= 0 AND position_ms <= duration_ms),
    CHECK (
        json_valid(CAST(snapshot AS TEXT))
        AND json_extract(CAST(snapshot AS TEXT), '$.audio.work_id') IS work_id
        AND json_extract(CAST(snapshot AS TEXT), '$.audio.unit_id') IS unit_id
        AND json_extract(CAST(snapshot AS TEXT), '$.audio.publication_id') IS publication_id
        AND json_extract(CAST(snapshot AS TEXT), '$.audio.rendition_id') IS rendition_id
        AND json_extract(CAST(snapshot AS TEXT), '$.audio.sha256') IS sha256
        AND json_extract(CAST(snapshot AS TEXT), '$.audio.duration_ms') IS duration_ms
        AND json_extract(CAST(snapshot AS TEXT), '$.position_ms') IS position_ms
        AND json_extract(CAST(snapshot AS TEXT), '$.completion') IS completion
        AND json_extract(CAST(snapshot AS TEXT), '$.revision') IS revision
        AND json_extract(CAST(snapshot AS TEXT), '$.listener_order') IS listener_order
        AND json_extract(CAST(snapshot AS TEXT), '$.accepted_at_ms') IS accepted_at
    )
) STRICT;
CREATE INDEX listener_progress_recent ON listener_progress(listener_id, listener_order DESC, work_id);
CREATE INDEX listener_progress_work_recent
    ON listener_progress(listener_id, work_id, listener_order DESC, publication_id);

CREATE TABLE listener_saved (
    listener_id TEXT NOT NULL REFERENCES listener_state(listener_id),
    work_id TEXT NOT NULL REFERENCES catalog_works(id),
    saved INTEGER NOT NULL CHECK (saved IN (0, 1)),
    revision INTEGER NOT NULL CHECK (revision > 0),
    listener_order INTEGER NOT NULL CHECK (listener_order > 0),
    accepted_at INTEGER NOT NULL CHECK (accepted_at >= 0),
    snapshot BLOB NOT NULL CHECK (length(snapshot) BETWEEN 1 AND 4096),
    snapshot_digest BLOB NOT NULL CHECK (length(snapshot_digest) = 32),
    PRIMARY KEY (listener_id, work_id),
    CHECK (
        json_valid(CAST(snapshot AS TEXT))
        AND json_extract(CAST(snapshot AS TEXT), '$.work_id') IS work_id
        AND json_extract(CAST(snapshot AS TEXT), '$.saved') IS saved
        AND json_extract(CAST(snapshot AS TEXT), '$.revision') IS revision
        AND json_extract(CAST(snapshot AS TEXT), '$.listener_order') IS listener_order
        AND json_extract(CAST(snapshot AS TEXT), '$.accepted_at_ms') IS accepted_at
    )
) STRICT;
CREATE INDEX listener_saved_recent ON listener_saved(listener_id, listener_order DESC, work_id) WHERE saved = 1;

-- Playback history keeps the latest start per Work; a clear deletes the account's rows.
CREATE TABLE listener_history (
    listener_id TEXT NOT NULL REFERENCES listener_state(listener_id),
    work_id TEXT NOT NULL REFERENCES catalog_works(id),
    publication_id TEXT NOT NULL REFERENCES audio_releases(id),
    unit_id TEXT NOT NULL REFERENCES catalog_units(id),
    listener_order INTEGER NOT NULL CHECK (listener_order > 0),
    started_at INTEGER NOT NULL CHECK (started_at >= 0),
    snapshot BLOB NOT NULL CHECK (length(snapshot) BETWEEN 1 AND 4096),
    snapshot_digest BLOB NOT NULL CHECK (length(snapshot_digest) = 32),
    PRIMARY KEY (listener_id, work_id)
) STRICT;
CREATE INDEX listener_history_recent ON listener_history(listener_id, listener_order DESC, work_id);

CREATE TABLE listener_operations (
    listener_id TEXT NOT NULL REFERENCES listener_state(listener_id),
    operation_id TEXT NOT NULL CHECK (
        coalesce(operation_id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$', 0)
    ),
    request_digest BLOB NOT NULL CHECK (length(request_digest) = 32),
    response_bytes BLOB NOT NULL CHECK (length(response_bytes) BETWEEN 1 AND 16384),
    response_digest BLOB NOT NULL CHECK (length(response_digest) = 32),
    PRIMARY KEY (listener_id, operation_id)
) STRICT;

CREATE TABLE listener_device_sequences (
    listener_id TEXT NOT NULL REFERENCES listener_state(listener_id),
    device_id TEXT NOT NULL CHECK (
        coalesce(device_id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$', 0)
    ),
    resource_key TEXT NOT NULL CHECK (octet_length(resource_key) BETWEEN 1 AND 128),
    sequence INTEGER NOT NULL CHECK (sequence > 0),
    PRIMARY KEY (listener_id, device_id, resource_key)
) STRICT;

CREATE TABLE listener_playback_sessions (
    listener_id TEXT NOT NULL REFERENCES listener_state(listener_id),
    device_id TEXT NOT NULL CHECK (
        coalesce(device_id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$', 0)
    ),
    engine_session_id TEXT NOT NULL CHECK (
        coalesce(engine_session_id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$', 0)
    ),
    audio_snapshot BLOB NOT NULL CHECK (length(audio_snapshot) BETWEEN 1 AND 4096),
    PRIMARY KEY (listener_id, device_id, engine_session_id)
) STRICT;

-- A bookmark is an immutable (id, exact audio identity, position) value; its only change is the
-- final tombstone. The live listing never reads a tombstone.
CREATE TABLE listener_bookmarks (
    listener_id TEXT NOT NULL REFERENCES listener_state(listener_id),
    bookmark_id TEXT NOT NULL CHECK (
        coalesce(bookmark_id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$', 0)
    ),
    publication_id TEXT NOT NULL REFERENCES audio_releases(id),
    work_id TEXT NOT NULL REFERENCES catalog_works(id),
    unit_id TEXT NOT NULL REFERENCES catalog_units(id),
    rendition_id TEXT NOT NULL REFERENCES audio_renditions(id),
    sha256 TEXT NOT NULL CHECK (coalesce(sha256 REGEXP '^[a-f0-9]{64}$', 0)),
    duration_ms INTEGER NOT NULL CHECK (duration_ms > 0),
    position_ms INTEGER NOT NULL,
    deleted INTEGER NOT NULL CHECK (deleted IN (0, 1)),
    listener_order INTEGER NOT NULL CHECK (listener_order > 0),
    accepted_at INTEGER NOT NULL CHECK (accepted_at >= 0),
    snapshot BLOB NOT NULL CHECK (length(snapshot) BETWEEN 1 AND 4096),
    snapshot_digest BLOB NOT NULL CHECK (length(snapshot_digest) = 32),
    PRIMARY KEY (listener_id, bookmark_id),
    CHECK (position_ms >= 0 AND position_ms <= duration_ms),
    CHECK (
        json_valid(CAST(snapshot AS TEXT))
        AND json_extract(CAST(snapshot AS TEXT), '$.bookmark_id') IS bookmark_id
        AND json_extract(CAST(snapshot AS TEXT), '$.audio.work_id') IS work_id
        AND json_extract(CAST(snapshot AS TEXT), '$.audio.unit_id') IS unit_id
        AND json_extract(CAST(snapshot AS TEXT), '$.audio.publication_id') IS publication_id
        AND json_extract(CAST(snapshot AS TEXT), '$.audio.rendition_id') IS rendition_id
        AND json_extract(CAST(snapshot AS TEXT), '$.audio.sha256') IS sha256
        AND json_extract(CAST(snapshot AS TEXT), '$.audio.duration_ms') IS duration_ms
        AND json_extract(CAST(snapshot AS TEXT), '$.position_ms') IS position_ms
        AND json_extract(CAST(snapshot AS TEXT), '$.deleted') IS deleted
        AND json_extract(CAST(snapshot AS TEXT), '$.listener_order') IS listener_order
        AND json_extract(CAST(snapshot AS TEXT), '$.accepted_at_ms') IS accepted_at
    )
) STRICT;
CREATE INDEX listener_bookmarks_recent
    ON listener_bookmarks(listener_id, listener_order DESC, bookmark_id) WHERE deleted = 0;

-- The account counters only move forward and the account is never deleted. (Added over
-- PostgreSQL, which relied on the adapter's `WHERE revision = ?`.)
CREATE TRIGGER listener_state_forward BEFORE UPDATE ON listener_state
WHEN NEW.listener_id <> OLD.listener_id OR NEW.revision < OLD.revision
    OR NEW.history_revision < OLD.history_revision OR NEW.history_epoch < OLD.history_epoch
BEGIN SELECT RAISE(ABORT, 'cantos:listener_state.forward'); END;
CREATE TRIGGER listener_state_no_delete BEFORE DELETE ON listener_state
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.listener_state'); END;
CREATE TRIGGER listener_state_no_replace BEFORE INSERT ON listener_state
WHEN EXISTS (SELECT 1 FROM listener_state WHERE listener_id = NEW.listener_id)
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.listener_state'); END;

-- A progress record keeps its audio identity; each accepted write is exactly one revision later
-- and strictly later in the account order (PostgreSQL `guard_listener_progress`).
CREATE TRIGGER listener_progress_guard BEFORE UPDATE ON listener_progress
WHEN NEW.listener_id IS NOT OLD.listener_id OR NEW.publication_id IS NOT OLD.publication_id
    OR NEW.work_id IS NOT OLD.work_id OR NEW.unit_id IS NOT OLD.unit_id
    OR NEW.rendition_id IS NOT OLD.rendition_id OR NEW.sha256 IS NOT OLD.sha256
    OR NEW.duration_ms IS NOT OLD.duration_ms
    OR NEW.revision <> OLD.revision + 1 OR NEW.listener_order <= OLD.listener_order
BEGIN SELECT RAISE(ABORT, 'cantos:listener_progress.forward'); END;
CREATE TRIGGER listener_progress_no_delete BEFORE DELETE ON listener_progress
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.listener_progress'); END;
CREATE TRIGGER listener_progress_no_replace BEFORE INSERT ON listener_progress
WHEN EXISTS (
    SELECT 1 FROM listener_progress
    WHERE listener_id = NEW.listener_id AND publication_id = NEW.publication_id
)
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.listener_progress'); END;

-- (PostgreSQL `guard_listener_saved`.)
CREATE TRIGGER listener_saved_guard BEFORE UPDATE ON listener_saved
WHEN NEW.listener_id IS NOT OLD.listener_id OR NEW.work_id IS NOT OLD.work_id
    OR NEW.revision <> OLD.revision + 1 OR NEW.listener_order <= OLD.listener_order
BEGIN SELECT RAISE(ABORT, 'cantos:listener_saved.forward'); END;
CREATE TRIGGER listener_saved_no_delete BEFORE DELETE ON listener_saved
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.listener_saved'); END;
CREATE TRIGGER listener_saved_no_replace BEFORE INSERT ON listener_saved
WHEN EXISTS (
    SELECT 1 FROM listener_saved WHERE listener_id = NEW.listener_id AND work_id = NEW.work_id
)
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.listener_saved'); END;

-- Receipts and playback sessions are settled the moment they are written.
CREATE TRIGGER listener_operations_no_update BEFORE UPDATE ON listener_operations
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.listener_operations'); END;
CREATE TRIGGER listener_operations_no_delete BEFORE DELETE ON listener_operations
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.listener_operations'); END;
CREATE TRIGGER listener_operations_no_replace BEFORE INSERT ON listener_operations
WHEN EXISTS (
    SELECT 1 FROM listener_operations
    WHERE listener_id = NEW.listener_id AND operation_id = NEW.operation_id
)
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.listener_operations'); END;

CREATE TRIGGER listener_playback_sessions_no_update BEFORE UPDATE ON listener_playback_sessions
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.listener_playback_sessions'); END;
CREATE TRIGGER listener_playback_sessions_no_delete BEFORE DELETE ON listener_playback_sessions
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.listener_playback_sessions'); END;
CREATE TRIGGER listener_playback_sessions_no_replace BEFORE INSERT ON listener_playback_sessions
WHEN EXISTS (
    SELECT 1 FROM listener_playback_sessions
    WHERE listener_id = NEW.listener_id AND device_id = NEW.device_id
        AND engine_session_id = NEW.engine_session_id
)
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.listener_playback_sessions'); END;

-- A device sequence never moves backwards or away (the adapter's upsert is conditional on it).
CREATE TRIGGER listener_device_sequences_forward BEFORE UPDATE ON listener_device_sequences
WHEN NEW.listener_id <> OLD.listener_id OR NEW.device_id <> OLD.device_id
    OR NEW.resource_key <> OLD.resource_key OR NEW.sequence <= OLD.sequence
BEGIN SELECT RAISE(ABORT, 'cantos:listener_device_sequences.forward'); END;
CREATE TRIGGER listener_device_sequences_no_delete BEFORE DELETE ON listener_device_sequences
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.listener_device_sequences'); END;

-- A bookmark's value never changes; the one permitted change is the final tombstone.
-- (PostgreSQL `guard_listener_bookmark`.)
CREATE TRIGGER listener_bookmarks_guard BEFORE UPDATE ON listener_bookmarks
WHEN NEW.listener_id IS NOT OLD.listener_id OR NEW.bookmark_id IS NOT OLD.bookmark_id
    OR NEW.publication_id IS NOT OLD.publication_id OR NEW.work_id IS NOT OLD.work_id
    OR NEW.unit_id IS NOT OLD.unit_id OR NEW.rendition_id IS NOT OLD.rendition_id
    OR NEW.sha256 IS NOT OLD.sha256 OR NEW.duration_ms IS NOT OLD.duration_ms
    OR NEW.position_ms IS NOT OLD.position_ms
    OR OLD.deleted = 1 OR NEW.deleted = 0 OR NEW.listener_order <= OLD.listener_order
BEGIN SELECT RAISE(ABORT, 'cantos:listener_bookmarks.tombstone'); END;
CREATE TRIGGER listener_bookmarks_no_delete BEFORE DELETE ON listener_bookmarks
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.listener_bookmarks'); END;
CREATE TRIGGER listener_bookmarks_no_replace BEFORE INSERT ON listener_bookmarks
WHEN EXISTS (
    SELECT 1 FROM listener_bookmarks
    WHERE listener_id = NEW.listener_id AND bookmark_id = NEW.bookmark_id
)
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.listener_bookmarks'); END;
