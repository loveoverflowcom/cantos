-- Canonical current schema, epoch 2. Fresh databases only; never replay on epoch 1.
-- PostgreSQL authority; external producers own execution. No retired provider/TTS tables.
-- Functions precede their referenced relations in this materialized DDL.
SET LOCAL check_function_bodies = false;
REVOKE CREATE ON SCHEMA public FROM PUBLIC;
COMMENT ON SCHEMA public IS 'standard public schema';

CREATE FUNCTION public.admin_append_audit(p_id uuid, p_actor text, p_action text, p_target text, p_outcome text, p_correlation uuid) RETURNS void
    LANGUAGE sql SECURITY DEFINER
    SET search_path TO 'pg_catalog'
    AS $$
    INSERT INTO public.admin_audit(id,actor_id,surface,action,target_actor_id,outcome,correlation_id)
        VALUES(p_id,p_actor,'admin',p_action,p_target,p_outcome,p_correlation);
$$;

CREATE FUNCTION public.admin_identity_facts(p_issuer text, p_subject text, p_target text) RETURNS TABLE(actor_id text, active boolean, revoked boolean, target_exists boolean, now_seconds bigint)
    LANGUAGE plpgsql SECURITY DEFINER
    SET search_path TO 'pg_catalog'
    AS $$
DECLARE actor text; lock_actor text;
BEGIN
    SELECT e.actor_id INTO actor FROM public.actor_external_identities e
        WHERE e.issuer=p_issuer AND e.subject=p_subject;
    IF actor IS NULL THEN RETURN; END IF;
    FOR lock_actor IN SELECT v FROM (SELECT DISTINCT v FROM unnest(ARRAY[actor,p_target]) v WHERE v IS NOT NULL) keys ORDER BY v COLLATE "C"
    LOOP
        PERFORM pg_advisory_xact_lock(('x'||substr(encode(sha256(convert_to('cantos-admin-v1:'||lock_actor,'UTF8')),'hex'),1,16))::bit(64)::bigint);
        PERFORM 1 FROM public.actors a WHERE a.id=lock_actor FOR SHARE;
    END LOOP;
    RETURN QUERY SELECT a.id,a.active,
        EXISTS(SELECT 1 FROM public.admin_access_revocations r WHERE r.actor_id=a.id),
        p_target IS NULL OR EXISTS(SELECT 1 FROM public.actors t WHERE t.id=p_target),
        floor(extract(epoch FROM clock_timestamp()))::bigint
        FROM public.actors a WHERE a.id=actor;
END; $$;

CREATE FUNCTION public.admin_immutable() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN RAISE EXCEPTION 'Admin record is append-only'; END; $$;

CREATE FUNCTION public.admin_operation(p_actor text, p_operation uuid) RETURNS TABLE(target_actor_id text, audit_id uuid)
    LANGUAGE sql SECURITY DEFINER
    SET search_path TO 'pg_catalog'
    AS $$
    SELECT o.target_actor_id,o.audit_id FROM public.admin_operations o WHERE o.actor_id=p_actor AND o.operation_id=p_operation;
$$;

CREATE FUNCTION public.admin_revoke_access(p_actor text, p_operation uuid, p_target text, p_audit uuid) RETURNS void
    LANGUAGE sql SECURITY DEFINER
    SET search_path TO 'pg_catalog'
    AS $$
    INSERT INTO public.admin_access_revocations(actor_id) VALUES(p_target) ON CONFLICT DO NOTHING;
    INSERT INTO public.admin_operations(actor_id,operation_id,target_actor_id,audit_id) VALUES(p_actor,p_operation,p_target,p_audit);
$$;

CREATE FUNCTION public.catalog_content_head_identity() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    IF NEW.unit_id<>OLD.unit_id THEN
        RAISE EXCEPTION 'content head identity is immutable' USING ERRCODE='55000';
    END IF;
    RETURN NEW;
END;
$$;

CREATE FUNCTION public.catalog_identity_immutable() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    IF TG_TABLE_NAME='catalog_works' THEN
        IF NEW.id<>OLD.id OR NEW.owner_id<>OLD.owner_id OR NEW.kind<>OLD.kind THEN
            RAISE EXCEPTION 'catalog identity is immutable' USING ERRCODE='55000';
        END IF;
    ELSE
        IF NEW.id<>OLD.id OR NEW.work_id<>OLD.work_id OR NEW.work_kind<>OLD.work_kind OR NEW.kind<>OLD.kind THEN
            RAISE EXCEPTION 'catalog identity is immutable' USING ERRCODE='55000';
        END IF;
    END IF;
    RETURN NEW;
END;
$$;

CREATE FUNCTION public.catalog_listener_discoverable(work text, unit text) RETURNS boolean
    LANGUAGE sql STABLE
    SET search_path TO 'pg_catalog'
    AS $$
    SELECT EXISTS(SELECT 1 FROM public.catalog_works w
      JOIN public.catalog_units u ON u.work_id=w.id AND u.id=unit
      JOIN public.catalog_work_revisions r ON r.work_id=w.id AND r.revision=w.head_revision
      WHERE w.id=work AND w.lifecycle<>'archived' AND u.lifecycle<>'archived'
      AND convert_from(r.snapshot,'UTF8')::jsonb->'details'->>'visibility'='public')
$$;

CREATE FUNCTION public.catalog_lock_actor(actor text) RETURNS boolean
    LANGUAGE sql SECURITY DEFINER
    SET search_path TO 'pg_catalog'
    AS $$ SELECT active FROM public.actors WHERE id=actor FOR SHARE $$;

CREATE FUNCTION public.catalog_lock_member(work text, actor text) RETURNS text
    LANGUAGE sql SECURITY DEFINER
    SET search_path TO 'pg_catalog'
    AS $$ SELECT role FROM public.catalog_members WHERE work_id=work AND actor_id=actor FOR SHARE $$;

CREATE FUNCTION public.check_audio_intent_binding() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM script_revisions r JOIN scripts s ON s.id=r.script_id
        WHERE r.script_id=NEW.script_id AND r.revision=NEW.script_revision
          AND r.content_digest=NEW.content_digest AND s.owner_id=NEW.created_by
    ) THEN
        RAISE EXCEPTION 'invalid audio intent binding' USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END;
$$;

CREATE FUNCTION public.guard_adaptation_run() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    IF TG_OP <> 'UPDATE' THEN
        RAISE EXCEPTION 'adaptation records cannot be removed' USING ERRCODE = '55000';
    END IF;
    IF (NEW.id,NEW.owner_id,NEW.operation_id,NEW.source_id,NEW.script_id,NEW.frozen_input,
        NEW.input_digest,NEW.recorded_at,NEW.input_version) IS DISTINCT FROM
       (OLD.id,OLD.owner_id,OLD.operation_id,OLD.source_id,OLD.script_id,OLD.frozen_input,
        OLD.input_digest,OLD.recorded_at,OLD.input_version) THEN
        RAISE EXCEPTION 'adaptation input is immutable' USING ERRCODE = '55000';
    END IF;
    IF NOT ((OLD.status='awaiting_proposal' AND NEW.status IN ('succeeded','cancelled'))
        OR (OLD.status='succeeded' AND NEW.status IN ('accepted','cancelled'))) THEN
        RAISE EXCEPTION 'invalid caller adaptation transition' USING ERRCODE = '55000';
    END IF;
    RETURN NEW;
END;
$$;

CREATE FUNCTION public.guard_audio_publication_pointer() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    IF NEW.unit_id <> OLD.unit_id OR NEW.owner_id <> OLD.owner_id OR NEW.version <> OLD.version + 1 THEN
        RAISE EXCEPTION 'publication pointer identity/version cannot change outside a forward transition';
    END IF;
    RETURN NEW;
END $$;

CREATE FUNCTION public.guard_listener_progress() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    IF (NEW.listener_id,NEW.publication_id,NEW.work_id,NEW.unit_id,NEW.rendition_id,NEW.sha256,NEW.duration_ms)
       IS DISTINCT FROM (OLD.listener_id,OLD.publication_id,OLD.work_id,OLD.unit_id,OLD.rendition_id,OLD.sha256,OLD.duration_ms)
       OR NEW.revision<>OLD.revision+1 OR NEW.listener_order<=OLD.listener_order THEN
        RAISE EXCEPTION 'listener progress identity/revision mismatch' USING ERRCODE='55000';
    END IF;
    RETURN NEW;
END;
$$;

CREATE FUNCTION public.guard_listener_saved() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    IF (NEW.listener_id,NEW.work_id) IS DISTINCT FROM (OLD.listener_id,OLD.work_id)
       OR NEW.revision<>OLD.revision+1 OR NEW.listener_order<=OLD.listener_order THEN
        RAISE EXCEPTION 'listener saved identity/revision mismatch' USING ERRCODE='55000';
    END IF;
    RETURN NEW;
END;
$$;

CREATE FUNCTION public.listener_actor_active(actor text) RETURNS boolean
    LANGUAGE sql STABLE
    SET search_path TO 'pg_catalog'
    AS $$
    SELECT EXISTS(SELECT 1 FROM public.actors a WHERE a.id=actor AND a.active)
$$;

CREATE FUNCTION public.listener_lock_actor(actor text) RETURNS boolean
    LANGUAGE sql SECURITY DEFINER
    SET search_path TO 'pg_catalog'
    AS $$ SELECT active FROM public.actors WHERE id=actor FOR SHARE $$;

CREATE FUNCTION public.production_brief_head_identity() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    IF NEW.unit_id<>OLD.unit_id OR NEW.version<>OLD.version+1 THEN
        RAISE EXCEPTION 'brief head advances one immutable version' USING ERRCODE='55000';
    END IF;
    RETURN NEW;
END;
$$;

CREATE FUNCTION public.production_lock_actor(text) RETURNS boolean
    LANGUAGE sql SECURITY DEFINER
    SET search_path TO 'pg_catalog'
    AS $_$
    SELECT a.active FROM public.actors a WHERE a.id=$1 FOR SHARE;
$_$;

CREATE FUNCTION public.production_lock_member(text, text) RETURNS text
    LANGUAGE sql SECURITY DEFINER
    SET search_path TO 'pg_catalog'
    AS $_$
    SELECT m.role FROM public.script_members m WHERE m.script_id=$1 AND m.actor_id=$2 FOR SHARE;
$_$;

CREATE FUNCTION public.reject_settled_change() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    RAISE EXCEPTION 'settled records are immutable' USING ERRCODE = '55000';
END;
$$;

CREATE FUNCTION public.studio_oidc_finish(p_state bytea, p_issuer text, p_subject text, p_redirect text, p_candidate text, p_token bytea, p_proof bytea, p_access text, p_expires bigint) RETURNS text
    LANGUAGE plpgsql SECURITY DEFINER
    SET search_path TO 'pg_catalog'
    AS $$
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

CREATE FUNCTION public.theatre_firebase_enrollment_guard() RETURNS trigger
    LANGUAGE plpgsql
    SET search_path TO 'pg_catalog'
    AS $$
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

CREATE FUNCTION public.theatre_firebase_enrollment_receipt() RETURNS trigger
    LANGUAGE plpgsql SECURITY DEFINER
    SET search_path TO 'pg_catalog'
    AS $$
BEGIN
    INSERT INTO public.theatre_firebase_enrollment_receipts
        (namespace,enrollment_id,previous_phase,phase,block_reason,recorded_at)
    VALUES(NEW.namespace,NEW.id,CASE WHEN TG_OP='INSERT' THEN NULL ELSE OLD.phase END,
        NEW.phase,NEW.block_reason,clock_timestamp());
    RETURN NEW;
END;
$$;

CREATE FUNCTION public.theatre_identity_finish(p_state bytea, p_issuer text, p_subject text, p_redirect text, p_candidate text, p_token bytea, p_proof bytea, p_access text, p_expires bigint) RETURNS text
    LANGUAGE plpgsql SECURITY DEFINER
    SET search_path TO 'pg_catalog'
    AS $$
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

CREATE FUNCTION public.theatre_native_finish(p_flow uuid, p_issuer text, p_subject text, p_candidate text, p_token bytea, p_proof bytea, p_access text, p_expires bigint, p_session uuid) RETURNS text
    LANGUAGE plpgsql SECURITY DEFINER
    SET search_path TO 'pg_catalog'
    AS $$
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

CREATE FUNCTION public.theatre_registration_guard() RETURNS trigger
    LANGUAGE plpgsql
    SET search_path TO 'pg_catalog'
    AS $$
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

CREATE TABLE public.actor_external_identities (
    issuer text NOT NULL COLLATE pg_catalog."C",
    subject text NOT NULL COLLATE pg_catalog."C",
    actor_id text NOT NULL,
    linked_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    -- Keep Unicode escapes literal: line splitting must not turn U+2028/U+2029 into LF.
    CONSTRAINT actor_external_identities_issuer_check CHECK (
        octet_length(issuer) BETWEEN 1 AND 1024
        AND issuer !~ U&'^[\0020\00A0\1680\2000-\200A\2028\2029\202F\205F\3000]*$'
        AND issuer !~ U&'[\0001-\001F\007F-\009F]'
    ),
    CONSTRAINT actor_external_identities_subject_check CHECK (
        octet_length(subject) BETWEEN 1 AND 512
        AND subject !~ U&'^[\0020\00A0\1680\2000-\200A\2028\2029\202F\205F\3000]*$'
        AND subject !~ U&'[\0001-\001F\007F-\009F]'
    )
);

CREATE TABLE public.actors (
    id text NOT NULL,
    active boolean DEFAULT true NOT NULL,
    CONSTRAINT actors_id_check CHECK ((id ~ '^[a-z0-9_-]{1,64}$'::text))
);

CREATE TABLE public.adaptation_acceptances (
    run_id text NOT NULL,
    owner_id text NOT NULL,
    operation_id text NOT NULL,
    request jsonb NOT NULL,
    script_id text NOT NULL,
    revision bigint NOT NULL,
    CONSTRAINT adaptation_acceptances_operation_id_check CHECK ((operation_id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'::text)),
    CONSTRAINT adaptation_acceptances_request_check CHECK (((jsonb_typeof(request) = 'object'::text) AND (octet_length((request)::text) <= 4194304))),
    CONSTRAINT adaptation_acceptances_script_id_check CHECK ((script_id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'::text))
);

CREATE TABLE public.adaptation_cancellations (
    owner_id text NOT NULL,
    operation_id text NOT NULL,
    run_id text NOT NULL,
    recorded_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    CONSTRAINT adaptation_cancellations_operation_id_check CHECK ((operation_id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'::text))
);

CREATE TABLE public.adaptation_proposals (
    run_id text NOT NULL,
    proposal jsonb NOT NULL,
    digest text NOT NULL,
    CONSTRAINT adaptation_proposals_digest_check CHECK ((digest ~ '^[a-f0-9]{64}$'::text)),
    CONSTRAINT adaptation_proposals_proposal_check CHECK (((jsonb_typeof(proposal) = 'object'::text) AND (octet_length((proposal)::text) <= 4194304)))
);

CREATE TABLE public.adaptation_runs (
    id text NOT NULL,
    owner_id text NOT NULL,
    operation_id text NOT NULL,
    source_id text NOT NULL,
    script_id text NOT NULL,
    frozen_input jsonb NOT NULL,
    input_digest text NOT NULL,
    status text NOT NULL,
    input_version text DEFAULT 'c1'::text NOT NULL,
    recorded_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    CONSTRAINT adaptation_runs_frozen_input_check CHECK (((jsonb_typeof(frozen_input) = 'object'::text) AND (octet_length((frozen_input)::text) <= 6291456))),
    CONSTRAINT adaptation_runs_id_check CHECK ((id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'::text)),
    CONSTRAINT adaptation_runs_input_digest_check CHECK ((input_digest ~ '^[a-f0-9]{64}$'::text)),
    CONSTRAINT adaptation_runs_input_version_check CHECK ((input_version = 'c1'::text)),
    CONSTRAINT adaptation_runs_operation_id_check CHECK ((operation_id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'::text)),
    CONSTRAINT adaptation_runs_script_id_check CHECK ((script_id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'::text)),
    CONSTRAINT adaptation_runs_status_check CHECK ((status = ANY (ARRAY['awaiting_proposal'::text, 'succeeded'::text, 'cancelled'::text, 'accepted'::text])))
);

CREATE TABLE public.adaptation_submissions (
    id text NOT NULL,
    run_id text NOT NULL,
    owner_id text NOT NULL,
    operation_id text NOT NULL,
    submission_number bigint NOT NULL,
    request jsonb NOT NULL,
    receipt jsonb NOT NULL,
    receipt_digest text NOT NULL,
    submitted_at timestamp with time zone NOT NULL,
    CONSTRAINT adaptation_submissions_id_check CHECK ((id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'::text)),
    CONSTRAINT adaptation_submissions_operation_id_check CHECK ((operation_id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'::text)),
    CONSTRAINT adaptation_submissions_receipt_check CHECK (((jsonb_typeof(receipt) = 'object'::text) AND (octet_length((receipt)::text) <= 4194304))),
    CONSTRAINT adaptation_submissions_receipt_digest_check CHECK ((receipt_digest ~ '^[a-f0-9]{64}$'::text)),
    CONSTRAINT adaptation_submissions_request_check CHECK (((jsonb_typeof(request) = 'object'::text) AND (octet_length((request)::text) <= 2097152))),
    CONSTRAINT adaptation_submissions_submission_number_check CHECK ((submission_number > 0))
);

CREATE TABLE public.admin_access_revocations (
    actor_id text NOT NULL,
    revoked_at timestamp with time zone DEFAULT clock_timestamp() NOT NULL
);

CREATE TABLE public.admin_audit (
    id uuid NOT NULL,
    actor_id text,
    surface text NOT NULL,
    action text NOT NULL,
    target_actor_id text,
    outcome text NOT NULL,
    occurred_at timestamp with time zone DEFAULT clock_timestamp() NOT NULL,
    correlation_id uuid NOT NULL,
    CONSTRAINT admin_audit_action_check CHECK ((action = ANY (ARRAY['capabilities'::text, 'revoke_admin_access'::text]))),
    CONSTRAINT admin_audit_outcome_check CHECK ((outcome = ANY (ARRAY['allowed'::text, 'denied'::text, 'replayed'::text, 'conflict'::text, 'unavailable'::text]))),
    CONSTRAINT admin_audit_surface_check CHECK ((surface = 'admin'::text))
);

CREATE TABLE public.admin_operations (
    actor_id text NOT NULL,
    operation_id uuid NOT NULL,
    target_actor_id text NOT NULL,
    audit_id uuid NOT NULL
);

CREATE TABLE public.audio_distribution_clearances (
    id text NOT NULL,
    candidate_id text NOT NULL,
    owner_id text NOT NULL,
    operation_id text NOT NULL,
    sequence bigint NOT NULL,
    snapshot bytea NOT NULL,
    digest text NOT NULL,
    CONSTRAINT audio_distribution_clearances_digest_check CHECK ((digest ~ '^dc-r1:sha256:[a-f0-9]{64}$'::text)),
    CONSTRAINT audio_distribution_clearances_id_check CHECK ((id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'::text)),
    CONSTRAINT audio_distribution_clearances_operation_id_check CHECK ((operation_id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'::text)),
    CONSTRAINT audio_distribution_clearances_sequence_check CHECK ((sequence > 0)),
    CONSTRAINT audio_distribution_clearances_snapshot_check CHECK (((octet_length(snapshot) >= 1) AND (octet_length(snapshot) <= 32768)))
);

CREATE TABLE public.audio_intake_receipts (
    id text NOT NULL,
    intent_id text NOT NULL,
    script_id text NOT NULL,
    script_revision bigint NOT NULL,
    content_digest text NOT NULL,
    profile_id text NOT NULL,
    operation_id text NOT NULL,
    accepted_by text NOT NULL,
    accepted_at timestamp with time zone NOT NULL,
    object_key text NOT NULL,
    receipt_bytes bytea NOT NULL,
    receipt_digest text NOT NULL,
    storage_target_id text DEFAULT 'legacy-filesystem'::text NOT NULL,
    CONSTRAINT audio_intake_receipts_id_check CHECK ((id ~ '^[a-f0-9-]{36}$'::text)),
    CONSTRAINT audio_intake_receipts_object_key_check CHECK ((object_key ~ '^[a-f0-9]{64}$'::text)),
    CONSTRAINT audio_intake_receipts_receipt_bytes_check CHECK (((octet_length(receipt_bytes) >= 1) AND (octet_length(receipt_bytes) <= 8192))),
    CONSTRAINT audio_intake_receipts_receipt_digest_check CHECK ((receipt_digest ~ '^audio-intake-r1:sha256:[a-f0-9]{64}$'::text)),
    CONSTRAINT audio_intake_receipts_storage_target_id_check CHECK (((storage_target_id ~ '^[a-z][a-z0-9-]{0,62}$'::text) AND (storage_target_id !~ '-$'::text)))
);

CREATE TABLE public.audio_listener_releases (
    release_id text NOT NULL,
    work_id text NOT NULL,
    unit_id text NOT NULL,
    published_at bigint NOT NULL,
    kind text NOT NULL,
    language text NOT NULL,
    search_text text NOT NULL,
    rights_digest text NOT NULL,
    approval_digest text NOT NULL,
    rights_start bigint NOT NULL,
    rights_end bigint NOT NULL,
    work_revision bigint NOT NULL,
    unit_revision bigint NOT NULL,
    content_revision_id text NOT NULL,
    CONSTRAINT audio_listener_releases_approval_digest_check CHECK ((approval_digest ~ '^pa-r1:sha256:[a-f0-9]{64}$'::text)),
    CONSTRAINT audio_listener_releases_check CHECK ((rights_end > rights_start)),
    CONSTRAINT audio_listener_releases_kind_check CHECK ((kind = ANY (ARRAY['novel'::text, 'story'::text, 'podcast'::text]))),
    CONSTRAINT audio_listener_releases_language_check CHECK (((octet_length(language) >= 1) AND (octet_length(language) <= 63))),
    CONSTRAINT audio_listener_releases_published_at_check CHECK ((published_at >= 0)),
    CONSTRAINT audio_listener_releases_rights_digest_check CHECK ((rights_digest ~ '^dc-r1:sha256:[a-f0-9]{64}$'::text)),
    CONSTRAINT audio_listener_releases_rights_start_check CHECK ((rights_start >= 0)),
    CONSTRAINT audio_listener_releases_search_text_check CHECK ((octet_length(search_text) <= 32768)),
    CONSTRAINT audio_listener_releases_unit_revision_check CHECK ((unit_revision > 0)),
    CONSTRAINT audio_listener_releases_work_revision_check CHECK ((work_revision > 0))
);

CREATE TABLE public.audio_listener_units (
    unit_id text NOT NULL,
    work_id text NOT NULL,
    first_position integer NOT NULL,
    CONSTRAINT audio_listener_units_first_position_check CHECK ((first_position >= 0))
);

CREATE TABLE public.audio_listener_works (
    work_id text NOT NULL,
    first_release_id text NOT NULL,
    first_published_at bigint NOT NULL,
    CONSTRAINT audio_listener_works_first_published_at_check CHECK ((first_published_at >= 0))
);

CREATE TABLE public.audio_publication_approvals (
    id text NOT NULL,
    candidate_id text NOT NULL,
    owner_id text NOT NULL,
    operation_id text NOT NULL,
    sequence bigint NOT NULL,
    snapshot bytea NOT NULL,
    digest text NOT NULL,
    CONSTRAINT audio_publication_approvals_digest_check CHECK ((digest ~ '^pa-r1:sha256:[a-f0-9]{64}$'::text)),
    CONSTRAINT audio_publication_approvals_id_check CHECK ((id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'::text)),
    CONSTRAINT audio_publication_approvals_operation_id_check CHECK ((operation_id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'::text)),
    CONSTRAINT audio_publication_approvals_sequence_check CHECK ((sequence > 0)),
    CONSTRAINT audio_publication_approvals_snapshot_check CHECK (((octet_length(snapshot) >= 1) AND (octet_length(snapshot) <= 8192)))
);

CREATE TABLE public.audio_publication_events (
    id text NOT NULL,
    unit_id text NOT NULL,
    pointer_version bigint NOT NULL,
    kind text NOT NULL,
    release_id text NOT NULL,
    actor_id text NOT NULL,
    recorded_at bigint NOT NULL,
    CONSTRAINT audio_publication_events_id_check CHECK ((id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'::text)),
    CONSTRAINT audio_publication_events_kind_check CHECK ((kind = ANY (ARRAY['activated'::text, 'withdrawn'::text]))),
    CONSTRAINT audio_publication_events_pointer_version_check CHECK ((pointer_version >= 0)),
    CONSTRAINT audio_publication_events_recorded_at_check CHECK ((recorded_at >= 0))
);

CREATE TABLE public.audio_publication_operations (
    owner_id text NOT NULL,
    operation_id text NOT NULL,
    kind text NOT NULL,
    request_digest text NOT NULL,
    unit_id text NOT NULL,
    release_id text NOT NULL,
    pointer_version bigint NOT NULL,
    CONSTRAINT audio_publication_operations_kind_check CHECK ((kind = ANY (ARRAY['publish'::text, 'withdraw'::text]))),
    CONSTRAINT audio_publication_operations_operation_id_check CHECK ((operation_id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'::text)),
    CONSTRAINT audio_publication_operations_pointer_version_check CHECK ((pointer_version >= 0)),
    CONSTRAINT audio_publication_operations_request_digest_check CHECK ((request_digest ~ '^publication-request1:sha256:[a-f0-9]{64}$'::text))
);

CREATE TABLE public.audio_publication_pointers (
    unit_id text NOT NULL,
    owner_id text NOT NULL,
    active_release_id text,
    version bigint NOT NULL,
    CONSTRAINT audio_publication_pointers_version_check CHECK ((version >= 0))
);

CREATE TABLE public.audio_release_candidates (
    id text NOT NULL,
    rendition_id text NOT NULL,
    owner_id text NOT NULL,
    operation_id text NOT NULL,
    snapshot bytea NOT NULL,
    digest text NOT NULL,
    CONSTRAINT audio_release_candidates_digest_check CHECK ((digest ~ '^rc-r1:sha256:[a-f0-9]{64}$'::text)),
    CONSTRAINT audio_release_candidates_id_check CHECK ((id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'::text)),
    CONSTRAINT audio_release_candidates_operation_id_check CHECK ((operation_id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'::text)),
    CONSTRAINT audio_release_candidates_snapshot_check CHECK (((octet_length(snapshot) >= 1) AND (octet_length(snapshot) <= 32768)))
);

CREATE TABLE public.audio_release_withdrawals (
    release_id text NOT NULL,
    actor_id text NOT NULL,
    recorded_at bigint NOT NULL,
    reason text NOT NULL,
    CONSTRAINT audio_release_withdrawals_reason_check CHECK (((octet_length(reason) >= 1) AND (octet_length(reason) <= 4096))),
    CONSTRAINT audio_release_withdrawals_recorded_at_check CHECK ((recorded_at >= 0))
);

CREATE TABLE public.audio_releases (
    id text NOT NULL,
    unit_id text NOT NULL,
    owner_id text NOT NULL,
    candidate_id text NOT NULL,
    rendition_id text NOT NULL,
    snapshot bytea NOT NULL,
    manifest_digest text NOT NULL,
    digest text NOT NULL,
    CONSTRAINT audio_releases_digest_check CHECK ((digest ~ '^rl-r1:sha256:[a-f0-9]{64}$'::text)),
    CONSTRAINT audio_releases_id_check CHECK ((id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'::text)),
    CONSTRAINT audio_releases_manifest_digest_check CHECK ((manifest_digest ~ '^rm-r1:sha256:[a-f0-9]{64}$'::text)),
    CONSTRAINT audio_releases_snapshot_check CHECK (((octet_length(snapshot) >= 1) AND (octet_length(snapshot) <= 262144)))
);

CREATE TABLE public.audio_renditions (
    id text NOT NULL,
    submission_id text NOT NULL,
    object_key text NOT NULL,
    snapshot bytea NOT NULL,
    digest text NOT NULL,
    storage_target_id text DEFAULT 'legacy-filesystem'::text NOT NULL,
    CONSTRAINT audio_renditions_digest_check CHECK ((digest ~ '^ar-r1:sha256:[a-f0-9]{64}$'::text)),
    CONSTRAINT audio_renditions_id_check CHECK ((id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'::text)),
    CONSTRAINT audio_renditions_object_key_check CHECK ((object_key ~ '^[a-f0-9]{64}$'::text)),
    CONSTRAINT audio_renditions_snapshot_check CHECK (((octet_length(snapshot) >= 1) AND (octet_length(snapshot) <= 32768))),
    CONSTRAINT audio_renditions_storage_target_id_check CHECK (((storage_target_id ~ '^[a-z][a-z0-9-]{0,62}$'::text) AND (storage_target_id !~ '-$'::text)))
);

CREATE TABLE public.audio_reviews (
    id text NOT NULL,
    rendition_id text NOT NULL,
    owner_id text NOT NULL,
    operation_id text NOT NULL,
    sequence bigint NOT NULL,
    snapshot bytea NOT NULL,
    digest text NOT NULL,
    CONSTRAINT audio_reviews_digest_check CHECK ((digest ~ '^aq-r1:sha256:[a-f0-9]{64}$'::text)),
    CONSTRAINT audio_reviews_id_check CHECK ((id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'::text)),
    CONSTRAINT audio_reviews_operation_id_check CHECK ((operation_id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'::text)),
    CONSTRAINT audio_reviews_sequence_check CHECK ((sequence > 0)),
    CONSTRAINT audio_reviews_snapshot_check CHECK (((octet_length(snapshot) >= 1) AND (octet_length(snapshot) <= 32768)))
);

CREATE TABLE public.audio_submission_revocations (
    submission_id text NOT NULL,
    owner_id text NOT NULL,
    revoked_at bigint NOT NULL,
    CONSTRAINT audio_submission_revocations_revoked_at_check CHECK ((revoked_at >= 0)),
    CONSTRAINT audio_submission_revocations_submission_id_check CHECK ((submission_id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'::text))
);

CREATE TABLE public.audio_submissions (
    id text NOT NULL,
    unit_id text NOT NULL,
    owner_id text NOT NULL,
    brief_id text NOT NULL,
    brief_digest text NOT NULL,
    operation_id text NOT NULL,
    request_digest text NOT NULL,
    token_hash bytea NOT NULL,
    snapshot bytea NOT NULL,
    digest text NOT NULL,
    CONSTRAINT audio_submissions_digest_check CHECK ((digest ~ '^as-r1:sha256:[a-f0-9]{64}$'::text)),
    CONSTRAINT audio_submissions_id_check CHECK ((id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'::text)),
    CONSTRAINT audio_submissions_operation_id_check CHECK ((operation_id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'::text)),
    CONSTRAINT audio_submissions_request_digest_check CHECK ((request_digest ~ '^as-request1:sha256:[a-f0-9]{64}$'::text)),
    CONSTRAINT audio_submissions_snapshot_check CHECK (((octet_length(snapshot) >= 1) AND (octet_length(snapshot) <= 32768))),
    CONSTRAINT audio_submissions_token_hash_check CHECK ((octet_length(token_hash) = 32))
);

CREATE TABLE public.audio_upload_intents (
    id text NOT NULL,
    script_id text NOT NULL,
    script_revision bigint NOT NULL,
    content_digest text NOT NULL,
    profile_id text NOT NULL,
    operation_id text NOT NULL,
    created_by text NOT NULL,
    created_at timestamp with time zone DEFAULT clock_timestamp() NOT NULL,
    CONSTRAINT audio_upload_intents_content_digest_check CHECK ((content_digest ~ '^sir-c1:sha256:[a-f0-9]{64}$'::text)),
    CONSTRAINT audio_upload_intents_id_check CHECK ((id ~ '^[a-f0-9-]{36}$'::text)),
    CONSTRAINT audio_upload_intents_operation_id_check CHECK ((operation_id ~ '^[a-f0-9-]{36}$'::text)),
    CONSTRAINT audio_upload_intents_profile_id_check CHECK ((profile_id = ANY (ARRAY['opus-mono-20-v1'::text, 'opus-mono-24-v1'::text, 'opus-mono-32-v1'::text, 'opus-mono-40-v1'::text]))),
    CONSTRAINT audio_upload_intents_script_revision_check CHECK ((script_revision > 0))
);

CREATE TABLE public.catalog_clock (
    singleton boolean DEFAULT true NOT NULL,
    revision bigint NOT NULL,
    CONSTRAINT catalog_clock_revision_check CHECK ((revision >= 0)),
    CONSTRAINT catalog_clock_singleton_check CHECK (singleton)
);

CREATE TABLE public.catalog_content_heads (
    unit_id text NOT NULL,
    revision_id text NOT NULL,
    revision bigint NOT NULL,
    CONSTRAINT catalog_content_heads_revision_check CHECK ((revision > 0))
);

CREATE TABLE public.catalog_content_revisions (
    id text NOT NULL,
    unit_id text NOT NULL,
    revision bigint NOT NULL,
    snapshot bytea NOT NULL,
    revision_digest text NOT NULL,
    recorded_by text NOT NULL,
    CONSTRAINT catalog_content_revisions_id_check CHECK ((id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'::text)),
    CONSTRAINT catalog_content_revisions_revision_check CHECK ((revision > 0)),
    CONSTRAINT catalog_content_revisions_revision_digest_check CHECK ((revision_digest ~ '^cc-r1:sha256:[a-f0-9]{64}$'::text)),
    CONSTRAINT catalog_content_revisions_snapshot_check CHECK (((octet_length(snapshot) >= 1) AND (octet_length(snapshot) <= 131072)))
);

CREATE TABLE public.catalog_delegated_operations (
    grant_id text NOT NULL,
    actor_id text NOT NULL,
    operation_id text NOT NULL
);

CREATE TABLE public.catalog_delegation_grants (
    id text NOT NULL,
    work_id text NOT NULL,
    grantor_id text NOT NULL,
    delegate_id text NOT NULL,
    work_read boolean NOT NULL,
    content_read boolean NOT NULL,
    content_write boolean NOT NULL,
    token_hash bytea NOT NULL,
    issued_at bigint NOT NULL,
    expires_at bigint NOT NULL,
    operation_id text NOT NULL,
    request_digest text NOT NULL,
    CONSTRAINT catalog_delegation_grants_check CHECK ((delegate_id <> grantor_id)),
    CONSTRAINT catalog_delegation_grants_check1 CHECK ((work_read OR content_read OR content_write)),
    CONSTRAINT catalog_delegation_grants_check2 CHECK (((expires_at > issued_at) AND ((expires_at - issued_at) <= '2592000000'::bigint))),
    CONSTRAINT catalog_delegation_grants_id_check CHECK ((id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'::text)),
    CONSTRAINT catalog_delegation_grants_issued_at_check CHECK ((issued_at >= 0)),
    CONSTRAINT catalog_delegation_grants_operation_id_check CHECK ((operation_id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'::text)),
    CONSTRAINT catalog_delegation_grants_request_digest_check CHECK ((request_digest ~ '^dg-request1:sha256:[a-f0-9]{64}$'::text)),
    CONSTRAINT catalog_delegation_grants_token_hash_check CHECK ((octet_length(token_hash) = 32))
);

CREATE TABLE public.catalog_delegation_revocations (
    grant_id text NOT NULL,
    revoked_by text NOT NULL,
    revoked_at bigint NOT NULL,
    CONSTRAINT catalog_delegation_revocations_revoked_at_check CHECK ((revoked_at >= 0))
);

CREATE TABLE public.catalog_members (
    work_id text NOT NULL,
    actor_id text NOT NULL,
    role text NOT NULL,
    CONSTRAINT catalog_members_role_check CHECK ((role = ANY (ARRAY['reader'::text, 'editor'::text])))
);

CREATE TABLE public.catalog_operations (
    actor_id text NOT NULL,
    operation_id text NOT NULL,
    fingerprint text NOT NULL,
    result_digest text NOT NULL,
    result_bytes bytea NOT NULL,
    CONSTRAINT catalog_operations_fingerprint_check CHECK ((fingerprint ~ '^[a-f0-9]{64}$'::text)),
    CONSTRAINT catalog_operations_operation_id_check CHECK ((operation_id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'::text)),
    CONSTRAINT catalog_operations_result_bytes_check CHECK (((octet_length(result_bytes) >= 1) AND (octet_length(result_bytes) <= 4194304))),
    CONSTRAINT catalog_operations_result_digest_check CHECK ((result_digest ~ '^[a-f0-9]{64}$'::text))
);

CREATE TABLE public.catalog_unit_revisions (
    unit_id text NOT NULL,
    revision bigint NOT NULL,
    snapshot bytea NOT NULL,
    snapshot_digest text NOT NULL,
    recorded_by text NOT NULL,
    CONSTRAINT catalog_unit_revisions_revision_check CHECK ((revision > 0)),
    CONSTRAINT catalog_unit_revisions_snapshot_check CHECK (((octet_length(snapshot) >= 1) AND (octet_length(snapshot) <= 131072))),
    CONSTRAINT catalog_unit_revisions_snapshot_digest_check CHECK ((snapshot_digest ~ '^[a-f0-9]{64}$'::text))
);

CREATE TABLE public.catalog_units (
    id text NOT NULL,
    work_id text NOT NULL,
    work_kind text NOT NULL,
    kind text NOT NULL,
    head_revision bigint NOT NULL,
    "position" integer NOT NULL,
    lifecycle text NOT NULL,
    CONSTRAINT catalog_units_check CHECK ((((work_kind = ANY (ARRAY['novel'::text, 'story'::text])) AND (kind = 'chapter'::text)) OR ((work_kind = 'podcast'::text) AND (kind = 'episode'::text)))),
    CONSTRAINT catalog_units_head_revision_check CHECK ((head_revision > 0)),
    CONSTRAINT catalog_units_id_check CHECK ((id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'::text)),
    CONSTRAINT catalog_units_kind_check CHECK ((kind = ANY (ARRAY['chapter'::text, 'episode'::text]))),
    CONSTRAINT catalog_units_lifecycle_check CHECK ((lifecycle = ANY (ARRAY['draft'::text, 'archived'::text]))),
    CONSTRAINT catalog_units_position_check CHECK (("position" >= 0))
);

CREATE TABLE public.catalog_work_revisions (
    work_id text NOT NULL,
    revision bigint NOT NULL,
    snapshot bytea NOT NULL,
    snapshot_digest text NOT NULL,
    recorded_by text NOT NULL,
    CONSTRAINT catalog_work_revisions_revision_check CHECK ((revision > 0)),
    CONSTRAINT catalog_work_revisions_snapshot_check CHECK (((octet_length(snapshot) >= 1) AND (octet_length(snapshot) <= 131072))),
    CONSTRAINT catalog_work_revisions_snapshot_digest_check CHECK ((snapshot_digest ~ '^[a-f0-9]{64}$'::text))
);

CREATE TABLE public.catalog_works (
    id text NOT NULL,
    owner_id text NOT NULL,
    kind text NOT NULL,
    lifecycle text NOT NULL,
    head_revision bigint NOT NULL,
    CONSTRAINT catalog_works_head_revision_check CHECK ((head_revision > 0)),
    CONSTRAINT catalog_works_id_check CHECK ((id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'::text)),
    CONSTRAINT catalog_works_kind_check CHECK ((kind = ANY (ARRAY['novel'::text, 'story'::text, 'podcast'::text]))),
    CONSTRAINT catalog_works_lifecycle_check CHECK ((lifecycle = ANY (ARRAY['draft'::text, 'archived'::text])))
);

CREATE TABLE public.listener_device_sequences (
    listener_id text NOT NULL,
    device_id text NOT NULL,
    resource_key text NOT NULL,
    sequence bigint NOT NULL,
    CONSTRAINT listener_device_sequences_device_id_check CHECK ((device_id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'::text)),
    CONSTRAINT listener_device_sequences_resource_key_check CHECK (((octet_length(resource_key) >= 1) AND (octet_length(resource_key) <= 128))),
    CONSTRAINT listener_device_sequences_sequence_check CHECK ((sequence > 0))
);

CREATE TABLE public.listener_history (
    listener_id text NOT NULL,
    work_id text NOT NULL,
    publication_id text NOT NULL,
    unit_id text NOT NULL,
    listener_order bigint NOT NULL,
    started_at bigint NOT NULL,
    snapshot bytea NOT NULL,
    snapshot_digest bytea NOT NULL,
    CONSTRAINT listener_history_listener_order_check CHECK ((listener_order > 0)),
    CONSTRAINT listener_history_snapshot_check CHECK (((octet_length(snapshot) >= 1) AND (octet_length(snapshot) <= 4096))),
    CONSTRAINT listener_history_snapshot_digest_check CHECK ((octet_length(snapshot_digest) = 32)),
    CONSTRAINT listener_history_started_at_check CHECK ((started_at >= 0))
);

CREATE TABLE public.listener_operations (
    listener_id text NOT NULL,
    operation_id text NOT NULL,
    request_digest bytea NOT NULL,
    response_bytes bytea NOT NULL,
    response_digest bytea NOT NULL,
    CONSTRAINT listener_operations_operation_id_check CHECK ((operation_id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'::text)),
    CONSTRAINT listener_operations_request_digest_check CHECK ((octet_length(request_digest) = 32)),
    CONSTRAINT listener_operations_response_bytes_check CHECK (((octet_length(response_bytes) >= 1) AND (octet_length(response_bytes) <= 16384))),
    CONSTRAINT listener_operations_response_digest_check CHECK ((octet_length(response_digest) = 32))
);

CREATE TABLE public.listener_playback_sessions (
    listener_id text NOT NULL,
    device_id text NOT NULL,
    engine_session_id text NOT NULL,
    audio_snapshot bytea NOT NULL,
    CONSTRAINT listener_playback_sessions_audio_snapshot_check CHECK (((octet_length(audio_snapshot) >= 1) AND (octet_length(audio_snapshot) <= 4096))),
    CONSTRAINT listener_playback_sessions_device_id_check CHECK ((device_id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'::text)),
    CONSTRAINT listener_playback_sessions_engine_session_id_check CHECK ((engine_session_id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'::text))
);

CREATE TABLE public.listener_progress (
    listener_id text NOT NULL,
    publication_id text NOT NULL,
    work_id text NOT NULL,
    unit_id text NOT NULL,
    rendition_id text NOT NULL,
    sha256 text NOT NULL,
    duration_ms bigint NOT NULL,
    position_ms bigint NOT NULL,
    completion text NOT NULL,
    revision bigint NOT NULL,
    listener_order bigint NOT NULL,
    accepted_at bigint NOT NULL,
    snapshot bytea NOT NULL,
    snapshot_digest bytea NOT NULL,
    CONSTRAINT listener_progress_accepted_at_check CHECK ((accepted_at >= 0)),
    CONSTRAINT listener_progress_check CHECK (((position_ms >= 0) AND (position_ms <= duration_ms))),
    CONSTRAINT listener_progress_check1 CHECK (((convert_from(snapshot, 'UTF8'::name))::jsonb @> jsonb_build_object('audio', jsonb_build_object('work_id', work_id, 'unit_id', unit_id, 'publication_id', publication_id, 'rendition_id', rendition_id, 'sha256', sha256, 'duration_ms', duration_ms), 'position_ms', position_ms, 'completion', completion, 'revision', revision, 'listener_order', listener_order, 'accepted_at_ms', accepted_at))),
    CONSTRAINT listener_progress_completion_check CHECK ((completion = ANY (ARRAY['in_progress'::text, 'completed'::text]))),
    CONSTRAINT listener_progress_duration_ms_check CHECK ((duration_ms > 0)),
    CONSTRAINT listener_progress_listener_order_check CHECK ((listener_order > 0)),
    CONSTRAINT listener_progress_revision_check CHECK ((revision > 0)),
    CONSTRAINT listener_progress_sha256_check CHECK ((sha256 ~ '^[a-f0-9]{64}$'::text)),
    CONSTRAINT listener_progress_snapshot_check CHECK (((octet_length(snapshot) >= 1) AND (octet_length(snapshot) <= 8192))),
    CONSTRAINT listener_progress_snapshot_digest_check CHECK ((octet_length(snapshot_digest) = 32))
);

CREATE TABLE public.listener_saved (
    listener_id text NOT NULL,
    work_id text NOT NULL,
    saved boolean NOT NULL,
    revision bigint NOT NULL,
    listener_order bigint NOT NULL,
    accepted_at bigint NOT NULL,
    snapshot bytea NOT NULL,
    snapshot_digest bytea NOT NULL,
    CONSTRAINT listener_saved_accepted_at_check CHECK ((accepted_at >= 0)),
    CONSTRAINT listener_saved_check CHECK (((convert_from(snapshot, 'UTF8'::name))::jsonb @> jsonb_build_object('work_id', work_id, 'saved', saved, 'revision', revision, 'listener_order', listener_order, 'accepted_at_ms', accepted_at))),
    CONSTRAINT listener_saved_listener_order_check CHECK ((listener_order > 0)),
    CONSTRAINT listener_saved_revision_check CHECK ((revision > 0)),
    CONSTRAINT listener_saved_snapshot_check CHECK (((octet_length(snapshot) >= 1) AND (octet_length(snapshot) <= 4096))),
    CONSTRAINT listener_saved_snapshot_digest_check CHECK ((octet_length(snapshot_digest) = 32))
);

CREATE TABLE public.listener_state (
    listener_id text NOT NULL,
    revision bigint DEFAULT 0 NOT NULL,
    history_revision bigint DEFAULT 0 NOT NULL,
    history_epoch bigint DEFAULT 0 NOT NULL,
    CONSTRAINT listener_state_check CHECK (((history_epoch >= 0) AND (history_epoch <= history_revision))),
    CONSTRAINT listener_state_history_revision_check CHECK ((history_revision >= 0)),
    CONSTRAINT listener_state_revision_check CHECK ((revision >= 0))
);

CREATE TABLE public.media_artwork_operations (
    owner_id text NOT NULL,
    operation_id text NOT NULL,
    work_id text NOT NULL,
    kind text NOT NULL,
    fingerprint text NOT NULL,
    response_bytes bytea NOT NULL,
    response_checksum text NOT NULL,
    CONSTRAINT media_artwork_operations_fingerprint_check CHECK ((fingerprint ~ '^image-operation1:sha256:[a-f0-9]{64}$'::text)),
    CONSTRAINT media_artwork_operations_kind_check CHECK ((kind = ANY (ARRAY['prepare_image'::text, 'set_artwork'::text]))),
    CONSTRAINT media_artwork_operations_operation_id_check CHECK ((operation_id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'::text)),
    CONSTRAINT media_artwork_operations_response_bytes_check CHECK (((octet_length(response_bytes) >= 1) AND (octet_length(response_bytes) <= 32768))),
    CONSTRAINT media_artwork_operations_response_checksum_check CHECK ((response_checksum ~ '^[a-f0-9]{64}$'::text))
);

CREATE TABLE public.media_image_assets (
    id text NOT NULL,
    intent_id text NOT NULL,
    work_id text NOT NULL,
    owner_id text NOT NULL,
    storage_target text NOT NULL,
    object_key text NOT NULL,
    sha256 text NOT NULL,
    byte_length bigint NOT NULL,
    content_type text NOT NULL,
    width_px integer NOT NULL,
    height_px integer NOT NULL,
    profile_id text NOT NULL,
    admitted_at_ms bigint NOT NULL,
    document_bytes bytea NOT NULL,
    digest text NOT NULL,
    CONSTRAINT media_image_assets_admitted_at_ms_check CHECK ((admitted_at_ms >= 0)),
    CONSTRAINT media_image_assets_byte_length_check CHECK (((byte_length >= 1) AND (byte_length <= 8388608))),
    CONSTRAINT media_image_assets_check CHECK ((id = intent_id)),
    CONSTRAINT media_image_assets_check1 CHECK (((sha256 ~ '^[a-f0-9]{64}$'::text) AND (object_key = sha256))),
    CONSTRAINT media_image_assets_check2 CHECK ((((width_px)::bigint * (height_px)::bigint) <= 16777216)),
    CONSTRAINT media_image_assets_content_type_check CHECK ((content_type = ANY (ARRAY['image/png'::text, 'image/jpeg'::text]))),
    CONSTRAINT media_image_assets_digest_check CHECK ((digest ~ '^ia-r1:sha256:[a-f0-9]{64}$'::text)),
    CONSTRAINT media_image_assets_document_bytes_check CHECK (((octet_length(document_bytes) >= 1) AND (octet_length(document_bytes) <= 32768))),
    CONSTRAINT media_image_assets_height_px_check CHECK (((height_px >= 1) AND (height_px <= 4096))),
    CONSTRAINT media_image_assets_id_check CHECK ((id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'::text)),
    CONSTRAINT media_image_assets_object_key_check CHECK ((object_key ~ '^[a-f0-9]{64}$'::text)),
    CONSTRAINT media_image_assets_profile_id_check CHECK ((profile_id = 'cover-image-v1'::text)),
    CONSTRAINT media_image_assets_storage_target_check CHECK (((storage_target ~ '^[a-z][a-z0-9-]{0,62}$'::text) AND (storage_target !~ '-$'::text))),
    CONSTRAINT media_image_assets_width_px_check CHECK (((width_px >= 1) AND (width_px <= 4096)))
);

CREATE TABLE public.media_image_intents (
    id text NOT NULL,
    work_id text NOT NULL,
    owner_id text NOT NULL,
    operation_id text NOT NULL,
    expected_sha256 text NOT NULL,
    expected_byte_length bigint NOT NULL,
    content_type text NOT NULL,
    profile_id text NOT NULL,
    created_at_ms bigint NOT NULL,
    expires_at_ms bigint NOT NULL,
    document_bytes bytea NOT NULL,
    digest text NOT NULL,
    CONSTRAINT media_image_intents_check CHECK (((expires_at_ms >= 0) AND ((expires_at_ms - created_at_ms) = 1800000))),
    CONSTRAINT media_image_intents_content_type_check CHECK ((content_type = ANY (ARRAY['image/png'::text, 'image/jpeg'::text]))),
    CONSTRAINT media_image_intents_created_at_ms_check CHECK ((created_at_ms >= 0)),
    CONSTRAINT media_image_intents_digest_check CHECK ((digest ~ '^ii-r1:sha256:[a-f0-9]{64}$'::text)),
    CONSTRAINT media_image_intents_document_bytes_check CHECK (((octet_length(document_bytes) >= 1) AND (octet_length(document_bytes) <= 32768))),
    CONSTRAINT media_image_intents_expected_byte_length_check CHECK (((expected_byte_length >= 1) AND (expected_byte_length <= 8388608))),
    CONSTRAINT media_image_intents_expected_sha256_check CHECK ((expected_sha256 ~ '^[a-f0-9]{64}$'::text)),
    CONSTRAINT media_image_intents_id_check CHECK ((id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'::text)),
    CONSTRAINT media_image_intents_operation_id_check CHECK ((operation_id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'::text)),
    CONSTRAINT media_image_intents_profile_id_check CHECK ((profile_id = 'cover-image-v1'::text))
);

CREATE TABLE public.media_work_artwork_changes (
    work_id text NOT NULL,
    work_metadata_revision bigint NOT NULL,
    owner_id text NOT NULL,
    image_asset_id text,
    image_asset_digest text,
    alt_text text,
    recorded_by text NOT NULL,
    recorded_at_ms bigint NOT NULL,
    document_bytes bytea NOT NULL,
    digest text NOT NULL,
    CONSTRAINT media_work_artwork_changes_alt_text_check CHECK (((alt_text IS NULL) OR (((octet_length(alt_text) >= 1) AND (octet_length(alt_text) <= 1024)) AND (length(btrim(alt_text)) > 0) AND (alt_text !~ '[[:cntrl:]]'::text)))),
    CONSTRAINT media_work_artwork_changes_check CHECK ((((image_asset_id IS NULL) AND (image_asset_digest IS NULL) AND (alt_text IS NULL)) OR ((image_asset_id IS NOT NULL) AND (image_asset_digest IS NOT NULL) AND (alt_text IS NOT NULL)))),
    CONSTRAINT media_work_artwork_changes_check1 CHECK ((recorded_by = owner_id)),
    CONSTRAINT media_work_artwork_changes_digest_check CHECK ((digest ~ '^wa-r1:sha256:[a-f0-9]{64}$'::text)),
    CONSTRAINT media_work_artwork_changes_document_bytes_check CHECK (((octet_length(document_bytes) >= 1) AND (octet_length(document_bytes) <= 32768))),
    CONSTRAINT media_work_artwork_changes_image_asset_digest_check CHECK (((image_asset_digest IS NULL) OR (image_asset_digest ~ '^ia-r1:sha256:[a-f0-9]{64}$'::text))),
    CONSTRAINT media_work_artwork_changes_image_asset_id_check CHECK (((image_asset_id IS NULL) OR (image_asset_id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'::text))),
    CONSTRAINT media_work_artwork_changes_recorded_at_ms_check CHECK ((recorded_at_ms >= 0)),
    CONSTRAINT media_work_artwork_changes_work_metadata_revision_check CHECK ((work_metadata_revision > 0))
);

CREATE TABLE public.production_approvals (
    id text NOT NULL,
    snapshot_id text NOT NULL,
    script_id text NOT NULL,
    operation_id text NOT NULL,
    approved_by text NOT NULL,
    approved_at timestamp with time zone DEFAULT clock_timestamp() NOT NULL,
    input_digest text NOT NULL,
    request_bytes bytea NOT NULL,
    CONSTRAINT production_approvals_input_digest_check CHECK ((input_digest ~ '^production-p1:sha256:[a-f0-9]{64}$'::text)),
    CONSTRAINT production_approvals_request_bytes_check CHECK (((octet_length(request_bytes) >= 1) AND (octet_length(request_bytes) <= 65536)))
);

CREATE TABLE public.production_brief_heads (
    unit_id text NOT NULL,
    brief_id text NOT NULL,
    version bigint NOT NULL,
    CONSTRAINT production_brief_heads_version_check CHECK ((version > 0))
);

CREATE TABLE public.production_briefs (
    id text NOT NULL,
    unit_id text NOT NULL,
    content_revision_id text NOT NULL,
    content_sequence bigint NOT NULL,
    owner_id text NOT NULL,
    version bigint NOT NULL,
    operation_id text NOT NULL,
    request_digest text NOT NULL,
    digest text NOT NULL,
    snapshot bytea NOT NULL,
    CONSTRAINT production_briefs_content_sequence_check CHECK ((content_sequence > 0)),
    CONSTRAINT production_briefs_digest_check CHECK ((digest ~ '^pb-r1:sha256:[a-f0-9]{64}$'::text)),
    CONSTRAINT production_briefs_id_check CHECK ((id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'::text)),
    CONSTRAINT production_briefs_request_digest_check CHECK ((request_digest ~ '^pb-request1:sha256:[a-f0-9]{64}$'::text)),
    CONSTRAINT production_briefs_snapshot_check CHECK (((octet_length(snapshot) >= 1) AND (octet_length(snapshot) <= 262144))),
    CONSTRAINT production_briefs_version_check CHECK ((version > 0))
);

CREATE TABLE public.production_producer_grants (
    id text NOT NULL,
    unit_id text NOT NULL,
    brief_id text NOT NULL,
    owner_id text NOT NULL,
    brief_digest text NOT NULL,
    token_hash bytea NOT NULL,
    expires_at bigint NOT NULL,
    created_at bigint NOT NULL,
    operation_id text NOT NULL,
    request_digest text NOT NULL,
    CONSTRAINT production_producer_grants_check CHECK (((created_at >= 0) AND (expires_at > created_at) AND ((expires_at - created_at) <= 86400000))),
    CONSTRAINT production_producer_grants_expires_at_check CHECK ((expires_at >= 0)),
    CONSTRAINT production_producer_grants_id_check CHECK ((id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'::text)),
    CONSTRAINT production_producer_grants_request_digest_check CHECK ((request_digest ~ '^pb-grant1:sha256:[a-f0-9]{64}$'::text)),
    CONSTRAINT production_producer_grants_token_hash_check CHECK ((octet_length(token_hash) = 32))
);

CREATE TABLE public.production_producer_revocations (
    grant_id text NOT NULL,
    revoked_by text NOT NULL,
    revoked_at bigint NOT NULL,
    CONSTRAINT production_producer_revocations_revoked_at_check CHECK ((revoked_at >= 0))
);

CREATE TABLE public.production_rights_claims (
    script_id text NOT NULL,
    owner_id text NOT NULL,
    kind text DEFAULT 'rights'::text NOT NULL,
    record_id text NOT NULL,
    version bigint NOT NULL,
    operation_id text NOT NULL,
    recorded_by text NOT NULL,
    recorded_at timestamp with time zone DEFAULT clock_timestamp() NOT NULL,
    request_bytes bytea NOT NULL,
    claim_bytes bytea NOT NULL,
    CONSTRAINT production_rights_claims_claim_bytes_check CHECK (((octet_length(claim_bytes) >= 1) AND (octet_length(claim_bytes) <= 65536))),
    CONSTRAINT production_rights_claims_kind_check CHECK ((kind = 'rights'::text)),
    CONSTRAINT production_rights_claims_request_bytes_check CHECK (((octet_length(request_bytes) >= 1) AND (octet_length(request_bytes) <= 65536))),
    CONSTRAINT production_rights_claims_version_check CHECK ((version > 0))
);

CREATE TABLE public.production_settings (
    script_id text NOT NULL,
    version bigint NOT NULL,
    script_revision bigint NOT NULL,
    operation_id text NOT NULL,
    recorded_by text NOT NULL,
    recorded_at timestamp with time zone DEFAULT clock_timestamp() NOT NULL,
    request_bytes bytea NOT NULL,
    settings_bytes bytea NOT NULL,
    CONSTRAINT production_settings_request_bytes_check CHECK (((octet_length(request_bytes) >= 1) AND (octet_length(request_bytes) <= 262144))),
    CONSTRAINT production_settings_settings_bytes_check CHECK (((octet_length(settings_bytes) >= 1) AND (octet_length(settings_bytes) <= 262144))),
    CONSTRAINT production_settings_version_check CHECK ((version > 0))
);

CREATE TABLE public.production_snapshots (
    id text NOT NULL,
    script_id text NOT NULL,
    owner_id text NOT NULL,
    script_revision bigint NOT NULL,
    settings_version bigint NOT NULL,
    operation_id text NOT NULL,
    created_by text NOT NULL,
    recorded_at timestamp with time zone DEFAULT clock_timestamp() NOT NULL,
    input_digest text NOT NULL,
    request_bytes bytea NOT NULL,
    document_bytes bytea NOT NULL,
    CONSTRAINT production_snapshots_document_bytes_check CHECK (((octet_length(document_bytes) >= 1) AND (octet_length(document_bytes) <= 4194304))),
    CONSTRAINT production_snapshots_input_digest_check CHECK ((input_digest ~ '^production-p1:sha256:[a-f0-9]{64}$'::text)),
    CONSTRAINT production_snapshots_request_bytes_check CHECK (((octet_length(request_bytes) >= 1) AND (octet_length(request_bytes) <= 65536)))
);

CREATE TABLE public.revision_evidence (
    script_id text NOT NULL,
    revision bigint NOT NULL,
    owner_id text NOT NULL,
    kind text NOT NULL,
    evidence_id text NOT NULL
);

CREATE TABLE public.script_evidence (
    owner_id text NOT NULL,
    kind text NOT NULL,
    id text NOT NULL,
    description text NOT NULL,
    CONSTRAINT script_evidence_id_check CHECK ((id ~ '^[a-z0-9_-]{1,64}$'::text)),
    CONSTRAINT script_evidence_kind_check CHECK ((kind = ANY (ARRAY['source'::text, 'rights'::text, 'generation'::text, 'asset'::text])))
);

CREATE TABLE public.script_members (
    script_id text NOT NULL,
    actor_id text NOT NULL,
    role text NOT NULL,
    CONSTRAINT script_members_role_check CHECK ((role = ANY (ARRAY['reader'::text, 'editor'::text])))
);

CREATE TABLE public.script_review_operations (
    script_id text NOT NULL,
    actor_id text NOT NULL,
    operation_id text NOT NULL,
    revision bigint NOT NULL
);

CREATE TABLE public.script_reviews (
    id text NOT NULL,
    script_id text NOT NULL,
    revision bigint NOT NULL,
    operation_id text NOT NULL,
    reviewed_by text NOT NULL,
    reviewed_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP NOT NULL
);

CREATE TABLE public.script_revisions (
    script_id text NOT NULL,
    revision bigint NOT NULL,
    expected_revision bigint NOT NULL,
    operation_id text NOT NULL,
    accepted_by text NOT NULL,
    accepted_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    schema_version text NOT NULL,
    canonical_export bytea NOT NULL,
    content_digest text NOT NULL,
    export_digest text NOT NULL,
    CONSTRAINT script_revisions_canonical_export_check CHECK (((octet_length(canonical_export) >= 1) AND (octet_length(canonical_export) <= 2097152))),
    CONSTRAINT script_revisions_check CHECK ((expected_revision = (revision - 1))),
    CONSTRAINT script_revisions_content_digest_check CHECK ((content_digest ~ '^sir-c1:sha256:[a-f0-9]{64}$'::text)),
    CONSTRAINT script_revisions_export_digest_check CHECK ((export_digest ~ '^sir-e1:sha256:[a-f0-9]{64}$'::text)),
    CONSTRAINT script_revisions_revision_check CHECK ((revision > 0)),
    CONSTRAINT script_revisions_schema_version_check CHECK ((schema_version = '0.1.0'::text))
);

CREATE TABLE public.scripts (
    id text NOT NULL,
    owner_id text NOT NULL,
    head_revision bigint DEFAULT 0 NOT NULL,
    CONSTRAINT scripts_head_revision_check CHECK ((head_revision >= 0))
);

CREATE TABLE public.sessions (
    token_hash bytea NOT NULL,
    actor_id text NOT NULL,
    expires_at timestamp with time zone NOT NULL,
    revoked boolean DEFAULT false NOT NULL,
    auth_kind text DEFAULT 'development'::text NOT NULL,
    session_id uuid DEFAULT gen_random_uuid() NOT NULL,
    CONSTRAINT sessions_auth_kind_check CHECK ((auth_kind = ANY (ARRAY['development'::text, 'studio_oidc'::text]))),
    CONSTRAINT sessions_token_hash_check CHECK ((octet_length(token_hash) = 32))
);

CREATE TABLE public.source_records (
    owner_id text NOT NULL,
    kind text DEFAULT 'source'::text NOT NULL,
    id text NOT NULL,
    reference text NOT NULL,
    original_text text,
    sha256 text NOT NULL,
    recorded_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    original_bytes bytea,
    import_metadata jsonb,
    import_outcome jsonb,
    import_digest text,
    imported_by text,
    import_operation_id text,
    CONSTRAINT imported_source_complete CHECK ((((original_bytes IS NULL) AND (import_metadata IS NULL) AND (import_outcome IS NULL) AND (import_digest IS NULL) AND (imported_by IS NULL) AND (import_operation_id IS NULL)) OR ((original_bytes IS NOT NULL) AND (import_metadata IS NOT NULL) AND (import_outcome IS NOT NULL) AND (imported_by IS NOT NULL) AND (import_operation_id IS NOT NULL) AND (import_digest IS NOT NULL) AND (import_digest ~ '^[a-f0-9]{64}$'::text) AND (imported_by = owner_id) AND (octet_length(original_bytes) <= 1048576) AND (jsonb_typeof(import_metadata) = 'object'::text) AND (jsonb_typeof(import_outcome) = 'object'::text) AND (import_operation_id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'::text)))),
    CONSTRAINT source_original_exists CHECK (((original_text IS NOT NULL) OR (original_bytes IS NOT NULL))),
    CONSTRAINT source_records_kind_check CHECK ((kind = 'source'::text)),
    CONSTRAINT source_records_original_text_check CHECK (((octet_length(original_text) >= 1) AND (octet_length(original_text) <= 1048576))),
    CONSTRAINT source_records_reference_check CHECK (((octet_length(reference) >= 1) AND (octet_length(reference) <= 2048))),
    CONSTRAINT source_records_sha256_check CHECK ((sha256 ~ '^[a-f0-9]{64}$'::text))
);

CREATE TABLE public.studio_oidc_flows (
    state_hash bytea NOT NULL,
    browser_hash bytea NOT NULL,
    nonce text NOT NULL,
    verifier text NOT NULL,
    issuer text NOT NULL COLLATE pg_catalog."C",
    client_id text NOT NULL,
    redirect_uri text NOT NULL,
    previous_hash bytea,
    expires_at timestamp with time zone NOT NULL,
    consumed boolean DEFAULT false NOT NULL,
    cancelled boolean DEFAULT false NOT NULL,
    completed boolean DEFAULT false NOT NULL,
    CONSTRAINT studio_oidc_flows_browser_hash_check CHECK ((octet_length(browser_hash) = 32)),
    CONSTRAINT studio_oidc_flows_client_id_check CHECK ((client_id = 'cantos-studio-web-bff'::text)),
    CONSTRAINT studio_oidc_flows_nonce_check CHECK ((length(nonce) = 64)),
    CONSTRAINT studio_oidc_flows_previous_hash_check CHECK ((octet_length(previous_hash) = 32)),
    CONSTRAINT studio_oidc_flows_state_hash_check CHECK ((octet_length(state_hash) = 32)),
    CONSTRAINT studio_oidc_flows_verifier_check CHECK ((length(verifier) = 64))
);

CREATE TABLE public.studio_oidc_sessions (
    token_hash bytea NOT NULL,
    issuer text NOT NULL COLLATE pg_catalog."C",
    subject text NOT NULL COLLATE pg_catalog."C",
    proof_hash bytea NOT NULL,
    access_token text NOT NULL,
    revocation_pending boolean DEFAULT false NOT NULL,
    source_state_hash bytea,
    lineage_root_hash bytea,
    revoked_by_token_hash bytea,
    CONSTRAINT studio_oidc_sessions_access_token_check CHECK (((octet_length(access_token) >= 1) AND (octet_length(access_token) <= 16384))),
    CONSTRAINT studio_oidc_sessions_proof_hash_check CHECK ((octet_length(proof_hash) = 32))
);

CREATE TABLE public.theatre_firebase_enrollment_receipts (
    receipt_id bigint NOT NULL,
    namespace text NOT NULL COLLATE pg_catalog."C",
    enrollment_id uuid NOT NULL,
    previous_phase text,
    phase text NOT NULL,
    block_reason text,
    recorded_at timestamp with time zone NOT NULL
);

ALTER TABLE public.theatre_firebase_enrollment_receipts ALTER COLUMN receipt_id ADD GENERATED ALWAYS AS IDENTITY (
    SEQUENCE NAME public.theatre_firebase_enrollment_receipts_receipt_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1
);

CREATE TABLE public.theatre_firebase_enrollments (
    namespace text NOT NULL COLLATE pg_catalog."C",
    id uuid NOT NULL,
    project text NOT NULL COLLATE pg_catalog."C",
    uid text NOT NULL COLLATE pg_catalog."C",
    email text NOT NULL,
    email_key text NOT NULL COLLATE pg_catalog."C",
    accepted_at timestamp with time zone NOT NULL,
    expires_at timestamp with time zone NOT NULL,
    last_sent_at timestamp with time zone NOT NULL,
    phase text NOT NULL,
    person_id uuid,
    block_reason text,
    CONSTRAINT theatre_firebase_enrollments_block_reason_check CHECK ((block_reason = ANY (ARRAY['bootstrap_rejected'::text, 'bootstrap_uncertain'::text, 'send_rejected'::text, 'send_uncertain'::text, 'consume_uncertain'::text, 'create_rejected'::text, 'create_uncertain'::text, 'binding_mismatch'::text, 'setup_rejected'::text, 'setup_uncertain'::text, 'handoff_refused'::text]))),
    CONSTRAINT theatre_firebase_enrollments_check CHECK ((uid = ('cantos_'::text || replace((id)::text, '-'::text, ''::text)))),
    CONSTRAINT theatre_firebase_enrollments_check1 CHECK ((email_key = lower((email COLLATE "C")))),
    CONSTRAINT theatre_firebase_enrollments_check2 CHECK ((expires_at > accepted_at)),
    CONSTRAINT theatre_firebase_enrollments_check3 CHECK ((person_id = id)),
    CONSTRAINT theatre_firebase_enrollments_check4 CHECK (((last_sent_at >= accepted_at) AND (last_sent_at < expires_at))),
    CONSTRAINT theatre_firebase_enrollments_check5 CHECK ((((phase = ANY (ARRAY['created'::text, 'setup_started'::text, 'handed_off'::text])) AND (person_id IS NOT NULL)) OR ((phase <> ALL (ARRAY['created'::text, 'setup_started'::text, 'handed_off'::text, 'blocked'::text])) AND (person_id IS NULL)) OR (phase = 'blocked'::text))),
    CONSTRAINT theatre_firebase_enrollments_check6 CHECK (((phase = 'blocked'::text) = (block_reason IS NOT NULL))),
    CONSTRAINT theatre_firebase_enrollments_email_check CHECK (((octet_length(email) >= 3) AND (octet_length(email) <= 254))),
    CONSTRAINT theatre_firebase_enrollments_id_check CHECK ((id <> '00000000-0000-0000-0000-000000000000'::uuid)),
    CONSTRAINT theatre_firebase_enrollments_namespace_check CHECK (((length(namespace) >= 1) AND (length(namespace) <= 2048))),
    CONSTRAINT theatre_firebase_enrollments_phase_check CHECK ((phase = ANY (ARRAY['bootstrap_started'::text, 'ready'::text, 'send_started'::text, 'mail_started'::text, 'awaiting_action'::text, 'inspecting'::text, 'consuming'::text, 'creating'::text, 'created'::text, 'setup_started'::text, 'handed_off'::text, 'blocked'::text, 'cancelled'::text, 'expired'::text]))),
    CONSTRAINT theatre_firebase_enrollments_project_check CHECK ((((length(project) >= 6) AND (length(project) <= 30)) AND (project ~ '^[a-z][a-z0-9-]*[a-z0-9]$'::text)))
);

CREATE TABLE public.theatre_firebase_send_starts (
    receipt_id bigint NOT NULL,
    namespace text NOT NULL COLLATE pg_catalog."C",
    enrollment_id uuid NOT NULL,
    network_key bytea NOT NULL,
    accepted_at timestamp with time zone NOT NULL,
    CONSTRAINT theatre_firebase_send_starts_network_key_check CHECK ((octet_length(network_key) = 32))
);

ALTER TABLE public.theatre_firebase_send_starts ALTER COLUMN receipt_id ADD GENERATED ALWAYS AS IDENTITY (
    SEQUENCE NAME public.theatre_firebase_send_starts_receipt_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1
);

CREATE TABLE public.theatre_identity_flows (
    state_hash bytea NOT NULL,
    browser_hash bytea NOT NULL,
    nonce text NOT NULL,
    verifier text NOT NULL,
    issuer text NOT NULL COLLATE pg_catalog."C",
    client_id text NOT NULL,
    redirect_uri text NOT NULL,
    previous_hash bytea,
    expires_at timestamp with time zone NOT NULL,
    consumed boolean DEFAULT false NOT NULL,
    cancelled boolean DEFAULT false NOT NULL,
    completed boolean DEFAULT false NOT NULL,
    lineage_root_hash bytea,
    CONSTRAINT theatre_identity_flows_browser_hash_check CHECK ((octet_length(browser_hash) = 32)),
    CONSTRAINT theatre_identity_flows_client_id_check CHECK ((client_id = 'cantos-theatre-web-bff'::text)),
    CONSTRAINT theatre_identity_flows_nonce_check CHECK ((length(nonce) = 64)),
    CONSTRAINT theatre_identity_flows_previous_hash_check CHECK ((octet_length(previous_hash) = 32)),
    CONSTRAINT theatre_identity_flows_state_hash_check CHECK ((octet_length(state_hash) = 32)),
    CONSTRAINT theatre_identity_flows_verifier_check CHECK ((length(verifier) = 64))
);

CREATE TABLE public.theatre_identity_sessions (
    token_hash bytea NOT NULL,
    actor_id text NOT NULL,
    session_id uuid DEFAULT gen_random_uuid() NOT NULL,
    client_id text NOT NULL,
    expires_at timestamp with time zone NOT NULL,
    revoked boolean DEFAULT false NOT NULL,
    issuer text NOT NULL COLLATE pg_catalog."C",
    subject text NOT NULL COLLATE pg_catalog."C",
    proof_hash bytea NOT NULL,
    access_token text NOT NULL,
    revocation_pending boolean DEFAULT false NOT NULL,
    source_state_hash bytea,
    revoked_by_token_hash bytea,
    lineage_root_hash bytea,
    CONSTRAINT theatre_identity_sessions_access_token_check CHECK (((octet_length(access_token) >= 1) AND (octet_length(access_token) <= 16384))),
    CONSTRAINT theatre_identity_sessions_client_id_check CHECK ((client_id = 'cantos-theatre-web-bff'::text)),
    CONSTRAINT theatre_identity_sessions_proof_hash_check CHECK ((octet_length(proof_hash) = 32)),
    CONSTRAINT theatre_identity_sessions_token_hash_check CHECK ((octet_length(token_hash) = 32))
);

CREATE TABLE public.theatre_native_flows (
    flow_id uuid NOT NULL,
    state_hash bytea NOT NULL,
    nonce text NOT NULL,
    challenge text NOT NULL,
    issuer text NOT NULL COLLATE pg_catalog."C",
    expires_at timestamp with time zone NOT NULL,
    consumed boolean DEFAULT false NOT NULL,
    completed boolean DEFAULT false NOT NULL,
    CONSTRAINT theatre_native_flows_challenge_check CHECK ((length(challenge) = 43)),
    CONSTRAINT theatre_native_flows_check CHECK (((NOT completed) OR consumed)),
    CONSTRAINT theatre_native_flows_nonce_check CHECK (((length(nonce) >= 43) AND (length(nonce) <= 128))),
    CONSTRAINT theatre_native_flows_state_hash_check CHECK ((octet_length(state_hash) = 32))
);

CREATE TABLE public.theatre_native_sessions (
    token_hash bytea NOT NULL,
    actor_id text NOT NULL,
    session_id uuid NOT NULL,
    client_id text DEFAULT 'cantos-theatre-native'::text NOT NULL,
    issuer text NOT NULL COLLATE pg_catalog."C",
    subject text NOT NULL COLLATE pg_catalog."C",
    expires_at timestamp with time zone NOT NULL,
    proof_hash bytea NOT NULL,
    access_token text NOT NULL,
    revoked boolean DEFAULT false NOT NULL,
    revocation_pending boolean DEFAULT false NOT NULL,
    CONSTRAINT theatre_native_sessions_access_token_check CHECK (((octet_length(access_token) >= 1) AND (octet_length(access_token) <= 16384))),
    CONSTRAINT theatre_native_sessions_client_id_check CHECK ((client_id = 'cantos-theatre-native'::text)),
    CONSTRAINT theatre_native_sessions_proof_hash_check CHECK ((octet_length(proof_hash) = 32)),
    CONSTRAINT theatre_native_sessions_token_hash_check CHECK ((octet_length(token_hash) = 32))
);

CREATE TABLE public.theatre_registration_mail_starts (
    namespace text NOT NULL COLLATE pg_catalog."C",
    token_hash bytea NOT NULL,
    registration_id uuid NOT NULL,
    network_key bytea NOT NULL,
    accepted_at timestamp with time zone NOT NULL,
    CONSTRAINT theatre_registration_mail_starts_network_key_check CHECK ((octet_length(network_key) = 32)),
    CONSTRAINT theatre_registration_mail_starts_token_hash_check CHECK ((octet_length(token_hash) = 32))
);

CREATE TABLE public.theatre_registrations (
    namespace text NOT NULL COLLATE pg_catalog."C",
    id uuid NOT NULL,
    email text NOT NULL,
    email_key text NOT NULL COLLATE pg_catalog."C",
    token_hash bytea NOT NULL,
    expires_at timestamp with time zone NOT NULL,
    last_sent_at timestamp with time zone NOT NULL,
    phase text NOT NULL,
    person_id uuid,
    block_reason text,
    CONSTRAINT theatre_registrations_block_reason_check CHECK ((block_reason = ANY (ARRAY['create_rejected'::text, 'create_uncertain'::text, 'setup_rejected'::text, 'setup_uncertain'::text, 'setup_delivery_rejected'::text, 'setup_delivery_uncertain'::text, 'binding_mismatch'::text]))),
    CONSTRAINT theatre_registrations_check CHECK ((email_key = lower((email COLLATE "C")))),
    CONSTRAINT theatre_registrations_check1 CHECK ((expires_at > last_sent_at)),
    CONSTRAINT theatre_registrations_check2 CHECK (((person_id IS NULL) OR (person_id = id))),
    CONSTRAINT theatre_registrations_check3 CHECK ((((phase = ANY (ARRAY['awaiting_verification'::text, 'cancelled'::text, 'creating'::text])) AND (person_id IS NULL)) OR ((phase = ANY (ARRAY['created'::text, 'setup_started'::text, 'delivering'::text, 'complete'::text])) AND (person_id IS NOT NULL)) OR (phase = 'blocked'::text))),
    CONSTRAINT theatre_registrations_check4 CHECK (((phase = 'blocked'::text) = (block_reason IS NOT NULL))),
    CONSTRAINT theatre_registrations_email_check CHECK (((octet_length(email) >= 3) AND (octet_length(email) <= 254))),
    CONSTRAINT theatre_registrations_id_check CHECK ((id <> '00000000-0000-0000-0000-000000000000'::uuid)),
    CONSTRAINT theatre_registrations_namespace_check CHECK (((length(namespace) >= 1) AND (length(namespace) <= 2048))),
    CONSTRAINT theatre_registrations_phase_check CHECK ((phase = ANY (ARRAY['awaiting_verification'::text, 'cancelled'::text, 'creating'::text, 'created'::text, 'setup_started'::text, 'delivering'::text, 'complete'::text, 'blocked'::text]))),
    CONSTRAINT theatre_registrations_token_hash_check CHECK ((octet_length(token_hash) = 32))
);

ALTER TABLE ONLY public.actor_external_identities
    ADD CONSTRAINT actor_external_identities_pkey PRIMARY KEY (issuer, subject);

ALTER TABLE ONLY public.actors
    ADD CONSTRAINT actors_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.adaptation_acceptances
    ADD CONSTRAINT adaptation_acceptances_owner_id_operation_id_key UNIQUE (owner_id, operation_id);

ALTER TABLE ONLY public.adaptation_acceptances
    ADD CONSTRAINT adaptation_acceptances_pkey PRIMARY KEY (run_id);

ALTER TABLE ONLY public.adaptation_cancellations
    ADD CONSTRAINT adaptation_cancellations_pkey PRIMARY KEY (owner_id, operation_id);

ALTER TABLE ONLY public.adaptation_proposals
    ADD CONSTRAINT adaptation_proposals_pkey PRIMARY KEY (run_id);

ALTER TABLE ONLY public.adaptation_runs
    ADD CONSTRAINT adaptation_runs_id_owner_id_key UNIQUE (id, owner_id);

ALTER TABLE ONLY public.adaptation_runs
    ADD CONSTRAINT adaptation_runs_owner_id_operation_id_key UNIQUE (owner_id, operation_id);

ALTER TABLE ONLY public.adaptation_runs
    ADD CONSTRAINT adaptation_runs_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.adaptation_submissions
    ADD CONSTRAINT adaptation_submissions_owner_id_operation_id_key UNIQUE (owner_id, operation_id);

ALTER TABLE ONLY public.adaptation_submissions
    ADD CONSTRAINT adaptation_submissions_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.adaptation_submissions
    ADD CONSTRAINT adaptation_submissions_run_id_submission_number_key UNIQUE (run_id, submission_number);

ALTER TABLE ONLY public.admin_access_revocations
    ADD CONSTRAINT admin_access_revocations_pkey PRIMARY KEY (actor_id);

ALTER TABLE ONLY public.admin_audit
    ADD CONSTRAINT admin_audit_correlation_id_key UNIQUE (correlation_id);

ALTER TABLE ONLY public.admin_audit
    ADD CONSTRAINT admin_audit_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.admin_operations
    ADD CONSTRAINT admin_operations_pkey PRIMARY KEY (actor_id, operation_id);

ALTER TABLE ONLY public.audio_distribution_clearances
    ADD CONSTRAINT audio_distribution_clearances_candidate_id_sequence_key UNIQUE (candidate_id, sequence);

ALTER TABLE ONLY public.audio_distribution_clearances
    ADD CONSTRAINT audio_distribution_clearances_owner_id_operation_id_key UNIQUE (owner_id, operation_id);

ALTER TABLE ONLY public.audio_distribution_clearances
    ADD CONSTRAINT audio_distribution_clearances_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.audio_intake_receipts
    ADD CONSTRAINT audio_intake_receipts_intent_id_key UNIQUE (intent_id);

ALTER TABLE ONLY public.audio_intake_receipts
    ADD CONSTRAINT audio_intake_receipts_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.audio_listener_releases
    ADD CONSTRAINT audio_listener_releases_pkey PRIMARY KEY (release_id);

ALTER TABLE ONLY public.audio_listener_units
    ADD CONSTRAINT audio_listener_units_pkey PRIMARY KEY (unit_id);

ALTER TABLE ONLY public.audio_listener_units
    ADD CONSTRAINT audio_listener_units_unit_id_work_id_key UNIQUE (unit_id, work_id);

ALTER TABLE ONLY public.audio_listener_works
    ADD CONSTRAINT audio_listener_works_pkey PRIMARY KEY (work_id);

ALTER TABLE ONLY public.audio_publication_approvals
    ADD CONSTRAINT audio_publication_approvals_candidate_id_sequence_key UNIQUE (candidate_id, sequence);

ALTER TABLE ONLY public.audio_publication_approvals
    ADD CONSTRAINT audio_publication_approvals_owner_id_operation_id_key UNIQUE (owner_id, operation_id);

ALTER TABLE ONLY public.audio_publication_approvals
    ADD CONSTRAINT audio_publication_approvals_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.audio_publication_events
    ADD CONSTRAINT audio_publication_events_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.audio_publication_events
    ADD CONSTRAINT audio_publication_events_unit_id_pointer_version_kind_relea_key UNIQUE (unit_id, pointer_version, kind, release_id);

ALTER TABLE ONLY public.audio_publication_operations
    ADD CONSTRAINT audio_publication_operations_pkey PRIMARY KEY (owner_id, operation_id, kind);

ALTER TABLE ONLY public.audio_publication_pointers
    ADD CONSTRAINT audio_publication_pointers_pkey PRIMARY KEY (unit_id);

ALTER TABLE ONLY public.audio_release_candidates
    ADD CONSTRAINT audio_release_candidate_owner_identity UNIQUE (id, owner_id);

ALTER TABLE ONLY public.audio_release_candidates
    ADD CONSTRAINT audio_release_candidates_owner_id_operation_id_key UNIQUE (owner_id, operation_id);

ALTER TABLE ONLY public.audio_release_candidates
    ADD CONSTRAINT audio_release_candidates_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.audio_release_withdrawals
    ADD CONSTRAINT audio_release_withdrawals_pkey PRIMARY KEY (release_id);

ALTER TABLE ONLY public.audio_releases
    ADD CONSTRAINT audio_releases_id_unit_id_owner_id_key UNIQUE (id, unit_id, owner_id);

ALTER TABLE ONLY public.audio_releases
    ADD CONSTRAINT audio_releases_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.audio_renditions
    ADD CONSTRAINT audio_renditions_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.audio_renditions
    ADD CONSTRAINT audio_renditions_submission_id_key UNIQUE (submission_id);

ALTER TABLE ONLY public.audio_reviews
    ADD CONSTRAINT audio_reviews_owner_id_operation_id_key UNIQUE (owner_id, operation_id);

ALTER TABLE ONLY public.audio_reviews
    ADD CONSTRAINT audio_reviews_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.audio_reviews
    ADD CONSTRAINT audio_reviews_rendition_id_sequence_key UNIQUE (rendition_id, sequence);

ALTER TABLE ONLY public.audio_submission_revocations
    ADD CONSTRAINT audio_submission_revocations_pkey PRIMARY KEY (submission_id);

ALTER TABLE ONLY public.audio_submissions
    ADD CONSTRAINT audio_submissions_id_owner_id_key UNIQUE (id, owner_id);

ALTER TABLE ONLY public.audio_submissions
    ADD CONSTRAINT audio_submissions_owner_id_operation_id_key UNIQUE (owner_id, operation_id);

ALTER TABLE ONLY public.audio_submissions
    ADD CONSTRAINT audio_submissions_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.audio_submissions
    ADD CONSTRAINT audio_submissions_token_hash_key UNIQUE (token_hash);

ALTER TABLE ONLY public.audio_upload_intents
    ADD CONSTRAINT audio_upload_intents_created_by_operation_id_key UNIQUE (created_by, operation_id);

ALTER TABLE ONLY public.audio_upload_intents
    ADD CONSTRAINT audio_upload_intents_id_script_id_script_revision_content_d_key UNIQUE (id, script_id, script_revision, content_digest, profile_id, operation_id, created_by);

ALTER TABLE ONLY public.audio_upload_intents
    ADD CONSTRAINT audio_upload_intents_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.catalog_clock
    ADD CONSTRAINT catalog_clock_pkey PRIMARY KEY (singleton);

ALTER TABLE ONLY public.catalog_content_heads
    ADD CONSTRAINT catalog_content_heads_pkey PRIMARY KEY (unit_id);

ALTER TABLE ONLY public.catalog_content_revisions
    ADD CONSTRAINT catalog_content_revisions_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.catalog_content_revisions
    ADD CONSTRAINT catalog_content_revisions_unit_id_id_revision_key UNIQUE (unit_id, id, revision);

ALTER TABLE ONLY public.catalog_content_revisions
    ADD CONSTRAINT catalog_content_revisions_unit_id_revision_key UNIQUE (unit_id, revision);

ALTER TABLE ONLY public.catalog_delegated_operations
    ADD CONSTRAINT catalog_delegated_operations_pkey PRIMARY KEY (actor_id, operation_id);

ALTER TABLE ONLY public.catalog_delegation_grants
    ADD CONSTRAINT catalog_delegation_grants_grantor_id_operation_id_key UNIQUE (grantor_id, operation_id);

ALTER TABLE ONLY public.catalog_delegation_grants
    ADD CONSTRAINT catalog_delegation_grants_id_delegate_id_key UNIQUE (id, delegate_id);

ALTER TABLE ONLY public.catalog_delegation_grants
    ADD CONSTRAINT catalog_delegation_grants_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.catalog_delegation_grants
    ADD CONSTRAINT catalog_delegation_grants_token_hash_key UNIQUE (token_hash);

ALTER TABLE ONLY public.catalog_delegation_revocations
    ADD CONSTRAINT catalog_delegation_revocations_pkey PRIMARY KEY (grant_id);

ALTER TABLE ONLY public.catalog_members
    ADD CONSTRAINT catalog_members_pkey PRIMARY KEY (work_id, actor_id);

ALTER TABLE ONLY public.catalog_operations
    ADD CONSTRAINT catalog_operations_pkey PRIMARY KEY (actor_id, operation_id);

ALTER TABLE ONLY public.catalog_unit_revisions
    ADD CONSTRAINT catalog_unit_revisions_pkey PRIMARY KEY (unit_id, revision);

ALTER TABLE ONLY public.catalog_units
    ADD CONSTRAINT catalog_units_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.catalog_units
    ADD CONSTRAINT catalog_units_work_id_position_key UNIQUE (work_id, "position") DEFERRABLE INITIALLY DEFERRED;

ALTER TABLE ONLY public.catalog_works
    ADD CONSTRAINT catalog_work_owner_pair UNIQUE (id, owner_id);

ALTER TABLE ONLY public.catalog_work_revisions
    ADD CONSTRAINT catalog_work_revisions_pkey PRIMARY KEY (work_id, revision);

ALTER TABLE ONLY public.catalog_works
    ADD CONSTRAINT catalog_works_id_kind_key UNIQUE (id, kind);

ALTER TABLE ONLY public.catalog_works
    ADD CONSTRAINT catalog_works_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.listener_device_sequences
    ADD CONSTRAINT listener_device_sequences_pkey PRIMARY KEY (listener_id, device_id, resource_key);

ALTER TABLE ONLY public.listener_history
    ADD CONSTRAINT listener_history_pkey PRIMARY KEY (listener_id, work_id);

ALTER TABLE ONLY public.listener_operations
    ADD CONSTRAINT listener_operations_pkey PRIMARY KEY (listener_id, operation_id);

ALTER TABLE ONLY public.listener_playback_sessions
    ADD CONSTRAINT listener_playback_sessions_pkey PRIMARY KEY (listener_id, device_id, engine_session_id);

ALTER TABLE ONLY public.listener_progress
    ADD CONSTRAINT listener_progress_pkey PRIMARY KEY (listener_id, publication_id);

ALTER TABLE ONLY public.listener_saved
    ADD CONSTRAINT listener_saved_pkey PRIMARY KEY (listener_id, work_id);

ALTER TABLE ONLY public.listener_state
    ADD CONSTRAINT listener_state_pkey PRIMARY KEY (listener_id);

ALTER TABLE ONLY public.media_artwork_operations
    ADD CONSTRAINT media_artwork_operations_pkey PRIMARY KEY (owner_id, operation_id);

ALTER TABLE ONLY public.media_image_assets
    ADD CONSTRAINT media_image_assets_id_work_id_owner_id_digest_key UNIQUE (id, work_id, owner_id, digest);

ALTER TABLE ONLY public.media_image_assets
    ADD CONSTRAINT media_image_assets_intent_id_key UNIQUE (intent_id);

ALTER TABLE ONLY public.media_image_assets
    ADD CONSTRAINT media_image_assets_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.media_image_intents
    ADD CONSTRAINT media_image_intents_id_work_id_owner_id_key UNIQUE (id, work_id, owner_id);

ALTER TABLE ONLY public.media_image_intents
    ADD CONSTRAINT media_image_intents_owner_id_operation_id_key UNIQUE (owner_id, operation_id);

ALTER TABLE ONLY public.media_image_intents
    ADD CONSTRAINT media_image_intents_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.media_work_artwork_changes
    ADD CONSTRAINT media_work_artwork_changes_pkey PRIMARY KEY (work_id, work_metadata_revision);

ALTER TABLE ONLY public.production_approvals
    ADD CONSTRAINT production_approvals_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.production_approvals
    ADD CONSTRAINT production_approvals_script_id_approved_by_operation_id_key UNIQUE (script_id, approved_by, operation_id);

ALTER TABLE ONLY public.production_approvals
    ADD CONSTRAINT production_approvals_snapshot_id_key UNIQUE (snapshot_id);

ALTER TABLE ONLY public.production_brief_heads
    ADD CONSTRAINT production_brief_heads_pkey PRIMARY KEY (unit_id);

ALTER TABLE ONLY public.production_briefs
    ADD CONSTRAINT production_briefs_id_unit_id_owner_id_digest_key UNIQUE (id, unit_id, owner_id, digest);

ALTER TABLE ONLY public.production_briefs
    ADD CONSTRAINT production_briefs_owner_id_operation_id_key UNIQUE (owner_id, operation_id);

ALTER TABLE ONLY public.production_briefs
    ADD CONSTRAINT production_briefs_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.production_briefs
    ADD CONSTRAINT production_briefs_unit_id_id_version_key UNIQUE (unit_id, id, version);

ALTER TABLE ONLY public.production_briefs
    ADD CONSTRAINT production_briefs_unit_id_version_key UNIQUE (unit_id, version);

ALTER TABLE ONLY public.production_producer_grants
    ADD CONSTRAINT production_producer_grants_owner_id_operation_id_key UNIQUE (owner_id, operation_id);

ALTER TABLE ONLY public.production_producer_grants
    ADD CONSTRAINT production_producer_grants_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.production_producer_grants
    ADD CONSTRAINT production_producer_grants_token_hash_key UNIQUE (token_hash);

ALTER TABLE ONLY public.production_producer_revocations
    ADD CONSTRAINT production_producer_revocations_pkey PRIMARY KEY (grant_id);

ALTER TABLE ONLY public.production_rights_claims
    ADD CONSTRAINT production_rights_claims_pkey PRIMARY KEY (script_id, record_id, version);

ALTER TABLE ONLY public.production_rights_claims
    ADD CONSTRAINT production_rights_claims_script_id_recorded_by_operation_id_key UNIQUE (script_id, recorded_by, operation_id);

ALTER TABLE ONLY public.production_settings
    ADD CONSTRAINT production_settings_pkey PRIMARY KEY (script_id, version);

ALTER TABLE ONLY public.production_settings
    ADD CONSTRAINT production_settings_script_id_recorded_by_operation_id_key UNIQUE (script_id, recorded_by, operation_id);

ALTER TABLE ONLY public.production_snapshots
    ADD CONSTRAINT production_snapshots_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.production_snapshots
    ADD CONSTRAINT production_snapshots_script_id_created_by_operation_id_key UNIQUE (script_id, created_by, operation_id);

ALTER TABLE ONLY public.revision_evidence
    ADD CONSTRAINT revision_evidence_pkey PRIMARY KEY (script_id, revision, kind, evidence_id);

ALTER TABLE ONLY public.script_evidence
    ADD CONSTRAINT script_evidence_pkey PRIMARY KEY (owner_id, kind, id);

ALTER TABLE ONLY public.script_members
    ADD CONSTRAINT script_members_pkey PRIMARY KEY (script_id, actor_id);

ALTER TABLE ONLY public.script_review_operations
    ADD CONSTRAINT script_review_operations_pkey PRIMARY KEY (script_id, actor_id, operation_id);

ALTER TABLE ONLY public.script_reviews
    ADD CONSTRAINT script_reviews_id_key UNIQUE (id);

ALTER TABLE ONLY public.script_reviews
    ADD CONSTRAINT script_reviews_pkey PRIMARY KEY (script_id, revision);

ALTER TABLE ONLY public.script_revisions
    ADD CONSTRAINT script_revisions_pkey PRIMARY KEY (script_id, revision);

ALTER TABLE ONLY public.script_revisions
    ADD CONSTRAINT script_revisions_script_id_accepted_by_operation_id_key UNIQUE (script_id, accepted_by, operation_id);

ALTER TABLE ONLY public.scripts
    ADD CONSTRAINT scripts_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.sessions
    ADD CONSTRAINT sessions_pkey PRIMARY KEY (token_hash);

ALTER TABLE ONLY public.source_records
    ADD CONSTRAINT source_records_pkey PRIMARY KEY (owner_id, id);

ALTER TABLE ONLY public.studio_oidc_flows
    ADD CONSTRAINT studio_oidc_flows_pkey PRIMARY KEY (state_hash);

ALTER TABLE ONLY public.studio_oidc_sessions
    ADD CONSTRAINT studio_oidc_sessions_pkey PRIMARY KEY (token_hash);

ALTER TABLE ONLY public.studio_oidc_sessions
    ADD CONSTRAINT studio_oidc_sessions_proof_hash_key UNIQUE (proof_hash);

ALTER TABLE ONLY public.theatre_firebase_enrollment_receipts
    ADD CONSTRAINT theatre_firebase_enrollment_receipts_pkey PRIMARY KEY (receipt_id);

ALTER TABLE ONLY public.theatre_firebase_enrollments
    ADD CONSTRAINT theatre_firebase_enrollments_pkey PRIMARY KEY (namespace, id);

ALTER TABLE ONLY public.theatre_firebase_enrollments
    ADD CONSTRAINT theatre_firebase_enrollments_project_id_key UNIQUE (project, id);

ALTER TABLE ONLY public.theatre_firebase_enrollments
    ADD CONSTRAINT theatre_firebase_enrollments_project_uid_key UNIQUE (project, uid);

ALTER TABLE ONLY public.theatre_firebase_send_starts
    ADD CONSTRAINT theatre_firebase_send_starts_pkey PRIMARY KEY (receipt_id);

ALTER TABLE ONLY public.theatre_identity_flows
    ADD CONSTRAINT theatre_identity_flows_pkey PRIMARY KEY (state_hash);

ALTER TABLE ONLY public.theatre_identity_sessions
    ADD CONSTRAINT theatre_identity_sessions_pkey PRIMARY KEY (token_hash);

ALTER TABLE ONLY public.theatre_identity_sessions
    ADD CONSTRAINT theatre_identity_sessions_proof_hash_key UNIQUE (proof_hash);

ALTER TABLE ONLY public.theatre_identity_sessions
    ADD CONSTRAINT theatre_identity_sessions_session_id_key UNIQUE (session_id);

ALTER TABLE ONLY public.theatre_native_flows
    ADD CONSTRAINT theatre_native_flows_pkey PRIMARY KEY (flow_id);

ALTER TABLE ONLY public.theatre_native_flows
    ADD CONSTRAINT theatre_native_flows_state_hash_key UNIQUE (state_hash);

ALTER TABLE ONLY public.theatre_native_sessions
    ADD CONSTRAINT theatre_native_sessions_pkey PRIMARY KEY (token_hash);

ALTER TABLE ONLY public.theatre_native_sessions
    ADD CONSTRAINT theatre_native_sessions_proof_hash_key UNIQUE (proof_hash);

ALTER TABLE ONLY public.theatre_native_sessions
    ADD CONSTRAINT theatre_native_sessions_session_id_key UNIQUE (session_id);

ALTER TABLE ONLY public.theatre_registration_mail_starts
    ADD CONSTRAINT theatre_registration_mail_starts_pkey PRIMARY KEY (namespace, token_hash);

ALTER TABLE ONLY public.theatre_registrations
    ADD CONSTRAINT theatre_registrations_namespace_email_key_key UNIQUE (namespace, email_key);

ALTER TABLE ONLY public.theatre_registrations
    ADD CONSTRAINT theatre_registrations_namespace_token_hash_key UNIQUE (namespace, token_hash);

ALTER TABLE ONLY public.theatre_registrations
    ADD CONSTRAINT theatre_registrations_pkey PRIMARY KEY (namespace, id);

CREATE INDEX actor_external_identities_actor ON public.actor_external_identities USING btree (actor_id);

CREATE INDEX audio_listener_releases_unit ON public.audio_listener_releases USING btree (unit_id, release_id);

CREATE INDEX audio_listener_releases_work ON public.audio_listener_releases USING btree (work_id, published_at DESC, release_id COLLATE "C");

CREATE INDEX audio_listener_units_order ON public.audio_listener_units USING btree (work_id, first_position, unit_id COLLATE "C");

CREATE INDEX audio_listener_works_recent ON public.audio_listener_works USING btree (first_published_at DESC, work_id COLLATE "C");

CREATE INDEX catalog_delegation_grants_work ON public.catalog_delegation_grants USING btree (work_id, id);

CREATE INDEX catalog_members_actor ON public.catalog_members USING btree (actor_id, work_id);

CREATE INDEX catalog_units_order ON public.catalog_units USING btree (work_id, "position", id);

CREATE INDEX catalog_works_owner ON public.catalog_works USING btree (owner_id, id);

CREATE INDEX listener_history_recent ON public.listener_history USING btree (listener_id, listener_order DESC, work_id COLLATE "C");

CREATE INDEX listener_progress_recent ON public.listener_progress USING btree (listener_id, listener_order DESC, work_id COLLATE "C");

CREATE INDEX listener_progress_work_recent ON public.listener_progress USING btree (listener_id, work_id COLLATE "C", listener_order DESC, publication_id COLLATE "C");

CREATE INDEX listener_saved_recent ON public.listener_saved USING btree (listener_id, listener_order DESC, work_id COLLATE "C") WHERE saved;

CREATE INDEX production_snapshot_history ON public.production_snapshots USING btree (script_id, recorded_at DESC, id DESC);

CREATE UNIQUE INDEX sessions_session_id ON public.sessions USING btree (session_id);

CREATE UNIQUE INDEX source_import_operations ON public.source_records USING btree (imported_by, import_operation_id) WHERE (import_operation_id IS NOT NULL);

CREATE INDEX studio_oidc_flows_browser ON public.studio_oidc_flows USING btree (browser_hash);

CREATE INDEX studio_oidc_sessions_lineage ON public.studio_oidc_sessions USING btree (lineage_root_hash);

CREATE INDEX studio_oidc_sessions_revoked_by ON public.studio_oidc_sessions USING btree (revoked_by_token_hash) WHERE revocation_pending;

CREATE UNIQUE INDEX studio_oidc_sessions_source ON public.studio_oidc_sessions USING btree (source_state_hash);

CREATE UNIQUE INDEX theatre_firebase_active_email ON public.theatre_firebase_enrollments USING btree (namespace, project, email_key) WHERE (phase <> ALL (ARRAY['cancelled'::text, 'expired'::text]));

CREATE INDEX theatre_firebase_network_send_budget ON public.theatre_firebase_send_starts USING btree (namespace, network_key, accepted_at);

CREATE INDEX theatre_firebase_send_budget ON public.theatre_firebase_send_starts USING btree (namespace, accepted_at);

CREATE INDEX theatre_identity_flows_browser ON public.theatre_identity_flows USING btree (browser_hash);

CREATE INDEX theatre_identity_sessions_revocation_batch ON public.theatre_identity_sessions USING btree (revoked_by_token_hash) WHERE (revoked AND revocation_pending);

CREATE INDEX theatre_identity_sessions_source_flow ON public.theatre_identity_sessions USING btree (source_state_hash);

CREATE INDEX theatre_registration_mail_budget ON public.theatre_registration_mail_starts USING btree (namespace, accepted_at);

CREATE INDEX theatre_registration_network_mail_budget ON public.theatre_registration_mail_starts USING btree (namespace, network_key, accepted_at);

CREATE TRIGGER actor_external_identities_immutable BEFORE DELETE OR UPDATE ON public.actor_external_identities FOR EACH ROW EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER actor_external_identities_no_truncate BEFORE TRUNCATE ON public.actor_external_identities FOR EACH STATEMENT EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER adaptation_run_guard BEFORE DELETE OR UPDATE ON public.adaptation_runs FOR EACH ROW EXECUTE FUNCTION public.guard_adaptation_run();

CREATE TRIGGER adaptation_run_no_truncate BEFORE TRUNCATE ON public.adaptation_runs FOR EACH STATEMENT EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER adaptation_submission_immutable BEFORE DELETE OR UPDATE ON public.adaptation_submissions FOR EACH ROW EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER adaptation_submission_no_truncate BEFORE TRUNCATE ON public.adaptation_submissions FOR EACH STATEMENT EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER admin_audit_immutable BEFORE DELETE OR UPDATE OR TRUNCATE ON public.admin_audit FOR EACH STATEMENT EXECUTE FUNCTION public.admin_immutable();

CREATE TRIGGER admin_operations_immutable BEFORE DELETE OR UPDATE OR TRUNCATE ON public.admin_operations FOR EACH STATEMENT EXECUTE FUNCTION public.admin_immutable();

CREATE TRIGGER admin_revocations_immutable BEFORE DELETE OR UPDATE OR TRUNCATE ON public.admin_access_revocations FOR EACH STATEMENT EXECUTE FUNCTION public.admin_immutable();

CREATE TRIGGER audio_intent_binding BEFORE INSERT ON public.audio_upload_intents FOR EACH ROW EXECUTE FUNCTION public.check_audio_intent_binding();

CREATE TRIGGER audio_intents_immutable BEFORE DELETE OR UPDATE ON public.audio_upload_intents FOR EACH ROW EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER audio_intents_no_truncate BEFORE TRUNCATE ON public.audio_upload_intents FOR EACH STATEMENT EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER audio_receipts_immutable BEFORE DELETE OR UPDATE ON public.audio_intake_receipts FOR EACH ROW EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER audio_receipts_no_truncate BEFORE TRUNCATE ON public.audio_intake_receipts FOR EACH STATEMENT EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER catalog_content_head_identity BEFORE UPDATE ON public.catalog_content_heads FOR EACH ROW EXECUTE FUNCTION public.catalog_content_head_identity();

CREATE TRIGGER catalog_content_history_immutable BEFORE DELETE OR UPDATE ON public.catalog_content_revisions FOR EACH ROW EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER catalog_content_history_no_truncate BEFORE TRUNCATE ON public.catalog_content_revisions FOR EACH STATEMENT EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER catalog_delegated_operations_immutable BEFORE DELETE OR UPDATE ON public.catalog_delegated_operations FOR EACH ROW EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER catalog_delegated_operations_no_truncate BEFORE TRUNCATE ON public.catalog_delegated_operations FOR EACH STATEMENT EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER catalog_delegation_grants_immutable BEFORE DELETE OR UPDATE ON public.catalog_delegation_grants FOR EACH ROW EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER catalog_delegation_grants_no_truncate BEFORE TRUNCATE ON public.catalog_delegation_grants FOR EACH STATEMENT EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER catalog_delegation_revocations_immutable BEFORE DELETE OR UPDATE ON public.catalog_delegation_revocations FOR EACH ROW EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER catalog_delegation_revocations_no_truncate BEFORE TRUNCATE ON public.catalog_delegation_revocations FOR EACH STATEMENT EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER catalog_operations_immutable BEFORE DELETE OR UPDATE ON public.catalog_operations FOR EACH ROW EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER catalog_operations_no_truncate BEFORE TRUNCATE ON public.catalog_operations FOR EACH STATEMENT EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER catalog_unit_history_immutable BEFORE DELETE OR UPDATE ON public.catalog_unit_revisions FOR EACH ROW EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER catalog_unit_history_no_truncate BEFORE TRUNCATE ON public.catalog_unit_revisions FOR EACH STATEMENT EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER catalog_unit_identity BEFORE UPDATE ON public.catalog_units FOR EACH ROW EXECUTE FUNCTION public.catalog_identity_immutable();

CREATE TRIGGER catalog_work_history_immutable BEFORE DELETE OR UPDATE ON public.catalog_work_revisions FOR EACH ROW EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER catalog_work_history_no_truncate BEFORE TRUNCATE ON public.catalog_work_revisions FOR EACH STATEMENT EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER catalog_work_identity BEFORE UPDATE ON public.catalog_works FOR EACH ROW EXECUTE FUNCTION public.catalog_identity_immutable();

CREATE TRIGGER evidence_immutable BEFORE DELETE OR UPDATE ON public.revision_evidence FOR EACH ROW EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER evidence_no_truncate BEFORE TRUNCATE ON public.revision_evidence FOR EACH STATEMENT EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER forward_pointer BEFORE UPDATE ON public.audio_publication_pointers FOR EACH ROW EXECUTE FUNCTION public.guard_audio_publication_pointer();

CREATE TRIGGER immutable BEFORE DELETE OR UPDATE ON public.adaptation_acceptances FOR EACH ROW EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER immutable BEFORE DELETE OR UPDATE ON public.adaptation_cancellations FOR EACH ROW EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER immutable BEFORE DELETE OR UPDATE ON public.adaptation_proposals FOR EACH ROW EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER immutable BEFORE DELETE OR UPDATE ON public.audio_distribution_clearances FOR EACH ROW EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER immutable BEFORE DELETE OR UPDATE ON public.audio_listener_releases FOR EACH ROW EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER immutable BEFORE DELETE OR UPDATE ON public.audio_listener_units FOR EACH ROW EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER immutable BEFORE DELETE OR UPDATE ON public.audio_listener_works FOR EACH ROW EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER immutable BEFORE DELETE OR UPDATE ON public.audio_publication_approvals FOR EACH ROW EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER immutable BEFORE DELETE OR UPDATE ON public.audio_publication_events FOR EACH ROW EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER immutable BEFORE DELETE OR UPDATE ON public.audio_publication_operations FOR EACH ROW EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER immutable BEFORE DELETE OR UPDATE ON public.audio_release_candidates FOR EACH ROW EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER immutable BEFORE DELETE OR UPDATE ON public.audio_release_withdrawals FOR EACH ROW EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER immutable BEFORE DELETE OR UPDATE ON public.audio_releases FOR EACH ROW EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER immutable BEFORE DELETE OR UPDATE ON public.audio_renditions FOR EACH ROW EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER immutable BEFORE DELETE OR UPDATE ON public.audio_reviews FOR EACH ROW EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER immutable BEFORE DELETE OR UPDATE ON public.audio_submission_revocations FOR EACH ROW EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER immutable BEFORE DELETE OR UPDATE ON public.audio_submissions FOR EACH ROW EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER immutable BEFORE DELETE OR UPDATE ON public.media_artwork_operations FOR EACH ROW EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER immutable BEFORE DELETE OR UPDATE ON public.media_image_assets FOR EACH ROW EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER immutable BEFORE DELETE OR UPDATE ON public.media_image_intents FOR EACH ROW EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER immutable BEFORE DELETE OR UPDATE ON public.media_work_artwork_changes FOR EACH ROW EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER listener_operations_immutable BEFORE DELETE OR UPDATE ON public.listener_operations FOR EACH ROW EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER listener_operations_no_truncate BEFORE TRUNCATE ON public.listener_operations FOR EACH STATEMENT EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER listener_progress_guard BEFORE UPDATE ON public.listener_progress FOR EACH ROW EXECUTE FUNCTION public.guard_listener_progress();

CREATE TRIGGER listener_saved_guard BEFORE UPDATE ON public.listener_saved FOR EACH ROW EXECUTE FUNCTION public.guard_listener_saved();

CREATE TRIGGER listener_sessions_immutable BEFORE DELETE OR UPDATE ON public.listener_playback_sessions FOR EACH ROW EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER listener_sessions_no_truncate BEFORE TRUNCATE ON public.listener_playback_sessions FOR EACH STATEMENT EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER no_delete_pointer BEFORE DELETE ON public.audio_publication_pointers FOR EACH ROW EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER no_truncate BEFORE TRUNCATE ON public.adaptation_acceptances FOR EACH STATEMENT EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER no_truncate BEFORE TRUNCATE ON public.adaptation_cancellations FOR EACH STATEMENT EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER no_truncate BEFORE TRUNCATE ON public.adaptation_proposals FOR EACH STATEMENT EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER no_truncate BEFORE TRUNCATE ON public.audio_distribution_clearances FOR EACH STATEMENT EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER no_truncate BEFORE TRUNCATE ON public.audio_listener_releases FOR EACH STATEMENT EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER no_truncate BEFORE TRUNCATE ON public.audio_listener_units FOR EACH STATEMENT EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER no_truncate BEFORE TRUNCATE ON public.audio_listener_works FOR EACH STATEMENT EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER no_truncate BEFORE TRUNCATE ON public.audio_publication_approvals FOR EACH STATEMENT EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER no_truncate BEFORE TRUNCATE ON public.audio_publication_events FOR EACH STATEMENT EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER no_truncate BEFORE TRUNCATE ON public.audio_publication_operations FOR EACH STATEMENT EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER no_truncate BEFORE TRUNCATE ON public.audio_release_candidates FOR EACH STATEMENT EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER no_truncate BEFORE TRUNCATE ON public.audio_release_withdrawals FOR EACH STATEMENT EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER no_truncate BEFORE TRUNCATE ON public.audio_releases FOR EACH STATEMENT EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER no_truncate BEFORE TRUNCATE ON public.audio_renditions FOR EACH STATEMENT EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER no_truncate BEFORE TRUNCATE ON public.audio_reviews FOR EACH STATEMENT EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER no_truncate BEFORE TRUNCATE ON public.audio_submission_revocations FOR EACH STATEMENT EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER no_truncate BEFORE TRUNCATE ON public.audio_submissions FOR EACH STATEMENT EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER no_truncate BEFORE TRUNCATE ON public.media_artwork_operations FOR EACH STATEMENT EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER no_truncate BEFORE TRUNCATE ON public.media_image_assets FOR EACH STATEMENT EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER no_truncate BEFORE TRUNCATE ON public.media_image_intents FOR EACH STATEMENT EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER no_truncate BEFORE TRUNCATE ON public.media_work_artwork_changes FOR EACH STATEMENT EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER no_truncate_pointer BEFORE TRUNCATE ON public.audio_publication_pointers FOR EACH STATEMENT EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER production_approvals_immutable BEFORE DELETE OR UPDATE ON public.production_approvals FOR EACH ROW EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER production_approvals_no_truncate BEFORE TRUNCATE ON public.production_approvals FOR EACH STATEMENT EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER production_brief_head_identity BEFORE UPDATE ON public.production_brief_heads FOR EACH ROW EXECUTE FUNCTION public.production_brief_head_identity();

CREATE TRIGGER production_briefs_immutable BEFORE DELETE OR UPDATE ON public.production_briefs FOR EACH ROW EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER production_briefs_no_truncate BEFORE TRUNCATE ON public.production_briefs FOR EACH STATEMENT EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER production_grants_immutable BEFORE DELETE OR UPDATE ON public.production_producer_grants FOR EACH ROW EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER production_grants_no_truncate BEFORE TRUNCATE ON public.production_producer_grants FOR EACH STATEMENT EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER production_revocations_immutable BEFORE DELETE OR UPDATE ON public.production_producer_revocations FOR EACH ROW EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER production_revocations_no_truncate BEFORE TRUNCATE ON public.production_producer_revocations FOR EACH STATEMENT EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER production_rights_immutable BEFORE DELETE OR UPDATE ON public.production_rights_claims FOR EACH ROW EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER production_rights_no_truncate BEFORE TRUNCATE ON public.production_rights_claims FOR EACH STATEMENT EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER production_settings_immutable BEFORE DELETE OR UPDATE ON public.production_settings FOR EACH ROW EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER production_settings_no_truncate BEFORE TRUNCATE ON public.production_settings FOR EACH STATEMENT EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER production_snapshots_immutable BEFORE DELETE OR UPDATE ON public.production_snapshots FOR EACH ROW EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER production_snapshots_no_truncate BEFORE TRUNCATE ON public.production_snapshots FOR EACH STATEMENT EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER registry_immutable BEFORE DELETE OR UPDATE ON public.script_evidence FOR EACH ROW EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER review_operations_immutable BEFORE DELETE OR UPDATE ON public.script_review_operations FOR EACH ROW EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER review_operations_no_truncate BEFORE TRUNCATE ON public.script_review_operations FOR EACH STATEMENT EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER reviews_immutable BEFORE DELETE OR UPDATE ON public.script_reviews FOR EACH ROW EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER reviews_no_truncate BEFORE TRUNCATE ON public.script_reviews FOR EACH STATEMENT EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER revisions_immutable BEFORE DELETE OR UPDATE ON public.script_revisions FOR EACH ROW EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER revisions_no_truncate BEFORE TRUNCATE ON public.script_revisions FOR EACH STATEMENT EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER sources_immutable BEFORE DELETE OR UPDATE ON public.source_records FOR EACH ROW EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER sources_no_truncate BEFORE TRUNCATE ON public.source_records FOR EACH STATEMENT EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER theatre_firebase_enrollment_guard BEFORE INSERT OR DELETE OR UPDATE ON public.theatre_firebase_enrollments FOR EACH ROW EXECUTE FUNCTION public.theatre_firebase_enrollment_guard();

CREATE TRIGGER theatre_firebase_enrollment_receipt AFTER INSERT OR UPDATE ON public.theatre_firebase_enrollments FOR EACH ROW EXECUTE FUNCTION public.theatre_firebase_enrollment_receipt();

CREATE TRIGGER theatre_firebase_enrollments_no_truncate BEFORE TRUNCATE ON public.theatre_firebase_enrollments FOR EACH STATEMENT EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER theatre_firebase_receipts_immutable BEFORE DELETE OR UPDATE ON public.theatre_firebase_enrollment_receipts FOR EACH ROW EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER theatre_firebase_receipts_no_truncate BEFORE TRUNCATE ON public.theatre_firebase_enrollment_receipts FOR EACH STATEMENT EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER theatre_firebase_send_starts_immutable BEFORE DELETE OR UPDATE ON public.theatre_firebase_send_starts FOR EACH ROW EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER theatre_firebase_send_starts_no_truncate BEFORE TRUNCATE ON public.theatre_firebase_send_starts FOR EACH STATEMENT EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER theatre_registration_mail_starts_immutable BEFORE DELETE OR UPDATE ON public.theatre_registration_mail_starts FOR EACH ROW EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER theatre_registration_mail_starts_no_truncate BEFORE TRUNCATE ON public.theatre_registration_mail_starts FOR EACH STATEMENT EXECUTE FUNCTION public.reject_settled_change();

CREATE TRIGGER theatre_registrations_guard BEFORE DELETE OR UPDATE ON public.theatre_registrations FOR EACH ROW EXECUTE FUNCTION public.theatre_registration_guard();

CREATE TRIGGER theatre_registrations_no_truncate BEFORE TRUNCATE ON public.theatre_registrations FOR EACH STATEMENT EXECUTE FUNCTION public.reject_settled_change();

ALTER TABLE ONLY public.actor_external_identities
    ADD CONSTRAINT actor_external_identities_actor_id_fkey FOREIGN KEY (actor_id) REFERENCES public.actors(id);

ALTER TABLE ONLY public.adaptation_acceptances
    ADD CONSTRAINT adaptation_acceptances_owner_id_fkey FOREIGN KEY (owner_id) REFERENCES public.actors(id);

ALTER TABLE ONLY public.adaptation_acceptances
    ADD CONSTRAINT adaptation_acceptances_run_id_fkey FOREIGN KEY (run_id) REFERENCES public.adaptation_runs(id);

ALTER TABLE ONLY public.adaptation_acceptances
    ADD CONSTRAINT adaptation_acceptances_script_id_revision_fkey FOREIGN KEY (script_id, revision) REFERENCES public.script_revisions(script_id, revision);

ALTER TABLE ONLY public.adaptation_cancellations
    ADD CONSTRAINT adaptation_cancellations_owner_id_fkey FOREIGN KEY (owner_id) REFERENCES public.actors(id);

ALTER TABLE ONLY public.adaptation_cancellations
    ADD CONSTRAINT adaptation_cancellations_run_id_fkey FOREIGN KEY (run_id) REFERENCES public.adaptation_runs(id);

ALTER TABLE ONLY public.adaptation_proposals
    ADD CONSTRAINT adaptation_proposals_run_id_fkey FOREIGN KEY (run_id) REFERENCES public.adaptation_runs(id);

ALTER TABLE ONLY public.adaptation_runs
    ADD CONSTRAINT adaptation_runs_owner_id_fkey FOREIGN KEY (owner_id) REFERENCES public.actors(id);

ALTER TABLE ONLY public.adaptation_runs
    ADD CONSTRAINT adaptation_runs_owner_id_source_id_fkey FOREIGN KEY (owner_id, source_id) REFERENCES public.source_records(owner_id, id);

ALTER TABLE ONLY public.adaptation_submissions
    ADD CONSTRAINT adaptation_submissions_owner_id_fkey FOREIGN KEY (owner_id) REFERENCES public.actors(id);

ALTER TABLE ONLY public.adaptation_submissions
    ADD CONSTRAINT adaptation_submissions_run_id_fkey FOREIGN KEY (run_id) REFERENCES public.adaptation_runs(id);

ALTER TABLE ONLY public.adaptation_submissions
    ADD CONSTRAINT adaptation_submissions_run_id_owner_id_fkey FOREIGN KEY (run_id, owner_id) REFERENCES public.adaptation_runs(id, owner_id);

ALTER TABLE ONLY public.admin_access_revocations
    ADD CONSTRAINT admin_access_revocations_actor_id_fkey FOREIGN KEY (actor_id) REFERENCES public.actors(id);

ALTER TABLE ONLY public.admin_audit
    ADD CONSTRAINT admin_audit_actor_id_fkey FOREIGN KEY (actor_id) REFERENCES public.actors(id);

ALTER TABLE ONLY public.admin_operations
    ADD CONSTRAINT admin_operations_actor_id_fkey FOREIGN KEY (actor_id) REFERENCES public.actors(id);

ALTER TABLE ONLY public.admin_operations
    ADD CONSTRAINT admin_operations_audit_id_fkey FOREIGN KEY (audit_id) REFERENCES public.admin_audit(id);

ALTER TABLE ONLY public.admin_operations
    ADD CONSTRAINT admin_operations_target_actor_id_fkey FOREIGN KEY (target_actor_id) REFERENCES public.actors(id);

ALTER TABLE ONLY public.audio_distribution_clearances
    ADD CONSTRAINT audio_distribution_clearances_candidate_id_owner_id_fkey FOREIGN KEY (candidate_id, owner_id) REFERENCES public.audio_release_candidates(id, owner_id);

ALTER TABLE ONLY public.audio_distribution_clearances
    ADD CONSTRAINT audio_distribution_clearances_owner_id_fkey FOREIGN KEY (owner_id) REFERENCES public.actors(id);

ALTER TABLE ONLY public.audio_intake_receipts
    ADD CONSTRAINT audio_intake_receipts_intent_id_script_id_script_revision__fkey FOREIGN KEY (intent_id, script_id, script_revision, content_digest, profile_id, operation_id, accepted_by) REFERENCES public.audio_upload_intents(id, script_id, script_revision, content_digest, profile_id, operation_id, created_by);

ALTER TABLE ONLY public.audio_listener_releases
    ADD CONSTRAINT audio_listener_releases_content_revision_id_fkey FOREIGN KEY (content_revision_id) REFERENCES public.catalog_content_revisions(id);

ALTER TABLE ONLY public.audio_listener_releases
    ADD CONSTRAINT audio_listener_releases_release_id_fkey FOREIGN KEY (release_id) REFERENCES public.audio_releases(id);

ALTER TABLE ONLY public.audio_listener_releases
    ADD CONSTRAINT audio_listener_releases_unit_id_work_id_fkey FOREIGN KEY (unit_id, work_id) REFERENCES public.audio_listener_units(unit_id, work_id);

ALTER TABLE ONLY public.audio_listener_releases
    ADD CONSTRAINT audio_listener_releases_work_id_fkey FOREIGN KEY (work_id) REFERENCES public.catalog_works(id);

ALTER TABLE ONLY public.audio_listener_units
    ADD CONSTRAINT audio_listener_units_unit_id_fkey FOREIGN KEY (unit_id) REFERENCES public.catalog_units(id);

ALTER TABLE ONLY public.audio_listener_units
    ADD CONSTRAINT audio_listener_units_work_id_fkey FOREIGN KEY (work_id) REFERENCES public.catalog_works(id);

ALTER TABLE ONLY public.audio_listener_works
    ADD CONSTRAINT audio_listener_works_first_release_id_fkey FOREIGN KEY (first_release_id) REFERENCES public.audio_releases(id);

ALTER TABLE ONLY public.audio_listener_works
    ADD CONSTRAINT audio_listener_works_work_id_fkey FOREIGN KEY (work_id) REFERENCES public.catalog_works(id);

ALTER TABLE ONLY public.audio_publication_approvals
    ADD CONSTRAINT audio_publication_approvals_candidate_id_owner_id_fkey FOREIGN KEY (candidate_id, owner_id) REFERENCES public.audio_release_candidates(id, owner_id);

ALTER TABLE ONLY public.audio_publication_approvals
    ADD CONSTRAINT audio_publication_approvals_owner_id_fkey FOREIGN KEY (owner_id) REFERENCES public.actors(id);

ALTER TABLE ONLY public.audio_publication_events
    ADD CONSTRAINT audio_publication_events_actor_id_fkey FOREIGN KEY (actor_id) REFERENCES public.actors(id);

ALTER TABLE ONLY public.audio_publication_events
    ADD CONSTRAINT audio_publication_events_release_id_fkey FOREIGN KEY (release_id) REFERENCES public.audio_releases(id);

ALTER TABLE ONLY public.audio_publication_events
    ADD CONSTRAINT audio_publication_events_unit_id_fkey FOREIGN KEY (unit_id) REFERENCES public.catalog_units(id);

ALTER TABLE ONLY public.audio_publication_operations
    ADD CONSTRAINT audio_publication_operations_owner_id_fkey FOREIGN KEY (owner_id) REFERENCES public.actors(id);

ALTER TABLE ONLY public.audio_publication_operations
    ADD CONSTRAINT audio_publication_operations_release_id_fkey FOREIGN KEY (release_id) REFERENCES public.audio_releases(id);

ALTER TABLE ONLY public.audio_publication_operations
    ADD CONSTRAINT audio_publication_operations_unit_id_fkey FOREIGN KEY (unit_id) REFERENCES public.catalog_units(id);

ALTER TABLE ONLY public.audio_publication_pointers
    ADD CONSTRAINT audio_publication_pointers_active_release_id_unit_id_owner_fkey FOREIGN KEY (active_release_id, unit_id, owner_id) REFERENCES public.audio_releases(id, unit_id, owner_id);

ALTER TABLE ONLY public.audio_publication_pointers
    ADD CONSTRAINT audio_publication_pointers_owner_id_fkey FOREIGN KEY (owner_id) REFERENCES public.actors(id);

ALTER TABLE ONLY public.audio_publication_pointers
    ADD CONSTRAINT audio_publication_pointers_unit_id_fkey FOREIGN KEY (unit_id) REFERENCES public.catalog_units(id);

ALTER TABLE ONLY public.audio_release_candidates
    ADD CONSTRAINT audio_release_candidates_owner_id_fkey FOREIGN KEY (owner_id) REFERENCES public.actors(id);

ALTER TABLE ONLY public.audio_release_candidates
    ADD CONSTRAINT audio_release_candidates_rendition_id_fkey FOREIGN KEY (rendition_id) REFERENCES public.audio_renditions(id);

ALTER TABLE ONLY public.audio_release_withdrawals
    ADD CONSTRAINT audio_release_withdrawals_actor_id_fkey FOREIGN KEY (actor_id) REFERENCES public.actors(id);

ALTER TABLE ONLY public.audio_release_withdrawals
    ADD CONSTRAINT audio_release_withdrawals_release_id_fkey FOREIGN KEY (release_id) REFERENCES public.audio_releases(id);

ALTER TABLE ONLY public.audio_releases
    ADD CONSTRAINT audio_releases_candidate_id_owner_id_fkey FOREIGN KEY (candidate_id, owner_id) REFERENCES public.audio_release_candidates(id, owner_id);

ALTER TABLE ONLY public.audio_releases
    ADD CONSTRAINT audio_releases_owner_id_fkey FOREIGN KEY (owner_id) REFERENCES public.actors(id);

ALTER TABLE ONLY public.audio_releases
    ADD CONSTRAINT audio_releases_rendition_id_fkey FOREIGN KEY (rendition_id) REFERENCES public.audio_renditions(id);

ALTER TABLE ONLY public.audio_releases
    ADD CONSTRAINT audio_releases_unit_id_fkey FOREIGN KEY (unit_id) REFERENCES public.catalog_units(id);

ALTER TABLE ONLY public.audio_renditions
    ADD CONSTRAINT audio_renditions_submission_id_fkey FOREIGN KEY (submission_id) REFERENCES public.audio_submissions(id);

ALTER TABLE ONLY public.audio_reviews
    ADD CONSTRAINT audio_reviews_owner_id_fkey FOREIGN KEY (owner_id) REFERENCES public.actors(id);

ALTER TABLE ONLY public.audio_reviews
    ADD CONSTRAINT audio_reviews_rendition_id_fkey FOREIGN KEY (rendition_id) REFERENCES public.audio_renditions(id);

ALTER TABLE ONLY public.audio_submission_revocations
    ADD CONSTRAINT audio_submission_revocations_submission_id_owner_id_fkey FOREIGN KEY (submission_id, owner_id) REFERENCES public.audio_submissions(id, owner_id);

ALTER TABLE ONLY public.audio_submissions
    ADD CONSTRAINT audio_submissions_brief_id_unit_id_owner_id_brief_digest_fkey FOREIGN KEY (brief_id, unit_id, owner_id, brief_digest) REFERENCES public.production_briefs(id, unit_id, owner_id, digest);

ALTER TABLE ONLY public.audio_upload_intents
    ADD CONSTRAINT audio_upload_intents_created_by_fkey FOREIGN KEY (created_by) REFERENCES public.actors(id);

ALTER TABLE ONLY public.audio_upload_intents
    ADD CONSTRAINT audio_upload_intents_script_id_fkey FOREIGN KEY (script_id) REFERENCES public.scripts(id);

ALTER TABLE ONLY public.audio_upload_intents
    ADD CONSTRAINT audio_upload_intents_script_id_script_revision_fkey FOREIGN KEY (script_id, script_revision) REFERENCES public.script_revisions(script_id, revision);

ALTER TABLE ONLY public.catalog_content_heads
    ADD CONSTRAINT catalog_content_heads_unit_id_fkey FOREIGN KEY (unit_id) REFERENCES public.catalog_units(id);

ALTER TABLE ONLY public.catalog_content_heads
    ADD CONSTRAINT catalog_content_heads_unit_id_revision_id_revision_fkey FOREIGN KEY (unit_id, revision_id, revision) REFERENCES public.catalog_content_revisions(unit_id, id, revision);

ALTER TABLE ONLY public.catalog_content_revisions
    ADD CONSTRAINT catalog_content_revisions_recorded_by_fkey FOREIGN KEY (recorded_by) REFERENCES public.actors(id);

ALTER TABLE ONLY public.catalog_content_revisions
    ADD CONSTRAINT catalog_content_revisions_unit_id_fkey FOREIGN KEY (unit_id) REFERENCES public.catalog_units(id);

ALTER TABLE ONLY public.catalog_delegated_operations
    ADD CONSTRAINT catalog_delegated_operations_actor_id_operation_id_fkey FOREIGN KEY (actor_id, operation_id) REFERENCES public.catalog_operations(actor_id, operation_id);

ALTER TABLE ONLY public.catalog_delegated_operations
    ADD CONSTRAINT catalog_delegated_operations_grant_id_actor_id_fkey FOREIGN KEY (grant_id, actor_id) REFERENCES public.catalog_delegation_grants(id, delegate_id);

ALTER TABLE ONLY public.catalog_delegation_grants
    ADD CONSTRAINT catalog_delegation_grants_delegate_id_fkey FOREIGN KEY (delegate_id) REFERENCES public.actors(id);

ALTER TABLE ONLY public.catalog_delegation_grants
    ADD CONSTRAINT catalog_delegation_grants_work_id_grantor_id_fkey FOREIGN KEY (work_id, grantor_id) REFERENCES public.catalog_works(id, owner_id);

ALTER TABLE ONLY public.catalog_delegation_revocations
    ADD CONSTRAINT catalog_delegation_revocations_grant_id_fkey FOREIGN KEY (grant_id) REFERENCES public.catalog_delegation_grants(id);

ALTER TABLE ONLY public.catalog_delegation_revocations
    ADD CONSTRAINT catalog_delegation_revocations_revoked_by_fkey FOREIGN KEY (revoked_by) REFERENCES public.actors(id);

ALTER TABLE ONLY public.catalog_members
    ADD CONSTRAINT catalog_members_actor_id_fkey FOREIGN KEY (actor_id) REFERENCES public.actors(id);

ALTER TABLE ONLY public.catalog_members
    ADD CONSTRAINT catalog_members_work_id_fkey FOREIGN KEY (work_id) REFERENCES public.catalog_works(id);

ALTER TABLE ONLY public.catalog_operations
    ADD CONSTRAINT catalog_operations_actor_id_fkey FOREIGN KEY (actor_id) REFERENCES public.actors(id);

ALTER TABLE ONLY public.catalog_units
    ADD CONSTRAINT catalog_unit_head_exists FOREIGN KEY (id, head_revision) REFERENCES public.catalog_unit_revisions(unit_id, revision) DEFERRABLE INITIALLY DEFERRED;

ALTER TABLE ONLY public.catalog_unit_revisions
    ADD CONSTRAINT catalog_unit_revisions_recorded_by_fkey FOREIGN KEY (recorded_by) REFERENCES public.actors(id);

ALTER TABLE ONLY public.catalog_unit_revisions
    ADD CONSTRAINT catalog_unit_revisions_unit_id_fkey FOREIGN KEY (unit_id) REFERENCES public.catalog_units(id);

ALTER TABLE ONLY public.catalog_units
    ADD CONSTRAINT catalog_units_work_id_work_kind_fkey FOREIGN KEY (work_id, work_kind) REFERENCES public.catalog_works(id, kind);

ALTER TABLE ONLY public.catalog_works
    ADD CONSTRAINT catalog_work_head_exists FOREIGN KEY (id, head_revision) REFERENCES public.catalog_work_revisions(work_id, revision) DEFERRABLE INITIALLY DEFERRED;

ALTER TABLE ONLY public.catalog_work_revisions
    ADD CONSTRAINT catalog_work_revisions_recorded_by_fkey FOREIGN KEY (recorded_by) REFERENCES public.actors(id);

ALTER TABLE ONLY public.catalog_work_revisions
    ADD CONSTRAINT catalog_work_revisions_work_id_fkey FOREIGN KEY (work_id) REFERENCES public.catalog_works(id);

ALTER TABLE ONLY public.catalog_works
    ADD CONSTRAINT catalog_works_owner_id_fkey FOREIGN KEY (owner_id) REFERENCES public.actors(id);

ALTER TABLE ONLY public.listener_device_sequences
    ADD CONSTRAINT listener_device_sequences_listener_id_fkey FOREIGN KEY (listener_id) REFERENCES public.listener_state(listener_id);

ALTER TABLE ONLY public.listener_history
    ADD CONSTRAINT listener_history_listener_id_fkey FOREIGN KEY (listener_id) REFERENCES public.listener_state(listener_id);

ALTER TABLE ONLY public.listener_history
    ADD CONSTRAINT listener_history_publication_id_fkey FOREIGN KEY (publication_id) REFERENCES public.audio_releases(id);

ALTER TABLE ONLY public.listener_history
    ADD CONSTRAINT listener_history_unit_id_fkey FOREIGN KEY (unit_id) REFERENCES public.catalog_units(id);

ALTER TABLE ONLY public.listener_history
    ADD CONSTRAINT listener_history_work_id_fkey FOREIGN KEY (work_id) REFERENCES public.catalog_works(id);

ALTER TABLE ONLY public.listener_operations
    ADD CONSTRAINT listener_operations_listener_id_fkey FOREIGN KEY (listener_id) REFERENCES public.listener_state(listener_id);

ALTER TABLE ONLY public.listener_playback_sessions
    ADD CONSTRAINT listener_playback_sessions_listener_id_fkey FOREIGN KEY (listener_id) REFERENCES public.listener_state(listener_id);

ALTER TABLE ONLY public.listener_progress
    ADD CONSTRAINT listener_progress_listener_id_fkey FOREIGN KEY (listener_id) REFERENCES public.listener_state(listener_id);

ALTER TABLE ONLY public.listener_progress
    ADD CONSTRAINT listener_progress_publication_id_fkey FOREIGN KEY (publication_id) REFERENCES public.audio_releases(id);

ALTER TABLE ONLY public.listener_progress
    ADD CONSTRAINT listener_progress_rendition_id_fkey FOREIGN KEY (rendition_id) REFERENCES public.audio_renditions(id);

ALTER TABLE ONLY public.listener_progress
    ADD CONSTRAINT listener_progress_unit_id_fkey FOREIGN KEY (unit_id) REFERENCES public.catalog_units(id);

ALTER TABLE ONLY public.listener_progress
    ADD CONSTRAINT listener_progress_work_id_fkey FOREIGN KEY (work_id) REFERENCES public.catalog_works(id);

ALTER TABLE ONLY public.listener_saved
    ADD CONSTRAINT listener_saved_listener_id_fkey FOREIGN KEY (listener_id) REFERENCES public.listener_state(listener_id);

ALTER TABLE ONLY public.listener_saved
    ADD CONSTRAINT listener_saved_work_id_fkey FOREIGN KEY (work_id) REFERENCES public.catalog_works(id);

ALTER TABLE ONLY public.listener_state
    ADD CONSTRAINT listener_state_listener_id_fkey FOREIGN KEY (listener_id) REFERENCES public.actors(id);

ALTER TABLE ONLY public.media_artwork_operations
    ADD CONSTRAINT media_artwork_operations_work_id_owner_id_fkey FOREIGN KEY (work_id, owner_id) REFERENCES public.catalog_works(id, owner_id);

ALTER TABLE ONLY public.media_image_assets
    ADD CONSTRAINT media_image_assets_intent_id_work_id_owner_id_fkey FOREIGN KEY (intent_id, work_id, owner_id) REFERENCES public.media_image_intents(id, work_id, owner_id);

ALTER TABLE ONLY public.media_image_intents
    ADD CONSTRAINT media_image_intents_work_id_owner_id_fkey FOREIGN KEY (work_id, owner_id) REFERENCES public.catalog_works(id, owner_id);

ALTER TABLE ONLY public.media_work_artwork_changes
    ADD CONSTRAINT media_work_artwork_changes_image_asset_id_work_id_owner_id_fkey FOREIGN KEY (image_asset_id, work_id, owner_id, image_asset_digest) REFERENCES public.media_image_assets(id, work_id, owner_id, digest);

ALTER TABLE ONLY public.media_work_artwork_changes
    ADD CONSTRAINT media_work_artwork_changes_recorded_by_fkey FOREIGN KEY (recorded_by) REFERENCES public.actors(id);

ALTER TABLE ONLY public.media_work_artwork_changes
    ADD CONSTRAINT media_work_artwork_changes_work_id_owner_id_fkey FOREIGN KEY (work_id, owner_id) REFERENCES public.catalog_works(id, owner_id);

ALTER TABLE ONLY public.media_work_artwork_changes
    ADD CONSTRAINT media_work_artwork_changes_work_id_work_metadata_revision_fkey FOREIGN KEY (work_id, work_metadata_revision) REFERENCES public.catalog_work_revisions(work_id, revision);

ALTER TABLE ONLY public.production_approvals
    ADD CONSTRAINT production_approvals_approved_by_fkey FOREIGN KEY (approved_by) REFERENCES public.actors(id);

ALTER TABLE ONLY public.production_approvals
    ADD CONSTRAINT production_approvals_script_id_fkey FOREIGN KEY (script_id) REFERENCES public.scripts(id);

ALTER TABLE ONLY public.production_approvals
    ADD CONSTRAINT production_approvals_snapshot_id_fkey FOREIGN KEY (snapshot_id) REFERENCES public.production_snapshots(id);

ALTER TABLE ONLY public.production_brief_heads
    ADD CONSTRAINT production_brief_heads_unit_id_brief_id_version_fkey FOREIGN KEY (unit_id, brief_id, version) REFERENCES public.production_briefs(unit_id, id, version);

ALTER TABLE ONLY public.production_brief_heads
    ADD CONSTRAINT production_brief_heads_unit_id_fkey FOREIGN KEY (unit_id) REFERENCES public.catalog_units(id);

ALTER TABLE ONLY public.production_briefs
    ADD CONSTRAINT production_briefs_owner_id_fkey FOREIGN KEY (owner_id) REFERENCES public.actors(id);

ALTER TABLE ONLY public.production_briefs
    ADD CONSTRAINT production_briefs_unit_id_content_revision_id_content_sequ_fkey FOREIGN KEY (unit_id, content_revision_id, content_sequence) REFERENCES public.catalog_content_revisions(unit_id, id, revision);

ALTER TABLE ONLY public.production_briefs
    ADD CONSTRAINT production_briefs_unit_id_fkey FOREIGN KEY (unit_id) REFERENCES public.catalog_units(id);

ALTER TABLE ONLY public.production_producer_grants
    ADD CONSTRAINT production_producer_grants_brief_id_unit_id_owner_id_brief_fkey FOREIGN KEY (brief_id, unit_id, owner_id, brief_digest) REFERENCES public.production_briefs(id, unit_id, owner_id, digest);

ALTER TABLE ONLY public.production_producer_revocations
    ADD CONSTRAINT production_producer_revocations_grant_id_fkey FOREIGN KEY (grant_id) REFERENCES public.production_producer_grants(id);

ALTER TABLE ONLY public.production_producer_revocations
    ADD CONSTRAINT production_producer_revocations_revoked_by_fkey FOREIGN KEY (revoked_by) REFERENCES public.actors(id);

ALTER TABLE ONLY public.production_rights_claims
    ADD CONSTRAINT production_rights_claims_owner_id_kind_record_id_fkey FOREIGN KEY (owner_id, kind, record_id) REFERENCES public.script_evidence(owner_id, kind, id);

ALTER TABLE ONLY public.production_rights_claims
    ADD CONSTRAINT production_rights_claims_recorded_by_fkey FOREIGN KEY (recorded_by) REFERENCES public.actors(id);

ALTER TABLE ONLY public.production_rights_claims
    ADD CONSTRAINT production_rights_claims_script_id_fkey FOREIGN KEY (script_id) REFERENCES public.scripts(id);

ALTER TABLE ONLY public.production_settings
    ADD CONSTRAINT production_settings_recorded_by_fkey FOREIGN KEY (recorded_by) REFERENCES public.actors(id);

ALTER TABLE ONLY public.production_settings
    ADD CONSTRAINT production_settings_script_id_fkey FOREIGN KEY (script_id) REFERENCES public.scripts(id);

ALTER TABLE ONLY public.production_settings
    ADD CONSTRAINT production_settings_script_id_script_revision_fkey FOREIGN KEY (script_id, script_revision) REFERENCES public.script_revisions(script_id, revision);

ALTER TABLE ONLY public.production_snapshots
    ADD CONSTRAINT production_snapshots_created_by_fkey FOREIGN KEY (created_by) REFERENCES public.actors(id);

ALTER TABLE ONLY public.production_snapshots
    ADD CONSTRAINT production_snapshots_owner_id_fkey FOREIGN KEY (owner_id) REFERENCES public.actors(id);

ALTER TABLE ONLY public.production_snapshots
    ADD CONSTRAINT production_snapshots_script_id_fkey FOREIGN KEY (script_id) REFERENCES public.scripts(id);

ALTER TABLE ONLY public.production_snapshots
    ADD CONSTRAINT production_snapshots_script_id_script_revision_fkey FOREIGN KEY (script_id, script_revision) REFERENCES public.script_revisions(script_id, revision);

ALTER TABLE ONLY public.production_snapshots
    ADD CONSTRAINT production_snapshots_script_id_settings_version_fkey FOREIGN KEY (script_id, settings_version) REFERENCES public.production_settings(script_id, version);

ALTER TABLE ONLY public.revision_evidence
    ADD CONSTRAINT revision_evidence_owner_id_kind_evidence_id_fkey FOREIGN KEY (owner_id, kind, evidence_id) REFERENCES public.script_evidence(owner_id, kind, id);

ALTER TABLE ONLY public.revision_evidence
    ADD CONSTRAINT revision_evidence_script_id_revision_fkey FOREIGN KEY (script_id, revision) REFERENCES public.script_revisions(script_id, revision);

ALTER TABLE ONLY public.script_evidence
    ADD CONSTRAINT script_evidence_owner_id_fkey FOREIGN KEY (owner_id) REFERENCES public.actors(id);

ALTER TABLE ONLY public.scripts
    ADD CONSTRAINT script_head_exists FOREIGN KEY (id, head_revision) REFERENCES public.script_revisions(script_id, revision) DEFERRABLE INITIALLY DEFERRED;

ALTER TABLE ONLY public.script_members
    ADD CONSTRAINT script_members_actor_id_fkey FOREIGN KEY (actor_id) REFERENCES public.actors(id);

ALTER TABLE ONLY public.script_members
    ADD CONSTRAINT script_members_script_id_fkey FOREIGN KEY (script_id) REFERENCES public.scripts(id);

ALTER TABLE ONLY public.script_review_operations
    ADD CONSTRAINT script_review_operations_actor_id_fkey FOREIGN KEY (actor_id) REFERENCES public.actors(id);

ALTER TABLE ONLY public.script_review_operations
    ADD CONSTRAINT script_review_operations_script_id_revision_fkey FOREIGN KEY (script_id, revision) REFERENCES public.script_reviews(script_id, revision);

ALTER TABLE ONLY public.script_reviews
    ADD CONSTRAINT script_reviews_reviewed_by_fkey FOREIGN KEY (reviewed_by) REFERENCES public.actors(id);

ALTER TABLE ONLY public.script_reviews
    ADD CONSTRAINT script_reviews_script_id_revision_fkey FOREIGN KEY (script_id, revision) REFERENCES public.script_revisions(script_id, revision);

ALTER TABLE ONLY public.script_revisions
    ADD CONSTRAINT script_revisions_accepted_by_fkey FOREIGN KEY (accepted_by) REFERENCES public.actors(id);

ALTER TABLE ONLY public.script_revisions
    ADD CONSTRAINT script_revisions_script_id_fkey FOREIGN KEY (script_id) REFERENCES public.scripts(id);

ALTER TABLE ONLY public.scripts
    ADD CONSTRAINT scripts_owner_id_fkey FOREIGN KEY (owner_id) REFERENCES public.actors(id);

ALTER TABLE ONLY public.sessions
    ADD CONSTRAINT sessions_actor_id_fkey FOREIGN KEY (actor_id) REFERENCES public.actors(id);

ALTER TABLE ONLY public.source_records
    ADD CONSTRAINT source_records_imported_by_fkey FOREIGN KEY (imported_by) REFERENCES public.actors(id);

ALTER TABLE ONLY public.source_records
    ADD CONSTRAINT source_records_owner_id_kind_id_fkey FOREIGN KEY (owner_id, kind, id) REFERENCES public.script_evidence(owner_id, kind, id);

ALTER TABLE ONLY public.studio_oidc_sessions
    ADD CONSTRAINT studio_oidc_sessions_issuer_subject_fkey FOREIGN KEY (issuer, subject) REFERENCES public.actor_external_identities(issuer, subject);

ALTER TABLE ONLY public.studio_oidc_sessions
    ADD CONSTRAINT studio_oidc_sessions_lineage_root_hash_fkey FOREIGN KEY (lineage_root_hash) REFERENCES public.studio_oidc_sessions(token_hash);

ALTER TABLE ONLY public.studio_oidc_sessions
    ADD CONSTRAINT studio_oidc_sessions_revoked_by_token_hash_fkey FOREIGN KEY (revoked_by_token_hash) REFERENCES public.studio_oidc_sessions(token_hash);

ALTER TABLE ONLY public.studio_oidc_sessions
    ADD CONSTRAINT studio_oidc_sessions_source_state_hash_fkey FOREIGN KEY (source_state_hash) REFERENCES public.studio_oidc_flows(state_hash);

ALTER TABLE ONLY public.studio_oidc_sessions
    ADD CONSTRAINT studio_oidc_sessions_token_hash_fkey FOREIGN KEY (token_hash) REFERENCES public.sessions(token_hash);

ALTER TABLE ONLY public.theatre_firebase_enrollment_receipts
    ADD CONSTRAINT theatre_firebase_enrollment_receip_namespace_enrollment_id_fkey FOREIGN KEY (namespace, enrollment_id) REFERENCES public.theatre_firebase_enrollments(namespace, id);

ALTER TABLE ONLY public.theatre_firebase_send_starts
    ADD CONSTRAINT theatre_firebase_send_starts_namespace_enrollment_id_fkey FOREIGN KEY (namespace, enrollment_id) REFERENCES public.theatre_firebase_enrollments(namespace, id);

ALTER TABLE ONLY public.theatre_identity_flows
    ADD CONSTRAINT theatre_identity_flows_lineage_root_hash_fkey FOREIGN KEY (lineage_root_hash) REFERENCES public.theatre_identity_sessions(token_hash);

ALTER TABLE ONLY public.theatre_identity_sessions
    ADD CONSTRAINT theatre_identity_sessions_actor_id_fkey FOREIGN KEY (actor_id) REFERENCES public.actors(id);

ALTER TABLE ONLY public.theatre_identity_sessions
    ADD CONSTRAINT theatre_identity_sessions_issuer_subject_fkey FOREIGN KEY (issuer, subject) REFERENCES public.actor_external_identities(issuer, subject);

ALTER TABLE ONLY public.theatre_identity_sessions
    ADD CONSTRAINT theatre_identity_sessions_lineage_root_hash_fkey FOREIGN KEY (lineage_root_hash) REFERENCES public.theatre_identity_sessions(token_hash);

ALTER TABLE ONLY public.theatre_identity_sessions
    ADD CONSTRAINT theatre_identity_sessions_revoked_by_token_hash_fkey FOREIGN KEY (revoked_by_token_hash) REFERENCES public.theatre_identity_sessions(token_hash);

ALTER TABLE ONLY public.theatre_identity_sessions
    ADD CONSTRAINT theatre_identity_sessions_source_state_hash_fkey FOREIGN KEY (source_state_hash) REFERENCES public.theatre_identity_flows(state_hash);

ALTER TABLE ONLY public.theatre_native_sessions
    ADD CONSTRAINT theatre_native_sessions_actor_id_fkey FOREIGN KEY (actor_id) REFERENCES public.actors(id);

ALTER TABLE ONLY public.theatre_native_sessions
    ADD CONSTRAINT theatre_native_sessions_issuer_subject_fkey FOREIGN KEY (issuer, subject) REFERENCES public.actor_external_identities(issuer, subject);

ALTER TABLE ONLY public.theatre_registration_mail_starts
    ADD CONSTRAINT theatre_registration_mail_starts_namespace_registration_id_fkey FOREIGN KEY (namespace, registration_id) REFERENCES public.theatre_registrations(namespace, id);

INSERT INTO public.catalog_clock VALUES (true,0);
REVOKE ALL ON ALL TABLES IN SCHEMA public FROM PUBLIC;
REVOKE ALL ON ALL FUNCTIONS IN SCHEMA public FROM PUBLIC;
