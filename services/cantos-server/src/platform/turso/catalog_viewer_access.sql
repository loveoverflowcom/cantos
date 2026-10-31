-- server.db v33, ADR 0054 D01/D04/D05. These private policies do not activate restricted
-- serving. Default PUBLIC is absence of a policy; once present, every public read excludes
-- RESTRICTED. Settled grants, pending bindings, revocations, events and receipts are immutable.
CREATE TABLE catalog_viewer_policies (
 work_id TEXT PRIMARY KEY NOT NULL, owner_id TEXT NOT NULL,
 version INTEGER NOT NULL CHECK(version>0), audience TEXT NOT NULL CHECK(audience IN ('public','restricted')),
 FOREIGN KEY(work_id,owner_id) REFERENCES catalog_works(id,owner_id)
) STRICT;
CREATE TABLE catalog_viewer_groups (
 id TEXT PRIMARY KEY NOT NULL, owner_id TEXT NOT NULL REFERENCES actors(id),
 name TEXT NOT NULL CHECK(octet_length(name) BETWEEN 1 AND 200),
 version INTEGER NOT NULL CHECK(version>0), revoked INTEGER NOT NULL CHECK(revoked IN(0,1)),
 UNIQUE(owner_id,id)
) STRICT;
CREATE TABLE catalog_viewer_members (
 owner_id TEXT NOT NULL, group_id TEXT NOT NULL, actor_id TEXT NOT NULL REFERENCES actors(id),
 expires_at_us INTEGER CHECK(expires_at_us>=0), removed INTEGER NOT NULL CHECK(removed IN(0,1)),
 PRIMARY KEY(group_id,actor_id), FOREIGN KEY(owner_id,group_id) REFERENCES catalog_viewer_groups(owner_id,id)
) STRICT;
CREATE TABLE catalog_viewer_grants (
 id TEXT PRIMARY KEY NOT NULL, work_id TEXT NOT NULL, owner_id TEXT NOT NULL,
 actor_id TEXT REFERENCES actors(id), group_id TEXT,
 expires_at_us INTEGER, created_at_us INTEGER NOT NULL CHECK(created_at_us>=0),
 CHECK((actor_id IS NULL) <> (group_id IS NULL)),
 CHECK(expires_at_us IS NULL OR expires_at_us>created_at_us),
 UNIQUE(id,actor_id), UNIQUE(id,work_id),
 FOREIGN KEY(work_id,owner_id) REFERENCES catalog_works(id,owner_id),
 FOREIGN KEY(owner_id,group_id) REFERENCES catalog_viewer_groups(owner_id,id)
) STRICT;
CREATE INDEX catalog_viewer_grants_target ON catalog_viewer_grants(work_id,actor_id,group_id,expires_at_us);
CREATE TABLE catalog_viewer_grant_revocations (
 grant_id TEXT PRIMARY KEY NOT NULL REFERENCES catalog_viewer_grants(id),
 revoked_by TEXT NOT NULL REFERENCES actors(id), recorded_at_us INTEGER NOT NULL CHECK(recorded_at_us>=0)
) STRICT;
CREATE TABLE catalog_viewer_pending (
 id TEXT PRIMARY KEY NOT NULL, work_id TEXT NOT NULL, owner_id TEXT NOT NULL,
 canonical_email TEXT NOT NULL CHECK(octet_length(canonical_email) BETWEEN 3 AND 254),
 created_at_us INTEGER NOT NULL CHECK(created_at_us>=0),
 expires_at_us INTEGER NOT NULL CHECK(expires_at_us=created_at_us+1209600000000 AND expires_at_us>created_at_us),
 FOREIGN KEY(work_id,owner_id) REFERENCES catalog_works(id,owner_id)
) STRICT;
CREATE INDEX catalog_viewer_pending_mailbox ON catalog_viewer_pending(canonical_email,expires_at_us,id);
CREATE TABLE catalog_viewer_pending_revocations (
 share_id TEXT PRIMARY KEY NOT NULL REFERENCES catalog_viewer_pending(id),
 revoked_by TEXT NOT NULL REFERENCES actors(id), recorded_at_us INTEGER NOT NULL CHECK(recorded_at_us>=0)
) STRICT;
CREATE TABLE catalog_viewer_claims (
 share_id TEXT PRIMARY KEY NOT NULL REFERENCES catalog_viewer_pending(id),
 grant_id TEXT NOT NULL, actor_id TEXT NOT NULL,
 subject BLOB NOT NULL CHECK(length(subject)=16), instance_epoch BLOB NOT NULL CHECK(length(instance_epoch)=16),
 security_version INTEGER NOT NULL CHECK(security_version>0),
 operation_id TEXT NOT NULL, recorded_at_us INTEGER NOT NULL CHECK(recorded_at_us>=0),
 FOREIGN KEY(grant_id,actor_id) REFERENCES catalog_viewer_grants(id,actor_id)
) STRICT;
CREATE TABLE catalog_viewer_events (
 work_id TEXT NOT NULL, version INTEGER NOT NULL CHECK(version>0), actor_id TEXT NOT NULL REFERENCES actors(id),
 operation_id TEXT NOT NULL, action TEXT NOT NULL, payload TEXT NOT NULL CHECK(json_valid(payload)),
 recorded_at_us INTEGER NOT NULL CHECK(recorded_at_us>=0),
 PRIMARY KEY(work_id,version), FOREIGN KEY(work_id) REFERENCES catalog_works(id)
) STRICT;
CREATE TABLE catalog_viewer_group_events (
 group_id TEXT NOT NULL REFERENCES catalog_viewer_groups(id), version INTEGER NOT NULL CHECK(version>0),
 actor_id TEXT NOT NULL REFERENCES actors(id), operation_id TEXT NOT NULL, payload TEXT NOT NULL CHECK(json_valid(payload)),
 recorded_at_us INTEGER NOT NULL CHECK(recorded_at_us>=0), PRIMARY KEY(group_id,version)
) STRICT;
CREATE TABLE catalog_viewer_operations (
 actor_id TEXT NOT NULL REFERENCES actors(id), operation_id TEXT NOT NULL,
 kind TEXT NOT NULL CHECK(kind IN('policy','group','claim')), resource_id TEXT NOT NULL,
 fingerprint TEXT NOT NULL CHECK(coalesce(fingerprint REGEXP '^[a-f0-9]{64}$',0)),
 response TEXT NOT NULL CHECK(json_valid(response)), recorded_at_us INTEGER NOT NULL CHECK(recorded_at_us>=0),
 PRIMARY KEY(actor_id,operation_id)
) STRICT;
CREATE TRIGGER catalog_viewer_policies_advance BEFORE UPDATE ON catalog_viewer_policies
WHEN NEW.work_id<>OLD.work_id OR NEW.owner_id<>OLD.owner_id OR NEW.version<>OLD.version+1
BEGIN SELECT RAISE(ABORT,'cantos:viewer.policy_version'); END;
CREATE TRIGGER catalog_viewer_groups_advance BEFORE UPDATE ON catalog_viewer_groups
WHEN NEW.id<>OLD.id OR NEW.owner_id<>OLD.owner_id OR NEW.name<>OLD.name OR NEW.version<>OLD.version+1 OR NEW.revoked<OLD.revoked
BEGIN SELECT RAISE(ABORT,'cantos:viewer.group_version'); END;
CREATE TRIGGER catalog_viewer_members_binding BEFORE UPDATE ON catalog_viewer_members
WHEN NEW.group_id<>OLD.group_id OR NEW.owner_id<>OLD.owner_id OR NEW.actor_id<>OLD.actor_id
BEGIN SELECT RAISE(ABORT,'cantos:viewer.member_binding'); END;
CREATE TRIGGER catalog_viewer_claim_binding BEFORE INSERT ON catalog_viewer_claims
WHEN NOT EXISTS(SELECT 1 FROM catalog_viewer_pending p JOIN catalog_viewer_grants g ON g.id=NEW.grant_id
 WHERE p.id=NEW.share_id AND p.work_id=g.work_id AND p.owner_id=g.owner_id
 AND g.actor_id=NEW.actor_id AND p.expires_at_us>NEW.recorded_at_us)
 OR EXISTS(SELECT 1 FROM catalog_viewer_pending_revocations WHERE share_id=NEW.share_id)
BEGIN SELECT RAISE(ABORT,'cantos:viewer.claim_binding'); END;
CREATE TRIGGER catalog_viewer_pending_bound BEFORE INSERT ON catalog_viewer_pending
WHEN (SELECT count(*) FROM catalog_viewer_pending p WHERE p.work_id=NEW.work_id AND p.expires_at_us>NEW.created_at_us
 AND NOT EXISTS(SELECT 1 FROM catalog_viewer_pending_revocations r WHERE r.share_id=p.id)
 AND NOT EXISTS(SELECT 1 FROM catalog_viewer_claims c WHERE c.share_id=p.id))>=100
 OR EXISTS(SELECT 1 FROM catalog_viewer_pending p WHERE p.work_id=NEW.work_id AND p.canonical_email=NEW.canonical_email
 AND p.expires_at_us>NEW.created_at_us AND NOT EXISTS(SELECT 1 FROM catalog_viewer_pending_revocations r WHERE r.share_id=p.id)
 AND NOT EXISTS(SELECT 1 FROM catalog_viewer_claims c WHERE c.share_id=p.id))
BEGIN SELECT RAISE(ABORT,'cantos:viewer.pending_bound'); END;
CREATE TRIGGER catalog_viewer_grants_no_update BEFORE UPDATE ON catalog_viewer_grants
BEGIN SELECT RAISE(ABORT,'cantos:viewer.immutable'); END;
CREATE TRIGGER catalog_viewer_grants_no_delete BEFORE DELETE ON catalog_viewer_grants
BEGIN SELECT RAISE(ABORT,'cantos:viewer.immutable'); END;
CREATE TRIGGER catalog_viewer_grants_no_replace BEFORE INSERT ON catalog_viewer_grants
WHEN EXISTS(SELECT 1 FROM catalog_viewer_grants WHERE id=NEW.id)
BEGIN SELECT RAISE(ABORT,'cantos:viewer.immutable'); END;
CREATE TRIGGER catalog_viewer_grant_revocations_no_update BEFORE UPDATE ON catalog_viewer_grant_revocations
BEGIN SELECT RAISE(ABORT,'cantos:viewer.immutable'); END;
CREATE TRIGGER catalog_viewer_grant_revocations_no_delete BEFORE DELETE ON catalog_viewer_grant_revocations
BEGIN SELECT RAISE(ABORT,'cantos:viewer.immutable'); END;
CREATE TRIGGER catalog_viewer_grant_revocations_no_replace BEFORE INSERT ON catalog_viewer_grant_revocations
WHEN EXISTS(SELECT 1 FROM catalog_viewer_grant_revocations WHERE grant_id=NEW.grant_id)
BEGIN SELECT RAISE(ABORT,'cantos:viewer.immutable'); END;
CREATE TRIGGER catalog_viewer_pending_no_update BEFORE UPDATE ON catalog_viewer_pending
BEGIN SELECT RAISE(ABORT,'cantos:viewer.immutable'); END;
CREATE TRIGGER catalog_viewer_pending_no_delete BEFORE DELETE ON catalog_viewer_pending
BEGIN SELECT RAISE(ABORT,'cantos:viewer.immutable'); END;
CREATE TRIGGER catalog_viewer_pending_no_replace BEFORE INSERT ON catalog_viewer_pending
WHEN EXISTS(SELECT 1 FROM catalog_viewer_pending WHERE id=NEW.id)
BEGIN SELECT RAISE(ABORT,'cantos:viewer.immutable'); END;
CREATE TRIGGER catalog_viewer_pending_revocations_no_update BEFORE UPDATE ON catalog_viewer_pending_revocations
BEGIN SELECT RAISE(ABORT,'cantos:viewer.immutable'); END;
CREATE TRIGGER catalog_viewer_pending_revocations_no_delete BEFORE DELETE ON catalog_viewer_pending_revocations
BEGIN SELECT RAISE(ABORT,'cantos:viewer.immutable'); END;
CREATE TRIGGER catalog_viewer_pending_revocations_no_replace BEFORE INSERT ON catalog_viewer_pending_revocations
WHEN EXISTS(SELECT 1 FROM catalog_viewer_pending_revocations WHERE share_id=NEW.share_id)
BEGIN SELECT RAISE(ABORT,'cantos:viewer.immutable'); END;
CREATE TRIGGER catalog_viewer_claims_no_update BEFORE UPDATE ON catalog_viewer_claims
BEGIN SELECT RAISE(ABORT,'cantos:viewer.immutable'); END;
CREATE TRIGGER catalog_viewer_claims_no_delete BEFORE DELETE ON catalog_viewer_claims
BEGIN SELECT RAISE(ABORT,'cantos:viewer.immutable'); END;
CREATE TRIGGER catalog_viewer_claims_no_replace BEFORE INSERT ON catalog_viewer_claims
WHEN EXISTS(SELECT 1 FROM catalog_viewer_claims WHERE share_id=NEW.share_id)
BEGIN SELECT RAISE(ABORT,'cantos:viewer.immutable'); END;
CREATE TRIGGER catalog_viewer_events_no_update BEFORE UPDATE ON catalog_viewer_events
BEGIN SELECT RAISE(ABORT,'cantos:viewer.immutable'); END;
CREATE TRIGGER catalog_viewer_events_no_delete BEFORE DELETE ON catalog_viewer_events
BEGIN SELECT RAISE(ABORT,'cantos:viewer.immutable'); END;
CREATE TRIGGER catalog_viewer_events_no_replace BEFORE INSERT ON catalog_viewer_events
WHEN EXISTS(SELECT 1 FROM catalog_viewer_events WHERE work_id=NEW.work_id AND version=NEW.version)
BEGIN SELECT RAISE(ABORT,'cantos:viewer.immutable'); END;
CREATE TRIGGER catalog_viewer_group_events_no_update BEFORE UPDATE ON catalog_viewer_group_events
BEGIN SELECT RAISE(ABORT,'cantos:viewer.immutable'); END;
CREATE TRIGGER catalog_viewer_group_events_no_delete BEFORE DELETE ON catalog_viewer_group_events
BEGIN SELECT RAISE(ABORT,'cantos:viewer.immutable'); END;
CREATE TRIGGER catalog_viewer_group_events_no_replace BEFORE INSERT ON catalog_viewer_group_events
WHEN EXISTS(SELECT 1 FROM catalog_viewer_group_events WHERE group_id=NEW.group_id AND version=NEW.version)
BEGIN SELECT RAISE(ABORT,'cantos:viewer.immutable'); END;
CREATE TRIGGER catalog_viewer_operations_no_update BEFORE UPDATE ON catalog_viewer_operations
BEGIN SELECT RAISE(ABORT,'cantos:viewer.immutable'); END;
CREATE TRIGGER catalog_viewer_operations_no_delete BEFORE DELETE ON catalog_viewer_operations
BEGIN SELECT RAISE(ABORT,'cantos:viewer.immutable'); END;
CREATE TRIGGER catalog_viewer_operations_no_replace BEFORE INSERT ON catalog_viewer_operations
WHEN EXISTS(SELECT 1 FROM catalog_viewer_operations WHERE actor_id=NEW.actor_id AND operation_id=NEW.operation_id)
BEGIN SELECT RAISE(ABORT,'cantos:viewer.immutable'); END;
CREATE TRIGGER catalog_viewer_policies_no_delete BEFORE DELETE ON catalog_viewer_policies
BEGIN SELECT RAISE(ABORT,'cantos:viewer.immutable'); END;
CREATE TRIGGER catalog_viewer_policies_no_replace BEFORE INSERT ON catalog_viewer_policies
WHEN EXISTS(SELECT 1 FROM catalog_viewer_policies WHERE work_id=NEW.work_id)
BEGIN SELECT RAISE(ABORT,'cantos:viewer.immutable'); END;
CREATE TRIGGER catalog_viewer_groups_no_delete BEFORE DELETE ON catalog_viewer_groups
BEGIN SELECT RAISE(ABORT,'cantos:viewer.immutable'); END;
CREATE TRIGGER catalog_viewer_groups_no_replace BEFORE INSERT ON catalog_viewer_groups
WHEN EXISTS(SELECT 1 FROM catalog_viewer_groups WHERE id=NEW.id)
BEGIN SELECT RAISE(ABORT,'cantos:viewer.immutable'); END;
CREATE TRIGGER catalog_viewer_members_no_delete BEFORE DELETE ON catalog_viewer_members
BEGIN SELECT RAISE(ABORT,'cantos:viewer.immutable'); END;
CREATE TRIGGER catalog_viewer_members_no_replace BEFORE INSERT ON catalog_viewer_members
WHEN EXISTS(SELECT 1 FROM catalog_viewer_members WHERE group_id=NEW.group_id AND actor_id=NEW.actor_id)
BEGIN SELECT RAISE(ABORT,'cantos:viewer.immutable'); END;

CREATE TRIGGER catalog_viewer_group_bound BEFORE INSERT ON catalog_viewer_groups
WHEN (SELECT count(*) FROM catalog_viewer_groups WHERE owner_id=NEW.owner_id AND revoked=0)>=100
BEGIN SELECT RAISE(ABORT,'cantos:viewer.group_bound'); END;
CREATE TRIGGER catalog_viewer_member_bound BEFORE INSERT ON catalog_viewer_members
WHEN NEW.removed=0 AND (SELECT count(*) FROM catalog_viewer_members WHERE group_id=NEW.group_id AND removed=0)>=1000
BEGIN SELECT RAISE(ABORT,'cantos:viewer.member_bound'); END;
CREATE TRIGGER catalog_viewer_member_readd_bound BEFORE UPDATE ON catalog_viewer_members
WHEN OLD.removed=1 AND NEW.removed=0 AND (SELECT count(*) FROM catalog_viewer_members WHERE group_id=NEW.group_id AND removed=0)>=1000
BEGIN SELECT RAISE(ABORT,'cantos:viewer.member_bound'); END;
CREATE TRIGGER catalog_viewer_grant_bound BEFORE INSERT ON catalog_viewer_grants
WHEN (SELECT count(*) FROM catalog_viewer_grants g WHERE g.work_id=NEW.work_id AND (g.expires_at_us IS NULL OR g.expires_at_us>NEW.created_at_us)
 AND NOT EXISTS(SELECT 1 FROM catalog_viewer_grant_revocations r WHERE r.grant_id=g.id))>=1000
 OR EXISTS(SELECT 1 FROM catalog_viewer_grants g WHERE g.work_id=NEW.work_id AND (g.actor_id=NEW.actor_id OR g.group_id=NEW.group_id)
 AND (g.expires_at_us IS NULL OR g.expires_at_us>NEW.created_at_us) AND NOT EXISTS(SELECT 1 FROM catalog_viewer_grant_revocations r WHERE r.grant_id=g.id))
BEGIN SELECT RAISE(ABORT,'cantos:viewer.grant_bound'); END;
CREATE TRIGGER catalog_viewer_grant_revoker BEFORE INSERT ON catalog_viewer_grant_revocations
WHEN NOT EXISTS(SELECT 1 FROM catalog_viewer_grants WHERE id=NEW.grant_id AND owner_id=NEW.revoked_by)
BEGIN SELECT RAISE(ABORT,'cantos:viewer.revoker'); END;
CREATE TRIGGER catalog_viewer_share_revoker BEFORE INSERT ON catalog_viewer_pending_revocations
WHEN NOT EXISTS(SELECT 1 FROM catalog_viewer_pending WHERE id=NEW.share_id AND owner_id=NEW.revoked_by)
BEGIN SELECT RAISE(ABORT,'cantos:viewer.revoker'); END;
