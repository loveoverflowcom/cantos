-- Server.db v25: immutable, owner-recorded metadata decisions. This table alone never grants
-- visibility: the application must authenticate and check the exact approved sample source,
-- snapshot fields, current visibility, rights and owner state in the same transaction.
CREATE TABLE catalog_sample_metadata_disclosures (
    work_id TEXT NOT NULL,
    subject_kind TEXT NOT NULL CHECK(subject_kind IN ('work','unit')),
    subject_id TEXT NOT NULL,
    sequence INTEGER NOT NULL CHECK(sequence > 0),
    owner_id TEXT NOT NULL,
    decision TEXT NOT NULL CHECK(decision IN ('disclosed','withdrawn')),
    work_revision INTEGER,
    work_digest TEXT,
    unit_revision INTEGER,
    unit_digest TEXT,
    source_id TEXT,
    source_sha256 TEXT,
    rights_kind TEXT,
    rights_id TEXT,
    recorded_by TEXT NOT NULL REFERENCES actors(id) CHECK(recorded_by=owner_id),
    recorded_at_ms INTEGER NOT NULL CHECK(recorded_at_ms BETWEEN 0 AND 9007199254740991),
    operation_id TEXT NOT NULL CHECK(coalesce(operation_id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$',0)),
    fingerprint TEXT NOT NULL CHECK(coalesce(fingerprint REGEXP '^[a-f0-9]{64}$',0)),
    PRIMARY KEY(subject_kind,subject_id,sequence),
    UNIQUE(owner_id,operation_id),
    FOREIGN KEY(work_id,owner_id) REFERENCES catalog_works(id,owner_id),
    FOREIGN KEY(work_id,work_revision) REFERENCES catalog_work_revisions(work_id,revision),
    FOREIGN KEY(subject_id,unit_revision) REFERENCES catalog_unit_revisions(unit_id,revision),
    FOREIGN KEY(owner_id,source_id) REFERENCES source_records(owner_id,id),
    FOREIGN KEY(owner_id,rights_kind,rights_id) REFERENCES script_evidence(owner_id,kind,id),
    CHECK((subject_kind='work' AND subject_id=work_id) OR subject_kind='unit'),
    CHECK(coalesce((
        (decision='withdrawn' AND work_revision IS NULL AND work_digest IS NULL
            AND unit_revision IS NULL AND unit_digest IS NULL AND source_id IS NULL
            AND source_sha256 IS NULL AND rights_kind IS NULL AND rights_id IS NULL)
        OR
        (decision='disclosed' AND work_revision > 0
            AND coalesce(work_digest REGEXP '^[a-f0-9]{64}$',0)
            AND length(source_id) BETWEEN 1 AND 64
            AND source_sha256='d0bca64204bebcd169752fbb325b5a79cd1314e121c6e7508db45fc8ab848212'
            AND rights_kind='rights' AND rights_id IS NOT NULL
            AND ((subject_kind='work' AND unit_revision IS NULL AND unit_digest IS NULL)
                OR (subject_kind='unit' AND unit_revision > 0
                    AND coalesce(unit_digest REGEXP '^[a-f0-9]{64}$',0))))
    ),0))
) STRICT;
CREATE INDEX catalog_sample_metadata_work ON catalog_sample_metadata_disclosures(work_id,subject_kind,subject_id,sequence);

-- REPLACE must not erase a settled record. A sequence is per subject and never skips a prior
-- decision, so withdrawing a newer disclosure cannot revive an older one.
CREATE TRIGGER catalog_sample_metadata_no_update BEFORE UPDATE ON catalog_sample_metadata_disclosures
BEGIN SELECT RAISE(ABORT,'cantos:immutable.sample_metadata'); END;
CREATE TRIGGER catalog_sample_metadata_no_delete BEFORE DELETE ON catalog_sample_metadata_disclosures
BEGIN SELECT RAISE(ABORT,'cantos:immutable.sample_metadata'); END;
CREATE TRIGGER catalog_sample_metadata_no_replace BEFORE INSERT ON catalog_sample_metadata_disclosures
WHEN EXISTS(SELECT 1 FROM catalog_sample_metadata_disclosures
    WHERE (subject_kind=NEW.subject_kind AND subject_id=NEW.subject_id AND sequence=NEW.sequence)
       OR (owner_id=NEW.owner_id AND operation_id=NEW.operation_id))
BEGIN SELECT RAISE(ABORT,'cantos:immutable.sample_metadata'); END;
CREATE TRIGGER catalog_sample_metadata_sequence BEFORE INSERT ON catalog_sample_metadata_disclosures
WHEN NOT EXISTS(SELECT 1 FROM catalog_sample_metadata_disclosures
    WHERE (subject_kind=NEW.subject_kind AND subject_id=NEW.subject_id AND sequence=NEW.sequence)
       OR (owner_id=NEW.owner_id AND operation_id=NEW.operation_id))
 AND NEW.sequence <> coalesce((SELECT max(sequence) FROM catalog_sample_metadata_disclosures
    WHERE subject_kind=NEW.subject_kind AND subject_id=NEW.subject_id),0)+1
BEGIN SELECT RAISE(ABORT,'cantos:binding.sample_metadata_sequence'); END;
CREATE TRIGGER catalog_sample_metadata_unit_parent BEFORE INSERT ON catalog_sample_metadata_disclosures
WHEN NEW.subject_kind='unit' AND NOT EXISTS(SELECT 1 FROM catalog_units
    WHERE id=NEW.subject_id AND work_id=NEW.work_id)
BEGIN SELECT RAISE(ABORT,'cantos:binding.sample_metadata_unit'); END;
CREATE TRIGGER catalog_sample_metadata_pins BEFORE INSERT ON catalog_sample_metadata_disclosures
WHEN NEW.decision='disclosed' AND (
    NOT EXISTS(SELECT 1 FROM catalog_work_revisions WHERE work_id=NEW.work_id
        AND revision=NEW.work_revision AND snapshot_digest=NEW.work_digest)
    OR (NEW.subject_kind='unit' AND NOT EXISTS(SELECT 1 FROM catalog_unit_revisions
        WHERE unit_id=NEW.subject_id AND revision=NEW.unit_revision AND snapshot_digest=NEW.unit_digest))
    OR NOT EXISTS(SELECT 1 FROM source_records WHERE owner_id=NEW.owner_id
        AND id=NEW.source_id AND sha256=NEW.source_sha256)
)
BEGIN SELECT RAISE(ABORT,'cantos:binding.sample_metadata_pins'); END;
