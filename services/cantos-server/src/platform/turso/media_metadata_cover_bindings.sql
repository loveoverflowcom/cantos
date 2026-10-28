-- v28: explicit metadata-only scope for one new public image-display decision.
-- No existing clearance is widened or backfilled, and no sample data/consent is created.
CREATE TABLE media_metadata_cover_bindings (
 work_id TEXT NOT NULL,
 disclosure_sequence INTEGER NOT NULL CHECK(disclosure_sequence > 0),
 work_metadata_revision INTEGER NOT NULL CHECK(work_metadata_revision > 0),
 evidence_reference TEXT NOT NULL CHECK(length(trim(evidence_reference)) BETWEEN 1 AND 1024),
 PRIMARY KEY(work_id, disclosure_sequence),
 FOREIGN KEY(work_id, disclosure_sequence) REFERENCES media_cover_disclosures(work_id, sequence),
 FOREIGN KEY(work_id, work_metadata_revision) REFERENCES catalog_work_revisions(work_id, revision)
) STRICT;
CREATE TRIGGER media_metadata_cover_binding_no_update BEFORE UPDATE ON media_metadata_cover_bindings
BEGIN SELECT RAISE(ABORT,'cantos:immutable.metadata_cover_binding'); END;
CREATE TRIGGER media_metadata_cover_binding_no_delete BEFORE DELETE ON media_metadata_cover_bindings
BEGIN SELECT RAISE(ABORT,'cantos:immutable.metadata_cover_binding'); END;
CREATE TRIGGER media_metadata_cover_binding_no_replace BEFORE INSERT ON media_metadata_cover_bindings
WHEN EXISTS(SELECT 1 FROM media_metadata_cover_bindings WHERE work_id=NEW.work_id AND disclosure_sequence=NEW.disclosure_sequence)
BEGIN SELECT RAISE(ABORT,'cantos:immutable.metadata_cover_binding'); END;
CREATE TRIGGER media_metadata_cover_binding_clearance BEFORE INSERT ON media_metadata_cover_bindings
WHEN NOT EXISTS(SELECT 1 FROM media_cover_disclosures d WHERE d.work_id=NEW.work_id AND d.sequence=NEW.disclosure_sequence AND d.decision='cleared')
BEGIN SELECT RAISE(ABORT,'cantos:binding.metadata_cover_clearance'); END;
