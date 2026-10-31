-- server.db v35. Current Unit restrictions are projections of immutable Work policy events.
-- Publication membership/order remains exclusively the Work generation pointer.
CREATE UNIQUE INDEX catalog_units_work_binding ON catalog_units(id,work_id);
CREATE TABLE catalog_unit_viewer_policies (
 unit_id TEXT PRIMARY KEY NOT NULL, work_id TEXT NOT NULL, owner_id TEXT NOT NULL,
 mode TEXT NOT NULL CHECK(mode IN('inherit','restricted')),
 UNIQUE(unit_id,work_id,owner_id),
 FOREIGN KEY(unit_id,work_id) REFERENCES catalog_units(id,work_id),
 FOREIGN KEY(work_id,owner_id) REFERENCES catalog_works(id,owner_id)
) STRICT;
CREATE TABLE catalog_unit_viewer_grants (
 unit_id TEXT NOT NULL, work_id TEXT NOT NULL, owner_id TEXT NOT NULL,
 principal_key TEXT NOT NULL, actor_id TEXT REFERENCES actors(id), group_id TEXT,
 expires_at_us INTEGER CHECK(expires_at_us>=0),
 CHECK((actor_id IS NULL)<>(group_id IS NULL)),
 CHECK(principal_key=coalesce('actor:'||actor_id,'group:'||group_id)),
 PRIMARY KEY(unit_id,principal_key),
 FOREIGN KEY(unit_id,work_id,owner_id) REFERENCES catalog_unit_viewer_policies(unit_id,work_id,owner_id),
 FOREIGN KEY(owner_id,group_id) REFERENCES catalog_viewer_groups(owner_id,id)
) STRICT;
CREATE INDEX catalog_unit_viewer_grants_work ON catalog_unit_viewer_grants(work_id,unit_id);
CREATE INDEX catalog_unit_viewer_grants_actor ON catalog_unit_viewer_grants(actor_id,work_id,expires_at_us);
CREATE INDEX catalog_unit_viewer_grants_group ON catalog_unit_viewer_grants(group_id,owner_id,work_id,expires_at_us);
CREATE TRIGGER catalog_unit_viewer_policy_binding BEFORE UPDATE ON catalog_unit_viewer_policies
WHEN NEW.unit_id<>OLD.unit_id OR NEW.work_id<>OLD.work_id OR NEW.owner_id<>OLD.owner_id
BEGIN SELECT RAISE(ABORT,'cantos:viewer.unit_binding'); END;
CREATE TRIGGER catalog_unit_viewer_policy_no_delete BEFORE DELETE ON catalog_unit_viewer_policies
BEGIN SELECT RAISE(ABORT,'cantos:viewer.unit_binding'); END;
CREATE TRIGGER catalog_unit_viewer_policy_no_replace BEFORE INSERT ON catalog_unit_viewer_policies
WHEN EXISTS(SELECT 1 FROM catalog_unit_viewer_policies WHERE unit_id=NEW.unit_id)
BEGIN SELECT RAISE(ABORT,'cantos:viewer.unit_binding'); END;
CREATE TRIGGER catalog_unit_viewer_grant_bound BEFORE INSERT ON catalog_unit_viewer_grants
WHEN (SELECT count(*) FROM catalog_unit_viewer_grants WHERE work_id=NEW.work_id)>=1000
 OR NOT EXISTS(SELECT 1 FROM catalog_unit_viewer_policies WHERE unit_id=NEW.unit_id AND mode='restricted')
BEGIN SELECT RAISE(ABORT,'cantos:viewer.unit_bound'); END;

-- Current lifecycle/actor changes invalidate in-flight lists and permits in the same
-- transaction. Draft edits do not: they cannot change a published revision's visibility.
CREATE TABLE catalog_viewer_live_clock (
 singleton INTEGER PRIMARY KEY CHECK(singleton=1),
 version INTEGER NOT NULL CHECK(version>=0)
) STRICT;
INSERT INTO catalog_viewer_live_clock(singleton,version) VALUES(1,0);
CREATE TRIGGER catalog_viewer_actor_liveness AFTER UPDATE OF active ON actors
WHEN OLD.active<>NEW.active
BEGIN UPDATE catalog_viewer_live_clock SET version=version+1 WHERE singleton=1; END;
CREATE TRIGGER catalog_viewer_work_liveness AFTER UPDATE OF lifecycle ON catalog_works
WHEN OLD.lifecycle<>NEW.lifecycle
BEGIN UPDATE catalog_viewer_live_clock SET version=version+1 WHERE singleton=1; END;
CREATE TRIGGER catalog_viewer_unit_liveness AFTER UPDATE OF lifecycle ON catalog_units
WHEN OLD.lifecycle<>NEW.lifecycle
BEGIN UPDATE catalog_viewer_live_clock SET version=version+1 WHERE singleton=1; END;
