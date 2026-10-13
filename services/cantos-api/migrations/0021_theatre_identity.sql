-- Separate Theatre BFF credentials; no Studio session admission.
CREATE TABLE theatre_identity_flows (
    state_hash bytea PRIMARY KEY CHECK (octet_length(state_hash)=32),
    browser_hash bytea NOT NULL CHECK (octet_length(browser_hash)=32),
    nonce text NOT NULL CHECK (length(nonce)=64),
    verifier text NOT NULL CHECK (length(verifier)=64),
    issuer text COLLATE "C" NOT NULL,
    client_id text NOT NULL CHECK (client_id='cantos-theatre-web-bff'),
    redirect_uri text NOT NULL,
    previous_hash bytea CHECK (octet_length(previous_hash)=32),
    expires_at timestamptz NOT NULL,
    consumed boolean NOT NULL DEFAULT false,
    cancelled boolean NOT NULL DEFAULT false,
    completed boolean NOT NULL DEFAULT false
);
CREATE INDEX theatre_identity_flows_browser ON theatre_identity_flows(browser_hash);
CREATE TABLE theatre_identity_sessions (
    token_hash bytea PRIMARY KEY CHECK (octet_length(token_hash)=32),
    actor_id text NOT NULL REFERENCES actors(id),
    session_id uuid NOT NULL UNIQUE DEFAULT gen_random_uuid(),
    client_id text NOT NULL CHECK (client_id='cantos-theatre-web-bff'),
    expires_at timestamptz NOT NULL,
    revoked boolean NOT NULL DEFAULT false,
    issuer text COLLATE "C" NOT NULL,
    subject text COLLATE "C" NOT NULL,
    proof_hash bytea UNIQUE NOT NULL CHECK (octet_length(proof_hash)=32),
    -- Provider bearer never leaves the backend; no refresh token is requested or retained.
    access_token text NOT NULL CHECK (octet_length(access_token) BETWEEN 1 AND 16384),
    revocation_pending boolean NOT NULL DEFAULT false,
    FOREIGN KEY (issuer,subject) REFERENCES actor_external_identities(issuer,subject)
);
REVOKE ALL ON theatre_identity_flows,theatre_identity_sessions FROM PUBLIC;

-- Narrow registration/session operation; HTTP has no direct registry/actor/session INSERT.
-- Credential verification is the private Rust witness boundary, not a SQL function's caller.
CREATE FUNCTION theatre_identity_finish(p_state bytea,p_issuer text,p_subject text,p_redirect text,
    p_candidate text,p_token bytea,p_proof bytea,p_access text,p_expires bigint)
RETURNS text LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
DECLARE f public.theatre_identity_flows; actor text; enabled boolean; old public.theatre_identity_sessions;
BEGIN
    SELECT * INTO f FROM public.theatre_identity_flows WHERE state_hash=p_state FOR UPDATE;
    IF NOT FOUND OR NOT f.consumed OR f.cancelled OR f.completed
       OR f.expires_at<=clock_timestamp() OR f.issuer<>p_issuer OR f.redirect_uri<>p_redirect
       OR to_timestamp(p_expires)<=clock_timestamp()
       OR to_timestamp(p_expires)>clock_timestamp()+interval '15 minutes' THEN
        RETURN NULL;
    END IF;
    IF f.previous_hash IS NOT NULL THEN
        SELECT * INTO old FROM public.theatre_identity_sessions WHERE token_hash=f.previous_hash FOR UPDATE;
        IF NOT FOUND OR old.revoked OR old.expires_at<=clock_timestamp() THEN
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
    INSERT INTO public.theatre_identity_sessions(token_hash,actor_id,client_id,expires_at,issuer,subject,proof_hash,access_token)
        VALUES(p_token,actor,f.client_id,to_timestamp(p_expires),p_issuer,p_subject,p_proof,p_access);
    IF f.previous_hash IS NOT NULL THEN
        UPDATE public.theatre_identity_sessions SET revoked=true WHERE token_hash=f.previous_hash;
        UPDATE public.theatre_identity_sessions SET revocation_pending=true WHERE token_hash=f.previous_hash;
    END IF;
    UPDATE public.theatre_identity_flows SET completed=true,verifier=repeat('0',64) WHERE state_hash=p_state;
    RETURN actor;
END;
$$;
REVOKE ALL ON FUNCTION theatre_identity_finish(bytea,text,text,text,text,bytea,bytea,text,bigint) FROM PUBLIC;
-- Grants are operator applied, never implicitly provisioned. See services/kanidm/README.md.
