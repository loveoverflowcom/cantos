CREATE TABLE identity_instance (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    instance_epoch BLOB NOT NULL CHECK (length(instance_epoch) = 16 AND instance_epoch <> x'00000000000000000000000000000000')
) STRICT;
CREATE TRIGGER identity_instance_no_update BEFORE UPDATE ON identity_instance
BEGIN SELECT RAISE(ABORT, 'cantos:identity.immutable_instance'); END;
CREATE TRIGGER identity_instance_no_delete BEFORE DELETE ON identity_instance
BEGIN SELECT RAISE(ABORT, 'cantos:identity.immutable_instance'); END;

CREATE TABLE identity_subjects (
    subject_id BLOB PRIMARY KEY NOT NULL CHECK (length(subject_id) = 16 AND subject_id <> x'00000000000000000000000000000000'),
    security_version INTEGER NOT NULL CHECK (security_version >= 1),
    state INTEGER NOT NULL CHECK (state IN (1, 2))
) STRICT;
CREATE TRIGGER identity_subjects_no_delete BEFORE DELETE ON identity_subjects
BEGIN SELECT RAISE(ABORT, 'cantos:identity.immutable_subject'); END;
CREATE TRIGGER identity_subjects_no_id_change BEFORE UPDATE ON identity_subjects
WHEN NEW.subject_id <> OLD.subject_id
BEGIN SELECT RAISE(ABORT, 'cantos:identity.immutable_subject'); END;
CREATE TRIGGER identity_subjects_transition BEFORE UPDATE ON identity_subjects
WHEN NEW.security_version <> OLD.security_version + 1 OR OLD.state <> 1 OR NEW.state <> 2
BEGIN SELECT RAISE(ABORT, 'cantos:identity.subject_transition'); END;

CREATE TABLE security_events (
    seq INTEGER PRIMARY KEY CHECK (seq >= 1),
    subject_id BLOB NOT NULL REFERENCES identity_subjects(subject_id),
    security_version INTEGER NOT NULL CHECK (security_version >= 1),
    state INTEGER NOT NULL,
    kind INTEGER NOT NULL,
    recorded_at INTEGER NOT NULL CHECK (recorded_at >= 0),
    CHECK ((kind = 1 AND state = 1 AND security_version = 1) OR (kind = 2 AND state = 2 AND security_version >= 2)),
    UNIQUE (subject_id, security_version),
    UNIQUE (seq, subject_id, security_version, kind)
) STRICT;
CREATE TRIGGER security_events_no_update BEFORE UPDATE ON security_events
BEGIN SELECT RAISE(ABORT, 'cantos:identity.immutable_event'); END;
CREATE TRIGGER security_events_no_delete BEFORE DELETE ON security_events
BEGIN SELECT RAISE(ABORT, 'cantos:identity.immutable_event'); END;

CREATE TABLE operation_receipts (
    operation_id BLOB PRIMARY KEY NOT NULL CHECK (length(operation_id) = 16 AND operation_id <> x'00000000000000000000000000000000'),
    subject_id BLOB NOT NULL REFERENCES identity_subjects(subject_id),
    kind INTEGER NOT NULL CHECK (kind IN (1, 2)),
    expected_version INTEGER NOT NULL CHECK (expected_version >= 0),
    committed_version INTEGER NOT NULL CHECK (committed_version >= 1),
    event_seq INTEGER NOT NULL UNIQUE,
    CHECK ((kind = 1 AND expected_version = 0 AND committed_version = 1) OR (kind = 2 AND expected_version >= 1 AND committed_version = expected_version + 1)),
    FOREIGN KEY (event_seq, subject_id, committed_version, kind) REFERENCES security_events(seq, subject_id, security_version, kind)
) STRICT;
CREATE TRIGGER operation_receipts_no_update BEFORE UPDATE ON operation_receipts
BEGIN SELECT RAISE(ABORT, 'cantos:identity.immutable_receipt'); END;
CREATE TRIGGER operation_receipts_no_delete BEFORE DELETE ON operation_receipts
BEGIN SELECT RAISE(ABORT, 'cantos:identity.immutable_receipt'); END;
