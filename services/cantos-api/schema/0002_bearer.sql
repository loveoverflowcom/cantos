-- Additive epoch-2 expansion; baseline checksum and existing sessions are preserved.
CREATE TABLE public.bearer_flows (
    id uuid PRIMARY KEY,
    client_id text NOT NULL CHECK(client_id IN ('cantos-studio-web-bff','cantos-theatre-web-bff','cantos-theatre-native')),
    issuer text COLLATE "C" NOT NULL,
    redirect_uri text NOT NULL,
    return_origin text NOT NULL,
    client_state text NOT NULL CHECK(length(client_state) BETWEEN 43 AND 128),
    challenge text NOT NULL CHECK(length(challenge)=43),
    provider_state_hash bytea UNIQUE NOT NULL CHECK(octet_length(provider_state_hash)=32),
    nonce text NOT NULL,
    verifier text NOT NULL,
    expires_at bigint NOT NULL,
    consumed boolean NOT NULL DEFAULT false,
    cancelled boolean NOT NULL DEFAULT false,
    completed boolean NOT NULL DEFAULT false,
    handoff_hash bytea CHECK(octet_length(handoff_hash)=32),
    subject text COLLATE "C",
    proof_hash bytea CHECK(octet_length(proof_hash)=32),
    provider_access text,
    authority_expires bigint,
    authenticated_at bigint
);
CREATE TABLE public.bearer_families (
    id uuid PRIMARY KEY REFERENCES public.bearer_flows(id),
    actor_id text NOT NULL REFERENCES public.actors(id),
    client_id text NOT NULL,
    issuer text COLLATE "C" NOT NULL,
    subject text COLLATE "C" NOT NULL,
    provider_access text NOT NULL CHECK(octet_length(provider_access) BETWEEN 1 AND 16384),
    proof_hash bytea NOT NULL UNIQUE CHECK(octet_length(proof_hash)=32),
    absolute_expires bigint NOT NULL,
    idle_expires bigint NOT NULL CHECK(idle_expires<=absolute_expires),
    authenticated_at bigint,
    revoked boolean NOT NULL DEFAULT false,
    revocation_pending boolean NOT NULL DEFAULT false,
    FOREIGN KEY (issuer,subject) REFERENCES public.actor_external_identities(issuer,subject),
    CHECK(client_id IN ('cantos-studio-web-bff','cantos-theatre-web-bff','cantos-theatre-native'))
);
CREATE TABLE public.bearer_tokens (
    access_hash bytea PRIMARY KEY CHECK(octet_length(access_hash)=32),
    refresh_hash bytea UNIQUE NOT NULL CHECK(octet_length(refresh_hash)=32),
    family_id uuid NOT NULL REFERENCES public.bearer_families(id),
    expires_at bigint NOT NULL,
    used boolean NOT NULL DEFAULT false
);
CREATE INDEX bearer_tokens_family ON public.bearer_tokens(family_id);
-- Registry binding is earned only by the consumed, still-live flow at the API verifier seam.
-- As with existing OIDC finish functions, the runtime is trusted to verify provider proof.
CREATE FUNCTION public.bearer_register(p_flow uuid,p_issuer text,p_subject text,p_candidate text) RETURNS text
LANGUAGE plpgsql SECURITY DEFINER SET search_path TO 'pg_catalog' AS $$
DECLARE actor text; enabled boolean;
BEGIN
    PERFORM 1 FROM public.bearer_flows WHERE id=p_flow AND issuer=p_issuer AND consumed
        AND NOT cancelled AND NOT completed AND expires_at>extract(epoch FROM clock_timestamp()) FOR UPDATE;
    IF NOT FOUND THEN RETURN NULL; END IF;
    PERFORM pg_advisory_xact_lock(('x'||encode(substring(sha256(
        convert_to('cantos-identity-registry-v1','UTF8') || int8send(octet_length(p_issuer)::bigint)
        || convert_to(p_issuer,'UTF8') || int8send(octet_length(p_subject)::bigint)
        || convert_to(p_subject,'UTF8')),1,8),'hex'))::bit(64)::bigint);
    SELECT a.id,a.active INTO actor,enabled FROM public.actor_external_identities e
        JOIN public.actors a ON a.id=e.actor_id WHERE e.issuer=p_issuer AND e.subject=p_subject FOR SHARE OF a;
    IF FOUND THEN
        IF NOT enabled THEN RETURN NULL; END IF;
    ELSE
        INSERT INTO public.actors(id) VALUES(p_candidate);
        INSERT INTO public.actor_external_identities(issuer,subject,actor_id) VALUES(p_issuer,p_subject,p_candidate);
        actor:=p_candidate;
    END IF;
    RETURN actor;
END; $$;
REVOKE ALL ON public.bearer_flows,public.bearer_families,public.bearer_tokens FROM PUBLIC;
REVOKE ALL ON FUNCTION public.bearer_register(uuid,text,text,text) FROM PUBLIC;
CREATE FUNCTION public.bearer_monotonic() RETURNS trigger
LANGUAGE plpgsql SET search_path TO 'pg_catalog' AS $$
BEGIN
    IF TG_TABLE_NAME='bearer_families' THEN
        IF OLD.revoked AND NOT NEW.revoked THEN
            RAISE EXCEPTION 'revocation is permanent' USING ERRCODE='23514';
        END IF;
    ELSE
        IF OLD.used AND NOT NEW.used THEN
            RAISE EXCEPTION 'consumption is permanent' USING ERRCODE='23514';
        END IF;
    END IF;
    RETURN NEW;
END; $$;
CREATE TRIGGER bearer_family_monotonic BEFORE UPDATE ON public.bearer_families
FOR EACH ROW EXECUTE FUNCTION public.bearer_monotonic();
CREATE TRIGGER bearer_token_monotonic BEFORE UPDATE ON public.bearer_tokens
FOR EACH ROW EXECUTE FUNCTION public.bearer_monotonic();
REVOKE ALL ON FUNCTION public.bearer_monotonic() FROM PUBLIC;
