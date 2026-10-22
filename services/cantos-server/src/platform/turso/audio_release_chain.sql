-- Server.db version 14: the external-audio release chain that Theatre discovery, manifests,
-- playback authorization and media delivery read (#333 CMP business API slice): submission ->
-- rendition -> candidate -> clearance/approval -> release -> publication pointer, the withdrawal
-- and submission-revocation facts, and the listener discovery index. Upload intents, intake
-- receipts, reviews, publication events and operations belong to the Media write path, which this
-- version does not port.
--
-- Every row is an immutable domain snapshot with its canonical digest; the pointer is the only
-- row that moves, and only forward by one version. Timestamps are the domain's Unix milliseconds.
-- `REPLACE` deletes a conflicting row without firing delete triggers, so each guarded table also
-- refuses an insert over any of its unique keys. DDL only.

CREATE TABLE audio_submissions (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        coalesce(id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$', 0)
    ),
    unit_id TEXT NOT NULL REFERENCES catalog_units(id),
    owner_id TEXT NOT NULL REFERENCES actors(id),
    brief_id TEXT NOT NULL,
    brief_digest TEXT NOT NULL,
    operation_id TEXT NOT NULL CHECK (
        coalesce(operation_id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$', 0)
    ),
    request_digest TEXT NOT NULL CHECK (
        coalesce(request_digest REGEXP '^as-request1:sha256:[a-f0-9]{64}$', 0)
    ),
    token_hash BLOB NOT NULL UNIQUE CHECK (length(token_hash) = 32),
    snapshot BLOB NOT NULL CHECK (length(snapshot) BETWEEN 1 AND 32768),
    digest TEXT NOT NULL CHECK (coalesce(digest REGEXP '^as-r1:sha256:[a-f0-9]{64}$', 0)),
    UNIQUE (id, owner_id),
    UNIQUE (owner_id, operation_id),
    FOREIGN KEY (brief_id, unit_id, owner_id, brief_digest)
        REFERENCES production_briefs(id, unit_id, owner_id, digest)
) STRICT;

CREATE TABLE audio_renditions (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        coalesce(id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$', 0)
    ),
    submission_id TEXT NOT NULL UNIQUE REFERENCES audio_submissions(id),
    object_key TEXT NOT NULL CHECK (coalesce(object_key REGEXP '^[a-f0-9]{64}$', 0)),
    snapshot BLOB NOT NULL CHECK (length(snapshot) BETWEEN 1 AND 32768),
    digest TEXT NOT NULL CHECK (coalesce(digest REGEXP '^ar-r1:sha256:[a-f0-9]{64}$', 0)),
    storage_target_id TEXT NOT NULL DEFAULT 'legacy-filesystem' CHECK (
        coalesce(storage_target_id REGEXP '^[a-z][a-z0-9-]{0,62}$', 0)
        AND NOT coalesce(storage_target_id REGEXP '-$', 1)
    )
) STRICT;

CREATE TABLE audio_release_candidates (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        coalesce(id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$', 0)
    ),
    rendition_id TEXT NOT NULL REFERENCES audio_renditions(id),
    owner_id TEXT NOT NULL REFERENCES actors(id),
    operation_id TEXT NOT NULL CHECK (
        coalesce(operation_id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$', 0)
    ),
    snapshot BLOB NOT NULL CHECK (length(snapshot) BETWEEN 1 AND 32768),
    digest TEXT NOT NULL CHECK (coalesce(digest REGEXP '^rc-r1:sha256:[a-f0-9]{64}$', 0)),
    UNIQUE (id, owner_id),
    UNIQUE (owner_id, operation_id)
) STRICT;

CREATE TABLE audio_distribution_clearances (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        coalesce(id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$', 0)
    ),
    candidate_id TEXT NOT NULL,
    owner_id TEXT NOT NULL REFERENCES actors(id),
    operation_id TEXT NOT NULL CHECK (
        coalesce(operation_id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$', 0)
    ),
    sequence INTEGER NOT NULL CHECK (sequence > 0),
    snapshot BLOB NOT NULL CHECK (length(snapshot) BETWEEN 1 AND 32768),
    digest TEXT NOT NULL CHECK (coalesce(digest REGEXP '^dc-r1:sha256:[a-f0-9]{64}$', 0)),
    UNIQUE (candidate_id, sequence),
    UNIQUE (owner_id, operation_id),
    FOREIGN KEY (candidate_id, owner_id) REFERENCES audio_release_candidates(id, owner_id)
) STRICT;

CREATE TABLE audio_publication_approvals (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        coalesce(id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$', 0)
    ),
    candidate_id TEXT NOT NULL,
    owner_id TEXT NOT NULL REFERENCES actors(id),
    operation_id TEXT NOT NULL CHECK (
        coalesce(operation_id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$', 0)
    ),
    sequence INTEGER NOT NULL CHECK (sequence > 0),
    snapshot BLOB NOT NULL CHECK (length(snapshot) BETWEEN 1 AND 8192),
    digest TEXT NOT NULL CHECK (coalesce(digest REGEXP '^pa-r1:sha256:[a-f0-9]{64}$', 0)),
    UNIQUE (candidate_id, sequence),
    UNIQUE (owner_id, operation_id),
    FOREIGN KEY (candidate_id, owner_id) REFERENCES audio_release_candidates(id, owner_id)
) STRICT;

CREATE TABLE audio_releases (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        coalesce(id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$', 0)
    ),
    unit_id TEXT NOT NULL REFERENCES catalog_units(id),
    owner_id TEXT NOT NULL REFERENCES actors(id),
    candidate_id TEXT NOT NULL,
    rendition_id TEXT NOT NULL REFERENCES audio_renditions(id),
    snapshot BLOB NOT NULL CHECK (length(snapshot) BETWEEN 1 AND 262144),
    manifest_digest TEXT NOT NULL CHECK (
        coalesce(manifest_digest REGEXP '^rm-r1:sha256:[a-f0-9]{64}$', 0)
    ),
    digest TEXT NOT NULL CHECK (coalesce(digest REGEXP '^rl-r1:sha256:[a-f0-9]{64}$', 0)),
    UNIQUE (id, unit_id, owner_id),
    FOREIGN KEY (candidate_id, owner_id) REFERENCES audio_release_candidates(id, owner_id)
) STRICT;

CREATE TABLE audio_publication_pointers (
    unit_id TEXT PRIMARY KEY NOT NULL REFERENCES catalog_units(id),
    owner_id TEXT NOT NULL REFERENCES actors(id),
    active_release_id TEXT,
    version INTEGER NOT NULL CHECK (version >= 0),
    -- A NULL release (withdrawn) skips the composite check, as in PostgreSQL's MATCH SIMPLE.
    FOREIGN KEY (active_release_id, unit_id, owner_id)
        REFERENCES audio_releases(id, unit_id, owner_id)
) STRICT;

CREATE TABLE audio_release_withdrawals (
    release_id TEXT PRIMARY KEY NOT NULL REFERENCES audio_releases(id),
    actor_id TEXT NOT NULL REFERENCES actors(id),
    recorded_at INTEGER NOT NULL CHECK (recorded_at >= 0),
    reason TEXT NOT NULL CHECK (octet_length(reason) BETWEEN 1 AND 4096)
) STRICT;

CREATE TABLE audio_submission_revocations (
    submission_id TEXT PRIMARY KEY NOT NULL CHECK (
        coalesce(submission_id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$', 0)
    ),
    owner_id TEXT NOT NULL,
    revoked_at INTEGER NOT NULL CHECK (revoked_at >= 0),
    FOREIGN KEY (submission_id, owner_id) REFERENCES audio_submissions(id, owner_id)
) STRICT;

-- The listener discovery index: the first release of a Work anchors its place in the catalog and
-- the first position of a Unit anchors its place in the Work, both for good.
CREATE TABLE audio_listener_works (
    work_id TEXT PRIMARY KEY NOT NULL REFERENCES catalog_works(id),
    first_release_id TEXT NOT NULL REFERENCES audio_releases(id),
    first_published_at INTEGER NOT NULL CHECK (first_published_at >= 0)
) STRICT;
CREATE INDEX audio_listener_works_recent ON audio_listener_works(first_published_at DESC, work_id);

CREATE TABLE audio_listener_units (
    unit_id TEXT PRIMARY KEY NOT NULL REFERENCES catalog_units(id),
    work_id TEXT NOT NULL REFERENCES catalog_works(id),
    first_position INTEGER NOT NULL CHECK (first_position >= 0),
    UNIQUE (unit_id, work_id)
) STRICT;
CREATE INDEX audio_listener_units_order ON audio_listener_units(work_id, first_position, unit_id);

CREATE TABLE audio_listener_releases (
    release_id TEXT PRIMARY KEY NOT NULL REFERENCES audio_releases(id),
    work_id TEXT NOT NULL REFERENCES catalog_works(id),
    unit_id TEXT NOT NULL,
    published_at INTEGER NOT NULL CHECK (published_at >= 0),
    kind TEXT NOT NULL CHECK (kind IN ('novel', 'story', 'podcast')),
    language TEXT NOT NULL CHECK (octet_length(language) BETWEEN 1 AND 63),
    search_text TEXT NOT NULL CHECK (octet_length(search_text) <= 32768),
    rights_digest TEXT NOT NULL CHECK (
        coalesce(rights_digest REGEXP '^dc-r1:sha256:[a-f0-9]{64}$', 0)
    ),
    approval_digest TEXT NOT NULL CHECK (
        coalesce(approval_digest REGEXP '^pa-r1:sha256:[a-f0-9]{64}$', 0)
    ),
    rights_start INTEGER NOT NULL CHECK (rights_start >= 0),
    rights_end INTEGER NOT NULL,
    work_revision INTEGER NOT NULL CHECK (work_revision > 0),
    unit_revision INTEGER NOT NULL CHECK (unit_revision > 0),
    content_revision_id TEXT NOT NULL REFERENCES catalog_content_revisions(id),
    CHECK (rights_end > rights_start),
    FOREIGN KEY (unit_id, work_id) REFERENCES audio_listener_units(unit_id, work_id)
) STRICT;
CREATE INDEX audio_listener_releases_unit ON audio_listener_releases(unit_id, release_id);
CREATE INDEX audio_listener_releases_work ON audio_listener_releases(work_id, published_at DESC, release_id);

-- Immutable settled rows: no update, no delete, no insert over an existing unique key.
CREATE TRIGGER audio_submissions_no_update BEFORE UPDATE ON audio_submissions
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.audio_submissions'); END;
CREATE TRIGGER audio_submissions_no_delete BEFORE DELETE ON audio_submissions
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.audio_submissions'); END;
CREATE TRIGGER audio_submissions_no_replace BEFORE INSERT ON audio_submissions
WHEN EXISTS (
    SELECT 1 FROM audio_submissions
    WHERE id = NEW.id OR token_hash = NEW.token_hash
        OR (owner_id = NEW.owner_id AND operation_id = NEW.operation_id)
)
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.audio_submissions'); END;

CREATE TRIGGER audio_renditions_no_update BEFORE UPDATE ON audio_renditions
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.audio_renditions'); END;
CREATE TRIGGER audio_renditions_no_delete BEFORE DELETE ON audio_renditions
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.audio_renditions'); END;
CREATE TRIGGER audio_renditions_no_replace BEFORE INSERT ON audio_renditions
WHEN EXISTS (SELECT 1 FROM audio_renditions WHERE id = NEW.id OR submission_id = NEW.submission_id)
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.audio_renditions'); END;

CREATE TRIGGER audio_release_candidates_no_update BEFORE UPDATE ON audio_release_candidates
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.audio_release_candidates'); END;
CREATE TRIGGER audio_release_candidates_no_delete BEFORE DELETE ON audio_release_candidates
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.audio_release_candidates'); END;
CREATE TRIGGER audio_release_candidates_no_replace BEFORE INSERT ON audio_release_candidates
WHEN EXISTS (
    SELECT 1 FROM audio_release_candidates
    WHERE id = NEW.id OR (owner_id = NEW.owner_id AND operation_id = NEW.operation_id)
)
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.audio_release_candidates'); END;

CREATE TRIGGER audio_distribution_clearances_no_update BEFORE UPDATE ON audio_distribution_clearances
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.audio_distribution_clearances'); END;
CREATE TRIGGER audio_distribution_clearances_no_delete BEFORE DELETE ON audio_distribution_clearances
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.audio_distribution_clearances'); END;
CREATE TRIGGER audio_distribution_clearances_no_replace BEFORE INSERT ON audio_distribution_clearances
WHEN EXISTS (
    SELECT 1 FROM audio_distribution_clearances
    WHERE id = NEW.id OR (candidate_id = NEW.candidate_id AND sequence = NEW.sequence)
        OR (owner_id = NEW.owner_id AND operation_id = NEW.operation_id)
)
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.audio_distribution_clearances'); END;

CREATE TRIGGER audio_publication_approvals_no_update BEFORE UPDATE ON audio_publication_approvals
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.audio_publication_approvals'); END;
CREATE TRIGGER audio_publication_approvals_no_delete BEFORE DELETE ON audio_publication_approvals
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.audio_publication_approvals'); END;
CREATE TRIGGER audio_publication_approvals_no_replace BEFORE INSERT ON audio_publication_approvals
WHEN EXISTS (
    SELECT 1 FROM audio_publication_approvals
    WHERE id = NEW.id OR (candidate_id = NEW.candidate_id AND sequence = NEW.sequence)
        OR (owner_id = NEW.owner_id AND operation_id = NEW.operation_id)
)
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.audio_publication_approvals'); END;

CREATE TRIGGER audio_releases_no_update BEFORE UPDATE ON audio_releases
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.audio_releases'); END;
CREATE TRIGGER audio_releases_no_delete BEFORE DELETE ON audio_releases
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.audio_releases'); END;
CREATE TRIGGER audio_releases_no_replace BEFORE INSERT ON audio_releases
WHEN EXISTS (SELECT 1 FROM audio_releases WHERE id = NEW.id)
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.audio_releases'); END;

CREATE TRIGGER audio_release_withdrawals_no_update BEFORE UPDATE ON audio_release_withdrawals
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.audio_release_withdrawals'); END;
CREATE TRIGGER audio_release_withdrawals_no_delete BEFORE DELETE ON audio_release_withdrawals
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.audio_release_withdrawals'); END;
CREATE TRIGGER audio_release_withdrawals_no_replace BEFORE INSERT ON audio_release_withdrawals
WHEN EXISTS (SELECT 1 FROM audio_release_withdrawals WHERE release_id = NEW.release_id)
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.audio_release_withdrawals'); END;

CREATE TRIGGER audio_submission_revocations_no_update BEFORE UPDATE ON audio_submission_revocations
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.audio_submission_revocations'); END;
CREATE TRIGGER audio_submission_revocations_no_delete BEFORE DELETE ON audio_submission_revocations
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.audio_submission_revocations'); END;
CREATE TRIGGER audio_submission_revocations_no_replace BEFORE INSERT ON audio_submission_revocations
WHEN EXISTS (SELECT 1 FROM audio_submission_revocations WHERE submission_id = NEW.submission_id)
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.audio_submission_revocations'); END;

CREATE TRIGGER audio_listener_works_no_update BEFORE UPDATE ON audio_listener_works
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.audio_listener_index'); END;
CREATE TRIGGER audio_listener_works_no_delete BEFORE DELETE ON audio_listener_works
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.audio_listener_index'); END;
CREATE TRIGGER audio_listener_works_no_replace BEFORE INSERT ON audio_listener_works
WHEN EXISTS (SELECT 1 FROM audio_listener_works WHERE work_id = NEW.work_id)
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.audio_listener_index'); END;

CREATE TRIGGER audio_listener_units_no_update BEFORE UPDATE ON audio_listener_units
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.audio_listener_index'); END;
CREATE TRIGGER audio_listener_units_no_delete BEFORE DELETE ON audio_listener_units
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.audio_listener_index'); END;
CREATE TRIGGER audio_listener_units_no_replace BEFORE INSERT ON audio_listener_units
WHEN EXISTS (SELECT 1 FROM audio_listener_units WHERE unit_id = NEW.unit_id)
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.audio_listener_index'); END;

CREATE TRIGGER audio_listener_releases_no_update BEFORE UPDATE ON audio_listener_releases
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.audio_listener_index'); END;
CREATE TRIGGER audio_listener_releases_no_delete BEFORE DELETE ON audio_listener_releases
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.audio_listener_index'); END;
CREATE TRIGGER audio_listener_releases_no_replace BEFORE INSERT ON audio_listener_releases
WHEN EXISTS (SELECT 1 FROM audio_listener_releases WHERE release_id = NEW.release_id)
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.audio_listener_index'); END;

-- The pointer moves only forward by exactly one version, never changes unit or owner, is never
-- deleted and is never replaced (PostgreSQL `guard_audio_publication_pointer`).
CREATE TRIGGER audio_publication_pointers_forward BEFORE UPDATE ON audio_publication_pointers
WHEN NEW.unit_id <> OLD.unit_id OR NEW.owner_id <> OLD.owner_id OR NEW.version <> OLD.version + 1
BEGIN SELECT RAISE(ABORT, 'cantos:audio_publication_pointers.forward'); END;
CREATE TRIGGER audio_publication_pointers_no_delete BEFORE DELETE ON audio_publication_pointers
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.audio_publication_pointers'); END;
CREATE TRIGGER audio_publication_pointers_no_replace BEFORE INSERT ON audio_publication_pointers
WHEN EXISTS (SELECT 1 FROM audio_publication_pointers WHERE unit_id = NEW.unit_id)
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.audio_publication_pointers'); END;
