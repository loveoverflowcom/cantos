-- Server.db v26: reserve one non-login principal for the exact owner-approved sample.
-- The migration creates no actor, credentials, sample data, disclosure or live authority.
CREATE TABLE sample_seed_principals (
    actor_id TEXT PRIMARY KEY NOT NULL REFERENCES actors(id)
        CHECK(actor_id = 'cantos_sample_seed_v1'),
    source_sha256 TEXT NOT NULL CHECK(source_sha256 = 'd0bca64204bebcd169752fbb325b5a79cd1314e121c6e7508db45fc8ab848212')
) STRICT;
CREATE TRIGGER sample_seed_principal_clean BEFORE INSERT ON sample_seed_principals
WHEN EXISTS(SELECT 1 FROM actor_external_identities WHERE actor_id=NEW.actor_id)
 OR EXISTS(SELECT 1 FROM identity_sessions WHERE actor_id=NEW.actor_id)
 OR EXISTS(SELECT 1 FROM development_sessions WHERE actor_id=NEW.actor_id)
 OR EXISTS(SELECT 1 FROM catalog_works WHERE owner_id=NEW.actor_id)
 OR EXISTS(SELECT 1 FROM catalog_members WHERE actor_id=NEW.actor_id)
 OR EXISTS(SELECT 1 FROM catalog_delegation_grants WHERE grantor_id=NEW.actor_id OR delegate_id=NEW.actor_id)
BEGIN SELECT RAISE(ABORT,'cantos:binding.sample_seed_principal'); END;
CREATE TRIGGER sample_seed_principal_no_update BEFORE UPDATE ON sample_seed_principals
BEGIN SELECT RAISE(ABORT,'cantos:immutable.sample_seed_principal'); END;
CREATE TRIGGER sample_seed_principal_no_delete BEFORE DELETE ON sample_seed_principals
BEGIN SELECT RAISE(ABORT,'cantos:immutable.sample_seed_principal'); END;
CREATE TRIGGER sample_seed_principal_no_replace BEFORE INSERT ON sample_seed_principals
WHEN EXISTS(SELECT 1 FROM sample_seed_principals WHERE actor_id=NEW.actor_id)
BEGIN SELECT RAISE(ABORT,'cantos:immutable.sample_seed_principal'); END;

-- No future credential, identity link or delegation can make the reserved actor a user.
CREATE TRIGGER sample_seed_no_external_identity BEFORE INSERT ON actor_external_identities
WHEN EXISTS(SELECT 1 FROM sample_seed_principals WHERE actor_id=NEW.actor_id)
BEGIN SELECT RAISE(ABORT,'cantos:binding.sample_seed_no_login'); END;
CREATE TRIGGER sample_seed_no_identity_session BEFORE INSERT ON identity_sessions
WHEN EXISTS(SELECT 1 FROM sample_seed_principals WHERE actor_id=NEW.actor_id)
BEGIN SELECT RAISE(ABORT,'cantos:binding.sample_seed_no_login'); END;
CREATE TRIGGER sample_seed_no_development_session BEFORE INSERT ON development_sessions
WHEN EXISTS(SELECT 1 FROM sample_seed_principals WHERE actor_id=NEW.actor_id)
BEGIN SELECT RAISE(ABORT,'cantos:binding.sample_seed_no_login'); END;
CREATE TRIGGER sample_seed_no_membership BEFORE INSERT ON catalog_members
WHEN EXISTS(SELECT 1 FROM sample_seed_principals WHERE actor_id=NEW.actor_id)
BEGIN SELECT RAISE(ABORT,'cantos:binding.sample_seed_no_grant'); END;
CREATE TRIGGER sample_seed_no_membership_update BEFORE UPDATE ON catalog_members
WHEN EXISTS(SELECT 1 FROM sample_seed_principals WHERE actor_id=NEW.actor_id)
BEGIN SELECT RAISE(ABORT,'cantos:binding.sample_seed_no_grant'); END;
CREATE TRIGGER sample_seed_no_delegation BEFORE INSERT ON catalog_delegation_grants
WHEN EXISTS(SELECT 1 FROM sample_seed_principals WHERE actor_id=NEW.grantor_id OR actor_id=NEW.delegate_id)
BEGIN SELECT RAISE(ABORT,'cantos:binding.sample_seed_no_grant'); END;
CREATE TRIGGER sample_seed_work_scope BEFORE INSERT ON catalog_works
WHEN EXISTS(SELECT 1 FROM sample_seed_principals WHERE actor_id=NEW.owner_id)
 AND NEW.id NOT IN ('b4630614-a59d-4a5f-8d34-ae9433f01d2a','5de35d0f-ae81-43b5-aed6-f64057f95ef7',
 '5a19cfec-a7d4-4bdd-bc35-dd27120a1885','278351ac-1a3f-4fab-81a8-ffac8dfbc122',
 '56e6709f-c1fd-4e5f-abc5-6eeb88f7656c','8e0580bc-f44f-4443-bed8-36b9ea66154a')
BEGIN SELECT RAISE(ABORT,'cantos:binding.sample_seed_scope'); END;
CREATE TRIGGER sample_seed_unit_scope BEFORE INSERT ON catalog_units
WHEN EXISTS(SELECT 1 FROM catalog_works w JOIN sample_seed_principals p ON p.actor_id=w.owner_id WHERE w.id=NEW.work_id)
 AND NOT (
   (NEW.work_id='b4630614-a59d-4a5f-8d34-ae9433f01d2a' AND NEW.id IN ('98e5f9c9-9add-4dad-b0b3-03dcf110f120','4a36b33e-1a36-4600-917e-71eb67be437f')) OR
   (NEW.work_id='5de35d0f-ae81-43b5-aed6-f64057f95ef7' AND NEW.id IN ('b6a2e16d-69fe-4bed-85c0-cba2d8e7c2d7','910292af-8df9-401a-a821-cc8b4190a69b')) OR
   (NEW.work_id='5a19cfec-a7d4-4bdd-bc35-dd27120a1885' AND NEW.id IN ('de442e3f-57ae-4ab6-887e-79b75f709116','5724f408-61f7-482a-8672-2e89900216f4')) OR
   (NEW.work_id='278351ac-1a3f-4fab-81a8-ffac8dfbc122' AND NEW.id IN ('59af9d56-1a5e-48a9-888f-0e349b2af2ea','db651483-9d07-4635-855c-5ace78284da9')) OR
   (NEW.work_id='56e6709f-c1fd-4e5f-abc5-6eeb88f7656c' AND NEW.id IN ('a1af4be2-6fa6-4214-8495-0e5fbbbf17c5','417de17f-a17c-436a-972c-56d47b4f0f20')) OR
   (NEW.work_id='8e0580bc-f44f-4443-bed8-36b9ea66154a' AND NEW.id IN ('032d2c0b-48ad-40e4-8c23-85f7e66f5ebb','4d384576-2ff0-4657-b87a-73bbd005a003'))
 )
BEGIN SELECT RAISE(ABORT,'cantos:binding.sample_seed_scope'); END;

-- Operator attribution is a self-declared reference, not an authenticated Studio identity.
-- The immutable record is written in the same transaction as every decision.
CREATE TABLE catalog_sample_seed_actions (
    operation_id TEXT PRIMARY KEY NOT NULL CHECK(coalesce(operation_id REGEXP '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$',0)),
    actor_id TEXT NOT NULL REFERENCES sample_seed_principals(actor_id),
    action TEXT NOT NULL CHECK(action IN ('publish','withdraw')),
    source_sha256 TEXT NOT NULL CHECK(source_sha256='d0bca64204bebcd169752fbb325b5a79cd1314e121c6e7508db45fc8ab848212'),
    operator_reference TEXT NOT NULL CHECK(length(operator_reference) BETWEEN 1 AND 1024),
    evidence_reference TEXT NOT NULL CHECK(length(evidence_reference) BETWEEN 1 AND 1024),
    statement TEXT NOT NULL CHECK(length(statement) BETWEEN 1 AND 1024),
    fingerprint TEXT NOT NULL CHECK(coalesce(fingerprint REGEXP '^[a-f0-9]{64}$',0)),
    recorded_at_ms INTEGER NOT NULL CHECK(recorded_at_ms BETWEEN 0 AND 9007199254740991)
) STRICT;
CREATE TRIGGER sample_seed_actions_no_update BEFORE UPDATE ON catalog_sample_seed_actions
BEGIN SELECT RAISE(ABORT,'cantos:immutable.sample_seed_action'); END;
CREATE TRIGGER sample_seed_actions_no_delete BEFORE DELETE ON catalog_sample_seed_actions
BEGIN SELECT RAISE(ABORT,'cantos:immutable.sample_seed_action'); END;
CREATE TRIGGER sample_seed_actions_no_replace BEFORE INSERT ON catalog_sample_seed_actions
WHEN EXISTS(SELECT 1 FROM catalog_sample_seed_actions WHERE operation_id=NEW.operation_id)
BEGIN SELECT RAISE(ABORT,'cantos:immutable.sample_seed_action'); END;
