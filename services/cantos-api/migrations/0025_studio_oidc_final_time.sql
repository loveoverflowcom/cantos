-- Additive repair: preserve the already published 0020 checksum and function/grant contract.
-- Recheck expiry after all effect waits. A private exception rolls back this function's entire
-- subtransaction before returning the existing typed refusal; the prior flow consume stays.
ALTER TABLE sessions ADD COLUMN session_id uuid NOT NULL DEFAULT gen_random_uuid();
CREATE UNIQUE INDEX sessions_session_id ON sessions(session_id);
-- A completed callback whose cookie acknowledgement is delayed still belongs to its flow.
ALTER TABLE studio_oidc_sessions ADD COLUMN source_state_hash bytea
    REFERENCES studio_oidc_flows(state_hash);
CREATE UNIQUE INDEX studio_oidc_sessions_source ON studio_oidc_sessions(source_state_hash);
ALTER TABLE studio_oidc_sessions ADD COLUMN lineage_root_hash bytea
    REFERENCES studio_oidc_sessions(token_hash);
UPDATE studio_oidc_sessions SET lineage_root_hash=token_hash;
CREATE INDEX studio_oidc_sessions_lineage ON studio_oidc_sessions(lineage_root_hash);
ALTER TABLE studio_oidc_sessions ADD COLUMN revoked_by_token_hash bytea
    REFERENCES studio_oidc_sessions(token_hash);
CREATE INDEX studio_oidc_sessions_revoked_by ON studio_oidc_sessions(revoked_by_token_hash)
    WHERE revocation_pending;
CREATE OR REPLACE FUNCTION studio_oidc_finish(p_state bytea,p_issuer text,p_subject text,p_redirect text,
    p_candidate text,p_token bytea,p_proof bytea,p_access text,p_expires bigint)
RETURNS text LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
DECLARE f public.studio_oidc_flows; actor text; enabled boolean; old public.sessions; mapped boolean; lineage bytea;
BEGIN
    -- Rotation and logout serialize the whole exact session lineage before taking any flow
    -- row. A completed response waiting on delivery cannot re-establish a logged-out lineage.
    SELECT * INTO f FROM public.studio_oidc_flows WHERE state_hash=p_state;
    IF NOT FOUND THEN RETURN NULL; END IF;
    lineage:=p_token;
    IF f.previous_hash IS NOT NULL THEN
        SELECT coalesce(o.lineage_root_hash,o.token_hash) INTO lineage
            FROM public.studio_oidc_sessions o WHERE o.token_hash=f.previous_hash AND o.issuer=p_issuer;
        IF NOT FOUND THEN RETURN NULL; END IF;
        PERFORM pg_advisory_xact_lock(('x'||encode(substring(sha256(
            convert_to('cantos-studio-session-lineage-v1','UTF8') || lineage),1,8),'hex'))::bit(64)::bigint);
    END IF;
    SELECT * INTO f FROM public.studio_oidc_flows WHERE state_hash=p_state FOR UPDATE;
    IF NOT FOUND OR NOT f.consumed OR f.cancelled OR f.completed
       OR f.expires_at<=clock_timestamp() OR f.issuer<>p_issuer OR f.redirect_uri<>p_redirect
       OR to_timestamp(p_expires)<=clock_timestamp()
       OR to_timestamp(p_expires)>clock_timestamp()+interval '15 minutes' THEN
        RETURN NULL;
    END IF;
    IF f.previous_hash IS NOT NULL THEN
        SELECT * INTO old FROM public.sessions WHERE token_hash=f.previous_hash FOR UPDATE;
        IF NOT FOUND OR old.revoked OR old.expires_at<=clock_timestamp() OR old.auth_kind<>'studio_oidc' THEN
            RETURN NULL;
        END IF;
    END IF;
    PERFORM pg_advisory_xact_lock(('x'||encode(substring(sha256(
        convert_to('cantos-identity-registry-v1','UTF8') || int8send(octet_length(p_issuer)::bigint)
        || convert_to(p_issuer,'UTF8') || int8send(octet_length(p_subject)::bigint)
        || convert_to(p_subject,'UTF8')),1,8),'hex'))::bit(64)::bigint);
    SELECT a.id,a.active INTO actor,enabled FROM public.actor_external_identities e
        JOIN public.actors a ON a.id=e.actor_id WHERE e.issuer=p_issuer AND e.subject=p_subject
        FOR SHARE OF a;
    mapped:=FOUND;
    IF mapped AND NOT enabled THEN RETURN NULL; END IF;
    -- Refuse cheaply after registry/actor waits. Effect/constraint locks may still wait later,
    -- so this early check is complemented by the final rollback fence below.
    IF f.expires_at<=clock_timestamp() OR to_timestamp(p_expires)<=clock_timestamp()
       OR (f.previous_hash IS NOT NULL AND old.expires_at<=clock_timestamp()) THEN
        RETURN NULL;
    END IF;
    IF NOT mapped THEN
        INSERT INTO public.actors(id) VALUES(p_candidate);
        INSERT INTO public.actor_external_identities(issuer,subject,actor_id)
            VALUES(p_issuer,p_subject,p_candidate);
        actor:=p_candidate;
    END IF;
    INSERT INTO public.sessions(token_hash,actor_id,expires_at,auth_kind)
        VALUES(p_token,actor,to_timestamp(p_expires),'studio_oidc');
    INSERT INTO public.studio_oidc_sessions(token_hash,issuer,subject,proof_hash,access_token,source_state_hash,lineage_root_hash)
        VALUES(p_token,p_issuer,p_subject,p_proof,p_access,p_state,lineage);
    IF f.previous_hash IS NOT NULL THEN
        UPDATE public.sessions SET revoked=true WHERE token_hash=f.previous_hash;
        UPDATE public.studio_oidc_sessions SET revocation_pending=true WHERE token_hash=f.previous_hash;
    END IF;
    UPDATE public.studio_oidc_flows SET completed=true,verifier=repeat('0',64) WHERE state_hash=p_state;
    -- All writes are speculative until this final observation. INSERT/UPDATE and their
    -- constraints can acquire locks too; returning NULL alone would commit those effects.
    IF f.expires_at<=clock_timestamp() OR to_timestamp(p_expires)<=clock_timestamp()
       OR (f.previous_hash IS NOT NULL AND old.expires_at<=clock_timestamp()) THEN
        RAISE EXCEPTION USING ERRCODE='P0E01', MESSAGE='studio_oidc_expired_at_issuance';
    END IF;
    RETURN actor;
EXCEPTION WHEN SQLSTATE 'P0E01' THEN
    -- PostgreSQL restores every change made in this block, including actor/session/receipt,
    -- previous-session revocation and flow completion. Other SQL faults still propagate.
    RETURN NULL;
END;
$$;
