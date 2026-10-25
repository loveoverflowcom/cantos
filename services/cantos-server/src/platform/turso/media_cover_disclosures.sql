-- Version 24: owner public-display decisions for one exact admitted Work image (ADR 0033).
-- Append-only; the latest `sequence` for a Work and image digest is the current decision.
CREATE TABLE media_cover_disclosures (
 work_id TEXT NOT NULL, sequence INTEGER NOT NULL CHECK(sequence>0), owner_id TEXT NOT NULL,
 image_asset_digest TEXT NOT NULL CHECK(coalesce(image_asset_digest REGEXP '^ia-r1:sha256:[a-f0-9]{64}$',0)),
 decision TEXT NOT NULL CHECK(decision IN ('cleared','revoked')),
 valid_until_ms INTEGER CHECK(valid_until_ms IS NULL OR valid_until_ms BETWEEN 1 AND 9007199254740991),
 recorded_by TEXT NOT NULL REFERENCES actors(id) CHECK(recorded_by=owner_id), recorded_at_ms INTEGER NOT NULL CHECK(recorded_at_ms>=0),
 operation_id TEXT NOT NULL CHECK(coalesce(operation_id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$',0)),
 fingerprint TEXT NOT NULL CHECK(coalesce(fingerprint REGEXP '^image-operation1:sha256:[a-f0-9]{64}$',0)),
 PRIMARY KEY(work_id,sequence), UNIQUE(owner_id,operation_id),
 CHECK((decision='cleared' AND valid_until_ms IS NOT NULL AND valid_until_ms>recorded_at_ms) OR (decision='revoked' AND valid_until_ms IS NULL)),
 FOREIGN KEY(work_id,owner_id) REFERENCES catalog_works(id,owner_id)
) STRICT;
CREATE INDEX media_cover_disclosures_latest ON media_cover_disclosures(work_id,image_asset_digest,sequence);
CREATE TRIGGER media_cover_disclosures_no_update BEFORE UPDATE ON media_cover_disclosures
BEGIN SELECT RAISE(ABORT,'cantos:immutable.cover_disclosure'); END;
CREATE TRIGGER media_cover_disclosures_no_delete BEFORE DELETE ON media_cover_disclosures
BEGIN SELECT RAISE(ABORT,'cantos:immutable.cover_disclosure'); END;
CREATE TRIGGER media_cover_disclosures_no_replace BEFORE INSERT ON media_cover_disclosures
WHEN EXISTS(SELECT 1 FROM media_cover_disclosures WHERE (work_id=NEW.work_id AND sequence=NEW.sequence) OR (owner_id=NEW.owner_id AND operation_id=NEW.operation_id))
BEGIN SELECT RAISE(ABORT,'cantos:immutable.cover_disclosure'); END;
-- A decision names an admitted image of exactly this Work and owner; another image is never consent.
CREATE TRIGGER media_cover_disclosures_binding BEFORE INSERT ON media_cover_disclosures
WHEN NOT EXISTS(SELECT 1 FROM media_image_assets a WHERE a.work_id=NEW.work_id AND a.owner_id=NEW.owner_id AND a.digest=NEW.image_asset_digest)
BEGIN SELECT RAISE(ABORT,'cantos:binding.cover_disclosure'); END;
