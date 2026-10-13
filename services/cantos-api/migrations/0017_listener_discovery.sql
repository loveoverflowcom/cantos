-- Narrow immutable read indexes. No private asset is promoted and no listener identity is created.
CREATE TABLE audio_listener_works (
    work_id text PRIMARY KEY REFERENCES catalog_works(id),
    first_release_id text NOT NULL REFERENCES audio_releases(id),
    first_published_at bigint NOT NULL CHECK(first_published_at>=0)
);
CREATE INDEX audio_listener_works_recent ON audio_listener_works(first_published_at DESC,work_id ASC);
CREATE TABLE audio_listener_units (
    unit_id text PRIMARY KEY REFERENCES catalog_units(id),
    work_id text NOT NULL REFERENCES catalog_works(id),
    first_position integer NOT NULL CHECK(first_position>=0),
    UNIQUE(unit_id,work_id)
);
CREATE INDEX audio_listener_units_order ON audio_listener_units(work_id,first_position,unit_id);
CREATE TABLE audio_listener_releases (
    release_id text PRIMARY KEY REFERENCES audio_releases(id),
    work_id text NOT NULL REFERENCES catalog_works(id),
    unit_id text NOT NULL,
    FOREIGN KEY(unit_id,work_id) REFERENCES audio_listener_units(unit_id,work_id),
    published_at bigint NOT NULL CHECK(published_at>=0),
    kind text NOT NULL CHECK(kind IN ('novel','story','podcast')),
    language text NOT NULL CHECK(octet_length(language) BETWEEN 1 AND 63),
    search_text text NOT NULL CHECK(octet_length(search_text)<=32768),
    rights_digest text NOT NULL CHECK(rights_digest ~ '^dc-r1:sha256:[a-f0-9]{64}$'),
    approval_digest text NOT NULL CHECK(approval_digest ~ '^pa-r1:sha256:[a-f0-9]{64}$'),
    rights_start bigint NOT NULL CHECK(rights_start>=0),
    rights_end bigint NOT NULL CHECK(rights_end>rights_start),
    work_revision bigint NOT NULL CHECK(work_revision>0),
    unit_revision bigint NOT NULL CHECK(unit_revision>0),
    content_revision_id text NOT NULL REFERENCES catalog_content_revisions(id)
);
CREATE INDEX audio_listener_releases_work ON audio_listener_releases(work_id,published_at DESC,release_id);
CREATE INDEX audio_listener_releases_unit ON audio_listener_releases(unit_id,release_id);
DO $$ DECLARE name text; BEGIN
    FOREACH name IN ARRAY ARRAY['audio_listener_works','audio_listener_units','audio_listener_releases'] LOOP
        EXECUTE format('CREATE TRIGGER immutable BEFORE UPDATE OR DELETE ON %I FOR EACH ROW EXECUTE FUNCTION reject_settled_change()',name);
        EXECUTE format('CREATE TRIGGER no_truncate BEFORE TRUNCATE ON %I FOR EACH STATEMENT EXECUTE FUNCTION reject_settled_change()',name);
        EXECUTE format('REVOKE ALL ON %I FROM PUBLIC',name);
    END LOOP;
END $$;
-- Existing releases stay fail-closed until a separately authorized verified backfill. Do not
-- synthesize publication-class delivery or reinterpret private visibility from newer drafts.

-- Catalog-owned read facade. This is a prefilter only; Rust re-admits snapshots and evaluates
-- the complete Media eligibility witness. SECURITY INVOKER preserves existing least privilege.
CREATE FUNCTION catalog_listener_discoverable(work text,unit text) RETURNS boolean
LANGUAGE sql STABLE SET search_path=pg_catalog AS $$
    SELECT EXISTS(SELECT 1 FROM public.catalog_works w
      JOIN public.catalog_units u ON u.work_id=w.id AND u.id=unit
      JOIN public.catalog_work_revisions r ON r.work_id=w.id AND r.revision=w.head_revision
      WHERE w.id=work AND w.lifecycle<>'archived' AND u.lifecycle<>'archived'
      AND convert_from(r.snapshot,'UTF8')::jsonb->'details'->>'visibility'='public')
$$;
-- Platform-owned actor fact, intentionally separate from Studio sessions.
CREATE FUNCTION listener_actor_active(actor text) RETURNS boolean
LANGUAGE sql STABLE SET search_path=pg_catalog AS $$
    SELECT EXISTS(SELECT 1 FROM public.actors a WHERE a.id=actor AND a.active)
$$;
REVOKE ALL ON FUNCTION catalog_listener_discoverable(text,text),listener_actor_active(text) FROM PUBLIC;
