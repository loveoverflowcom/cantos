-- Additive epoch-2 expansion (#335): sessions issued from a verified cantos-identity
-- authentication fact. Baseline, bearer and bookmark checksums and every existing session are
-- untouched.
--
-- These tables are provider-free by construction: no provider access token, no OIDC flow, no
-- issuer URL. They are Server business data (sessions bound to ActorId) that must survive the
-- retirement of the provider-auth tables; a retirement migration must NOT drop them.
CREATE TABLE public.identity_sessions (
    id uuid PRIMARY KEY,
    actor_id text NOT NULL REFERENCES public.actors(id),
    -- Carries the surface; a Web cookie never doubles as a native Bearer and Studio is Web-only.
    channel text NOT NULL CHECK(channel IN ('theatre-web','studio-web','theatre-native')),
    issuer text COLLATE "C" NOT NULL CHECK(issuer = 'cantos-identity'),
    subject text COLLATE "C" NOT NULL CHECK(subject ~ '^[0-9a-f]{32}$'),
    assurance text NOT NULL CHECK(assurance = 'password'),
    authenticated_at bigint NOT NULL CHECK(authenticated_at > 0),
    -- What Identity reported when the session was issued; admission requires them unchanged.
    observed_security_version bigint NOT NULL CHECK(observed_security_version > 0),
    observed_instance_epoch bytea NOT NULL CHECK(octet_length(observed_instance_epoch) = 16),
    absolute_expires bigint NOT NULL,
    idle_expires bigint NOT NULL CHECK(idle_expires <= absolute_expires),
    revoked boolean NOT NULL DEFAULT false,
    revoked_reason text CHECK(revoked_reason IN ('logout','replay','subject-mismatch','subject-missing','disabled','epoch-changed','version-advanced','version-regressed')),
    CHECK(revoked = (revoked_reason IS NOT NULL)),
    FOREIGN KEY (issuer, subject) REFERENCES public.actor_external_identities(issuer, subject)
);
CREATE INDEX identity_sessions_subject ON public.identity_sessions(subject) WHERE NOT revoked;
CREATE INDEX identity_sessions_actor ON public.identity_sessions(actor_id);
-- Only SHA-256 digests of random 256-bit secrets are stored. A Web cookie has no refresh hash.
CREATE TABLE public.identity_session_tokens (
    access_hash bytea PRIMARY KEY CHECK(octet_length(access_hash) = 32),
    refresh_hash bytea UNIQUE CHECK(octet_length(refresh_hash) = 32),
    session_id uuid NOT NULL REFERENCES public.identity_sessions(id),
    expires_at bigint NOT NULL,
    used boolean NOT NULL DEFAULT false,
    CHECK(NOT used OR refresh_hash IS NOT NULL)
);
CREATE INDEX identity_session_tokens_session ON public.identity_session_tokens(session_id);
-- Registry binding for the one issuer this file owns. The runtime role cannot write the
-- registry directly; as with `bearer_register` the runtime is trusted to have verified the fact.
-- Same key framing and advisory lock as the operator registry, so linking cannot race it.
CREATE FUNCTION public.identity_session_register(p_subject text,p_candidate text) RETURNS text
LANGUAGE plpgsql SECURITY DEFINER SET search_path TO 'pg_catalog' AS $$
DECLARE actor text; enabled boolean;
BEGIN
    IF p_subject !~ '^[0-9a-f]{32}$' THEN RETURN NULL; END IF;
    PERFORM pg_advisory_xact_lock(('x'||encode(substring(sha256(
        convert_to('cantos-identity-registry-v1','UTF8') || int8send(octet_length('cantos-identity')::bigint)
        || convert_to('cantos-identity','UTF8') || int8send(octet_length(p_subject)::bigint)
        || convert_to(p_subject,'UTF8')),1,8),'hex'))::bit(64)::bigint);
    SELECT a.id,a.active INTO actor,enabled FROM public.actor_external_identities e
        JOIN public.actors a ON a.id=e.actor_id WHERE e.issuer='cantos-identity' AND e.subject=p_subject FOR SHARE OF a;
    IF FOUND THEN
        IF NOT enabled THEN RETURN NULL; END IF;
        RETURN actor;
    END IF;
    INSERT INTO public.actors(id) VALUES(p_candidate);
    INSERT INTO public.actor_external_identities(issuer,subject,actor_id) VALUES('cantos-identity',p_subject,p_candidate);
    RETURN p_candidate;
END; $$;
REVOKE ALL ON public.identity_sessions,public.identity_session_tokens FROM PUBLIC;
REVOKE ALL ON FUNCTION public.identity_session_register(text,text) FROM PUBLIC;
-- Revocation and consumption are permanent; a session is never re-bound or shortened/extended
-- beyond its absolute deadline, whatever the writer's column privileges.
CREATE FUNCTION public.identity_session_guard() RETURNS trigger
LANGUAGE plpgsql SET search_path TO 'pg_catalog' AS $$
BEGIN
    IF TG_TABLE_NAME='identity_sessions' THEN
        IF OLD.revoked AND (NOT NEW.revoked OR NEW.revoked_reason IS DISTINCT FROM OLD.revoked_reason
            OR NEW.idle_expires IS DISTINCT FROM OLD.idle_expires) THEN
            RAISE EXCEPTION 'revocation is permanent' USING ERRCODE='23514';
        END IF;
        IF NEW.id IS DISTINCT FROM OLD.id OR NEW.actor_id IS DISTINCT FROM OLD.actor_id
            OR NEW.channel IS DISTINCT FROM OLD.channel OR NEW.subject IS DISTINCT FROM OLD.subject
            OR NEW.issuer IS DISTINCT FROM OLD.issuer OR NEW.assurance IS DISTINCT FROM OLD.assurance
            OR NEW.authenticated_at IS DISTINCT FROM OLD.authenticated_at
            OR NEW.observed_security_version IS DISTINCT FROM OLD.observed_security_version
            OR NEW.observed_instance_epoch IS DISTINCT FROM OLD.observed_instance_epoch
            OR NEW.absolute_expires IS DISTINCT FROM OLD.absolute_expires
            OR NEW.idle_expires < OLD.idle_expires THEN
            RAISE EXCEPTION 'session binding is immutable' USING ERRCODE='23514';
        END IF;
    ELSE
        IF OLD.used AND NOT NEW.used THEN
            RAISE EXCEPTION 'consumption is permanent' USING ERRCODE='23514';
        END IF;
        IF NEW.access_hash IS DISTINCT FROM OLD.access_hash
            OR NEW.refresh_hash IS DISTINCT FROM OLD.refresh_hash
            OR NEW.session_id IS DISTINCT FROM OLD.session_id THEN
            RAISE EXCEPTION 'token binding is immutable' USING ERRCODE='23514';
        END IF;
    END IF;
    RETURN NEW;
END; $$;
CREATE TRIGGER identity_session_guard BEFORE UPDATE ON public.identity_sessions
FOR EACH ROW EXECUTE FUNCTION public.identity_session_guard();
CREATE TRIGGER identity_session_token_guard BEFORE UPDATE ON public.identity_session_tokens
FOR EACH ROW EXECUTE FUNCTION public.identity_session_guard();
REVOKE ALL ON FUNCTION public.identity_session_guard() FROM PUBLIC;
