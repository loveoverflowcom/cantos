-- Admin alone. No Studio/Theatre/producer authority, grant, resource ACL or account deletion.
CREATE TABLE admin_access_revocations (
    actor_id text PRIMARY KEY REFERENCES actors(id),
    revoked_at timestamptz NOT NULL DEFAULT clock_timestamp()
);
CREATE TABLE admin_audit (
    id uuid PRIMARY KEY,
    actor_id text REFERENCES actors(id),
    surface text NOT NULL CHECK (surface='admin'),
    action text NOT NULL CHECK (action IN ('capabilities','revoke_admin_access')),
    target_actor_id text,
    outcome text NOT NULL CHECK (outcome IN ('allowed','denied','replayed','conflict','unavailable')),
    occurred_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    correlation_id uuid NOT NULL UNIQUE
);
CREATE TABLE admin_operations (
    actor_id text NOT NULL REFERENCES actors(id),
    operation_id uuid NOT NULL,
    target_actor_id text NOT NULL REFERENCES actors(id),
    audit_id uuid NOT NULL REFERENCES admin_audit(id),
    PRIMARY KEY(actor_id,operation_id)
);
REVOKE ALL ON admin_access_revocations,admin_audit,admin_operations FROM PUBLIC,cantos_app;
CREATE FUNCTION admin_immutable() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN RAISE EXCEPTION 'Admin record is append-only'; END; $$;
CREATE TRIGGER admin_audit_immutable BEFORE UPDATE OR DELETE OR TRUNCATE ON admin_audit
    FOR EACH STATEMENT EXECUTE FUNCTION admin_immutable();
CREATE TRIGGER admin_operations_immutable BEFORE UPDATE OR DELETE OR TRUNCATE ON admin_operations
    FOR EACH STATEMENT EXECUTE FUNCTION admin_immutable();
CREATE TRIGGER admin_revocations_immutable BEFORE UPDATE OR DELETE OR TRUNCATE ON admin_access_revocations
    FOR EACH STATEMENT EXECUTE FUNCTION admin_immutable();

-- Minimal privileged read. Only existing exact registry bindings; no implicit Admin registration.
-- Canonical ordered actor SHARE/advisory locks fence local disable and Admin revocation.
-- Role, proof freshness and action decisions stay in the Rust functional core.
CREATE FUNCTION admin_identity_facts(p_issuer text,p_subject text,p_target text)
RETURNS TABLE(actor_id text,active boolean,revoked boolean,target_exists boolean,now_seconds bigint)
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
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
CREATE FUNCTION admin_append_audit(p_id uuid,p_actor text,p_action text,p_target text,p_outcome text,p_correlation uuid)
RETURNS void LANGUAGE sql SECURITY DEFINER SET search_path=pg_catalog AS $$
    INSERT INTO public.admin_audit(id,actor_id,surface,action,target_actor_id,outcome,correlation_id)
        VALUES(p_id,p_actor,'admin',p_action,p_target,p_outcome,p_correlation);
$$;
CREATE FUNCTION admin_operation(p_actor text,p_operation uuid)
RETURNS TABLE(target_actor_id text,audit_id uuid) LANGUAGE sql SECURITY DEFINER SET search_path=pg_catalog AS $$
    SELECT o.target_actor_id,o.audit_id FROM public.admin_operations o WHERE o.actor_id=p_actor AND o.operation_id=p_operation;
$$;
CREATE FUNCTION admin_revoke_access(p_actor text,p_operation uuid,p_target text,p_audit uuid)
RETURNS void LANGUAGE sql SECURITY DEFINER SET search_path=pg_catalog AS $$
    INSERT INTO public.admin_access_revocations(actor_id) VALUES(p_target) ON CONFLICT DO NOTHING;
    INSERT INTO public.admin_operations(actor_id,operation_id,target_actor_id,audit_id) VALUES(p_actor,p_operation,p_target,p_audit);
$$;
REVOKE ALL ON FUNCTION admin_identity_facts(text,text,text),admin_append_audit(uuid,text,text,text,text,uuid),
    admin_operation(text,uuid),admin_revoke_access(text,uuid,text,uuid),admin_immutable() FROM PUBLIC,cantos_app;
-- Operator-applied opt-in function EXECUTE grants only. No automatic live rights/config bootstrap.
