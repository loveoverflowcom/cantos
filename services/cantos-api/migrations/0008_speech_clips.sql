-- Verified speech-clip registry and attempt-less reuse selection, on the existing PostgreSQL
-- authority. A row is a registration of an accepted attempt's audited output, not publication
-- approval, and cross-run reuse still needs a fresh object probe, scope and rights standing
-- (see packages/contracts/clip-reuse-v1.md). Media remain private filesystem records.
CREATE TABLE speech_clips (
    id text PRIMARY KEY,
    registration_seq bigint GENERATED ALWAYS AS IDENTITY UNIQUE,
    owner_id text NOT NULL REFERENCES actors(id),
    speech_fingerprint text NOT NULL
        CHECK (speech_fingerprint ~ '^speech-v1:sha256:[a-f0-9]{64}$'),
    -- The canonical effective-input document, stored so an audit can name the changed field.
    fingerprint_input bytea NOT NULL CHECK (octet_length(fingerprint_input) BETWEEN 2 AND 2097152),
    producer text NOT NULL CHECK (producer IN ('local_espeak', 'synthetic_fake')),
    voice_rights_record_id text NOT NULL CHECK (octet_length(voice_rights_record_id) BETWEEN 1 AND 256),
    output_contract text NOT NULL CHECK (octet_length(output_contract) BETWEEN 1 AND 256),
    audio_sha256 text NOT NULL CHECK (audio_sha256 ~ '^[a-f0-9]{64}$'),
    audio_bytes bigint NOT NULL CHECK (audio_bytes > 0),
    output_path text NOT NULL CHECK (octet_length(output_path) BETWEEN 1 AND 4096),
    attempt_id text NOT NULL UNIQUE REFERENCES durable_attempts(id),
    registered_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    -- The stored document must hash to the stored fingerprint; no application bug can pair them
    -- wrongly. The bytea literal is "cantos/speech-fingerprint/v1" followed by a newline.
    CHECK (speech_fingerprint = 'speech-v1:sha256:' || encode(sha256(
        '\x63616e746f732f7370656563682d66696e6765727072696e742f76310a'::bytea || fingerprint_input
    ), 'hex'))
);
CREATE INDEX speech_clips_lookup ON speech_clips(owner_id, speech_fingerprint, registration_seq);

-- A registration must restate the accepted observation of one attempt in one run owned by the
-- same creator with the same producer, and it must be the clip of that attempt's own step: the
-- step's recorded fingerprint, and the frozen line's voice-rights record and output contract.
-- It cannot be forged from a failed, unaccepted or another owner's attempt, cannot disagree with
-- the audited receipt, and cannot claim another line's identity or rights scope.
CREATE FUNCTION speech_clip_scope() RETURNS trigger LANGUAGE plpgsql
    SET search_path = pg_catalog AS $$
DECLARE
    receipt jsonb;
    frozen jsonb;
    step_fingerprint text;
    step_dialogue text;
BEGIN
    SELECT convert_from(o.receipt_bytes, 'UTF8')::jsonb,
           convert_from(p.document_bytes, 'UTF8')::jsonb,
           s.speech_fingerprint,
           s.dialogue_id
    INTO receipt, frozen, step_fingerprint, step_dialogue
    FROM public.durable_attempts a
    JOIN public.durable_steps s ON s.id = a.step_id AND s.accepted_attempt_id = a.id
    JOIN public.durable_jobs j ON j.id = s.job_id
    JOIN public.durable_runs r ON r.id = j.run_id
    JOIN public.production_snapshots p ON p.id = r.snapshot_id
    JOIN public.durable_observations o ON o.attempt_id = a.id
    WHERE a.id = NEW.attempt_id
      AND r.created_by = NEW.owner_id
      AND r.mode = NEW.producer
      AND o.output_path = NEW.output_path;
    IF receipt IS NULL
        OR receipt #>> '{outcome,status}' IS DISTINCT FROM 'output'
        OR receipt #>> '{outcome,audio_sha256}' IS DISTINCT FROM NEW.audio_sha256 THEN
        RAISE EXCEPTION 'speech clip is not the accepted observation of its attempt'
            USING ERRCODE = '23503';
    END IF;
    IF step_fingerprint IS DISTINCT FROM NEW.speech_fingerprint THEN
        RAISE EXCEPTION 'speech clip fingerprint is not its step fingerprint'
            USING ERRCODE = '23503';
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM jsonb_array_elements(frozen -> 'resolved') line
        WHERE line ->> 'dialogue_id' = step_dialogue
          AND line ->> 'voice_rights_record_id' = NEW.voice_rights_record_id
          AND line ->> 'output_contract' = NEW.output_contract
    ) THEN
        RAISE EXCEPTION 'speech clip rights scope or contract is not its frozen line'
            USING ERRCODE = '23503';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER speech_clips_scope BEFORE INSERT ON speech_clips
    FOR EACH ROW EXECUTE FUNCTION speech_clip_scope();
CREATE TRIGGER speech_clips_immutable BEFORE UPDATE OR DELETE ON speech_clips
    FOR EACH ROW EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER speech_clips_no_truncate BEFORE TRUNCATE ON speech_clips
    FOR EACH STATEMENT EXECUTE FUNCTION reject_settled_change();

-- Steps carry the position-free identity (NULL for steps created before this migration, which
-- therefore never reuse) and, when satisfied by a verified clip instead of a provider attempt,
-- a write-once reference to it. Reuse creates no attempt, reservation or dispatch row.
ALTER TABLE durable_steps
    ADD COLUMN speech_fingerprint text
        CHECK (speech_fingerprint ~ '^speech-v1:sha256:[a-f0-9]{64}$'),
    ADD COLUMN reused_clip_id text REFERENCES speech_clips(id);

DO $$
DECLARE
    old_check text;
BEGIN
    SELECT c.conname INTO old_check
    FROM pg_constraint c
    WHERE c.conrelid = 'public.durable_steps'::regclass
      AND c.contype = 'c'
      AND pg_get_constraintdef(c.oid) LIKE '%succeeded%accepted_attempt_id%';
    IF old_check IS NULL THEN
        RAISE EXCEPTION 'durable_steps success constraint not found';
    END IF;
    EXECUTE format('ALTER TABLE public.durable_steps DROP CONSTRAINT %I', old_check);
END;
$$;
ALTER TABLE durable_steps ADD CONSTRAINT durable_step_success_source CHECK (
    (status = 'succeeded') = (accepted_attempt_id IS NOT NULL OR reused_clip_id IS NOT NULL)
    AND (accepted_attempt_id IS NULL OR reused_clip_id IS NULL)
);

CREATE OR REPLACE FUNCTION durable_step_transition() RETURNS trigger LANGUAGE plpgsql
    SET search_path = pg_catalog AS $$
BEGIN
    IF NEW.id IS DISTINCT FROM OLD.id OR NEW.job_id IS DISTINCT FROM OLD.job_id
        OR NEW.dialogue_id IS DISTINCT FROM OLD.dialogue_id
        OR NEW.input_fingerprint IS DISTINCT FROM OLD.input_fingerprint
        OR NEW.speech_fingerprint IS DISTINCT FROM OLD.speech_fingerprint THEN
        RAISE EXCEPTION 'durable step identity is immutable' USING ERRCODE = '55000';
    END IF;
    IF NEW.lease_token < OLD.lease_token THEN
        RAISE EXCEPTION 'durable lease token cannot decrease' USING ERRCODE = '23514';
    END IF;
    IF OLD.accepted_attempt_id IS NOT NULL
        AND NEW.accepted_attempt_id IS DISTINCT FROM OLD.accepted_attempt_id THEN
        RAISE EXCEPTION 'durable accepted result is immutable' USING ERRCODE = '55000';
    END IF;
    IF OLD.reused_clip_id IS NOT NULL
        AND NEW.reused_clip_id IS DISTINCT FROM OLD.reused_clip_id THEN
        RAISE EXCEPTION 'durable reused clip is immutable' USING ERRCODE = '55000';
    END IF;
    IF OLD.reused_clip_id IS NULL AND NEW.reused_clip_id IS NOT NULL THEN
        -- A verified clip may satisfy a step only for the same fingerprint, owner and producer,
        -- and only before any provider attempt exists for that step.
        IF NEW.speech_fingerprint IS NULL OR NOT EXISTS (
            SELECT 1
            FROM public.speech_clips c
            JOIN public.durable_jobs j ON j.id = NEW.job_id
            JOIN public.durable_runs r ON r.id = j.run_id
            WHERE c.id = NEW.reused_clip_id
              AND c.speech_fingerprint = NEW.speech_fingerprint
              AND c.owner_id = r.created_by
              AND c.producer = r.mode
        ) OR EXISTS (
            SELECT 1 FROM public.durable_attempts a WHERE a.step_id = NEW.id
        ) THEN
            RAISE EXCEPTION 'durable step reuse scope mismatch' USING ERRCODE = '23503';
        END IF;
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

-- No provider attempt or reservation may be recorded for a step that a verified clip already
-- satisfies. The step row is read under FOR SHARE, which conflicts with the update that sets
-- reused_clip_id, so a concurrent writer that bypasses the application's job lock still
-- serializes with the reuse: whichever commits second sees the other.
CREATE FUNCTION durable_step_not_reused() RETURNS trigger LANGUAGE plpgsql
    SET search_path = pg_catalog AS $$
DECLARE
    adopted text;
BEGIN
    SELECT s.reused_clip_id INTO adopted
    FROM public.durable_steps s WHERE s.id = NEW.step_id FOR SHARE;
    IF adopted IS NOT NULL THEN
        RAISE EXCEPTION 'durable step is already satisfied by a reused clip'
            USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER durable_attempts_not_reused BEFORE INSERT ON durable_attempts
    FOR EACH ROW EXECUTE FUNCTION durable_step_not_reused();
CREATE TRIGGER durable_reservations_not_reused BEFORE INSERT ON durable_reservations
    FOR EACH ROW EXECUTE FUNCTION durable_step_not_reused();

REVOKE ALL ON speech_clips FROM PUBLIC;
REVOKE ALL ON FUNCTION speech_clip_scope(), durable_step_not_reused() FROM PUBLIC;
-- Migration also works before deployment provisions its restricted application role.
DO $$ BEGIN
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname='cantos_app') THEN
        GRANT SELECT,INSERT ON speech_clips TO cantos_app;
    END IF;
END; $$;
