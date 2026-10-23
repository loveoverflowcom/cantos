CREATE TABLE mailbox_proof_issues (
    proof_id BLOB PRIMARY KEY NOT NULL CHECK (length(proof_id) = 16 AND proof_id <> x'00000000000000000000000000000000'),
    workflow_id BLOB NOT NULL CHECK (length(workflow_id) = 16 AND workflow_id <> x'00000000000000000000000000000000'),
    secret_digest BLOB NOT NULL UNIQUE CHECK (length(secret_digest) = 32),
    issued_at INTEGER NOT NULL CHECK (issued_at >= 0),
    expires_at INTEGER NOT NULL CHECK (expires_at > issued_at),
    UNIQUE (proof_id, workflow_id)
) STRICT;
CREATE TRIGGER mailbox_proof_issues_no_update BEFORE UPDATE ON mailbox_proof_issues
BEGIN SELECT RAISE(ABORT, 'cantos:identity.immutable_proof_issue'); END;
CREATE TRIGGER mailbox_proof_issues_no_delete BEFORE DELETE ON mailbox_proof_issues
BEGIN SELECT RAISE(ABORT, 'cantos:identity.immutable_proof_issue'); END;

CREATE UNIQUE INDEX identity_instance_epoch_unique ON identity_instance(instance_epoch);
CREATE TABLE mailbox_proof_workflows (
    workflow_id BLOB PRIMARY KEY NOT NULL CHECK (length(workflow_id) = 16 AND workflow_id <> x'00000000000000000000000000000000'),
    instance_epoch BLOB NOT NULL REFERENCES identity_instance(instance_epoch),
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

CREATE TABLE mailbox_proof_receipts (
    command_id BLOB PRIMARY KEY NOT NULL CHECK (length(command_id) = 16 AND command_id <> x'00000000000000000000000000000000'),
    workflow_id BLOB NOT NULL REFERENCES mailbox_proof_workflows(workflow_id),
    action INTEGER NOT NULL CHECK (action IN (1, 2, 3, 4)),
    request_digest BLOB NOT NULL CHECK (length(request_digest) = 32),
    result_proof_id BLOB NOT NULL,
    result_state INTEGER NOT NULL,
    CHECK ((action IN (1, 4) AND result_state = 1) OR (action = 2 AND result_state = 2) OR (action = 3 AND result_state = 3)),
    FOREIGN KEY (result_proof_id, workflow_id) REFERENCES mailbox_proof_issues(proof_id, workflow_id)
) STRICT;
CREATE TRIGGER mailbox_proof_receipts_no_update BEFORE UPDATE ON mailbox_proof_receipts
BEGIN SELECT RAISE(ABORT, 'cantos:identity.immutable_proof_receipt'); END;
CREATE TRIGGER mailbox_proof_receipts_no_delete BEFORE DELETE ON mailbox_proof_receipts
BEGIN SELECT RAISE(ABORT, 'cantos:identity.immutable_proof_receipt'); END;
