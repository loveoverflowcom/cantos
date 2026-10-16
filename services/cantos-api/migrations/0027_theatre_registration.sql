-- Theatre self-registration has no credential columns or Studio admission writes.
-- Exact issuer namespaces keep a later configuration change from claiming old proofs.
CREATE TABLE theatre_registrations (
    namespace text COLLATE "C" NOT NULL CHECK(length(namespace) BETWEEN 1 AND 2048),
    id uuid NOT NULL CHECK(id<>'00000000-0000-0000-0000-000000000000'::uuid),
    email text NOT NULL CHECK(octet_length(email) BETWEEN 3 AND 254),
    email_key text COLLATE "C" NOT NULL CHECK(email_key=lower(email COLLATE "C")),
    token_hash bytea NOT NULL CHECK(octet_length(token_hash)=32),
    expires_at timestamptz NOT NULL,
    last_sent_at timestamptz NOT NULL,
    phase text NOT NULL CHECK(phase IN ('awaiting_verification','cancelled','creating','created',
        'setup_started','delivering','complete','blocked')),
    person_id uuid,
    block_reason text CHECK(block_reason IN ('create_rejected','create_uncertain',
        'setup_rejected','setup_uncertain','setup_delivery_rejected',
        'setup_delivery_uncertain','binding_mismatch')),
    PRIMARY KEY(namespace,id),
    UNIQUE(namespace,email_key),
    UNIQUE(namespace,token_hash),
    CHECK(expires_at>last_sent_at),
    -- A positive create witness binds the preselected random UUID, never a found account.
    CHECK(person_id IS NULL OR person_id=id),
    CHECK((phase IN ('awaiting_verification','cancelled','creating') AND person_id IS NULL)
       OR (phase IN ('created','setup_started','delivering','complete') AND person_id IS NOT NULL)
       OR phase='blocked'),
    CHECK((phase='blocked')=(block_reason IS NOT NULL))
);

-- The database also refuses rollback into a retryable phase after a provider effect was
-- fenced. Retrying a timeout must not reset a pre-existing person or dispatch create twice.
CREATE FUNCTION theatre_registration_guard() RETURNS trigger
LANGUAGE plpgsql SET search_path=pg_catalog AS $$
BEGIN
    IF TG_OP<>'UPDATE' THEN
        RAISE EXCEPTION 'registration history is retained' USING ERRCODE='23514';
    END IF;
    IF NEW.namespace IS DISTINCT FROM OLD.namespace OR NEW.id IS DISTINCT FROM OLD.id
       OR NEW.email IS DISTINCT FROM OLD.email OR NEW.email_key IS DISTINCT FROM OLD.email_key THEN
        RAISE EXCEPTION 'registration binding is immutable' USING ERRCODE='23514';
    END IF;
    IF NOT ((OLD.phase IN ('awaiting_verification','cancelled') AND NEW.phase='awaiting_verification')
         OR (OLD.phase='awaiting_verification' AND NEW.phase IN ('creating','cancelled'))
         OR (OLD.phase='creating' AND NEW.phase='created')
         OR (OLD.phase='created' AND NEW.phase='setup_started')
         OR (OLD.phase='setup_started' AND NEW.phase='delivering')
         OR (OLD.phase='delivering' AND NEW.phase='complete')
         OR (OLD.phase IN ('creating','created','setup_started','delivering') AND NEW.phase='blocked')) THEN
        RAISE EXCEPTION 'registration phase cannot be replayed' USING ERRCODE='23514';
    END IF;
    IF NOT (OLD.phase IN ('awaiting_verification','cancelled') AND NEW.phase='awaiting_verification')
       AND (NEW.token_hash IS DISTINCT FROM OLD.token_hash
         OR NEW.expires_at IS DISTINCT FROM OLD.expires_at
         OR NEW.last_sent_at IS DISTINCT FROM OLD.last_sent_at) THEN
        RAISE EXCEPTION 'consumed challenge is immutable' USING ERRCODE='23514';
    END IF;
    IF NOT (OLD.phase='creating' AND NEW.phase='created')
       AND NEW.person_id IS DISTINCT FROM OLD.person_id THEN
        RAISE EXCEPTION 'created-person binding is immutable' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER theatre_registrations_guard BEFORE UPDATE OR DELETE ON theatre_registrations
    FOR EACH ROW EXECUTE FUNCTION theatre_registration_guard();
CREATE TRIGGER theatre_registrations_no_truncate BEFORE TRUNCATE ON theatre_registrations
    FOR EACH STATEMENT EXECUTE FUNCTION reject_settled_change();
REVOKE ALL ON FUNCTION theatre_registration_guard() FROM PUBLIC;

-- Every accepted verification-mail start/resend consumes the same durable global budget,
-- including a later failed or uncertain delivery. No raw proof or message body is retained.
CREATE TABLE theatre_registration_mail_starts (
    namespace text COLLATE "C" NOT NULL,
    token_hash bytea NOT NULL CHECK(octet_length(token_hash)=32),
    registration_id uuid NOT NULL,
    network_key bytea NOT NULL CHECK(octet_length(network_key)=32),
    accepted_at timestamptz NOT NULL,
    PRIMARY KEY(namespace,token_hash),
    FOREIGN KEY(namespace,registration_id) REFERENCES theatre_registrations(namespace,id)
);
CREATE INDEX theatre_registration_mail_budget
    ON theatre_registration_mail_starts(namespace,accepted_at);
CREATE INDEX theatre_registration_network_mail_budget
    ON theatre_registration_mail_starts(namespace,network_key,accepted_at);
CREATE TRIGGER theatre_registration_mail_starts_immutable
    BEFORE UPDATE OR DELETE ON theatre_registration_mail_starts
    FOR EACH ROW EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER theatre_registration_mail_starts_no_truncate
    BEFORE TRUNCATE ON theatre_registration_mail_starts
    FOR EACH STATEMENT EXECUTE FUNCTION reject_settled_change();

REVOKE ALL ON theatre_registrations,theatre_registration_mail_starts FROM PUBLIC;
-- Application SELECT/INSERT/UPDATE and append-only mail-start INSERT/SELECT grants require
-- an explicit operator action; migration never creates provider accounts or standing grants.
