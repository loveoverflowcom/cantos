-- Firebase proves mailbox possession only. It does not create a Cantos login session.
-- Every enrollment owns a dedicated, never-reused Firebase UID and a preselected
-- Kanidm person UUID. No action code, provider token or credential-setup URL is stored.
CREATE TABLE theatre_firebase_enrollments (
    namespace text COLLATE "C" NOT NULL CHECK(length(namespace) BETWEEN 1 AND 2048),
    id uuid NOT NULL CHECK(id<>'00000000-0000-0000-0000-000000000000'::uuid),
    project text COLLATE "C" NOT NULL CHECK(length(project) BETWEEN 6 AND 30
        AND project ~ '^[a-z][a-z0-9-]*[a-z0-9]$'),
    uid text COLLATE "C" NOT NULL CHECK(uid='cantos_'||replace(id::text,'-','')),
    email text NOT NULL CHECK(octet_length(email) BETWEEN 3 AND 254),
    email_key text COLLATE "C" NOT NULL CHECK(email_key=lower(email COLLATE "C")),
    accepted_at timestamptz NOT NULL,
    expires_at timestamptz NOT NULL CHECK(expires_at>accepted_at),
    last_sent_at timestamptz NOT NULL,
    phase text NOT NULL CHECK(phase IN ('bootstrap_started','ready','send_started','mail_started',
        'awaiting_action','inspecting','consuming','creating','created','setup_started',
        'handed_off','blocked','cancelled','expired')),
    person_id uuid CHECK(person_id=id),
    block_reason text CHECK(block_reason IN ('bootstrap_rejected','bootstrap_uncertain',
        'send_rejected','send_uncertain','consume_uncertain','create_rejected',
        'create_uncertain','binding_mismatch','setup_rejected','setup_uncertain',
        'handoff_refused')),
    PRIMARY KEY(namespace,id),
    -- A later issuer namespace cannot adopt an earlier project's UID/operation.
    UNIQUE(project,uid),
    UNIQUE(project,id),
    CHECK(last_sent_at>=accepted_at AND last_sent_at<expires_at),
    CHECK((phase IN ('created','setup_started','handed_off') AND person_id IS NOT NULL)
       OR (phase NOT IN ('created','setup_started','handed_off','blocked') AND person_id IS NULL)
       OR phase='blocked'),
    CHECK((phase='blocked')=(block_reason IS NOT NULL))
);

-- Terminal expired/cancelled enrollments retain their binding. Any subsequent
-- enrollment gets a new operation and UID; in-flight/unknown outcomes stay fenced.
CREATE UNIQUE INDEX theatre_firebase_active_email
    ON theatre_firebase_enrollments(namespace,project,email_key)
    WHERE phase NOT IN ('cancelled','expired');

CREATE TABLE theatre_firebase_send_starts (
    receipt_id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    namespace text COLLATE "C" NOT NULL,
    enrollment_id uuid NOT NULL,
    network_key bytea NOT NULL CHECK(octet_length(network_key)=32),
    accepted_at timestamptz NOT NULL,
    FOREIGN KEY(namespace,enrollment_id) REFERENCES theatre_firebase_enrollments(namespace,id)
);
CREATE INDEX theatre_firebase_send_budget
    ON theatre_firebase_send_starts(namespace,accepted_at);
CREATE INDEX theatre_firebase_network_send_budget
    ON theatre_firebase_send_starts(namespace,network_key,accepted_at);
CREATE TRIGGER theatre_firebase_send_starts_immutable
    BEFORE UPDATE OR DELETE ON theatre_firebase_send_starts
    FOR EACH ROW EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER theatre_firebase_send_starts_no_truncate
    BEFORE TRUNCATE ON theatre_firebase_send_starts
    FOR EACH STATEMENT EXECUTE FUNCTION reject_settled_change();

CREATE TABLE theatre_firebase_enrollment_receipts (
    receipt_id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    namespace text COLLATE "C" NOT NULL,
    enrollment_id uuid NOT NULL,
    previous_phase text,
    phase text NOT NULL,
    block_reason text,
    recorded_at timestamptz NOT NULL,
    FOREIGN KEY(namespace,enrollment_id) REFERENCES theatre_firebase_enrollments(namespace,id)
);
CREATE TRIGGER theatre_firebase_receipts_immutable
    BEFORE UPDATE OR DELETE ON theatre_firebase_enrollment_receipts
    FOR EACH ROW EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER theatre_firebase_receipts_no_truncate
    BEFORE TRUNCATE ON theatre_firebase_enrollment_receipts
    FOR EACH STATEMENT EXECUTE FUNCTION reject_settled_change();

CREATE FUNCTION theatre_firebase_enrollment_guard() RETURNS trigger
LANGUAGE plpgsql SET search_path=pg_catalog AS $$
BEGIN
    IF TG_OP='DELETE' THEN
        RAISE EXCEPTION 'Firebase enrollment history is retained' USING ERRCODE='23514';
    END IF;
    IF TG_OP='INSERT' THEN
        IF NEW.phase<>'bootstrap_started' OR NEW.person_id IS NOT NULL THEN
            RAISE EXCEPTION 'Firebase enrollment must reserve bootstrap first' USING ERRCODE='23514';
        END IF;
        RETURN NEW;
    END IF;
    IF NEW.namespace IS DISTINCT FROM OLD.namespace OR NEW.id IS DISTINCT FROM OLD.id
       OR NEW.project IS DISTINCT FROM OLD.project OR NEW.uid IS DISTINCT FROM OLD.uid
       OR NEW.email IS DISTINCT FROM OLD.email OR NEW.email_key IS DISTINCT FROM OLD.email_key
       OR NEW.accepted_at IS DISTINCT FROM OLD.accepted_at
       OR NEW.expires_at IS DISTINCT FROM OLD.expires_at THEN
        RAISE EXCEPTION 'Firebase enrollment binding and lifetime are immutable' USING ERRCODE='23514';
    END IF;
    IF NOT ((OLD.phase='bootstrap_started' AND NEW.phase='ready')
         OR (OLD.phase IN ('ready','awaiting_action') AND NEW.phase='send_started')
         OR (OLD.phase='send_started' AND NEW.phase='mail_started')
         OR (OLD.phase='mail_started' AND NEW.phase='awaiting_action')
         OR (OLD.phase='awaiting_action' AND NEW.phase='inspecting')
         OR (OLD.phase='inspecting' AND NEW.phase IN ('awaiting_action','consuming'))
         OR (OLD.phase='consuming' AND NEW.phase IN ('creating','cancelled'))
         OR (OLD.phase='creating' AND NEW.phase='created')
         OR (OLD.phase='created' AND NEW.phase='setup_started')
         OR (OLD.phase='setup_started' AND NEW.phase='handed_off')
         OR (OLD.phase IN ('awaiting_action','inspecting') AND NEW.phase IN ('cancelled','expired'))
         OR (OLD.phase IN ('bootstrap_started','ready','send_started','mail_started','inspecting',
             'consuming','creating','created','setup_started') AND NEW.phase='blocked')) THEN
        RAISE EXCEPTION 'Firebase enrollment phase cannot be replayed' USING ERRCODE='23514';
    END IF;
    IF NOT (OLD.phase='awaiting_action' AND NEW.phase='send_started')
       AND NEW.last_sent_at IS DISTINCT FROM OLD.last_sent_at THEN
        RAISE EXCEPTION 'Firebase send reservation is immutable' USING ERRCODE='23514';
    END IF;
    IF NOT (OLD.phase='creating' AND NEW.phase='created')
       AND NEW.person_id IS DISTINCT FROM OLD.person_id THEN
        RAISE EXCEPTION 'Firebase created-person receipt is immutable' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER theatre_firebase_enrollment_guard BEFORE INSERT OR UPDATE OR DELETE
    ON theatre_firebase_enrollments FOR EACH ROW EXECUTE FUNCTION theatre_firebase_enrollment_guard();
CREATE TRIGGER theatre_firebase_enrollments_no_truncate
    BEFORE TRUNCATE ON theatre_firebase_enrollments
    FOR EACH STATEMENT EXECUTE FUNCTION reject_settled_change();

-- The narrowly scoped trigger writes its own immutable receipt; the runtime role
-- receives no ability to fabricate, edit or remove transition receipts.
CREATE FUNCTION theatre_firebase_enrollment_receipt() RETURNS trigger
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
BEGIN
    INSERT INTO public.theatre_firebase_enrollment_receipts
        (namespace,enrollment_id,previous_phase,phase,block_reason,recorded_at)
    VALUES(NEW.namespace,NEW.id,CASE WHEN TG_OP='INSERT' THEN NULL ELSE OLD.phase END,
        NEW.phase,NEW.block_reason,clock_timestamp());
    RETURN NEW;
END;
$$;
CREATE TRIGGER theatre_firebase_enrollment_receipt AFTER INSERT OR UPDATE
    ON theatre_firebase_enrollments FOR EACH ROW EXECUTE FUNCTION theatre_firebase_enrollment_receipt();

REVOKE ALL ON theatre_firebase_enrollments,theatre_firebase_send_starts,
    theatre_firebase_enrollment_receipts FROM PUBLIC;
REVOKE ALL ON FUNCTION theatre_firebase_enrollment_guard(),theatre_firebase_enrollment_receipt() FROM PUBLIC;
REVOKE ALL ON SEQUENCE theatre_firebase_send_starts_receipt_id_seq,
    theatre_firebase_enrollment_receipts_receipt_id_seq FROM PUBLIC;
-- Grants, live provider credentials and activation remain explicit operator work.
