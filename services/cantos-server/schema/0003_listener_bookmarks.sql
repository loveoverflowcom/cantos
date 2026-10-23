-- Additive epoch-2 expansion (#299): release-bound listener bookmarks. Baseline and bearer
-- checksums, existing listener state and every other table are untouched. A bookmark is an
-- immutable (id, exact audio identity, position) value; its only change is the final tombstone.
CREATE TABLE public.listener_bookmarks (
    listener_id text NOT NULL REFERENCES public.listener_state(listener_id),
    bookmark_id text NOT NULL CHECK (bookmark_id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'),
    publication_id text NOT NULL REFERENCES public.audio_releases(id),
    work_id text NOT NULL REFERENCES public.catalog_works(id),
    unit_id text NOT NULL REFERENCES public.catalog_units(id),
    rendition_id text NOT NULL REFERENCES public.audio_renditions(id),
    sha256 text NOT NULL CHECK (sha256 ~ '^[a-f0-9]{64}$'),
    duration_ms bigint NOT NULL CHECK (duration_ms > 0),
    position_ms bigint NOT NULL,
    deleted boolean NOT NULL,
    listener_order bigint NOT NULL CHECK (listener_order > 0),
    accepted_at bigint NOT NULL CHECK (accepted_at >= 0),
    snapshot bytea NOT NULL CHECK (octet_length(snapshot) BETWEEN 1 AND 4096),
    snapshot_digest bytea NOT NULL CHECK (octet_length(snapshot_digest) = 32),
    PRIMARY KEY (listener_id, bookmark_id),
    CHECK (position_ms >= 0 AND position_ms <= duration_ms),
    CHECK ((convert_from(snapshot, 'UTF8'::name))::jsonb @> jsonb_build_object(
        'bookmark_id', bookmark_id,
        'audio', jsonb_build_object('work_id', work_id, 'unit_id', unit_id,
            'publication_id', publication_id, 'rendition_id', rendition_id,
            'sha256', sha256, 'duration_ms', duration_ms),
        'position_ms', position_ms, 'deleted', deleted,
        'listener_order', listener_order, 'accepted_at_ms', accepted_at))
);

-- The listing walks live bookmarks newest first per account; tombstones never enter the index.
CREATE INDEX listener_bookmarks_recent ON public.listener_bookmarks
    USING btree (listener_id, listener_order DESC, bookmark_id COLLATE "C") WHERE NOT deleted;

CREATE FUNCTION public.guard_listener_bookmark() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    IF (NEW.listener_id,NEW.bookmark_id,NEW.publication_id,NEW.work_id,NEW.unit_id,
        NEW.rendition_id,NEW.sha256,NEW.duration_ms,NEW.position_ms)
       IS DISTINCT FROM (OLD.listener_id,OLD.bookmark_id,OLD.publication_id,OLD.work_id,OLD.unit_id,
        OLD.rendition_id,OLD.sha256,OLD.duration_ms,OLD.position_ms)
       OR OLD.deleted OR NOT NEW.deleted OR NEW.listener_order<=OLD.listener_order THEN
        RAISE EXCEPTION 'listener bookmark value is immutable and its tombstone is final' USING ERRCODE='55000';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER listener_bookmarks_guard BEFORE UPDATE ON public.listener_bookmarks
    FOR EACH ROW EXECUTE FUNCTION public.guard_listener_bookmark();
CREATE TRIGGER listener_bookmarks_no_delete BEFORE DELETE ON public.listener_bookmarks
    FOR EACH ROW EXECUTE FUNCTION public.reject_settled_change();
CREATE TRIGGER listener_bookmarks_no_truncate BEFORE TRUNCATE ON public.listener_bookmarks
    FOR EACH STATEMENT EXECUTE FUNCTION public.reject_settled_change();

-- As in the baseline, nothing here is executable or readable through PUBLIC; the operator-applied
-- runtime grants (infra/vps/postgres/runtime-grants.sql) are the only authority.
REVOKE ALL ON public.listener_bookmarks FROM PUBLIC;
REVOKE ALL ON FUNCTION public.guard_listener_bookmark() FROM PUBLIC;
