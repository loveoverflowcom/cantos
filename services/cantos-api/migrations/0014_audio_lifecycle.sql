-- Immutable Content/Brief-bound private audio. No release pointer or listener projection.
CREATE TABLE audio_submissions (
    id text PRIMARY KEY CHECK(id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'),
    unit_id text NOT NULL,
    owner_id text NOT NULL,
    brief_id text NOT NULL,
    brief_digest text NOT NULL,
    FOREIGN KEY(brief_id,unit_id,owner_id,brief_digest) REFERENCES production_briefs(id,unit_id,owner_id,digest),
    operation_id text NOT NULL CHECK(operation_id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'),
    request_digest text NOT NULL CHECK(request_digest ~ '^as-request1:sha256:[a-f0-9]{64}$'),
    token_hash bytea NOT NULL UNIQUE CHECK(octet_length(token_hash)=32),
    snapshot bytea NOT NULL CHECK(octet_length(snapshot) BETWEEN 1 AND 32768),
    digest text NOT NULL CHECK(digest ~ '^as-r1:sha256:[a-f0-9]{64}$'),
    UNIQUE(owner_id,operation_id), UNIQUE(id,owner_id)
);
CREATE TABLE audio_submission_revocations (
    submission_id text PRIMARY KEY CHECK(id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'),
    owner_id text NOT NULL,
    FOREIGN KEY(submission_id,owner_id) REFERENCES audio_submissions(id,owner_id),
    revoked_at bigint NOT NULL CHECK(revoked_at>=0)
);
CREATE TABLE audio_renditions (
    id text PRIMARY KEY CHECK(id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'),
    submission_id text NOT NULL UNIQUE REFERENCES audio_submissions(id),
    object_key text NOT NULL CHECK(object_key ~ '^[a-f0-9]{64}$'),
    snapshot bytea NOT NULL CHECK(octet_length(snapshot) BETWEEN 1 AND 32768),
    digest text NOT NULL CHECK(digest ~ '^ar-r1:sha256:[a-f0-9]{64}$')
);
CREATE TABLE audio_reviews (
    id text PRIMARY KEY CHECK(id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'),
    rendition_id text NOT NULL REFERENCES audio_renditions(id),
    owner_id text NOT NULL REFERENCES actors(id),
    operation_id text NOT NULL CHECK(operation_id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'),
    sequence bigint NOT NULL CHECK(sequence>0),
    snapshot bytea NOT NULL CHECK(octet_length(snapshot) BETWEEN 1 AND 32768),
    digest text NOT NULL CHECK(digest ~ '^aq-r1:sha256:[a-f0-9]{64}$'),
    UNIQUE(rendition_id,sequence), UNIQUE(owner_id,operation_id)
);
CREATE TABLE audio_release_candidates (
    id text PRIMARY KEY CHECK(id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'),
    rendition_id text NOT NULL REFERENCES audio_renditions(id),
    owner_id text NOT NULL REFERENCES actors(id),
    operation_id text NOT NULL CHECK(operation_id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'),
    snapshot bytea NOT NULL CHECK(octet_length(snapshot) BETWEEN 1 AND 32768),
    digest text NOT NULL CHECK(digest ~ '^rc-r1:sha256:[a-f0-9]{64}$'),
    UNIQUE(owner_id,operation_id)
);
DO $$
DECLARE name text;
BEGIN
    FOREACH name IN ARRAY ARRAY['audio_submissions','audio_submission_revocations','audio_renditions','audio_reviews','audio_release_candidates'] LOOP
        EXECUTE format('CREATE TRIGGER immutable BEFORE UPDATE OR DELETE ON %I FOR EACH ROW EXECUTE FUNCTION reject_settled_change()',name);
        EXECUTE format('CREATE TRIGGER no_truncate BEFORE TRUNCATE ON %I FOR EACH STATEMENT EXECUTE FUNCTION reject_settled_change()',name);
        EXECUTE format('REVOKE ALL ON %I FROM PUBLIC',name);
    END LOOP;
END $$;
