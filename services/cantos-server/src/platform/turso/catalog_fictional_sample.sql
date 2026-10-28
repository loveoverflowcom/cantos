-- Server.db v27: a separate, pinned, original fictional sample.
-- Existing v25/v26 records and constraints remain unchanged.
-- Reserve one non-login principal for the exact fictional sample.
-- The migration creates no actor, credentials, sample data, disclosure or live authority.
CREATE TABLE fictional_seed_principals (
    actor_id TEXT PRIMARY KEY NOT NULL REFERENCES actors(id)
        CHECK(actor_id = 'cantos_fictional_seed_v2'),
    source_sha256 TEXT NOT NULL CHECK(source_sha256 = '0ad007cb18d46b58df25dbc2e8ea98bf58bce752c9e6d640fc4a78a90ef779d6')
) STRICT;
CREATE TRIGGER fictional_seed_principal_clean BEFORE INSERT ON fictional_seed_principals
WHEN EXISTS(SELECT 1 FROM actor_external_identities WHERE actor_id=NEW.actor_id)
 OR EXISTS(SELECT 1 FROM identity_sessions WHERE actor_id=NEW.actor_id)
 OR EXISTS(SELECT 1 FROM development_sessions WHERE actor_id=NEW.actor_id)
 OR EXISTS(SELECT 1 FROM catalog_works WHERE owner_id=NEW.actor_id)
 OR EXISTS(SELECT 1 FROM catalog_members WHERE actor_id=NEW.actor_id)
 OR EXISTS(SELECT 1 FROM catalog_delegation_grants WHERE grantor_id=NEW.actor_id OR delegate_id=NEW.actor_id)
BEGIN SELECT RAISE(ABORT,'cantos:binding.fictional_seed_principal'); END;
CREATE TRIGGER fictional_seed_principal_no_update BEFORE UPDATE ON fictional_seed_principals
BEGIN SELECT RAISE(ABORT,'cantos:immutable.fictional_seed_principal'); END;
CREATE TRIGGER fictional_seed_principal_no_delete BEFORE DELETE ON fictional_seed_principals
BEGIN SELECT RAISE(ABORT,'cantos:immutable.fictional_seed_principal'); END;
CREATE TRIGGER fictional_seed_principal_no_replace BEFORE INSERT ON fictional_seed_principals
WHEN EXISTS(SELECT 1 FROM fictional_seed_principals WHERE actor_id=NEW.actor_id)
BEGIN SELECT RAISE(ABORT,'cantos:immutable.fictional_seed_principal'); END;

-- No future credential, identity link or delegation can make the reserved actor a user.
CREATE TRIGGER fictional_seed_no_external_identity BEFORE INSERT ON actor_external_identities
WHEN EXISTS(SELECT 1 FROM fictional_seed_principals WHERE actor_id=NEW.actor_id)
BEGIN SELECT RAISE(ABORT,'cantos:binding.fictional_seed_no_login'); END;
CREATE TRIGGER fictional_seed_no_identity_session BEFORE INSERT ON identity_sessions
WHEN EXISTS(SELECT 1 FROM fictional_seed_principals WHERE actor_id=NEW.actor_id)
BEGIN SELECT RAISE(ABORT,'cantos:binding.fictional_seed_no_login'); END;
CREATE TRIGGER fictional_seed_no_development_session BEFORE INSERT ON development_sessions
WHEN EXISTS(SELECT 1 FROM fictional_seed_principals WHERE actor_id=NEW.actor_id)
BEGIN SELECT RAISE(ABORT,'cantos:binding.fictional_seed_no_login'); END;
CREATE TRIGGER fictional_seed_no_membership BEFORE INSERT ON catalog_members
WHEN EXISTS(SELECT 1 FROM fictional_seed_principals WHERE actor_id=NEW.actor_id)
BEGIN SELECT RAISE(ABORT,'cantos:binding.fictional_seed_no_grant'); END;
CREATE TRIGGER fictional_seed_no_membership_update BEFORE UPDATE ON catalog_members
WHEN EXISTS(SELECT 1 FROM fictional_seed_principals WHERE actor_id=NEW.actor_id)
BEGIN SELECT RAISE(ABORT,'cantos:binding.fictional_seed_no_grant'); END;
CREATE TRIGGER fictional_seed_no_delegation BEFORE INSERT ON catalog_delegation_grants
WHEN EXISTS(SELECT 1 FROM fictional_seed_principals WHERE actor_id=NEW.grantor_id OR actor_id=NEW.delegate_id)
BEGIN SELECT RAISE(ABORT,'cantos:binding.fictional_seed_no_grant'); END;
CREATE TRIGGER fictional_seed_work_scope BEFORE INSERT ON catalog_works
WHEN EXISTS(SELECT 1 FROM fictional_seed_principals WHERE actor_id=NEW.owner_id)
 AND NEW.id NOT IN ('dbe1f751-0e5f-47d2-8910-62e2eafb2dcf','42661b77-c21d-4514-b2b2-40cdb17298f0',
 'e9f5296d-4c5e-4c65-90ea-58a418682069','97a3501c-5bf9-4dab-82eb-0393262d057e',
 '1d2f6feb-4a05-427f-9989-0e4728d2cc06','9877af6b-81a7-4758-ab61-c4d3727f2bd0')
BEGIN SELECT RAISE(ABORT,'cantos:binding.fictional_seed_scope'); END;
CREATE TRIGGER fictional_seed_unit_scope BEFORE INSERT ON catalog_units
WHEN EXISTS(SELECT 1 FROM catalog_works w JOIN fictional_seed_principals p ON p.actor_id=w.owner_id WHERE w.id=NEW.work_id)
 AND NOT (
   (NEW.work_id='dbe1f751-0e5f-47d2-8910-62e2eafb2dcf' AND NEW.id IN ('34202026-c07d-4ab6-82f3-37d3ed98b95e','7a509fab-775a-42d5-bf0e-406b2502e6ad')) OR
   (NEW.work_id='42661b77-c21d-4514-b2b2-40cdb17298f0' AND NEW.id IN ('a84d1f6f-ba65-412b-862d-9569a01aee54','725784ed-9a24-42ab-83a9-0861191c0de1')) OR
   (NEW.work_id='e9f5296d-4c5e-4c65-90ea-58a418682069' AND NEW.id IN ('5aec04fb-1a1d-49e3-ae7f-9cdeeb64d55e','cef0395b-da85-4624-aa49-8b38bc752d9e')) OR
   (NEW.work_id='97a3501c-5bf9-4dab-82eb-0393262d057e' AND NEW.id IN ('f8a2ea82-8f34-4b01-acdc-68dc66a85059','fd9622e5-7db4-46be-af37-ab4b1f481457')) OR
   (NEW.work_id='1d2f6feb-4a05-427f-9989-0e4728d2cc06' AND NEW.id IN ('5bff5046-a49b-4d44-bdb7-0d9def735dbd','c18090da-1bf3-43fa-b8f9-0dc70dce945d')) OR
   (NEW.work_id='9877af6b-81a7-4758-ab61-c4d3727f2bd0' AND NEW.id IN ('3b375110-4b03-46fb-9524-cf56a29b05e9','e7de074e-d6c5-4846-8eac-527f985e49f6'))
 )
BEGIN SELECT RAISE(ABORT,'cantos:binding.fictional_seed_scope'); END;

-- Operator attribution is a self-declared reference, not an authenticated Studio identity.
-- The immutable record is written in the same transaction as every decision.
CREATE TABLE catalog_fictional_seed_actions (
    operation_id TEXT PRIMARY KEY NOT NULL CHECK(coalesce(operation_id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$',0)),
    actor_id TEXT NOT NULL REFERENCES fictional_seed_principals(actor_id),
    action TEXT NOT NULL CHECK(action IN ('publish','withdraw')),
    source_sha256 TEXT NOT NULL CHECK(source_sha256='0ad007cb18d46b58df25dbc2e8ea98bf58bce752c9e6d640fc4a78a90ef779d6'),
    operator_reference TEXT NOT NULL CHECK(length(operator_reference) BETWEEN 1 AND 1024),
    evidence_reference TEXT NOT NULL CHECK(length(evidence_reference) BETWEEN 1 AND 1024),
    statement TEXT NOT NULL CHECK(length(statement) BETWEEN 1 AND 1024),
    fingerprint TEXT NOT NULL CHECK(coalesce(fingerprint REGEXP '^[a-f0-9]{64}$',0)),
    recorded_at_ms INTEGER NOT NULL CHECK(recorded_at_ms BETWEEN 0 AND 9007199254740991)
) STRICT;
CREATE TRIGGER fictional_seed_actions_no_update BEFORE UPDATE ON catalog_fictional_seed_actions
BEGIN SELECT RAISE(ABORT,'cantos:immutable.fictional_seed_action'); END;
CREATE TRIGGER fictional_seed_actions_no_delete BEFORE DELETE ON catalog_fictional_seed_actions
BEGIN SELECT RAISE(ABORT,'cantos:immutable.fictional_seed_action'); END;
CREATE TRIGGER fictional_seed_actions_no_replace BEFORE INSERT ON catalog_fictional_seed_actions
WHEN EXISTS(SELECT 1 FROM catalog_fictional_seed_actions WHERE operation_id=NEW.operation_id)
BEGIN SELECT RAISE(ABORT,'cantos:immutable.fictional_seed_action'); END;

-- Immutable, operator-recorded metadata decisions. This table alone never grants
-- visibility: the application must authenticate and check the exact approved sample source,
-- snapshot fields, current visibility, rights and owner state in the same transaction.
CREATE TABLE catalog_fictional_metadata_disclosures (
    work_id TEXT NOT NULL,
    subject_kind TEXT NOT NULL CHECK(subject_kind IN ('work','unit')),
    subject_id TEXT NOT NULL,
    sequence INTEGER NOT NULL CHECK(sequence > 0),
    owner_id TEXT NOT NULL CHECK(owner_id='cantos_fictional_seed_v2'),
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
            AND source_sha256='0ad007cb18d46b58df25dbc2e8ea98bf58bce752c9e6d640fc4a78a90ef779d6'
            AND rights_kind='rights' AND rights_id IS NOT NULL
            AND ((subject_kind='work' AND unit_revision IS NULL AND unit_digest IS NULL)
                OR (subject_kind='unit' AND unit_revision > 0
                    AND coalesce(unit_digest REGEXP '^[a-f0-9]{64}$',0))))
    ),0))
) STRICT;
CREATE INDEX catalog_fictional_metadata_work ON catalog_fictional_metadata_disclosures(work_id,subject_kind,subject_id,sequence);

-- REPLACE must not erase a settled record. A sequence is per subject and never skips a prior
-- decision, so withdrawing a newer disclosure cannot revive an older one.
CREATE TRIGGER catalog_fictional_metadata_no_update BEFORE UPDATE ON catalog_fictional_metadata_disclosures
BEGIN SELECT RAISE(ABORT,'cantos:immutable.fictional_metadata'); END;
CREATE TRIGGER catalog_fictional_metadata_no_delete BEFORE DELETE ON catalog_fictional_metadata_disclosures
BEGIN SELECT RAISE(ABORT,'cantos:immutable.fictional_metadata'); END;
CREATE TRIGGER catalog_fictional_metadata_no_replace BEFORE INSERT ON catalog_fictional_metadata_disclosures
WHEN EXISTS(SELECT 1 FROM catalog_fictional_metadata_disclosures
    WHERE (subject_kind=NEW.subject_kind AND subject_id=NEW.subject_id AND sequence=NEW.sequence)
       OR (owner_id=NEW.owner_id AND operation_id=NEW.operation_id))
BEGIN SELECT RAISE(ABORT,'cantos:immutable.fictional_metadata'); END;
CREATE TRIGGER catalog_fictional_metadata_sequence BEFORE INSERT ON catalog_fictional_metadata_disclosures
WHEN NOT EXISTS(SELECT 1 FROM catalog_fictional_metadata_disclosures
    WHERE (subject_kind=NEW.subject_kind AND subject_id=NEW.subject_id AND sequence=NEW.sequence)
       OR (owner_id=NEW.owner_id AND operation_id=NEW.operation_id))
 AND NEW.sequence <> coalesce((SELECT max(sequence) FROM catalog_fictional_metadata_disclosures
    WHERE subject_kind=NEW.subject_kind AND subject_id=NEW.subject_id),0)+1
BEGIN SELECT RAISE(ABORT,'cantos:binding.fictional_metadata_sequence'); END;
CREATE TRIGGER catalog_fictional_metadata_unit_parent BEFORE INSERT ON catalog_fictional_metadata_disclosures
WHEN NEW.subject_kind='unit' AND NOT EXISTS(SELECT 1 FROM catalog_units
    WHERE id=NEW.subject_id AND work_id=NEW.work_id)
BEGIN SELECT RAISE(ABORT,'cantos:binding.fictional_metadata_unit'); END;
CREATE TRIGGER catalog_fictional_metadata_pins BEFORE INSERT ON catalog_fictional_metadata_disclosures
WHEN NEW.decision='disclosed' AND (
    NOT EXISTS(SELECT 1 FROM catalog_work_revisions WHERE work_id=NEW.work_id
        AND revision=NEW.work_revision AND snapshot_digest=NEW.work_digest)
    OR (NEW.subject_kind='unit' AND NOT EXISTS(SELECT 1 FROM catalog_unit_revisions
        WHERE unit_id=NEW.subject_id AND revision=NEW.unit_revision AND snapshot_digest=NEW.unit_digest))
    OR NOT EXISTS(SELECT 1 FROM source_records WHERE owner_id=NEW.owner_id
        AND id=NEW.source_id AND sha256=NEW.source_sha256)
)
BEGIN SELECT RAISE(ABORT,'cantos:binding.fictional_metadata_pins'); END;
