-- Private Work artwork is an additive Media sidecar. Existing snapshots remain unchanged.
ALTER TABLE catalog_works ADD CONSTRAINT catalog_work_owner_pair UNIQUE(id,owner_id);

CREATE TABLE media_image_intents (
    id text PRIMARY KEY CHECK(id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'),
    work_id text NOT NULL,
    owner_id text NOT NULL,
    FOREIGN KEY(work_id,owner_id) REFERENCES catalog_works(id,owner_id),
    operation_id text NOT NULL CHECK(operation_id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'),
    expected_sha256 text NOT NULL CHECK(expected_sha256 ~ '^[a-f0-9]{64}$'),
    expected_byte_length bigint NOT NULL CHECK(expected_byte_length BETWEEN 1 AND 8388608),
    content_type text NOT NULL CHECK(content_type IN ('image/png','image/jpeg')),
    profile_id text NOT NULL CHECK(profile_id='cover-image-v1'),
    created_at_ms bigint NOT NULL CHECK(created_at_ms>=0),
    expires_at_ms bigint NOT NULL CHECK(expires_at_ms>=0 AND expires_at_ms-created_at_ms=1800000),
    document_bytes bytea NOT NULL CHECK(octet_length(document_bytes) BETWEEN 1 AND 32768),
    digest text NOT NULL CHECK(digest ~ '^ii-r1:sha256:[a-f0-9]{64}$'),
    UNIQUE(id,work_id,owner_id),
    UNIQUE(owner_id,operation_id)
);
CREATE TABLE media_image_assets (
    id text PRIMARY KEY CHECK(id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'),
    intent_id text NOT NULL UNIQUE,
    work_id text NOT NULL,
    owner_id text NOT NULL,
    FOREIGN KEY(intent_id,work_id,owner_id) REFERENCES media_image_intents(id,work_id,owner_id),
    CHECK(id=intent_id),
    storage_target text NOT NULL CHECK(storage_target ~ '^[a-z][a-z0-9-]{0,62}$' AND storage_target !~ '-$'),
    object_key text NOT NULL CHECK(object_key ~ '^[a-f0-9]{64}$'),
    sha256 text NOT NULL CHECK(sha256 ~ '^[a-f0-9]{64}$' AND object_key=sha256),
    byte_length bigint NOT NULL CHECK(byte_length BETWEEN 1 AND 8388608),
    content_type text NOT NULL CHECK(content_type IN ('image/png','image/jpeg')),
    width_px integer NOT NULL CHECK(width_px BETWEEN 1 AND 4096),
    height_px integer NOT NULL CHECK(height_px BETWEEN 1 AND 4096),
    CHECK(width_px::bigint*height_px::bigint<=16777216),
    profile_id text NOT NULL CHECK(profile_id='cover-image-v1'),
    admitted_at_ms bigint NOT NULL CHECK(admitted_at_ms>=0),
    document_bytes bytea NOT NULL CHECK(octet_length(document_bytes) BETWEEN 1 AND 32768),
    digest text NOT NULL CHECK(digest ~ '^ia-r1:sha256:[a-f0-9]{64}$'),
    UNIQUE(id,work_id,owner_id,digest)
);
CREATE TABLE media_work_artwork_changes (
    work_id text NOT NULL,
    work_metadata_revision bigint NOT NULL CHECK(work_metadata_revision>0),
    owner_id text NOT NULL,
    FOREIGN KEY(work_id,work_metadata_revision) REFERENCES catalog_work_revisions(work_id,revision),
    FOREIGN KEY(work_id,owner_id) REFERENCES catalog_works(id,owner_id),
    image_asset_id text,
    image_asset_digest text,
    alt_text text,
    CHECK((image_asset_id IS NULL AND image_asset_digest IS NULL AND alt_text IS NULL) OR
          (image_asset_id IS NOT NULL AND image_asset_digest IS NOT NULL AND alt_text IS NOT NULL)),
    CHECK(image_asset_id IS NULL OR image_asset_id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'),
    CHECK(image_asset_digest IS NULL OR image_asset_digest ~ '^ia-r1:sha256:[a-f0-9]{64}$'),
    CHECK(alt_text IS NULL OR (octet_length(alt_text) BETWEEN 1 AND 1024 AND length(btrim(alt_text))>0 AND alt_text !~ '[[:cntrl:]]')),
    FOREIGN KEY(image_asset_id,work_id,owner_id,image_asset_digest) REFERENCES media_image_assets(id,work_id,owner_id,digest),
    recorded_by text NOT NULL REFERENCES actors(id) CHECK(recorded_by=owner_id),
    recorded_at_ms bigint NOT NULL CHECK(recorded_at_ms>=0),
    document_bytes bytea NOT NULL CHECK(octet_length(document_bytes) BETWEEN 1 AND 32768),
    digest text NOT NULL CHECK(digest ~ '^wa-r1:sha256:[a-f0-9]{64}$'),
    PRIMARY KEY(work_id,work_metadata_revision)
);
CREATE TABLE media_artwork_operations (
    owner_id text NOT NULL,
    operation_id text NOT NULL CHECK(operation_id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'),
    work_id text NOT NULL,
    FOREIGN KEY(work_id,owner_id) REFERENCES catalog_works(id,owner_id),
    kind text NOT NULL CHECK(kind IN ('prepare_image','set_artwork')),
    fingerprint text NOT NULL CHECK(fingerprint ~ '^image-operation1:sha256:[a-f0-9]{64}$'),
    response_bytes bytea NOT NULL CHECK(octet_length(response_bytes) BETWEEN 1 AND 32768),
    response_checksum text NOT NULL CHECK(response_checksum ~ '^[a-f0-9]{64}$'),
    PRIMARY KEY(owner_id,operation_id)
);
DO $$
DECLARE name text;
BEGIN
    FOREACH name IN ARRAY ARRAY['media_image_intents','media_image_assets','media_work_artwork_changes','media_artwork_operations'] LOOP
        EXECUTE format('CREATE TRIGGER immutable BEFORE UPDATE OR DELETE ON %I FOR EACH ROW EXECUTE FUNCTION reject_settled_change()',name);
        EXECUTE format('CREATE TRIGGER no_truncate BEFORE TRUNCATE ON %I FOR EACH STATEMENT EXECUTE FUNCTION reject_settled_change()',name);
        EXECUTE format('REVOKE ALL ON %I FROM PUBLIC',name);
    END LOOP;
END $$;
