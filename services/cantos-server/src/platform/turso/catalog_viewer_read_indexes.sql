-- server.db v34. Principal candidate lookups for shared discovery; never authorization.
CREATE INDEX catalog_viewer_grants_actor ON catalog_viewer_grants(actor_id,work_id);
CREATE INDEX catalog_viewer_grants_group ON catalog_viewer_grants(group_id,owner_id,work_id);
CREATE INDEX catalog_viewer_members_actor ON catalog_viewer_members(actor_id,owner_id,group_id);
