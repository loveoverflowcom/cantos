-- Preserve every a1 frozen value and digest. New caller contexts have their own explicit format.
ALTER TABLE adaptation_runs ADD COLUMN input_version text NOT NULL DEFAULT 'a1'
    CHECK (input_version IN ('a1','c1'));
ALTER TABLE adaptation_runs DROP CONSTRAINT adaptation_runs_status_check;
ALTER TABLE adaptation_runs ADD CONSTRAINT adaptation_runs_status_check
    CHECK (status IN ('awaiting_proposal','queued','running','succeeded','invalid_output','failed','ambiguous','cancelled','accepted'));
ALTER TABLE adaptation_runs ADD CONSTRAINT adaptation_workflow_status CHECK (
    (input_version='a1' AND status<>'awaiting_proposal') OR
    (input_version='c1' AND status IN ('awaiting_proposal','succeeded','cancelled','accepted') AND dispatch_deadline IS NULL)
);
CREATE OR REPLACE FUNCTION guard_adaptation_run() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP <> 'UPDATE' THEN
        RAISE EXCEPTION 'adaptation records cannot be removed' USING ERRCODE = '55000';
    END IF;
    IF NEW.id IS DISTINCT FROM OLD.id OR NEW.owner_id IS DISTINCT FROM OLD.owner_id
       OR NEW.operation_id IS DISTINCT FROM OLD.operation_id OR NEW.source_id IS DISTINCT FROM OLD.source_id
       OR NEW.script_id IS DISTINCT FROM OLD.script_id OR NEW.frozen_input IS DISTINCT FROM OLD.frozen_input
       OR NEW.input_digest IS DISTINCT FROM OLD.input_digest OR NEW.recorded_at IS DISTINCT FROM OLD.recorded_at
       OR NEW.input_version IS DISTINCT FROM OLD.input_version THEN
        RAISE EXCEPTION 'adaptation input is immutable' USING ERRCODE = '55000';
    END IF;
    IF OLD.status NOT IN ('queued','running') AND NEW.problem IS DISTINCT FROM OLD.problem THEN
        RAISE EXCEPTION 'settled adaptation problem is immutable' USING ERRCODE = '55000';
    END IF;
    IF OLD.input_version='c1' THEN
        IF NOT ((OLD.status='awaiting_proposal' AND NEW.status IN ('succeeded','cancelled'))
            OR (OLD.status='succeeded' AND NEW.status IN ('accepted','cancelled'))) THEN
            RAISE EXCEPTION 'invalid caller adaptation transition' USING ERRCODE = '55000';
        END IF;
    ELSIF NOT ((OLD.status='queued' AND NEW.status IN ('failed','cancelled'))
        OR (OLD.status='running' AND NEW.status IN ('ambiguous','cancelled'))
        OR (OLD.status='succeeded' AND NEW.status IN ('accepted','cancelled'))
        OR (OLD.status IN ('failed','invalid_output','ambiguous') AND NEW.status='cancelled')) THEN
        -- Historical runs may be read, reconciled as unknown, cancelled or explicitly accepted.
        -- No server generation can start or complete after retiring the inference path.
        RAISE EXCEPTION 'invalid historical adaptation transition' USING ERRCODE = '55000';
    END IF;
    RETURN NEW;
END;
$$;
ALTER TABLE adaptation_runs ADD CONSTRAINT adaptation_run_owner_identity UNIQUE(id,owner_id);
CREATE TABLE adaptation_submissions (
    id text PRIMARY KEY CHECK (id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'),
    run_id text NOT NULL REFERENCES adaptation_runs(id),
    owner_id text NOT NULL REFERENCES actors(id),
    operation_id text NOT NULL CHECK (operation_id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'),
    submission_number bigint NOT NULL CHECK (submission_number>0),
    request jsonb NOT NULL CHECK (jsonb_typeof(request)='object' AND octet_length(request::text)<=2097152),
    receipt jsonb NOT NULL CHECK (jsonb_typeof(receipt)='object' AND octet_length(receipt::text)<=4194304),
    receipt_digest text NOT NULL CHECK (receipt_digest ~ '^[a-f0-9]{64}$'),
    submitted_at timestamptz NOT NULL,
    UNIQUE(owner_id,operation_id),
    UNIQUE(run_id,submission_number),
    FOREIGN KEY(run_id,owner_id) REFERENCES adaptation_runs(id,owner_id)
);
CREATE TRIGGER adaptation_submission_immutable BEFORE UPDATE OR DELETE ON adaptation_submissions
    FOR EACH ROW EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER adaptation_submission_no_truncate BEFORE TRUNCATE ON adaptation_submissions
    FOR EACH STATEMENT EXECUTE FUNCTION reject_settled_change();
REVOKE ALL ON adaptation_submissions FROM PUBLIC;
