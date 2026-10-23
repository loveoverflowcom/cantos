-- Version 23: owner review history and exact publication event/operation receipts.
CREATE TABLE audio_reviews (
 id TEXT NOT NULL CHECK(coalesce(id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$',0)) PRIMARY KEY, rendition_id TEXT NOT NULL REFERENCES audio_renditions(id), owner_id TEXT NOT NULL REFERENCES actors(id),
 operation_id TEXT NOT NULL CHECK(coalesce(operation_id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$',0)), sequence INTEGER NOT NULL CHECK(sequence>0), snapshot BLOB NOT NULL CHECK(length(snapshot) BETWEEN 1 AND 32768),
 digest TEXT NOT NULL CHECK(coalesce(digest REGEXP '^aq-r1:sha256:[a-f0-9]{64}$',0)), UNIQUE(owner_id,operation_id), UNIQUE(rendition_id,sequence)
) STRICT;
CREATE TABLE audio_publication_events (
 id TEXT NOT NULL CHECK(coalesce(id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$',0)) PRIMARY KEY, unit_id TEXT NOT NULL REFERENCES catalog_units(id), pointer_version INTEGER NOT NULL CHECK(pointer_version>=0),
 kind TEXT NOT NULL CHECK(kind IN ('activated','withdrawn')), release_id TEXT NOT NULL REFERENCES audio_releases(id),
 actor_id TEXT NOT NULL REFERENCES actors(id), recorded_at INTEGER NOT NULL CHECK(recorded_at>=0),
 UNIQUE(unit_id,pointer_version,kind,release_id)
) STRICT;
CREATE TABLE audio_publication_operations (
 owner_id TEXT NOT NULL REFERENCES actors(id), operation_id TEXT NOT NULL CHECK(coalesce(operation_id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$',0)), kind TEXT NOT NULL CHECK(kind IN ('publish','withdraw')),
 request_digest TEXT NOT NULL CHECK(coalesce(request_digest REGEXP '^publication-request1:sha256:[a-f0-9]{64}$',0)), unit_id TEXT NOT NULL REFERENCES catalog_units(id),
 release_id TEXT NOT NULL REFERENCES audio_releases(id), pointer_version INTEGER NOT NULL CHECK(pointer_version>=0),
 PRIMARY KEY(owner_id,operation_id,kind)
) STRICT;
CREATE TRIGGER audio_reviews_no_update BEFORE UPDATE ON audio_reviews
BEGIN SELECT RAISE(ABORT,'cantos:immutable.publication_receipt'); END;
CREATE TRIGGER audio_reviews_no_delete BEFORE DELETE ON audio_reviews
BEGIN SELECT RAISE(ABORT,'cantos:immutable.publication_receipt'); END;
CREATE TRIGGER audio_reviews_no_replace BEFORE INSERT ON audio_reviews
WHEN EXISTS(SELECT 1 FROM audio_reviews WHERE (id=NEW.id) OR (owner_id=NEW.owner_id AND operation_id=NEW.operation_id) OR (rendition_id=NEW.rendition_id AND sequence=NEW.sequence))
BEGIN SELECT RAISE(ABORT,'cantos:immutable.publication_receipt'); END;
CREATE TRIGGER audio_publication_events_no_update BEFORE UPDATE ON audio_publication_events
BEGIN SELECT RAISE(ABORT,'cantos:immutable.publication_receipt'); END;
CREATE TRIGGER audio_publication_events_no_delete BEFORE DELETE ON audio_publication_events
BEGIN SELECT RAISE(ABORT,'cantos:immutable.publication_receipt'); END;
CREATE TRIGGER audio_publication_events_no_replace BEFORE INSERT ON audio_publication_events
WHEN EXISTS(SELECT 1 FROM audio_publication_events WHERE (id=NEW.id) OR (unit_id=NEW.unit_id AND pointer_version=NEW.pointer_version AND kind=NEW.kind AND release_id=NEW.release_id))
BEGIN SELECT RAISE(ABORT,'cantos:immutable.publication_receipt'); END;
CREATE TRIGGER audio_publication_operations_no_update BEFORE UPDATE ON audio_publication_operations
BEGIN SELECT RAISE(ABORT,'cantos:immutable.publication_receipt'); END;
CREATE TRIGGER audio_publication_operations_no_delete BEFORE DELETE ON audio_publication_operations
BEGIN SELECT RAISE(ABORT,'cantos:immutable.publication_receipt'); END;
CREATE TRIGGER audio_publication_operations_no_replace BEFORE INSERT ON audio_publication_operations
WHEN EXISTS(SELECT 1 FROM audio_publication_operations WHERE (owner_id=NEW.owner_id AND operation_id=NEW.operation_id AND kind=NEW.kind))
BEGIN SELECT RAISE(ABORT,'cantos:immutable.publication_receipt'); END;
