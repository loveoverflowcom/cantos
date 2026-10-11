-- Bounded private local/fake dialogue execution, on the existing PostgreSQL authority.
-- Media remain private filesystem records; these rows confer no publication authority.
CREATE TABLE durable_runs (
    id text PRIMARY KEY,
    script_id text NOT NULL REFERENCES scripts(id),
    snapshot_id text NOT NULL REFERENCES production_snapshots(id),
    created_by text NOT NULL REFERENCES actors(id),
    operation_id text NOT NULL,
    input_digest text NOT NULL CHECK (input_digest ~ '^production-p1:sha256:[a-f0-9]{64}$'),
    mode text NOT NULL CHECK (mode IN ('local_espeak', 'synthetic_fake')),
    request_bytes bytea NOT NULL CHECK (octet_length(request_bytes) BETWEEN 1 AND 65536),
    max_attempts integer NOT NULL CHECK (max_attempts BETWEEN 1 AND 8),
    base_seconds bigint NOT NULL CHECK (base_seconds BETWEEN 1 AND 86400),
    cap_seconds bigint NOT NULL CHECK (cap_seconds BETWEEN base_seconds AND 86400),
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    UNIQUE (script_id, created_by, operation_id)
);
CREATE TABLE durable_jobs (
    id text PRIMARY KEY,
    run_id text NOT NULL UNIQUE REFERENCES durable_runs(id),
    status text NOT NULL DEFAULT 'queued'
        CHECK (status IN ('queued', 'running', 'completed', 'failed', 'paused', 'cancelled')),
    reason text CHECK (octet_length(reason) BETWEEN 1 AND 256),
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    CHECK ((status IN ('paused', 'failed')) = (reason IS NOT NULL))
);
CREATE TABLE durable_steps (
    id text PRIMARY KEY,
    job_id text NOT NULL REFERENCES durable_jobs(id),
    dialogue_id text NOT NULL CHECK (dialogue_id ~ '^[a-z0-9_-]{1,64}$'),
    input_fingerprint text NOT NULL CHECK (octet_length(input_fingerprint) BETWEEN 1 AND 256),
    status text NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'leased', 'succeeded', 'failed', 'cancelled')),
    lease_owner text CHECK (octet_length(lease_owner) BETWEEN 1 AND 128),
    lease_token bigint NOT NULL DEFAULT 0 CHECK (lease_token >= 0),
    lease_expires_at timestamptz,
    not_before timestamptz NOT NULL DEFAULT clock_timestamp(),
    accepted_attempt_id text,
    UNIQUE (job_id, dialogue_id),
    CHECK ((status = 'leased') = (lease_owner IS NOT NULL AND lease_expires_at IS NOT NULL)),
    CHECK ((lease_owner IS NULL) = (lease_expires_at IS NULL)),
    CHECK ((status = 'succeeded') = (accepted_attempt_id IS NOT NULL))
);
CREATE INDEX durable_step_claim ON durable_steps(not_before, id)
    WHERE status IN ('pending', 'leased');
CREATE INDEX durable_steps_by_job ON durable_steps(job_id, status);
CREATE TABLE durable_budgets (
    run_id text PRIMARY KEY REFERENCES durable_runs(id),
    currency text NOT NULL CHECK (currency IN ('USD', 'VND')),
    limit_minor bigint NOT NULL CHECK (limit_minor >= 0),
    reserved_minor bigint NOT NULL DEFAULT 0 CHECK (reserved_minor >= 0),
    settled_minor bigint NOT NULL DEFAULT 0 CHECK (settled_minor >= 0),
    -- Only bounded local/fake provisional settlement exists in this slice. Commercial
    -- reported overruns require a separate ledger/pausing policy before dispatch is enabled.
    CHECK (reserved_minor <= limit_minor - settled_minor),
    UNIQUE (run_id, currency)
);
CREATE TABLE durable_reservations (
    id text PRIMARY KEY,
    run_id text NOT NULL REFERENCES durable_runs(id),
    step_id text NOT NULL REFERENCES durable_steps(id),
    amount_minor bigint NOT NULL CHECK (amount_minor >= 0),
    currency text NOT NULL CHECK (currency IN ('USD', 'VND')),
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    UNIQUE (id, step_id),
    FOREIGN KEY (run_id, currency) REFERENCES durable_budgets(run_id, currency)
);
CREATE TABLE durable_attempts (
    id text PRIMARY KEY,
    step_id text NOT NULL REFERENCES durable_steps(id),
    lease_token bigint NOT NULL CHECK (lease_token > 0),
    operation_id text NOT NULL UNIQUE,
    reservation_id text NOT NULL UNIQUE,
    intent_bytes bytea NOT NULL CHECK (octet_length(intent_bytes) BETWEEN 1 AND 65536),
    attempt_number integer NOT NULL CHECK (attempt_number > 0),
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    UNIQUE (step_id, attempt_number),
    UNIQUE (id, step_id),
    FOREIGN KEY (reservation_id, step_id) REFERENCES durable_reservations(id, step_id)
);
CREATE TABLE durable_observations (
    attempt_id text PRIMARY KEY REFERENCES durable_attempts(id),
    receipt_bytes bytea NOT NULL CHECK (octet_length(receipt_bytes) BETWEEN 1 AND 65536),
    output_path text CHECK (octet_length(output_path) BETWEEN 1 AND 4096),
    created_at timestamptz NOT NULL DEFAULT clock_timestamp()
);
ALTER TABLE durable_steps ADD CONSTRAINT durable_accepted_attempt_scope
    FOREIGN KEY (accepted_attempt_id, id) REFERENCES durable_attempts(id, step_id);
ALTER TABLE durable_steps ADD CONSTRAINT durable_accepted_attempt_observed
    FOREIGN KEY (accepted_attempt_id) REFERENCES durable_observations(attempt_id);
CREATE TABLE durable_settlements (
    reservation_id text PRIMARY KEY REFERENCES durable_reservations(id),
    kind text NOT NULL CHECK (kind IN ('released', 'provisional_local', 'reported_synthetic')),
    amount_minor bigint NOT NULL CHECK (amount_minor >= 0),
    created_at timestamptz NOT NULL DEFAULT clock_timestamp()
);
CREATE TABLE durable_resolutions (
    attempt_id text PRIMARY KEY REFERENCES durable_attempts(id),
    resolved_by text NOT NULL REFERENCES actors(id),
    evidence text NOT NULL CHECK (octet_length(evidence) BETWEEN 1 AND 2048),
    kind text NOT NULL CHECK (kind = 'not_executed'),
    created_at timestamptz NOT NULL DEFAULT clock_timestamp()
);
CREATE TABLE durable_controls (
    id text PRIMARY KEY,
    job_id text NOT NULL REFERENCES durable_jobs(id),
    actor_id text NOT NULL REFERENCES actors(id),
    operation_id text NOT NULL,
    action text NOT NULL CHECK (action IN ('pause', 'resume', 'cancel', 'retry')),
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    UNIQUE (job_id, actor_id, operation_id)
);

-- Preserve exact immutable snapshot scope instead of relying on a snapshot ID alone.
CREATE FUNCTION durable_run_scope() RETURNS trigger LANGUAGE plpgsql
    SET search_path = pg_catalog AS $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM public.production_snapshots s
        WHERE s.id=NEW.snapshot_id AND s.script_id=NEW.script_id AND s.input_digest=NEW.input_digest
    ) THEN
        RAISE EXCEPTION 'durable run snapshot scope mismatch' USING ERRCODE = '23503';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER durable_runs_scope BEFORE INSERT ON durable_runs
    FOR EACH ROW EXECUTE FUNCTION durable_run_scope();

-- A reservation cannot authorize a step from another run. Job/run and step/job bindings
-- below are immutable, so the insert-time join remains valid for the reservation's lifetime.
CREATE FUNCTION durable_reservation_scope() RETURNS trigger LANGUAGE plpgsql
    SET search_path = pg_catalog AS $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM public.durable_steps s JOIN public.durable_jobs j ON j.id=s.job_id
        WHERE s.id=NEW.step_id AND j.run_id=NEW.run_id
    ) THEN
        RAISE EXCEPTION 'durable reservation scope mismatch' USING ERRCODE = '23503';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER durable_reservations_scope BEFORE INSERT ON durable_reservations
    FOR EACH ROW EXECUTE FUNCTION durable_reservation_scope();

CREATE FUNCTION durable_job_transition() RETURNS trigger LANGUAGE plpgsql
    SET search_path = pg_catalog AS $$
BEGIN
    IF NEW.id IS DISTINCT FROM OLD.id OR NEW.run_id IS DISTINCT FROM OLD.run_id
        OR NEW.created_at IS DISTINCT FROM OLD.created_at THEN
        RAISE EXCEPTION 'durable job identity is immutable' USING ERRCODE = '55000';
    END IF;
    IF NEW.status IS DISTINCT FROM OLD.status AND NOT (
        (OLD.status='queued' AND NEW.status IN ('running', 'paused', 'cancelled')) OR
        (OLD.status='running' AND NEW.status IN ('completed', 'paused', 'failed', 'cancelled')) OR
        (OLD.status='paused' AND NEW.status IN ('queued', 'cancelled')) OR
        (OLD.status='failed' AND NEW.status IN ('queued', 'cancelled'))
    ) THEN
        RAISE EXCEPTION 'invalid durable job transition' USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER durable_jobs_transition BEFORE UPDATE ON durable_jobs
    FOR EACH ROW EXECUTE FUNCTION durable_job_transition();

CREATE FUNCTION durable_step_transition() RETURNS trigger LANGUAGE plpgsql
    SET search_path = pg_catalog AS $$
BEGIN
    IF NEW.id IS DISTINCT FROM OLD.id OR NEW.job_id IS DISTINCT FROM OLD.job_id
        OR NEW.dialogue_id IS DISTINCT FROM OLD.dialogue_id
        OR NEW.input_fingerprint IS DISTINCT FROM OLD.input_fingerprint THEN
        RAISE EXCEPTION 'durable step identity is immutable' USING ERRCODE = '55000';
    END IF;
    IF NEW.lease_token < OLD.lease_token THEN
        RAISE EXCEPTION 'durable lease token cannot decrease' USING ERRCODE = '23514';
    END IF;
    IF OLD.accepted_attempt_id IS NOT NULL
        AND NEW.accepted_attempt_id IS DISTINCT FROM OLD.accepted_attempt_id THEN
        RAISE EXCEPTION 'durable accepted result is immutable' USING ERRCODE = '55000';
    END IF;
    IF NEW.status IS DISTINCT FROM OLD.status AND NOT (
        (OLD.status='pending' AND NEW.status IN ('leased', 'failed', 'cancelled')) OR
        (OLD.status='leased' AND NEW.status IN ('pending', 'succeeded', 'failed', 'cancelled')) OR
        (OLD.status='failed' AND NEW.status IN ('pending', 'cancelled'))
    ) THEN
        RAISE EXCEPTION 'invalid durable step transition' USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER durable_steps_transition BEFORE UPDATE ON durable_steps
    FOR EACH ROW EXECUTE FUNCTION durable_step_transition();

CREATE FUNCTION durable_budget_identity() RETURNS trigger LANGUAGE plpgsql
    SET search_path = pg_catalog AS $$
BEGIN
    IF NEW.run_id IS DISTINCT FROM OLD.run_id OR NEW.currency IS DISTINCT FROM OLD.currency
        OR NEW.limit_minor IS DISTINCT FROM OLD.limit_minor THEN
        RAISE EXCEPTION 'durable budget policy is immutable' USING ERRCODE = '55000';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER durable_budgets_identity BEFORE UPDATE ON durable_budgets
    FOR EACH ROW EXECUTE FUNCTION durable_budget_identity();

CREATE TRIGGER durable_runs_immutable BEFORE UPDATE OR DELETE ON durable_runs
    FOR EACH ROW EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER durable_runs_no_truncate BEFORE TRUNCATE ON durable_runs
    FOR EACH STATEMENT EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER durable_reservations_immutable BEFORE UPDATE OR DELETE ON durable_reservations
    FOR EACH ROW EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER durable_reservations_no_truncate BEFORE TRUNCATE ON durable_reservations
    FOR EACH STATEMENT EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER durable_attempts_immutable BEFORE UPDATE OR DELETE ON durable_attempts
    FOR EACH ROW EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER durable_attempts_no_truncate BEFORE TRUNCATE ON durable_attempts
    FOR EACH STATEMENT EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER durable_observations_immutable BEFORE UPDATE OR DELETE ON durable_observations
    FOR EACH ROW EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER durable_observations_no_truncate BEFORE TRUNCATE ON durable_observations
    FOR EACH STATEMENT EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER durable_settlements_immutable BEFORE UPDATE OR DELETE ON durable_settlements
    FOR EACH ROW EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER durable_settlements_no_truncate BEFORE TRUNCATE ON durable_settlements
    FOR EACH STATEMENT EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER durable_resolutions_immutable BEFORE UPDATE OR DELETE ON durable_resolutions
    FOR EACH ROW EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER durable_resolutions_no_truncate BEFORE TRUNCATE ON durable_resolutions
    FOR EACH STATEMENT EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER durable_controls_immutable BEFORE UPDATE OR DELETE ON durable_controls
    FOR EACH ROW EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER durable_controls_no_truncate BEFORE TRUNCATE ON durable_controls
    FOR EACH STATEMENT EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER durable_jobs_no_delete BEFORE DELETE ON durable_jobs
    FOR EACH ROW EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER durable_jobs_no_truncate BEFORE TRUNCATE ON durable_jobs
    FOR EACH STATEMENT EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER durable_steps_no_delete BEFORE DELETE ON durable_steps
    FOR EACH ROW EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER durable_steps_no_truncate BEFORE TRUNCATE ON durable_steps
    FOR EACH STATEMENT EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER durable_budgets_no_delete BEFORE DELETE ON durable_budgets
    FOR EACH ROW EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER durable_budgets_no_truncate BEFORE TRUNCATE ON durable_budgets
    FOR EACH STATEMENT EXECUTE FUNCTION reject_settled_change();
REVOKE ALL ON durable_runs,durable_jobs,durable_steps,durable_budgets,durable_reservations,
    durable_attempts,durable_observations,durable_settlements,durable_resolutions,durable_controls FROM PUBLIC;
REVOKE ALL ON FUNCTION durable_run_scope(),durable_reservation_scope(),durable_job_transition(),
    durable_step_transition(),durable_budget_identity() FROM PUBLIC;
-- Migration also works before deployment provisions its restricted application role.
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname='cantos_app') THEN
        GRANT SELECT,INSERT ON durable_runs,durable_reservations,durable_attempts,
            durable_observations,durable_settlements,durable_resolutions,durable_controls TO cantos_app;
        GRANT SELECT,INSERT,UPDATE ON durable_jobs,durable_steps,durable_budgets TO cantos_app;
    END IF;
END;
$$;

-- Unique committed send authorization supplements the conservative intent; a crash here is
-- still ambiguous because a provider send follows outside this transaction.
CREATE TABLE durable_dispatches (
    attempt_id text PRIMARY KEY REFERENCES durable_attempts(id),
    lease_token bigint NOT NULL CHECK (lease_token > 0),
    created_at timestamptz NOT NULL DEFAULT clock_timestamp()
);
CREATE TRIGGER durable_dispatches_immutable BEFORE UPDATE OR DELETE ON durable_dispatches
    FOR EACH ROW EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER durable_dispatches_no_truncate BEFORE TRUNCATE ON durable_dispatches
    FOR EACH STATEMENT EXECUTE FUNCTION reject_settled_change();
REVOKE ALL ON durable_dispatches FROM PUBLIC;
DO $$ BEGIN
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname='cantos_app') THEN
        GRANT SELECT,INSERT ON durable_dispatches TO cantos_app;
    END IF;
END; $$;
