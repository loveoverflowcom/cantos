-- Listener account state only. 0018 belongs to identity mapping; no new registry or credential.
CREATE TABLE listener_state (
    listener_id text PRIMARY KEY REFERENCES actors(id),
    revision bigint NOT NULL DEFAULT 0 CHECK(revision>=0),
    history_revision bigint NOT NULL DEFAULT 0 CHECK(history_revision>=0),
    history_epoch bigint NOT NULL DEFAULT 0 CHECK(history_epoch>=0 AND history_epoch<=history_revision)
);
CREATE TABLE listener_saved (
    listener_id text NOT NULL REFERENCES listener_state(listener_id),
    work_id text NOT NULL REFERENCES catalog_works(id),
    saved boolean NOT NULL,
    revision bigint NOT NULL CHECK(revision>0),
    listener_order bigint NOT NULL CHECK(listener_order>0),
    accepted_at bigint NOT NULL CHECK(accepted_at>=0),
    snapshot bytea NOT NULL CHECK(octet_length(snapshot) BETWEEN 1 AND 4096),
    snapshot_digest bytea NOT NULL CHECK(octet_length(snapshot_digest)=32),
    CHECK(convert_from(snapshot,'UTF8')::jsonb @> jsonb_build_object('work_id',work_id,'saved',saved,'revision',revision,'listener_order',listener_order,'accepted_at_ms',accepted_at)),
    PRIMARY KEY(listener_id,work_id)
);
CREATE INDEX listener_saved_recent ON listener_saved(listener_id,listener_order DESC,work_id COLLATE "C") WHERE saved;
CREATE TABLE listener_progress (
    listener_id text NOT NULL REFERENCES listener_state(listener_id),
    publication_id text NOT NULL REFERENCES audio_releases(id),
    work_id text NOT NULL REFERENCES catalog_works(id),
    unit_id text NOT NULL REFERENCES catalog_units(id),
    rendition_id text NOT NULL REFERENCES audio_renditions(id),
    sha256 text NOT NULL CHECK(sha256 ~ '^[a-f0-9]{64}$'),
    duration_ms bigint NOT NULL CHECK(duration_ms>0),
    position_ms bigint NOT NULL CHECK(position_ms>=0 AND position_ms<=duration_ms),
    completion text NOT NULL CHECK(completion IN ('in_progress','completed')),
    revision bigint NOT NULL CHECK(revision>0),
    listener_order bigint NOT NULL CHECK(listener_order>0),
    accepted_at bigint NOT NULL CHECK(accepted_at>=0),
    snapshot bytea NOT NULL CHECK(octet_length(snapshot) BETWEEN 1 AND 8192),
    snapshot_digest bytea NOT NULL CHECK(octet_length(snapshot_digest)=32),
    CHECK(convert_from(snapshot,'UTF8')::jsonb @> jsonb_build_object('audio',jsonb_build_object('work_id',work_id,'unit_id',unit_id,'publication_id',publication_id,'rendition_id',rendition_id,'sha256',sha256,'duration_ms',duration_ms),'position_ms',position_ms,'completion',completion,'revision',revision,'listener_order',listener_order,'accepted_at_ms',accepted_at)),
    PRIMARY KEY(listener_id,publication_id)
);
CREATE INDEX listener_progress_work_recent ON listener_progress(listener_id,work_id COLLATE "C",listener_order DESC,publication_id COLLATE "C");
CREATE INDEX listener_progress_recent ON listener_progress(listener_id,listener_order DESC,work_id COLLATE "C");
CREATE TABLE listener_history (
    listener_id text NOT NULL REFERENCES listener_state(listener_id),
    work_id text NOT NULL REFERENCES catalog_works(id),
    publication_id text NOT NULL REFERENCES audio_releases(id),
    unit_id text NOT NULL REFERENCES catalog_units(id),
    listener_order bigint NOT NULL CHECK(listener_order>0),
    started_at bigint NOT NULL CHECK(started_at>=0),
    snapshot bytea NOT NULL CHECK(octet_length(snapshot) BETWEEN 1 AND 4096),
    snapshot_digest bytea NOT NULL CHECK(octet_length(snapshot_digest)=32),
    PRIMARY KEY(listener_id,work_id)
);
CREATE INDEX listener_history_recent ON listener_history(listener_id,listener_order DESC,work_id COLLATE "C");
-- Session tombstones and receipts deliberately survive clear. No automatic retention/GC policy.
CREATE TABLE listener_playback_sessions (
    listener_id text NOT NULL REFERENCES listener_state(listener_id),
    device_id text NOT NULL CHECK(device_id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'),
    engine_session_id text NOT NULL CHECK(engine_session_id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'),
    audio_snapshot bytea NOT NULL CHECK(octet_length(audio_snapshot) BETWEEN 1 AND 4096),
    PRIMARY KEY(listener_id,device_id,engine_session_id)
);
CREATE TABLE listener_device_sequences (
    listener_id text NOT NULL REFERENCES listener_state(listener_id),
    device_id text NOT NULL CHECK(device_id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'),
    resource_key text NOT NULL CHECK(octet_length(resource_key) BETWEEN 1 AND 128),
    sequence bigint NOT NULL CHECK(sequence>0),
    PRIMARY KEY(listener_id,device_id,resource_key)
);
CREATE TABLE listener_operations (
    listener_id text NOT NULL REFERENCES listener_state(listener_id),
    operation_id text NOT NULL CHECK(operation_id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'),
    request_digest bytea NOT NULL CHECK(octet_length(request_digest)=32),
    response_bytes bytea NOT NULL CHECK(octet_length(response_bytes) BETWEEN 1 AND 16384),
    response_digest bytea NOT NULL CHECK(octet_length(response_digest)=32),
    PRIMARY KEY(listener_id,operation_id)
);
CREATE TRIGGER listener_operations_immutable BEFORE UPDATE OR DELETE ON listener_operations FOR EACH ROW EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER listener_operations_no_truncate BEFORE TRUNCATE ON listener_operations FOR EACH STATEMENT EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER listener_sessions_immutable BEFORE UPDATE OR DELETE ON listener_playback_sessions FOR EACH ROW EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER listener_sessions_no_truncate BEFORE TRUNCATE ON listener_playback_sessions FOR EACH STATEMENT EXECUTE FUNCTION reject_settled_change();
CREATE FUNCTION guard_listener_progress() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF (NEW.listener_id,NEW.publication_id,NEW.work_id,NEW.unit_id,NEW.rendition_id,NEW.sha256,NEW.duration_ms)
       IS DISTINCT FROM (OLD.listener_id,OLD.publication_id,OLD.work_id,OLD.unit_id,OLD.rendition_id,OLD.sha256,OLD.duration_ms)
       OR NEW.revision<>OLD.revision+1 OR NEW.listener_order<=OLD.listener_order THEN
        RAISE EXCEPTION 'listener progress identity/revision mismatch' USING ERRCODE='55000';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER listener_progress_guard BEFORE UPDATE ON listener_progress FOR EACH ROW EXECUTE FUNCTION guard_listener_progress();
CREATE FUNCTION listener_lock_actor(actor text) RETURNS boolean LANGUAGE sql SECURITY DEFINER SET search_path=pg_catalog AS $$ SELECT active FROM public.actors WHERE id=actor FOR SHARE $$;
REVOKE ALL ON listener_state,listener_saved,listener_progress,listener_history,listener_playback_sessions,listener_device_sequences,listener_operations FROM PUBLIC;
REVOKE ALL ON FUNCTION guard_listener_progress(),listener_lock_actor(text) FROM PUBLIC;

CREATE FUNCTION guard_listener_saved() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF (NEW.listener_id,NEW.work_id) IS DISTINCT FROM (OLD.listener_id,OLD.work_id)
       OR NEW.revision<>OLD.revision+1 OR NEW.listener_order<=OLD.listener_order THEN
        RAISE EXCEPTION 'listener saved identity/revision mismatch' USING ERRCODE='55000';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER listener_saved_guard BEFORE UPDATE ON listener_saved FOR EACH ROW EXECUTE FUNCTION guard_listener_saved();
REVOKE ALL ON FUNCTION guard_listener_saved() FROM PUBLIC;
