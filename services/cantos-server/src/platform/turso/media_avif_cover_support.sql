-- Version 29: widen private Work artwork intents and assets to admit image/avif alongside PNG and JPEG.
CREATE TABLE media_v29_intents (
 id TEXT NOT NULL, work_id TEXT NOT NULL, owner_id TEXT NOT NULL, operation_id TEXT NOT NULL, expected_sha256 TEXT NOT NULL,
 expected_byte_length INTEGER NOT NULL, content_type TEXT NOT NULL, profile_id TEXT NOT NULL,
 created_at_ms INTEGER NOT NULL, expires_at_ms INTEGER NOT NULL, document_bytes BLOB NOT NULL, digest TEXT NOT NULL
) STRICT;
CREATE TABLE media_v29_assets (
 id TEXT NOT NULL, intent_id TEXT NOT NULL, work_id TEXT NOT NULL, owner_id TEXT NOT NULL,
 storage_target TEXT NOT NULL, object_key TEXT NOT NULL, sha256 TEXT NOT NULL, byte_length INTEGER NOT NULL,
 content_type TEXT NOT NULL, width_px INTEGER NOT NULL, height_px INTEGER NOT NULL,
 profile_id TEXT NOT NULL, admitted_at_ms INTEGER NOT NULL, document_bytes BLOB NOT NULL, digest TEXT NOT NULL
) STRICT;
CREATE TABLE media_v29_changes (
 work_id TEXT NOT NULL, work_metadata_revision INTEGER NOT NULL, owner_id TEXT NOT NULL,
 image_asset_id TEXT, image_asset_digest TEXT, alt_text TEXT,
 recorded_by TEXT NOT NULL, recorded_at_ms INTEGER NOT NULL, document_bytes BLOB NOT NULL, digest TEXT NOT NULL
) STRICT;

INSERT INTO media_v29_intents SELECT * FROM media_image_intents;
INSERT INTO media_v29_assets SELECT * FROM media_image_assets;
INSERT INTO media_v29_changes SELECT * FROM media_work_artwork_changes;

DROP TRIGGER media_work_artwork_changes_no_update;
DROP TRIGGER media_work_artwork_changes_no_delete;
DROP TRIGGER media_work_artwork_changes_no_replace;
DROP TRIGGER media_image_assets_no_update;
DROP TRIGGER media_image_assets_no_delete;
DROP TRIGGER media_image_assets_no_replace;
DROP TRIGGER media_image_intents_no_update;
DROP TRIGGER media_image_intents_no_delete;
DROP TRIGGER media_image_intents_no_replace;

DROP TABLE media_work_artwork_changes;
DROP TABLE media_image_assets;
DROP TABLE media_image_intents;

CREATE TABLE media_image_intents (
 id TEXT NOT NULL CHECK(coalesce(id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$',0)) PRIMARY KEY, work_id TEXT NOT NULL, owner_id TEXT NOT NULL, operation_id TEXT NOT NULL CHECK(coalesce(operation_id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$',0)), expected_sha256 TEXT NOT NULL CHECK(coalesce(expected_sha256 REGEXP '^[a-f0-9]{64}$',0)),
 expected_byte_length INTEGER NOT NULL CHECK(expected_byte_length BETWEEN 1 AND 8388608),
 content_type TEXT NOT NULL CHECK(content_type IN ('image/png','image/jpeg','image/avif')), profile_id TEXT NOT NULL CHECK(profile_id='cover-image-v1'),
 created_at_ms INTEGER NOT NULL CHECK(created_at_ms>=0), expires_at_ms INTEGER NOT NULL CHECK(expires_at_ms>=0 AND expires_at_ms-created_at_ms=1800000),
 document_bytes BLOB NOT NULL CHECK(length(document_bytes) BETWEEN 1 AND 32768), digest TEXT NOT NULL CHECK(coalesce(digest REGEXP '^ii-r1:sha256:[a-f0-9]{64}$',0)),
 UNIQUE(owner_id,operation_id), UNIQUE(id,work_id,owner_id), FOREIGN KEY(work_id,owner_id) REFERENCES catalog_works(id,owner_id)
) STRICT;
CREATE TABLE media_image_assets (
 id TEXT NOT NULL CHECK(coalesce(id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$',0)) PRIMARY KEY, intent_id TEXT NOT NULL UNIQUE CHECK(id=intent_id), work_id TEXT NOT NULL, owner_id TEXT NOT NULL,
 storage_target TEXT NOT NULL CHECK(coalesce(storage_target REGEXP '^[a-z][a-z0-9-]{0,62}$',0) AND substr(storage_target,-1)<>'-'),
 object_key TEXT NOT NULL CHECK(coalesce(object_key REGEXP '^[a-f0-9]{64}$',0)), sha256 TEXT NOT NULL CHECK(coalesce(sha256 REGEXP '^[a-f0-9]{64}$',0)), byte_length INTEGER NOT NULL CHECK(byte_length BETWEEN 1 AND 8388608),
 content_type TEXT NOT NULL CHECK(content_type IN ('image/png','image/jpeg','image/avif')),
 width_px INTEGER NOT NULL CHECK(width_px BETWEEN 1 AND 4096), height_px INTEGER NOT NULL CHECK(height_px BETWEEN 1 AND 4096),
 profile_id TEXT NOT NULL CHECK(profile_id='cover-image-v1'), admitted_at_ms INTEGER NOT NULL CHECK(admitted_at_ms>=0), document_bytes BLOB NOT NULL CHECK(length(document_bytes) BETWEEN 1 AND 32768), digest TEXT NOT NULL CHECK(coalesce(digest REGEXP '^ia-r1:sha256:[a-f0-9]{64}$',0)),
 CHECK(object_key=sha256), CHECK(width_px*height_px<=16777216), UNIQUE(id,work_id,owner_id,digest),
 FOREIGN KEY(intent_id,work_id,owner_id) REFERENCES media_image_intents(id,work_id,owner_id)
) STRICT;
CREATE TABLE media_work_artwork_changes (
 work_id TEXT NOT NULL, work_metadata_revision INTEGER NOT NULL CHECK(work_metadata_revision>0), owner_id TEXT NOT NULL,
 image_asset_id TEXT CHECK(image_asset_id IS NULL OR coalesce(image_asset_id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$',0)),
 image_asset_digest TEXT CHECK(image_asset_digest IS NULL OR coalesce(image_asset_digest REGEXP '^ia-r1:sha256:[a-f0-9]{64}$',0)),
 alt_text TEXT CHECK(alt_text IS NULL OR (octet_length(alt_text) BETWEEN 1 AND 1024 AND length(trim(alt_text))>0 AND NOT coalesce(alt_text REGEXP '[\x00-\x1f\x7f]',1))),
 recorded_by TEXT NOT NULL REFERENCES actors(id) CHECK(recorded_by=owner_id), recorded_at_ms INTEGER NOT NULL CHECK(recorded_at_ms>=0),
 document_bytes BLOB NOT NULL CHECK(length(document_bytes) BETWEEN 1 AND 32768), digest TEXT NOT NULL CHECK(coalesce(digest REGEXP '^wa-r1:sha256:[a-f0-9]{64}$',0)), PRIMARY KEY(work_id,work_metadata_revision),
 CHECK((image_asset_id IS NULL AND image_asset_digest IS NULL AND alt_text IS NULL) OR (image_asset_id IS NOT NULL AND image_asset_digest IS NOT NULL AND alt_text IS NOT NULL)),
 FOREIGN KEY(work_id,owner_id) REFERENCES catalog_works(id,owner_id),
 FOREIGN KEY(work_id,work_metadata_revision) REFERENCES catalog_work_revisions(work_id,revision),
 FOREIGN KEY(image_asset_id,work_id,owner_id,image_asset_digest) REFERENCES media_image_assets(id,work_id,owner_id,digest)
) STRICT;

INSERT INTO media_image_intents SELECT * FROM media_v29_intents;
INSERT INTO media_image_assets SELECT * FROM media_v29_assets;
INSERT INTO media_work_artwork_changes SELECT * FROM media_v29_changes;

DROP TABLE media_v29_changes;
DROP TABLE media_v29_assets;
DROP TABLE media_v29_intents;

CREATE TRIGGER media_image_intents_no_update BEFORE UPDATE ON media_image_intents
BEGIN SELECT RAISE(ABORT,'cantos:immutable.private_media'); END;
CREATE TRIGGER media_image_intents_no_delete BEFORE DELETE ON media_image_intents
BEGIN SELECT RAISE(ABORT,'cantos:immutable.private_media'); END;
CREATE TRIGGER media_image_intents_no_replace BEFORE INSERT ON media_image_intents
WHEN EXISTS(SELECT 1 FROM media_image_intents WHERE (id=NEW.id) OR (owner_id=NEW.owner_id AND operation_id=NEW.operation_id))
BEGIN SELECT RAISE(ABORT,'cantos:immutable.private_media'); END;
CREATE TRIGGER media_image_assets_no_update BEFORE UPDATE ON media_image_assets
BEGIN SELECT RAISE(ABORT,'cantos:immutable.private_media'); END;
CREATE TRIGGER media_image_assets_no_delete BEFORE DELETE ON media_image_assets
BEGIN SELECT RAISE(ABORT,'cantos:immutable.private_media'); END;
CREATE TRIGGER media_image_assets_no_replace BEFORE INSERT ON media_image_assets
WHEN EXISTS(SELECT 1 FROM media_image_assets WHERE (id=NEW.id) OR (intent_id=NEW.intent_id))
BEGIN SELECT RAISE(ABORT,'cantos:immutable.private_media'); END;
CREATE TRIGGER media_work_artwork_changes_no_update BEFORE UPDATE ON media_work_artwork_changes
BEGIN SELECT RAISE(ABORT,'cantos:immutable.private_media'); END;
CREATE TRIGGER media_work_artwork_changes_no_delete BEFORE DELETE ON media_work_artwork_changes
BEGIN SELECT RAISE(ABORT,'cantos:immutable.private_media'); END;
CREATE TRIGGER media_work_artwork_changes_no_replace BEFORE INSERT ON media_work_artwork_changes
WHEN EXISTS(SELECT 1 FROM media_work_artwork_changes WHERE (work_id=NEW.work_id AND work_metadata_revision=NEW.work_metadata_revision))
BEGIN SELECT RAISE(ABORT,'cantos:immutable.private_media'); END;
