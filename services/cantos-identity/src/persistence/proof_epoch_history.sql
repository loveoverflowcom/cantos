-- Preserve historical proof epochs instead of binding them to the mutable current epoch.
-- Rebuild the complete FK-dependent graph in one owner migration; credentials are untouched.
CREATE TABLE identity_epochs (
    instance_epoch BLOB PRIMARY KEY NOT NULL CHECK(length(instance_epoch)=16 AND instance_epoch<>x'00000000000000000000000000000000')
) STRICT;
INSERT INTO identity_epochs SELECT instance_epoch FROM identity_instance
UNION SELECT previous_epoch FROM identity_restore_epochs
UNION SELECT new_epoch FROM identity_restore_epochs;
CREATE TRIGGER identity_epochs_no_update BEFORE UPDATE ON identity_epochs
BEGIN SELECT RAISE(ABORT, 'cantos:identity.immutable_instance'); END;
CREATE TRIGGER identity_epochs_no_delete BEFORE DELETE ON identity_epochs
BEGIN SELECT RAISE(ABORT, 'cantos:identity.immutable_instance'); END;
CREATE TRIGGER identity_epochs_initial AFTER INSERT ON identity_instance
BEGIN INSERT INTO identity_epochs(instance_epoch) VALUES(NEW.instance_epoch); END;
CREATE TRIGGER identity_epochs_rotation AFTER UPDATE ON identity_instance
WHEN NEW.instance_epoch<>OLD.instance_epoch
BEGIN INSERT INTO identity_epochs(instance_epoch) VALUES(NEW.instance_epoch); END;

CREATE TABLE mailbox_proof_workflows_v7 (
    workflow_id BLOB PRIMARY KEY NOT NULL CHECK (length(workflow_id) = 16 AND workflow_id <> x'00000000000000000000000000000000'),
    instance_epoch BLOB NOT NULL REFERENCES identity_epochs(instance_epoch),
    intent INTEGER NOT NULL CHECK (intent IN (1, 2)),
    recovery_subject BLOB REFERENCES identity_subjects(subject_id),
    recovery_version INTEGER,
    purpose INTEGER,
    current_proof_id BLOB NOT NULL,
    state INTEGER NOT NULL CHECK (state IN (1, 2, 3)),
    CHECK ((intent = 1 AND recovery_subject IS NULL AND recovery_version IS NULL AND purpose IS NULL)
        OR (intent = 2 AND recovery_subject IS NOT NULL AND recovery_version IS NOT NULL
            AND recovery_version >= 1 AND purpose IS NOT NULL AND purpose IN (1, 2))),
    FOREIGN KEY (current_proof_id, workflow_id) REFERENCES mailbox_proof_issues(proof_id, workflow_id)
) STRICT;
INSERT INTO mailbox_proof_workflows_v7 SELECT * FROM mailbox_proof_workflows;

CREATE TABLE mailbox_proof_receipts_v7 (
    command_id BLOB PRIMARY KEY NOT NULL CHECK (length(command_id) = 16 AND command_id <> x'00000000000000000000000000000000'),
    workflow_id BLOB NOT NULL REFERENCES mailbox_proof_workflows_v7(workflow_id),
    action INTEGER NOT NULL CHECK (action IN (1, 2, 3, 4)),
    request_digest BLOB NOT NULL CHECK (length(request_digest) = 32),
    result_proof_id BLOB NOT NULL,
    result_state INTEGER NOT NULL,
    CHECK ((action IN (1, 4) AND result_state = 1) OR (action = 2 AND result_state = 2) OR (action = 3 AND result_state = 3)),
    FOREIGN KEY (result_proof_id, workflow_id) REFERENCES mailbox_proof_issues(proof_id, workflow_id)
) STRICT;
INSERT INTO mailbox_proof_receipts_v7 SELECT * FROM mailbox_proof_receipts;

CREATE TABLE signup_workflows_v7 (
    workflow_id BLOB PRIMARY KEY NOT NULL REFERENCES mailbox_proof_workflows_v7(workflow_id)
        CHECK (length(workflow_id) = 16 AND workflow_id <> x'00000000000000000000000000000000'),
    email TEXT NOT NULL CHECK (length(email) BETWEEN 3 AND 254),
    requested_at INTEGER NOT NULL CHECK (requested_at >= 0)
) STRICT;
INSERT INTO signup_workflows_v7 SELECT * FROM signup_workflows;

CREATE TABLE identity_emails_v7 (
    email TEXT PRIMARY KEY NOT NULL CHECK (length(email) BETWEEN 3 AND 254),
    subject_id BLOB NOT NULL UNIQUE REFERENCES identity_subjects(subject_id),
    verified_at INTEGER NOT NULL CHECK (verified_at >= 0),
    signup_workflow BLOB NOT NULL UNIQUE REFERENCES signup_workflows_v7(workflow_id)
) STRICT;
INSERT INTO identity_emails_v7 SELECT * FROM identity_emails;

CREATE TABLE signup_completions_v7 (
    command_id BLOB PRIMARY KEY NOT NULL REFERENCES mailbox_proof_receipts_v7(command_id),
    workflow_id BLOB NOT NULL UNIQUE REFERENCES signup_workflows_v7(workflow_id),
    subject_id BLOB NOT NULL UNIQUE REFERENCES identity_subjects(subject_id)
) STRICT;
INSERT INTO signup_completions_v7 SELECT * FROM signup_completions;

CREATE TABLE firebase_signup_enrollments_v7 (
    operation_id BLOB PRIMARY KEY NOT NULL REFERENCES signup_workflows_v7(workflow_id),
    proof_id BLOB NOT NULL REFERENCES mailbox_proof_issues(proof_id),
    project TEXT NOT NULL CHECK(length(project) BETWEEN 6 AND 30),
    email TEXT NOT NULL,
    envelope BLOB CHECK(envelope IS NULL OR length(envelope)=60),
    expires_at INTEGER NOT NULL CHECK(expires_at > 0),
    state INTEGER NOT NULL CHECK(state BETWEEN 1 AND 12),
    created INTEGER NOT NULL CHECK(created IN (0,1)),
    last_send INTEGER NOT NULL CHECK(last_send>=0),
    request_digest BLOB CHECK(request_digest IS NULL OR length(request_digest)=32),
    CHECK(envelope IS NOT NULL OR state=11),
    CHECK(state NOT IN (9,10,12) OR (request_digest IS NOT NULL AND created=1))
) STRICT;
INSERT INTO firebase_signup_enrollments_v7 SELECT * FROM firebase_signup_enrollments;

CREATE TABLE firebase_signup_mailboxes_v7 (
    project TEXT NOT NULL,
    email TEXT NOT NULL,
    operation_id BLOB NOT NULL UNIQUE REFERENCES firebase_signup_enrollments_v7(operation_id),
    PRIMARY KEY(project,email)
) STRICT;
INSERT INTO firebase_signup_mailboxes_v7 SELECT * FROM firebase_signup_mailboxes;

CREATE TABLE firebase_verification_events_v7 (
    event_seq INTEGER PRIMARY KEY NOT NULL CHECK(event_seq>0),
    operation_id BLOB NOT NULL REFERENCES firebase_signup_enrollments_v7(operation_id),
    state INTEGER NOT NULL CHECK(state BETWEEN 1 AND 12),
    created INTEGER NOT NULL CHECK(created IN (0,1))
) STRICT;
INSERT INTO firebase_verification_events_v7 SELECT * FROM firebase_verification_events;

DROP TABLE firebase_verification_events;
DROP TABLE firebase_signup_mailboxes;
DROP TABLE firebase_signup_enrollments;
DROP TABLE signup_completions;
DROP TABLE identity_emails;
DROP TABLE signup_workflows;
DROP TABLE mailbox_proof_receipts;
DROP TABLE mailbox_proof_workflows;

ALTER TABLE mailbox_proof_workflows_v7 RENAME TO mailbox_proof_workflows;
ALTER TABLE mailbox_proof_receipts_v7 RENAME TO mailbox_proof_receipts;
ALTER TABLE signup_workflows_v7 RENAME TO signup_workflows;
ALTER TABLE identity_emails_v7 RENAME TO identity_emails;
ALTER TABLE signup_completions_v7 RENAME TO signup_completions;
ALTER TABLE firebase_signup_enrollments_v7 RENAME TO firebase_signup_enrollments;
ALTER TABLE firebase_signup_mailboxes_v7 RENAME TO firebase_signup_mailboxes;
ALTER TABLE firebase_verification_events_v7 RENAME TO firebase_verification_events;

CREATE TRIGGER mailbox_proof_workflows_no_delete BEFORE DELETE ON mailbox_proof_workflows
BEGIN SELECT RAISE(ABORT, 'cantos:identity.immutable_proof_workflow'); END;
CREATE TRIGGER mailbox_proof_workflows_binding BEFORE UPDATE ON mailbox_proof_workflows
WHEN NEW.workflow_id <> OLD.workflow_id OR NEW.instance_epoch <> OLD.instance_epoch
    OR NEW.intent <> OLD.intent OR NEW.recovery_subject IS NOT OLD.recovery_subject
    OR NEW.recovery_version IS NOT OLD.recovery_version OR NEW.purpose IS NOT OLD.purpose
BEGIN SELECT RAISE(ABORT, 'cantos:identity.immutable_proof_workflow'); END;
CREATE TRIGGER mailbox_proof_workflows_transition BEFORE UPDATE ON mailbox_proof_workflows
WHEN OLD.state <> 1 OR NOT (
    (NEW.state IN (2, 3) AND NEW.current_proof_id = OLD.current_proof_id)
    OR (NEW.state = 1 AND NEW.current_proof_id <> OLD.current_proof_id))
BEGIN SELECT RAISE(ABORT, 'cantos:identity.proof_transition'); END;
CREATE TRIGGER mailbox_proof_receipts_no_update BEFORE UPDATE ON mailbox_proof_receipts
BEGIN SELECT RAISE(ABORT, 'cantos:identity.immutable_proof_receipt'); END;
CREATE TRIGGER mailbox_proof_receipts_no_delete BEFORE DELETE ON mailbox_proof_receipts
BEGIN SELECT RAISE(ABORT, 'cantos:identity.immutable_proof_receipt'); END;
CREATE TRIGGER signup_workflows_no_update BEFORE UPDATE ON signup_workflows
BEGIN SELECT RAISE(ABORT, 'cantos:identity.immutable_signup'); END;
CREATE TRIGGER signup_workflows_no_delete BEFORE DELETE ON signup_workflows
BEGIN SELECT RAISE(ABORT, 'cantos:identity.immutable_signup'); END;
CREATE TRIGGER identity_emails_no_update BEFORE UPDATE ON identity_emails
BEGIN SELECT RAISE(ABORT, 'cantos:identity.immutable_email'); END;
CREATE TRIGGER identity_emails_no_delete BEFORE DELETE ON identity_emails
BEGIN SELECT RAISE(ABORT, 'cantos:identity.immutable_email'); END;
CREATE TRIGGER signup_completions_no_update BEFORE UPDATE ON signup_completions
BEGIN SELECT RAISE(ABORT, 'cantos:identity.immutable_signup_completion'); END;
CREATE TRIGGER signup_completions_no_delete BEFORE DELETE ON signup_completions
BEGIN SELECT RAISE(ABORT, 'cantos:identity.immutable_signup_completion'); END;
CREATE TRIGGER firebase_signup_binding BEFORE UPDATE ON firebase_signup_enrollments
WHEN NEW.operation_id<>OLD.operation_id OR NEW.proof_id<>OLD.proof_id
    OR NEW.project<>OLD.project OR NEW.email<>OLD.email OR NEW.expires_at<>OLD.expires_at
    OR (NEW.envelope IS NOT OLD.envelope AND NOT(NEW.envelope IS NULL AND NEW.state=11))
    OR (OLD.created=1 AND NEW.created<>1)
    OR (OLD.request_digest IS NOT NULL AND NEW.request_digest IS NOT OLD.request_digest)
BEGIN SELECT RAISE(ABORT, 'cantos:identity.firebase_binding'); END;
CREATE TRIGGER firebase_signup_no_delete BEFORE DELETE ON firebase_signup_enrollments
BEGIN SELECT RAISE(ABORT, 'cantos:identity.firebase_receipt'); END;
CREATE TRIGGER firebase_verification_events_no_update BEFORE UPDATE ON firebase_verification_events
BEGIN SELECT RAISE(ABORT, 'cantos:identity.immutable_firebase_event'); END;
CREATE TRIGGER firebase_verification_events_no_delete BEFORE DELETE ON firebase_verification_events
BEGIN SELECT RAISE(ABORT, 'cantos:identity.immutable_firebase_event'); END;
CREATE TRIGGER firebase_signup_record_insert AFTER INSERT ON firebase_signup_enrollments
BEGIN
    INSERT INTO firebase_verification_events(event_seq,operation_id,state,created)
    SELECT COALESCE(MAX(event_seq),0)+1,NEW.operation_id,NEW.state,NEW.created FROM firebase_verification_events;
END;
CREATE TRIGGER firebase_signup_record_update AFTER UPDATE ON firebase_signup_enrollments
WHEN NEW.state<>OLD.state OR NEW.created<>OLD.created
BEGIN
    INSERT INTO firebase_verification_events(event_seq,operation_id,state,created)
    SELECT COALESCE(MAX(event_seq),0)+1,NEW.operation_id,NEW.state,NEW.created FROM firebase_verification_events;
END;
CREATE TRIGGER mailbox_proof_workflows_current_epoch BEFORE INSERT ON mailbox_proof_workflows
WHEN EXISTS(SELECT 1 FROM identity_epochs WHERE instance_epoch=NEW.instance_epoch)
    AND NOT EXISTS(SELECT 1 FROM identity_instance WHERE instance_epoch=NEW.instance_epoch)
BEGIN SELECT RAISE(ABORT, 'cantos:identity.proof_epoch'); END;
