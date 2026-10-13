-- Publication is metadata authority only. Private intake objects never become public here.
-- 0015 is reserved independently for provider-neutral private storage targets.
CREATE TABLE audio_distribution_clearances (
    id text PRIMARY KEY CHECK(id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'),
    candidate_id text NOT NULL REFERENCES audio_release_candidates(id),
    owner_id text NOT NULL REFERENCES actors(id),
    operation_id text NOT NULL CHECK(operation_id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'),
    sequence bigint NOT NULL CHECK(sequence>0),
    snapshot bytea NOT NULL CHECK(octet_length(snapshot) BETWEEN 1 AND 32768),
    digest text NOT NULL CHECK(digest ~ '^dc-r1:sha256:[a-f0-9]{64}$'),
    UNIQUE(candidate_id,sequence), UNIQUE(owner_id,operation_id)
);
CREATE TABLE audio_publication_approvals (
    id text PRIMARY KEY CHECK(id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'),
    candidate_id text NOT NULL REFERENCES audio_release_candidates(id),
    owner_id text NOT NULL REFERENCES actors(id),
    operation_id text NOT NULL CHECK(operation_id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'),
    sequence bigint NOT NULL CHECK(sequence>0),
    snapshot bytea NOT NULL CHECK(octet_length(snapshot) BETWEEN 1 AND 8192),
    digest text NOT NULL CHECK(digest ~ '^pa-r1:sha256:[a-f0-9]{64}$'),
    UNIQUE(candidate_id,sequence), UNIQUE(owner_id,operation_id)
);
CREATE TABLE audio_releases (
    id text PRIMARY KEY CHECK(id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'),
    unit_id text NOT NULL REFERENCES catalog_units(id),
    owner_id text NOT NULL REFERENCES actors(id),
    candidate_id text NOT NULL REFERENCES audio_release_candidates(id),
    rendition_id text NOT NULL REFERENCES audio_renditions(id),
    snapshot bytea NOT NULL CHECK(octet_length(snapshot) BETWEEN 1 AND 262144),
    manifest_digest text NOT NULL CHECK(manifest_digest ~ '^rm-r1:sha256:[a-f0-9]{64}$'),
    digest text NOT NULL CHECK(digest ~ '^rl-r1:sha256:[a-f0-9]{64}$'),
    UNIQUE(id,unit_id,owner_id)
);
CREATE TABLE audio_publication_pointers (
    unit_id text PRIMARY KEY REFERENCES catalog_units(id),
    owner_id text NOT NULL REFERENCES actors(id),
    active_release_id text,
    version bigint NOT NULL CHECK(version>=0),
    FOREIGN KEY(active_release_id,unit_id,owner_id) REFERENCES audio_releases(id,unit_id,owner_id)
);
CREATE TABLE audio_release_withdrawals (
    release_id text PRIMARY KEY REFERENCES audio_releases(id),
    actor_id text NOT NULL REFERENCES actors(id),
    recorded_at bigint NOT NULL CHECK(recorded_at>=0),
    reason text NOT NULL CHECK(octet_length(reason) BETWEEN 1 AND 4096)
);
CREATE TABLE audio_publication_operations (
    owner_id text NOT NULL REFERENCES actors(id),
    operation_id text NOT NULL CHECK(operation_id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'),
    kind text NOT NULL CHECK(kind IN ('publish','withdraw')),
    request_digest text NOT NULL CHECK(request_digest ~ '^publication-request1:sha256:[a-f0-9]{64}$'),
    unit_id text NOT NULL REFERENCES catalog_units(id),
    release_id text NOT NULL REFERENCES audio_releases(id),
    pointer_version bigint NOT NULL CHECK(pointer_version>=0),
    PRIMARY KEY(owner_id,operation_id,kind)
);
-- A concrete durable audit/change feed for publication and the next discovery consumer.
-- No cache/CDN side effect or speculative dispatch worker exists in this slice.
CREATE TABLE audio_publication_events (
    id text PRIMARY KEY CHECK(id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'),
    unit_id text NOT NULL REFERENCES catalog_units(id),
    pointer_version bigint NOT NULL CHECK(pointer_version>=0),
    kind text NOT NULL CHECK(kind IN ('activated','withdrawn')),
    release_id text NOT NULL REFERENCES audio_releases(id),
    actor_id text NOT NULL REFERENCES actors(id),
    recorded_at bigint NOT NULL CHECK(recorded_at>=0),
    UNIQUE(unit_id,pointer_version,kind,release_id)
);
DO $$ DECLARE name text; BEGIN
    FOREACH name IN ARRAY ARRAY['audio_distribution_clearances','audio_publication_approvals','audio_releases','audio_release_withdrawals','audio_publication_operations','audio_publication_events'] LOOP
        EXECUTE format('CREATE TRIGGER immutable BEFORE UPDATE OR DELETE ON %I FOR EACH ROW EXECUTE FUNCTION reject_settled_change()',name);
        EXECUTE format('CREATE TRIGGER no_truncate BEFORE TRUNCATE ON %I FOR EACH STATEMENT EXECUTE FUNCTION reject_settled_change()',name);
        EXECUTE format('REVOKE ALL ON %I FROM PUBLIC',name);
    END LOOP;
END $$;
REVOKE ALL ON audio_publication_pointers FROM PUBLIC;
