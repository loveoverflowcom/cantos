# Editorial and Ingestion on `server.db`

Implementation slice of [#333](https://github.com/loveoverflowcom/cantos/issues/333), based on
`refactor/cantos-native-turso@12fb751e`. This ports the production Editorial/Ingestion adapters
and their Studio HTTP consumers to the existing single-owner embedded Turso `=0.8.3` file.
It does not complete the [Server checkpoint](../server-turso-porting.md#checkpoint-boundary).

Migration **16**, `server_editorial_ingestion`, appends eight STRICT tables: `scripts`,
`script_members`, `script_evidence`, `script_revisions`, `revision_evidence`, `script_reviews`,
`script_review_operations` and `source_records`. Registered migrations 1–15, including the
comment-only slots 4–11, retain their exact bytes. No PostgreSQL driver, network database,
runtime double, new dependency or second file owner is introduced.

## Observable boundary

The existing Studio head/revision/save/history/review and manuscript import/receipt/original/
linked-source routes now execute their real adapters on Turso. The deprecated Studio alias
and JSON, versioned Protobuf and legacy MessagePack retain their existing route-specific
support and error taxonomy. DTOs, Script IR readers/write version `0.1.0`, canonical bytes,
`sir-c1`/`sir-e1` digests and import receipt hashing are unchanged.

Imported rights metadata remains a supplied claim, never publication clearance. Editorial review
is pinned to an immutable revision and remains distinct from publication approval. Historical
receipts do not grant current access. Membership permits a linked source's text view without
granting access to another actor's original-import endpoint.

## PostgreSQL mechanism → Turso mechanism → regression

Source objects are preserved in `services/cantos-server/schema/0001_current.sql`; the old
`legacy-pg-tests/tests/{revisions,imports}_postgres.rs` remain historical oracles. Their applicable
HTTP/codec, admission, receipt and source assertions are ported into ordinary discovered tests,
without PostgreSQL setup or ignored annotations. The legacy files retain other, unported
provider/operator/process and measurement scenarios; their historical results are not native
acceptance.

| Source rule / object | Turso mechanism | Test (ordinary Server tests) |
| --- | --- | --- |
| `scripts` PK, owner FK, nonnegative head; deferred `script_head_exists` | Canonical UUID TEXT, actor FK, INTEGER CHECK and deferred `(id,head_revision)` FK; zero is provisional only inside a write unit | `editorial_constraints_foreign_keys_and_zero_row_cas_are_engine_enforced`, `rejecting_a_final_step_rolls_back_the_provisional_script_and_review` |
| Script identity cannot change; application role only updates the head | Identity UPDATE guard, DELETE guard and duplicate INSERT guard; typed application admission | `settled_editorial_and_ingestion_rows_reject_update_delete_replace_and_upsert` |
| `script_members` PK, script/actor FKs and role CHECK | Composite PK, both FKs, closed `reader`/`editor` CHECK; current membership queried in each unit | `editorial_constraints_foreign_keys_and_zero_row_cas_are_engine_enforced`, `backend_denies_anonymous_wrong_owner_reader_revoked_and_expired_sessions` |
| `script_evidence` PK, owner FK, kind and ID CHECKs | Composite key/FK; closed kind and fail-closed REGEXP CHECK; existing records are pre-checked before an idempotent insert | `editorial_constraints_foreign_keys_and_zero_row_cas_are_engine_enforced`, `invalid_exports_and_foreign_evidence_leave_no_partial_script` |
| Revision PK and `(script_id,accepted_by,operation_id)` uniqueness | Composite PK/unique index, same actor/script operation lookup before pure `decide_save`; duplicate INSERT guard covers both keys | `complete_exports_metadata_retries_and_pinned_history_survive_edits`, `competing_saves_have_one_winner_and_duplicate_delivery_has_one_revision`, `settled_editorial_and_ingestion_rows_reject_update_delete_replace_and_upsert` |
| Revision script/actor FKs; positive revision and exact predecessor; export size 1–2 MiB; schema `0.1.0`; both digest CHECKs | STRICT BLOB/INTEGER/TEXT, original bounds, FKs and REGEXP with `coalesce(...,0)`; checked signed conversion | `editorial_constraints_foreign_keys_and_zero_row_cas_are_engine_enforced`, `invalid_exports_and_foreign_evidence_leave_no_partial_script` |
| `revision_evidence` composite PK and both FKs | Same revision/evidence tuple keys; writing unit checks evidence belongs to the script owner before linking | `editorial_constraints_foreign_keys_and_zero_row_cas_are_engine_enforced`, `invalid_exports_and_foreign_evidence_leave_no_partial_script` |
| Review ID uniqueness, revision PK, reviewer/revision FKs and review-operation PK/FKs | STRICT tables with matching unique identities, FKs, UUID/time CHECKs and immutable operation receipts | `review_keys_times_and_all_reference_targets_are_checked_by_the_engine`, `editorial_reviews_are_pinned_replayable_owner_only_and_history_is_bounded` |
| `revisions_immutable`, `evidence_immutable`, `registry_immutable`, `reviews_immutable`, `review_operations_immutable`, `sources_immutable` and statement-level no-truncate guards | Row UPDATE/DELETE rejection plus BEFORE INSERT rejection for every settled unique identity. This includes REPLACE, which bypasses DELETE triggers on this engine. Turso has no TRUNCATE statement or table roles | `settled_editorial_and_ingestion_rows_reject_update_delete_replace_and_upsert` |
| `source_records` PK, evidence/importer FKs and `source_import_operations` partial uniqueness | Same composite evidence FK and importer FK; partial unique index on actor/operation; duplicate INSERT guard covers primary and operation keys | `source_checks_keep_legacy_text_and_complete_imports_as_the_only_two_shapes`, `competing_imports_preserve_one_receipt_and_reject_changed_operation` |
| Source kind, byte-length, SHA-256, original-exists and `imported_source_complete` CHECKs | Preserve text-only legacy shape or complete imported shape. Explicit non-NULL terms; fail-closed regex and JSON object checks; 1 MiB byte bounds | `source_checks_keep_legacy_text_and_complete_imports_as_the_only_two_shapes`, `legacy_text_only_sources_remain_readable_without_fabricating_an_import_receipt` |
| PostgreSQL timestamps/default clock and `to_char` wire formatting | Bind `tx.now_micros()` to INTEGER at acceptance; checked UTC formatting retains exactly six fractional digits. SQL never reads time | `wire_timestamps_preserve_six_digits_and_refuse_unrepresentable_stored_time`, `lost_commit_acknowledgements_reconcile_each_receipt_without_duplicate_effects` |
| Row/advisory locking, including `pg_advisory_xact_lock` for an actor's imports | Existing bounded serialized `BEGIN IMMEDIATE` writer; admission after writer acquisition; guarded head update with affected-row check | `waiting_saves_and_imports_recheck_admission_after_the_ordered_revocation`, `competing_saves_have_one_winner_and_duplicate_delivery_has_one_revision`, `competing_imports_preserve_one_receipt_and_reject_changed_operation` |
| Atomic save + evidence links + head, review + operation, import + source evidence | One short database write unit; failed statement abandons the whole unit; operation replay reconciles an unknown COMMIT | `rejecting_a_final_step_rolls_back_the_provisional_script_and_review`, `invalid_admission_and_injected_insert_failure_leave_no_partial_registry`, `lost_commit_acknowledgements_reconcile_each_receipt_without_duplicate_effects` |
| Recomputed canonical/import bytes, digests and predecessor at load | Existing pure readers/receipt validators plus checked column/time/schema decoding; no default on malformed rows | `stored_corruption_is_refused_on_revision_history_review_and_import_reads`, `script_links_share_the_preserved_original_but_never_grant_source_ownership` |
| Original bytes survive reopen, including failed extraction, BOM, CRLF and NFD text | Immutable BLOB, unchanged extraction/receipt core; legacy text-only rows remain text-only | `originals_receipts_failed_extraction_and_registry_survive_reopen_exactly`, `legacy_text_only_sources_remain_readable_without_fabricating_an_import_receipt` |
| Registry startup must not silently migrate, rewrite history or accept a newer file | Existing owner registry; explicit migration 15→16; earlier checksums and Listener rows retained; v15 binary refuses v16 | `version_fifteen_upgrades_by_suffix_and_preserves_existing_business_rows`, existing foundation refusal tests |

## Trust shift

Turso has no PostgreSQL table roles, grants or SECURITY DEFINER boundary. The private OS-owned
directory, single owner process, parameterized adapters and current transactional admission
replace that part of the trust model. Settled bytes also have real database guards, while
semantic Script IR/import integrity is rechecked by the existing pure validators on every load.
An operator able to change schema/file bytes can defeat triggers; corruption tests deliberately
drop a guard to simulate such tampering and verify refusal, not to model a runtime privilege.

Production authentication continues through the existing Identity v1 owner and exact-surface
HTTP middleware. Tests in this slice initially seed development Studio sessions explicitly;
they establish business HTTP behavior, not a real Identity login or browser journey. Nothing
enables development sessions in production or adds a fallback for unavailable infrastructure.

## Concurrency and read paths

Write facts and authorization are resolved after acquiring the existing writer. A request
waiting behind a committed actor deactivation is refused before inserting a script or source.
No RPC, media/object access, password hashing or extraction waits inside these transactions;
import admission/extraction and initial Script IR validation stay outside the unit.

Read-only head/history/import/linked-source use cases now use `conn.read()` for one pinned
snapshot. A read can finish against its admitted snapshot while an uncommitted writer waits;
the next read observes committed revocation. The test
`pinned_editorial_reads_do_not_wait_for_or_observe_an_uncommitted_writer` exercises both outcomes.
The existing `authorize` LockMode argument remains only for source compatibility with unported
consumers; the Turso adapter issues no PostgreSQL lock SQL.

Cross-context facts stay with their owner: Editorial resolves a source citation before Ingestion
reads the source, and the platform actor helper checks whether an editorial reviewer is active.
Both lookups share the caller's snapshot/write unit. They replace the two legacy cross-context
joins without adding an architecture-guard exception; their extra statement cost is unmeasured.

Head/revision and operation reads use exact composite keys. Revision/review history is ordered
by revision, bounded to 50 and traversed by a revision key. Import reads use the source PK or
the actor/operation partial unique index. There is no cache or incremental engine. Query-plan,
latency, RSS and footprint measurements are **NOT_RUN**; no performance improvement is claimed.

The shared facade exposes only its existing checked `Row::try_get` accessor publicly so the
file-backed fixture's result rows can be asserted from integration tests. `utc_timestamp` is a
crate-private formatter with two concrete adapter consumers. No SQL facade translation or
catch-and-return-success behavior is added.

## Compatibility and remaining acceptance

This is an additive native schema suffix, not a PostgreSQL importer. A v15 file upgrades only
under explicit `Intent::Migrate`; serving refuses pending migrations. A v15 binary cannot open
a v16 file. Binary rollback therefore requires the matching pre-migration file or forward
recovery preserving accepted new revisions/receipts; reverting source alone is not data rollback.
No real data was converted or reset. Default production migration remains Tier B under #336.

Catalog write/membership/delegation/content, Adaptation, Production, Admin and Media publication
transactions remain separate ports. `cargo xtask seed theatre` remains unavailable. The existing
CMP read/Library adapters are reused. Repository-wide failures, complete non-auth HTTP parity,
two-binary Studio authentication, real process crash/disk-full, D1 Kani, restore/import rehearsal,
browser/device and deployed acceptance remain separate evidence; this slice closes none of those
gates and does not complete #333. Queue order and dependencies are unchanged.
