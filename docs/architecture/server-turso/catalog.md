# Catalog on `server.db`: the CMP business API closure (migrations 12–13)

This record describes the initial CMP closure (versions 12–13). The subsequent
[Studio port](remaining-pg-retirement.md) appends v17 for members, operations, clock and delegation,
plus v21 producer grants. The current native Catalog adapter serves Studio writes, authorization,
content and two-phase reorder. Version 5 remains an immutable placeholder. The initial slice
scope below is historical and does not delimit the current implementation.

| Version | Name | Tables |
| --- | --- | --- |
| 12 | `server_catalog_publication_read` | `catalog_works`, `catalog_work_revisions`, `catalog_units`, `catalog_unit_revisions`, `catalog_content_revisions`, `catalog_content_heads` |
| 13 | `server_production_briefs` | `production_briefs` |

`production_briefs` is present only because `audio_submissions` references it; brief heads, producer
grants, settings, snapshots, approvals and rights claims are not part of any CMP route.

## Mapping

Raw-SQL barriers are in `tests/server_turso_barriers.rs`; registry upgrade/refusal tests are in
`src/platform/turso/business_tests.rs`. The named adapter suites cover the listener read side,
not the retained Studio write oracles (see the [coverage ledger](legacy-test-oracles.md)).

| Source rule (PostgreSQL object / adapter behavior) | Turso mechanism | Test |
| --- | --- | --- |
| UUID identity `CHECK (id ~ …)` | `CHECK (coalesce(id REGEXP …, 0))` (a broken pattern refuses rows) | `catalog_rows_are_well_formed_keep_their_identity_and_their_history_is_append_only` |
| kind / lifecycle / unit-kind combinations (`catalog_units_check`) | table `CHECK`; `(work_id, work_kind)` foreign key to `catalog_works(id, kind)` | same |
| snapshot size, digest shape, `revision > 0` | `CHECK (length(snapshot) BETWEEN …)`, `coalesce(… REGEXP '^[a-f0-9]{64}$', 0)` | same |
| `catalog_*_history_immutable` (`reject_settled_change`) | `BEFORE UPDATE`, `BEFORE DELETE` guards **and** `BEFORE INSERT … WHEN EXISTS` over every unique key, so `INSERT OR REPLACE`, `REPLACE INTO` and an upsert are refused (an engine `REPLACE` deletes without firing delete triggers) | same (update, delete, replace, upsert for work, unit and content revisions) |
| `catalog_identity_immutable` on works/units | `BEFORE UPDATE` guard on id/owner/kind (works) and id/work/work kind/kind (units) | same |
| no `DELETE` grant on works/units/heads | row guards `BEFORE DELETE` (stronger than PostgreSQL, whose runtime role merely lacked the grant) | same |
| `catalog_content_head_identity` | `BEFORE UPDATE … WHEN NEW.unit_id <> OLD.unit_id` | same |
| `catalog_work_head_exists` / `catalog_unit_head_exists` (`DEFERRABLE INITIALLY DEFERRED`) | the same deferred foreign key; a head with no revision row is refused when the unit of work commits | same (commit-time rejection) |
| `UNIQUE (work_id, position) DEFERRABLE INITIALLY DEFERRED` | plain `UNIQUE` plus the replace guard. The engine has no deferrable uniqueness; the Studio reorder port must shift positions out of range and renumber inside one transaction. No reorder is ported here | `a unit taking an existing position` case |
| `catalog_listener_discoverable(work, unit)` | `LISTENER_CURRENT_FILTER`: an inline `EXISTS` over the Work's **head** revision with `json_extract(CAST(snapshot AS TEXT), '$.details.visibility') = 'public'` and neither Work nor Unit archived | `private_staged_withdrawn_revoked_expired_superseded_archived_and_disabled_are_refused`, `released_metadata_is_pinned_while_current_privacy_still_governs` |
| `jsonb_to_recordset($1::text::jsonb)` batch of ≤ 51 release requests | `json_each(?1)` joined to the pinned work/unit revision, the current head and the content revision; one query, no per-item SQL | `discovery_orders_filters_searches_and_pages_by_immutable_anchor`, library suites |
| digest check on read (`SHA-256(snapshot) = snapshot_digest`, `revision_digest`) | unchanged in the adapter (`listener_metadata`); a mismatch is `CorruptRevision`, never served | `a_row_that_fails_its_integrity_checks_is_corruption_and_is_never_served` |
| `lock_listener_catalog` (`catalog_clock … FOR SHARE`) | deleted: the serialized writer orders the Library write against every later catalog/media write; a read uses a pinned snapshot | `a_logout_or_revocation_that_commits_first…`, `concurrent_readers_see_a_whole_publication…` |
| `production_briefs_*` immutable, uniqueness `(id, unit_id, owner_id, digest)`, `(owner_id, operation_id)`, `(unit_id, version)` | `BEFORE UPDATE/DELETE` guards; insert guard over id, `(owner, operation)` and `(unit, version)`; the same `UNIQUE` constraints; foreign key to the content revision `(unit_id, id, sequence)` | `the_audio_release_chain_is_digest_checked_bound_and_immutable…` (brief cases) |

## Trust shift

PostgreSQL withheld `UPDATE`/`DELETE` from the runtime role and used `SECURITY DEFINER` lock helpers.
There is no role here: the boundary is the single owner process, parameterized adapter SQL and the
triggers above. A row written by someone with file access can be anything the schema admits; reads
therefore re-check every digest and binding and refuse a row that fails (see the corruption test).

## Not established

- The Catalog Studio write path, delegation and member authority on Turso.
- Reorder of Units (two-phase renumbering).
- That `catalog_listener_discoverable`'s PostgreSQL JSON path and the `json_extract` path agree on
  adversarial snapshot text; the reader re-decodes the head through the production decoder and the
  discovery projection checks pinned and current visibility again in the domain.
