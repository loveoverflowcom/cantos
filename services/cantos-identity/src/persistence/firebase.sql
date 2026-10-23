CREATE TABLE firebase_signup_enrollments (
    operation_id BLOB PRIMARY KEY NOT NULL REFERENCES signup_workflows(workflow_id),
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
CREATE TRIGGER firebase_signup_binding BEFORE UPDATE ON firebase_signup_enrollments
WHEN NEW.operation_id<>OLD.operation_id OR NEW.proof_id<>OLD.proof_id
    OR NEW.project<>OLD.project OR NEW.email<>OLD.email OR NEW.expires_at<>OLD.expires_at
    OR (NEW.envelope IS NOT OLD.envelope AND NOT(NEW.envelope IS NULL AND NEW.state=11))
    OR (OLD.created=1 AND NEW.created<>1)
    OR (OLD.request_digest IS NOT NULL AND NEW.request_digest IS NOT OLD.request_digest)
BEGIN SELECT RAISE(ABORT, 'cantos:identity.firebase_binding'); END;
CREATE TRIGGER firebase_signup_no_delete BEFORE DELETE ON firebase_signup_enrollments
BEGIN SELECT RAISE(ABORT, 'cantos:identity.firebase_receipt'); END;
CREATE TABLE firebase_signup_mailboxes (
    project TEXT NOT NULL,
    email TEXT NOT NULL,
    operation_id BLOB NOT NULL UNIQUE REFERENCES firebase_signup_enrollments(operation_id),
    PRIMARY KEY(project,email)
) STRICT;

CREATE TABLE firebase_verification_events (
    event_seq INTEGER PRIMARY KEY NOT NULL CHECK(event_seq>0),
    operation_id BLOB NOT NULL REFERENCES firebase_signup_enrollments(operation_id),
    state INTEGER NOT NULL CHECK(state BETWEEN 1 AND 12),
    created INTEGER NOT NULL CHECK(created IN (0,1))
) STRICT;
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
