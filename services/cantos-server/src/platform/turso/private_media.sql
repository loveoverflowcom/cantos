-- Version 20: private original audio intake and owner artwork. Verified bytes stay outside SQL.
-- Audio timestamps use INTEGER microseconds; image/publication domains retain milliseconds.
CREATE TABLE audio_upload_intents (
 id TEXT NOT NULL CHECK(coalesce(id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$',0)) PRIMARY KEY, script_id TEXT NOT NULL REFERENCES scripts(id), script_revision INTEGER NOT NULL CHECK(script_revision>0),
 content_digest TEXT NOT NULL CHECK(coalesce(content_digest REGEXP '^sir-c1:sha256:[a-f0-9]{64}$',0)), profile_id TEXT NOT NULL CHECK(profile_id IN ('opus-mono-20-v1','opus-mono-24-v1','opus-mono-32-v1','opus-mono-40-v1')),
 operation_id TEXT NOT NULL CHECK(coalesce(operation_id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$',0)), created_by TEXT NOT NULL REFERENCES actors(id), created_at INTEGER NOT NULL CHECK(created_at>=0),
 UNIQUE(created_by,operation_id), UNIQUE(id,script_id,script_revision,content_digest,profile_id,operation_id,created_by),
 FOREIGN KEY(script_id,script_revision) REFERENCES script_revisions(script_id,revision)
) STRICT;
CREATE TABLE audio_intake_receipts (
 id TEXT NOT NULL CHECK(coalesce(id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$',0)) PRIMARY KEY, intent_id TEXT NOT NULL UNIQUE, script_id TEXT NOT NULL, script_revision INTEGER NOT NULL,
 content_digest TEXT NOT NULL, profile_id TEXT NOT NULL, operation_id TEXT NOT NULL, accepted_by TEXT NOT NULL,
 accepted_at INTEGER NOT NULL CHECK(accepted_at>=0), object_key TEXT NOT NULL CHECK(coalesce(object_key REGEXP '^[a-f0-9]{64}$',0)), receipt_bytes BLOB NOT NULL CHECK(length(receipt_bytes) BETWEEN 1 AND 8192), receipt_digest TEXT NOT NULL CHECK(coalesce(receipt_digest REGEXP '^audio-intake-r1:sha256:[a-f0-9]{64}$',0)),
 storage_target_id TEXT NOT NULL DEFAULT 'legacy-filesystem' CHECK(coalesce(storage_target_id REGEXP '^[a-z][a-z0-9-]{0,62}$',0) AND substr(storage_target_id,-1)<>'-'),
 FOREIGN KEY(intent_id,script_id,script_revision,content_digest,profile_id,operation_id,accepted_by)
 REFERENCES audio_upload_intents(id,script_id,script_revision,content_digest,profile_id,operation_id,created_by)
) STRICT;
CREATE TABLE media_image_intents (
 id TEXT NOT NULL CHECK(coalesce(id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$',0)) PRIMARY KEY, work_id TEXT NOT NULL, owner_id TEXT NOT NULL, operation_id TEXT NOT NULL CHECK(coalesce(operation_id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$',0)), expected_sha256 TEXT NOT NULL CHECK(coalesce(expected_sha256 REGEXP '^[a-f0-9]{64}$',0)),
 expected_byte_length INTEGER NOT NULL CHECK(expected_byte_length BETWEEN 1 AND 8388608),
 content_type TEXT NOT NULL CHECK(content_type IN ('image/png','image/jpeg')), profile_id TEXT NOT NULL CHECK(profile_id='cover-image-v1'),
 created_at_ms INTEGER NOT NULL CHECK(created_at_ms>=0), expires_at_ms INTEGER NOT NULL CHECK(expires_at_ms>=0 AND expires_at_ms-created_at_ms=1800000),
 document_bytes BLOB NOT NULL CHECK(length(document_bytes) BETWEEN 1 AND 32768), digest TEXT NOT NULL CHECK(coalesce(digest REGEXP '^ii-r1:sha256:[a-f0-9]{64}$',0)),
 UNIQUE(owner_id,operation_id), UNIQUE(id,work_id,owner_id), FOREIGN KEY(work_id,owner_id) REFERENCES catalog_works(id,owner_id)
) STRICT;
CREATE TABLE media_image_assets (
 id TEXT NOT NULL CHECK(coalesce(id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$',0)) PRIMARY KEY, intent_id TEXT NOT NULL UNIQUE CHECK(id=intent_id), work_id TEXT NOT NULL, owner_id TEXT NOT NULL,
 storage_target TEXT NOT NULL CHECK(coalesce(storage_target REGEXP '^[a-z][a-z0-9-]{0,62}$',0) AND substr(storage_target,-1)<>'-'),
 object_key TEXT NOT NULL CHECK(coalesce(object_key REGEXP '^[a-f0-9]{64}$',0)), sha256 TEXT NOT NULL CHECK(coalesce(sha256 REGEXP '^[a-f0-9]{64}$',0)), byte_length INTEGER NOT NULL CHECK(byte_length BETWEEN 1 AND 8388608),
 content_type TEXT NOT NULL CHECK(content_type IN ('image/png','image/jpeg')),
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
CREATE TABLE media_artwork_operations (
 owner_id TEXT NOT NULL, operation_id TEXT NOT NULL CHECK(coalesce(operation_id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$',0)), work_id TEXT NOT NULL,
 kind TEXT NOT NULL CHECK(kind IN ('prepare_image','set_artwork')), fingerprint TEXT NOT NULL CHECK(coalesce(fingerprint REGEXP '^image-operation1:sha256:[a-f0-9]{64}$',0)),
 response_bytes BLOB NOT NULL CHECK(length(response_bytes) BETWEEN 1 AND 32768), response_checksum TEXT NOT NULL CHECK(coalesce(response_checksum REGEXP '^[a-f0-9]{64}$',0)), PRIMARY KEY(owner_id,operation_id),
 FOREIGN KEY(work_id,owner_id) REFERENCES catalog_works(id,owner_id)
) STRICT;
CREATE TRIGGER audio_upload_intents_no_update BEFORE UPDATE ON audio_upload_intents
BEGIN SELECT RAISE(ABORT,'cantos:immutable.private_media'); END;
CREATE TRIGGER audio_upload_intents_no_delete BEFORE DELETE ON audio_upload_intents
BEGIN SELECT RAISE(ABORT,'cantos:immutable.private_media'); END;
CREATE TRIGGER audio_upload_intents_no_replace BEFORE INSERT ON audio_upload_intents
WHEN EXISTS(SELECT 1 FROM audio_upload_intents WHERE (id=NEW.id) OR (created_by=NEW.created_by AND operation_id=NEW.operation_id))
BEGIN SELECT RAISE(ABORT,'cantos:immutable.private_media'); END;
CREATE TRIGGER audio_intake_receipts_no_update BEFORE UPDATE ON audio_intake_receipts
BEGIN SELECT RAISE(ABORT,'cantos:immutable.private_media'); END;
CREATE TRIGGER audio_intake_receipts_no_delete BEFORE DELETE ON audio_intake_receipts
BEGIN SELECT RAISE(ABORT,'cantos:immutable.private_media'); END;
CREATE TRIGGER audio_intake_receipts_no_replace BEFORE INSERT ON audio_intake_receipts
WHEN EXISTS(SELECT 1 FROM audio_intake_receipts WHERE (id=NEW.id) OR (intent_id=NEW.intent_id))
BEGIN SELECT RAISE(ABORT,'cantos:immutable.private_media'); END;
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
CREATE TRIGGER media_artwork_operations_no_update BEFORE UPDATE ON media_artwork_operations
BEGIN SELECT RAISE(ABORT,'cantos:immutable.private_media'); END;
CREATE TRIGGER media_artwork_operations_no_delete BEFORE DELETE ON media_artwork_operations
BEGIN SELECT RAISE(ABORT,'cantos:immutable.private_media'); END;
CREATE TRIGGER media_artwork_operations_no_replace BEFORE INSERT ON media_artwork_operations
WHEN EXISTS(SELECT 1 FROM media_artwork_operations WHERE (owner_id=NEW.owner_id AND operation_id=NEW.operation_id))
BEGIN SELECT RAISE(ABORT,'cantos:immutable.private_media'); END;

-- Independently enforce the exact revision digest and owner binding, as the retained PG oracle did.
CREATE TRIGGER audio_intents_binding BEFORE INSERT ON audio_upload_intents
WHEN NOT EXISTS(SELECT 1 FROM script_revisions r JOIN scripts s ON s.id=r.script_id
 WHERE r.script_id=NEW.script_id AND r.revision=NEW.script_revision AND r.content_digest=NEW.content_digest AND s.owner_id=NEW.created_by)
BEGIN SELECT RAISE(ABORT,'cantos:binding.audio_intent'); END;
