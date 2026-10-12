-- Committed change feed for the experimental incremental job-summary read model.
-- PostgreSQL stays the only authority: durable_steps is unchanged, these rows are derived in the
-- writing transaction and become visible exactly when it commits (a rollback leaves none).
--
-- Each row is one signed change of a step's projected facts (job, succeeded, reused): an insert
-- is +1, an update that changes a projected fact is -1 for the old facts and +1 for the new, a
-- delete is -1. Rows carry the writer's top-level transaction id, so a reader can take whole
-- transactions in xid8 order below its snapshot's xmin, where no running transaction can still
-- add a row; see services/cantos-api/src/incremental.rs for the committed-position contract.
CREATE TABLE durable_step_changes (
    txid xid8 NOT NULL DEFAULT pg_current_xact_id(),
    seq bigint GENERATED ALWAYS AS IDENTITY,
    job_id text NOT NULL,
    step_id text NOT NULL,
    weight smallint NOT NULL CHECK (weight IN (-1, 1)),
    succeeded boolean NOT NULL,
    reused boolean NOT NULL,
    PRIMARY KEY (txid, seq)
);
-- The per-request freshness fence asks whether a job has any change at or after a frontier.
CREATE INDEX durable_step_changes_by_job ON durable_step_changes(job_id, txid);

-- Runs with the definer's rights so the application role can only read the feed, never forge it.
CREATE FUNCTION durable_step_change_capture() RETURNS trigger LANGUAGE plpgsql
    SECURITY DEFINER SET search_path = pg_catalog AS $$
BEGIN
    IF TG_OP = 'UPDATE' THEN
        IF OLD.id = NEW.id AND OLD.job_id = NEW.job_id
            AND (OLD.status = 'succeeded') = (NEW.status = 'succeeded')
            AND (OLD.reused_clip_id IS NULL) = (NEW.reused_clip_id IS NULL) THEN
            -- Leases, heartbeats and other transitions do not change the projection.
            RETURN NULL;
        END IF;
    END IF;
    IF TG_OP IN ('UPDATE', 'DELETE') THEN
        INSERT INTO public.durable_step_changes(job_id, step_id, weight, succeeded, reused)
        VALUES (OLD.job_id, OLD.id, -1, OLD.status = 'succeeded', OLD.reused_clip_id IS NOT NULL);
    END IF;
    IF TG_OP IN ('INSERT', 'UPDATE') THEN
        INSERT INTO public.durable_step_changes(job_id, step_id, weight, succeeded, reused)
        VALUES (NEW.job_id, NEW.id, 1, NEW.status = 'succeeded', NEW.reused_clip_id IS NOT NULL);
    END IF;
    RETURN NULL;
END;
$$;
CREATE TRIGGER durable_steps_change_capture AFTER INSERT OR UPDATE OR DELETE ON durable_steps
    FOR EACH ROW EXECUTE FUNCTION durable_step_change_capture();

-- Captured changes are settled facts. Pruning is an operator-owned maintenance task that is not
-- implemented; the application role receives no DELETE or TRUNCATE right.
CREATE TRIGGER durable_step_changes_immutable BEFORE UPDATE ON durable_step_changes
    FOR EACH ROW EXECUTE FUNCTION reject_settled_change();

REVOKE ALL ON durable_step_changes FROM PUBLIC;
REVOKE ALL ON FUNCTION durable_step_change_capture() FROM PUBLIC;
-- Migration also works before deployment provisions its restricted application role.
DO $$ BEGIN
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname='cantos_app') THEN
        GRANT SELECT ON durable_step_changes TO cantos_app;
    END IF;
END; $$;
