-- Adaptation is private editorial work. No row grants publication eligibility.
CREATE TABLE adaptation_runs (
    id text PRIMARY KEY CHECK (id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'),
    owner_id text NOT NULL REFERENCES actors(id),
    operation_id text NOT NULL CHECK (operation_id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'),
    source_id text NOT NULL,
    script_id text NOT NULL CHECK (script_id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'),
    frozen_input jsonb NOT NULL CHECK (jsonb_typeof(frozen_input) = 'object' AND octet_length(frozen_input::text) <= 6291456),
    input_digest text NOT NULL CHECK (input_digest ~ '^[a-f0-9]{64}$'),
    status text NOT NULL CHECK (status IN ('queued','running','succeeded','invalid_output','failed','ambiguous','cancelled','accepted')),
    problem jsonb,
    dispatch_deadline timestamptz,
    recorded_at timestamptz NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at timestamptz NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (owner_id, operation_id),
    FOREIGN KEY (owner_id, source_id) REFERENCES source_records(owner_id,id),
    CHECK ((status = 'running') = (dispatch_deadline IS NOT NULL)),
    CHECK (problem IS NULL OR jsonb_typeof(problem) = 'object')
);
CREATE TABLE adaptation_attempts (
    run_id text PRIMARY KEY REFERENCES adaptation_runs(id),
    dispatched_at timestamptz NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE adaptation_observations (
    run_id text NOT NULL REFERENCES adaptation_runs(id),
    kind text NOT NULL CHECK (kind IN ('provider_result','interrupted')),
    observation jsonb NOT NULL CHECK (jsonb_typeof(observation) = 'object' AND octet_length(observation::text) <= 65536),
    recorded_at timestamptz NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (run_id,kind)
);
CREATE TABLE adaptation_proposals (
    run_id text PRIMARY KEY REFERENCES adaptation_runs(id),
    proposal jsonb NOT NULL CHECK (jsonb_typeof(proposal) = 'object' AND octet_length(proposal::text) <= 4194304),
    digest text NOT NULL CHECK (digest ~ '^[a-f0-9]{64}$')
);
CREATE TABLE adaptation_cancellations (
    owner_id text NOT NULL REFERENCES actors(id),
    operation_id text NOT NULL CHECK (operation_id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'),
    run_id text NOT NULL REFERENCES adaptation_runs(id),
    recorded_at timestamptz NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (owner_id,operation_id)
);
CREATE TABLE adaptation_acceptances (
    run_id text PRIMARY KEY REFERENCES adaptation_runs(id),
    owner_id text NOT NULL REFERENCES actors(id),
    operation_id text NOT NULL CHECK (operation_id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'),
    request jsonb NOT NULL CHECK (jsonb_typeof(request) = 'object' AND octet_length(request::text) <= 4194304),
    script_id text NOT NULL CHECK (script_id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'),
    revision bigint NOT NULL,
    UNIQUE(owner_id,operation_id),
    FOREIGN KEY(script_id,revision) REFERENCES script_revisions(script_id,revision)
);
CREATE FUNCTION guard_adaptation_run() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP <> 'UPDATE' THEN
        RAISE EXCEPTION 'adaptation records cannot be removed' USING ERRCODE = '55000';
    END IF;
    IF NEW.id IS DISTINCT FROM OLD.id OR NEW.owner_id IS DISTINCT FROM OLD.owner_id
       OR NEW.operation_id IS DISTINCT FROM OLD.operation_id OR NEW.source_id IS DISTINCT FROM OLD.source_id
       OR NEW.script_id IS DISTINCT FROM OLD.script_id OR NEW.frozen_input IS DISTINCT FROM OLD.frozen_input
       OR NEW.input_digest IS DISTINCT FROM OLD.input_digest OR NEW.recorded_at IS DISTINCT FROM OLD.recorded_at THEN
        RAISE EXCEPTION 'adaptation input is immutable' USING ERRCODE = '55000';
    END IF;
    IF OLD.status NOT IN ('queued','running') AND NEW.problem IS DISTINCT FROM OLD.problem THEN
        RAISE EXCEPTION 'settled adaptation problem is immutable' USING ERRCODE = '55000';
    END IF;
    IF NOT ((OLD.status = 'queued' AND NEW.status IN ('running','failed','cancelled'))
        OR (OLD.status = 'running' AND NEW.status IN ('succeeded','invalid_output','failed','ambiguous','cancelled'))
        OR (OLD.status = 'succeeded' AND NEW.status IN ('accepted','cancelled'))
        OR (OLD.status IN ('failed','invalid_output','ambiguous') AND NEW.status = 'cancelled')) THEN
        RAISE EXCEPTION 'invalid adaptation transition' USING ERRCODE = '55000';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER adaptation_run_guard BEFORE UPDATE OR DELETE ON adaptation_runs
    FOR EACH ROW EXECUTE FUNCTION guard_adaptation_run();
CREATE TRIGGER adaptation_run_no_truncate BEFORE TRUNCATE ON adaptation_runs
    FOR EACH STATEMENT EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER adaptation_attempt_immutable BEFORE UPDATE OR DELETE ON adaptation_attempts
    FOR EACH ROW EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER adaptation_attempt_no_truncate BEFORE TRUNCATE ON adaptation_attempts
    FOR EACH STATEMENT EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER adaptation_observation_immutable BEFORE UPDATE OR DELETE ON adaptation_observations
    FOR EACH ROW EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER adaptation_observation_no_truncate BEFORE TRUNCATE ON adaptation_observations
    FOR EACH STATEMENT EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER adaptation_proposal_immutable BEFORE UPDATE OR DELETE ON adaptation_proposals
    FOR EACH ROW EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER adaptation_proposal_no_truncate BEFORE TRUNCATE ON adaptation_proposals
    FOR EACH STATEMENT EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER adaptation_cancel_immutable BEFORE UPDATE OR DELETE ON adaptation_cancellations
    FOR EACH ROW EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER adaptation_cancel_no_truncate BEFORE TRUNCATE ON adaptation_cancellations
    FOR EACH STATEMENT EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER adaptation_acceptance_immutable BEFORE UPDATE OR DELETE ON adaptation_acceptances
    FOR EACH ROW EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER adaptation_acceptance_no_truncate BEFORE TRUNCATE ON adaptation_acceptances
    FOR EACH STATEMENT EXECUTE FUNCTION reject_settled_change();
REVOKE ALL ON adaptation_runs,adaptation_attempts,adaptation_observations,adaptation_proposals,adaptation_cancellations,adaptation_acceptances FROM PUBLIC;
REVOKE ALL ON FUNCTION guard_adaptation_run() FROM PUBLIC;
