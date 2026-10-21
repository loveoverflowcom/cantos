CREATE TABLE signup_workflows (
    workflow_id BLOB PRIMARY KEY NOT NULL REFERENCES mailbox_proof_workflows(workflow_id)
        CHECK (length(workflow_id) = 16 AND workflow_id <> x'00000000000000000000000000000000'),
    email TEXT NOT NULL CHECK (length(email) BETWEEN 3 AND 254),
    requested_at INTEGER NOT NULL CHECK (requested_at >= 0)
) STRICT;
CREATE TRIGGER signup_workflows_no_update BEFORE UPDATE ON signup_workflows
BEGIN SELECT RAISE(ABORT, 'cantos:identity.immutable_signup'); END;
CREATE TRIGGER signup_workflows_no_delete BEFORE DELETE ON signup_workflows
BEGIN SELECT RAISE(ABORT, 'cantos:identity.immutable_signup'); END;

CREATE TABLE identity_emails (
    email TEXT PRIMARY KEY NOT NULL CHECK (length(email) BETWEEN 3 AND 254),
    subject_id BLOB NOT NULL UNIQUE REFERENCES identity_subjects(subject_id),
    verified_at INTEGER NOT NULL CHECK (verified_at >= 0),
    signup_workflow BLOB NOT NULL UNIQUE REFERENCES signup_workflows(workflow_id)
) STRICT;
CREATE TRIGGER identity_emails_no_update BEFORE UPDATE ON identity_emails
BEGIN SELECT RAISE(ABORT, 'cantos:identity.immutable_email'); END;
CREATE TRIGGER identity_emails_no_delete BEFORE DELETE ON identity_emails
BEGIN SELECT RAISE(ABORT, 'cantos:identity.immutable_email'); END;

CREATE TABLE password_credentials (
    subject_id BLOB PRIMARY KEY NOT NULL REFERENCES identity_subjects(subject_id),
    credential_version INTEGER NOT NULL CHECK (credential_version >= 1),
    phc TEXT NOT NULL CHECK (length(phc) BETWEEN 32 AND 512),
    created_at INTEGER NOT NULL CHECK (created_at >= 0)
) STRICT;
CREATE TRIGGER password_credentials_no_update BEFORE UPDATE ON password_credentials
BEGIN SELECT RAISE(ABORT, 'cantos:identity.immutable_credential'); END;
CREATE TRIGGER password_credentials_no_delete BEFORE DELETE ON password_credentials
BEGIN SELECT RAISE(ABORT, 'cantos:identity.immutable_credential'); END;

CREATE TABLE signup_completions (
    command_id BLOB PRIMARY KEY NOT NULL REFERENCES mailbox_proof_receipts(command_id),
    workflow_id BLOB NOT NULL UNIQUE REFERENCES signup_workflows(workflow_id),
    subject_id BLOB NOT NULL UNIQUE REFERENCES identity_subjects(subject_id)
) STRICT;
CREATE TRIGGER signup_completions_no_update BEFORE UPDATE ON signup_completions
BEGIN SELECT RAISE(ABORT, 'cantos:identity.immutable_signup_completion'); END;
CREATE TRIGGER signup_completions_no_delete BEFORE DELETE ON signup_completions
BEGIN SELECT RAISE(ABORT, 'cantos:identity.immutable_signup_completion'); END;
