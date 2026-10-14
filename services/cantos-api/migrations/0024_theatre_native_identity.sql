-- Native public OAuth client has its own durable flows and bearer-purpose sessions.
CREATE TABLE theatre_native_flows (
    flow_id uuid PRIMARY KEY,
    state_hash bytea NOT NULL UNIQUE CHECK(octet_length(state_hash)=32),
    nonce text NOT NULL CHECK(length(nonce) BETWEEN 43 AND 128),
    challenge text NOT NULL CHECK(length(challenge)=43),
    issuer text COLLATE "C" NOT NULL,
    expires_at timestamptz NOT NULL,
    consumed boolean NOT NULL DEFAULT false,
    completed boolean NOT NULL DEFAULT false,
    CHECK (NOT completed OR consumed)
);
CREATE TABLE theatre_native_sessions (
    token_hash bytea PRIMARY KEY CHECK(octet_length(token_hash)=32),
    actor_id text NOT NULL REFERENCES actors(id),
    session_id uuid NOT NULL UNIQUE,
    client_id text NOT NULL DEFAULT 'cantos-theatre-native' CHECK(client_id='cantos-theatre-native'),
    issuer text COLLATE "C" NOT NULL,
    subject text COLLATE "C" NOT NULL,
    expires_at timestamptz NOT NULL,
    proof_hash bytea NOT NULL UNIQUE CHECK(octet_length(proof_hash)=32),
    access_token text NOT NULL CHECK(octet_length(access_token) BETWEEN 1 AND 16384),
    revoked boolean NOT NULL DEFAULT false,
    revocation_pending boolean NOT NULL DEFAULT false,
    FOREIGN KEY(issuer,subject) REFERENCES actor_external_identities(issuer,subject)
);
REVOKE ALL ON theatre_native_flows,theatre_native_sessions FROM PUBLIC;

CREATE FUNCTION theatre_native_finish(p_flow uuid,p_issuer text,p_subject text,p_candidate text,
    p_token bytea,p_proof bytea,p_access text,p_expires bigint,p_session uuid)
RETURNS text LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
DECLARE f public.theatre_native_flows; actor text; enabled boolean;
BEGIN
    SELECT * INTO f FROM public.theatre_native_flows WHERE flow_id=p_flow FOR UPDATE;
    IF NOT FOUND OR NOT f.consumed OR f.completed OR f.issuer<>p_issuer
       OR f.expires_at<=clock_timestamp() OR to_timestamp(p_expires)<=clock_timestamp()
       OR to_timestamp(p_expires)>clock_timestamp()+interval '15 minutes' THEN RETURN NULL; END IF;
    PERFORM pg_advisory_xact_lock(('x'||encode(substring(sha256(
        convert_to('cantos-identity-registry-v1','UTF8') || int8send(octet_length(p_issuer)::bigint)
        || convert_to(p_issuer,'UTF8') || int8send(octet_length(p_subject)::bigint)
        || convert_to(p_subject,'UTF8')),1,8),'hex'))::bit(64)::bigint);
    SELECT a.id,a.active INTO actor,enabled FROM public.actor_external_identities e
        JOIN public.actors a ON a.id=e.actor_id WHERE e.issuer=p_issuer AND e.subject=p_subject FOR SHARE OF a;
    IF f.expires_at<=clock_timestamp() OR to_timestamp(p_expires)<=clock_timestamp() THEN RETURN NULL; END IF;
    IF FOUND THEN
        IF NOT enabled THEN RETURN NULL; END IF;
    ELSE
        INSERT INTO public.actors(id) VALUES(p_candidate);
        INSERT INTO public.actor_external_identities(issuer,subject,actor_id) VALUES(p_issuer,p_subject,p_candidate);
        actor:=p_candidate;
    END IF;
    INSERT INTO public.theatre_native_sessions(token_hash,actor_id,session_id,issuer,subject,expires_at,proof_hash,access_token)
        VALUES(p_token,actor,p_session,p_issuer,p_subject,to_timestamp(p_expires),p_proof,p_access);
    UPDATE public.theatre_native_flows SET completed=true,nonce=repeat('0',43) WHERE flow_id=p_flow;
    -- Triggers/constraint/index waits may outlast proof/flow validity after the earlier reads.
    -- Raising here rolls back registration and the session, rather than committing a NULL.
    IF f.expires_at<=clock_timestamp() OR to_timestamp(p_expires)<=clock_timestamp() THEN
        RAISE EXCEPTION 'native issuance expired' USING ERRCODE='PT401';
    END IF;
    RETURN actor;
END;
$$;
REVOKE ALL ON FUNCTION theatre_native_finish(uuid,text,text,text,bytea,bytea,text,bigint,uuid) FROM PUBLIC;

-- Preserve0021 checksum. A bounded immutable browser inventory can cancel both pending
-- callbacks and sessions already issued from the same proven flow cookies.
ALTER TABLE theatre_identity_sessions ADD COLUMN source_state_hash bytea
    REFERENCES theatre_identity_flows(state_hash);
ALTER TABLE theatre_identity_sessions ADD COLUMN revoked_by_token_hash bytea
    REFERENCES theatre_identity_sessions(token_hash);
CREATE INDEX theatre_identity_sessions_source_flow ON theatre_identity_sessions(source_state_hash);
CREATE INDEX theatre_identity_sessions_revocation_batch ON theatre_identity_sessions(revoked_by_token_hash)
    WHERE revoked AND revocation_pending;
ALTER TABLE theatre_identity_sessions ADD COLUMN lineage_root_hash bytea
    REFERENCES theatre_identity_sessions(token_hash);
ALTER TABLE theatre_identity_flows ADD COLUMN lineage_root_hash bytea
    REFERENCES theatre_identity_sessions(token_hash);
UPDATE theatre_identity_sessions SET lineage_root_hash=token_hash;
UPDATE theatre_identity_flows SET lineage_root_hash=previous_hash WHERE previous_hash IS NOT NULL;

-- Recheck validity after every issuance write/trigger wait.
CREATE OR REPLACE FUNCTION theatre_identity_finish(p_state bytea,p_issuer text,p_subject text,p_redirect text,
    p_candidate text,p_token bytea,p_proof bytea,p_access text,p_expires bigint)
RETURNS text LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
DECLARE f public.theatre_identity_flows; actor text; enabled boolean; old public.theatre_identity_sessions; root bytea;
BEGIN
    SELECT COALESCE(lineage_root_hash,p_token) INTO root FROM public.theatre_identity_flows WHERE state_hash=p_state;
    IF NOT FOUND THEN RETURN NULL; END IF;
    -- Root is serialization only; cancellation still needs an exact captured private binder.
    PERFORM pg_advisory_xact_lock(('x'||encode(substring(sha256(
        convert_to('cantos-theatre-lineage-v1','UTF8')||root),1,8),'hex'))::bit(64)::bigint);
    SELECT * INTO f FROM public.theatre_identity_flows WHERE state_hash=p_state FOR UPDATE;
    IF NOT FOUND OR NOT f.consumed OR f.cancelled OR f.completed
       OR f.expires_at<=clock_timestamp() OR f.issuer<>p_issuer OR f.redirect_uri<>p_redirect
       OR to_timestamp(p_expires)<=clock_timestamp()
       OR to_timestamp(p_expires)>clock_timestamp()+interval '15 minutes' THEN
        RETURN NULL;
    END IF;
    IF f.previous_hash IS NOT NULL THEN
        SELECT * INTO old FROM public.theatre_identity_sessions WHERE token_hash=f.previous_hash FOR UPDATE;
        IF NOT FOUND OR old.revoked OR old.expires_at<=clock_timestamp()
           OR f.lineage_root_hash IS NULL OR old.lineage_root_hash<>f.lineage_root_hash THEN
            RETURN NULL;
        END IF;
    END IF;
    -- Identical length-framed SHA256 lock to the PR1 registry. Collisions only serialize;
    -- the C-collated exact primary key remains the identity authority.
    PERFORM pg_advisory_xact_lock(('x'||encode(substring(sha256(
        convert_to('cantos-identity-registry-v1','UTF8') || int8send(octet_length(p_issuer)::bigint)
        || convert_to(p_issuer,'UTF8') || int8send(octet_length(p_subject)::bigint)
        || convert_to(p_subject,'UTF8')),1,8),'hex'))::bit(64)::bigint);
    SELECT a.id,a.active INTO actor,enabled FROM public.actor_external_identities e
        JOIN public.actors a ON a.id=e.actor_id WHERE e.issuer=p_issuer AND e.subject=p_subject
        FOR SHARE OF a;
    -- Refresh expiry after registry/actor locks, before even registration can mutate.
    IF f.expires_at<=clock_timestamp() OR to_timestamp(p_expires)<=clock_timestamp()
       OR (f.previous_hash IS NOT NULL AND old.expires_at<=clock_timestamp()) THEN
        RETURN NULL;
    END IF;
    IF FOUND THEN
        IF NOT enabled THEN RETURN NULL; END IF;
    ELSE
        -- No account linking or matching by email/name. Candidate collision aborts atomically.
        INSERT INTO public.actors(id) VALUES(p_candidate);
        INSERT INTO public.actor_external_identities(issuer,subject,actor_id)
            VALUES(p_issuer,p_subject,p_candidate);
        actor:=p_candidate;
    END IF;
    INSERT INTO public.theatre_identity_sessions(token_hash,actor_id,client_id,expires_at,issuer,subject,proof_hash,access_token,source_state_hash,lineage_root_hash)
        VALUES(p_token,actor,f.client_id,to_timestamp(p_expires),p_issuer,p_subject,p_proof,p_access,p_state,root);
    IF f.previous_hash IS NOT NULL THEN
        UPDATE public.theatre_identity_sessions SET revoked=true WHERE token_hash=f.previous_hash;
        UPDATE public.theatre_identity_sessions SET revocation_pending=true WHERE token_hash=f.previous_hash;
    END IF;
    UPDATE public.theatre_identity_flows SET completed=true,verifier=repeat('0',64) WHERE state_hash=p_state;
    IF f.expires_at<=clock_timestamp() OR to_timestamp(p_expires)<=clock_timestamp()
       OR (f.previous_hash IS NOT NULL AND old.expires_at<=clock_timestamp()) THEN
        RAISE EXCEPTION 'Theatre issuance expired' USING ERRCODE='PT401';
    END IF;
    RETURN actor;
END;
$$;
REVOKE ALL ON FUNCTION theatre_identity_finish(bytea,text,text,text,text,bytea,bytea,text,bigint) FROM PUBLIC;
