-- Read only, through the owning pinned Turso connection while the Server is stopped.
-- Not sqlite3 acceptance. Current service registry owns every table.
SELECT id, owner_id, kind, lifecycle, head_revision FROM catalog_works ORDER BY id;
SELECT work_id, id, position, lifecycle, head_revision FROM catalog_units ORDER BY work_id, position, id;
SELECT work_id, revision, hex(snapshot) FROM catalog_work_revisions ORDER BY work_id, revision;
SELECT unit_id, revision, hex(snapshot) FROM catalog_unit_revisions ORDER BY unit_id, revision;
SELECT actor_id, operation_id, fingerprint, result_digest FROM catalog_operations ORDER BY actor_id, operation_id;
SELECT owner_id, kind, lifecycle, count(*) FROM catalog_works GROUP BY owner_id, kind, lifecycle ORDER BY owner_id, kind, lifecycle;
SELECT count(*) FROM audio_releases;
-- Use OwnedDatabase::check_integrity(), including its actual FK metadata anti-joins.
-- A successful empty PRAGMA foreign_key_check is not a supported integrity oracle on 0.8.3.
