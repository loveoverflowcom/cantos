# Content PostgreSQL oracle inventory

Appendix to the [retained-oracle coverage ledger](legacy-test-oracles.md). Retained rows keep the original source-only audit; the removed Editorial/Ingestion rows below name executed native replacements at the `84eea4c3` base plus this change.


Source: `12fb751ec71e20233fe4caaee35e4544fe4a0d4b` (BASE and inspected HEAD). All paths below are relative to `services/cantos-server/` unless explicitly prefixed with `docs/`. Read all 10,113 source lines and helpers in the six listed files. This is a source/assertion inventory, not executed acceptance.

The original audit had native execution **NOT_RUN** because Cargo was absent from that environment. Native candidates below are statically Cargo-discoverable through ordinary integration-test discovery or the library module graph; Cargo metadata/test-list execution was not possible. No PASS is inferred from source.

## Decision and structural evidence

| Legacy file under `legacy-pg-tests/tests/` | Tests | Disposition | Exact remaining port boundary |
| --- | ---: | --- | --- |
| `adaptations_postgres.rs` | 20 | NEEDS_PORT | #333 Adaptation/Editorial/Ingestion persistence; contexts/submissions/cancellation/acceptance, authorization, transactions, corruption, replay/restart, successful Studio journey |
| `catalog_http_postgres.rs` | 17 | NEEDS_PORT | #333 Studio Catalog/Content successful HTTP reads/writes, receipts, members, reorder, authoritative links and recovery |
| `catalog_postgres.rs` | 20 | NEEDS_PORT | #333 Catalog Studio adapter, memberships/operations, concurrent mutation, historical links and recovery |
| `delegation_http_postgres.rs` | 4 | NEEDS_PORT | #333 durable grant/revocation/lineage tables, owner/delegate HTTP authority and write/revoke ordering |

**61 retained tests in four files.** The other 30 original tests are retired by the mappings below. Partial coverage of the remaining Catalog/Adaptation/Delegation surface is still insufficient for deletion. PostgreSQL-only grants/SQLSTATEs/lock probes/EXPLAIN/restart mechanisms need explicit translation or narrowly justified retirement, not removal of retained business assertions.

- `Cargo.toml` has ordinary integration discovery and no registrations for archived files. `src/lib.rs` → `contexts/mod.rs` reaches domain/application tests; `src/platform/turso/mod.rs` → `tests.rs` → `business_tests.rs` reaches native registry tests.
- `src/platform/turso/{editorial,adaptation,catalog}.sql` versions 4/7/5 retain immutable placeholder comments. Version 16 appends Editorial/Ingestion; Catalog and Adaptation writes remain unported.
- `src/contexts/catalog/turso.rs` explicitly limits its port to public-listener metadata. Studio writes remain in `postgres.rs` and `postgres/{content,delegation}.rs`.
- `docs/architecture/server-turso/catalog.md` excludes Studio writes, member/delegation authority and reorder. `docs/architecture/server-turso/cmp-business-api.md:138` retains Catalog Studio writes/delegation/members/reorder, Editorial, Ingestion and Adaptation under #333.
- `docs/architecture/server-turso-porting.md` explicitly distinguishes archived migration oracles from native acceptance and states archived suites do not run through Cargo discovery.
- `tests/wire_contract.rs` explicitly describes detached-store refusals before DB access; its successful database-backed parity now runs in `editorial_turso.rs`.
- At the pinned source, the catalog mapping document's barrier path was stale (corrected by this cleanup): the actual native catalog raw-SQL test is in `tests/server_turso_barriers.rs`, not `src/platform/turso/business_tests.rs`.

## adaptations_postgres.rs — NEEDS_PORT (#333), 20 tests

Common helpers assert `no-store`/`nosniff`; exact original bytes, import receipt and SHA; independent counts for runs/proposals/acceptances/revisions and submissions. These helper assertions belong to the inventory too.

### Native partial mapping, not replacement

`tests/adaptation.rs` actually exercises pure proposal/admission: `proposal_preserves_warning_unknown_attribution_coverage_and_trusted_evidence`, `malformed_oversized_and_semantically_invalid_output_yields_no_partial_proposal`, `edited_acceptance_cannot_rebind_source_attempt_and_pending_rights`, `instruction_like_source_remains_inert_exported_data`, `context_export_refuses_sources_past_the_byte_or_block_limit`, `caller_declarations_allow_missing_facts_without_provider_or_accounting_attestation`, `invalid_caller_declarations_return_all_safe_paths_without_private_values`, `caller_configuration_is_a_bounded_json_object_and_preserves_missing_as_unknown`, `every_raw_binding_reuse_is_readmitted_by_both_proposal_entry_points`.

`src/contexts/adaptation/domain/commands.rs::tests` covers decision facts only: `a_recorded_acceptance_replays_only_for_the_same_run_and_request`, `a_new_acceptance_needs_an_admitted_proposal_on_the_pinned_base`, `cancellation_is_idempotent_per_operation_and_never_undoes_acceptance`, `a_submission_checks_context_then_generation_then_status`, `a_context_source_must_be_parsed_pinned_and_rights_declared`.

`tests/wire_contract.rs::origin_and_session_refusals_are_the_same_json_errors_for_all_representations` and `tests/surface_routes.rs` supply limited pre-DB middleware/route evidence. `src/platform/store/tests.rs::development_tokens_admit_only_live_active_unrevoked_sessions_inside_the_unit` supplies real-file session admission, not reauthorization inside adaptation writes. None of these implements persisted contexts, receipt uniqueness, rollback, corrupt-row reads, restart or successful Studio flow.

### Per-test ledger

1. **:334 `context_exports_frozen_source_and_valid_submission_requires_explicit_reviewed_acceptance`** — Frozen source/request/creator/prompt/contract/status and empty proposal/base/acceptance/submission fields; digest/schema and inert source blocks. Valid submission preserves SHA, generation, unknown usage/cost; proposal canonical bytes, provenance, coverage and findings. No automatic review/revision; unreviewed acceptance rejected. Explicit acceptance/replay/pinned load/counts exact; source unchanged.
2. **:482 `concurrent_context_and_submission_retries_are_exact_and_changed_operations_conflict`** — Concurrent identical contexts/submissions produce identical results once. Same operation with changed target script, proposal bytes, host, model, configuration or usage conflicts. New operation cannot replace valid proposal. Exact receipt replay, one submission/proposal and original source intact.
3. **:549 `invalid_model_bytes_record_immutable_caller_receipts_and_new_operation_can_correct_them`** — Non-JSON, empty object, invalid source citation and >256KiB output yield immutable invalid receipts, exact SHA/usage, unknown cost rather than zero, replay and awaiting/no-proposal state. New valid operation corrects after four invalid receipts; all five submissions retained; source unchanged.
4. **:611 `source_checksum_extractor_rights_and_bounds_are_checked_before_context_export`** — Reject mismatched checksum/extractor, false rights authorization, absent permission claim, failed extraction and >24KiB source. Typed errors differ for failed extraction versus field rejection. No run/proposal/acceptance/revision created; source unchanged.
5. **:676 `context_digest_prompt_version_and_caller_metadata_cannot_be_forged`** — Reject forged digest/prompt, blank host, non-JSON configuration, provider-reported cost and another context's digest without receipt inserts. Preserve accepted caller-declared VND cost with unknown provider/model; exact counts.
6. **:730 `hostile_http_unknown_fields_malformed_json_bad_origin_and_oversized_bodies_write_nothing`** — Context extra command field/malformed JSON/bad origin/>64KiB body rejected. Proposal generation extra field/malformed JSON/>1MiB HTTP body and direct submission size rejected. Exact HTTP statuses and zero receipt/business side effects.
7. **:822 `anonymous_foreign_revoked_and_expired_sessions_cannot_export_submit_review_or_accept`** — Foreign source/context hidden; anonymous versus foreign context/review/proposal requests give 401/404. Foreign acceptance hidden; revoked load/submit/accept and expired creation denied. Exact run/proposal/receipt/revision counts unchanged.
8. **:918 `frozen_base_stale_acceptance_and_concurrent_exact_acceptance_preserve_all_revisions`** — Input revision stays frozen. Stale context creation, submission and acceptance report exact current revision without changing context. Concurrent identical acceptance writes once; changed acceptance operation conflicts; all old revisions/source remain; exact aggregate counts.
9. **:989 `failed_context_creation_rolls_back_run_and_registered_evidence_then_exact_retry_recovers`** — Inject run insert failure; no run or registered-evidence leak. Remove fault; exact retry/replay produces one context only.
10. **:1039 `failed_submission_receipt_or_proposal_insert_rolls_back_whole_transition_and_exact_retry_recovers`** — Independently fault receipt and proposal inserts. No partial submissions/proposals/status transition; context exact. Remove fault; retry/replay yields one receipt/proposal.
11. **:1082 `failed_acceptance_insert_rolls_back_revision_head_and_receipt_then_exact_retry_recovers`** — Fault acceptance insert; script creation/revision/head/acceptance/context transition all roll back. Remove fault; exact retry and replay yield one accepted revision.
12. **:1128 `context_and_submission_replay_after_reopened_store_never_dispatch_or_replace_history`** — Invalid and valid submission receipts, acceptance and original context operation survive successive Store reopenings. Original context replay returns accepted state; immutable history/counts/source preserved.
13. **:1199 `cancellation_fences_late_external_submission_and_exact_cancel_receipts_replay`** — Awaiting and proposed contexts cancel, keeping proposal. Exact normal/reopened cancellation replay; late submit refused; cancelled proposed acceptance refused. No new revisions; source intact.
14. **:1253 `settled_contexts_submissions_proposals_and_accepted_revisions_are_immutable`** — App cannot rewrite/delete/truncate frozen/history rows or disable barriers; accepted→succeeded denied. Owner cannot rewrite frozen input/version/history or truncate. Bypassed receipt-host tampering makes context corrupt while accepted revision/source remain exact.
15. **:1358 `frozen_source_checksum_extractor_prompt_contract_and_base_digest_tampering_are_detected`** — Stored source SHA, extractor, prompt, contract, context version and base digest independently tampered. Load and submit reject CorruptRevision; no new receipt/revision; earlier accepted revision/source remain exact.
16. **:1423 `authorization_expiry_and_revocation_after_lock_wait_roll_back_context_submit_and_accept`** — Context/submit/accept × expiry/deactivation after deterministic lock wait: all unauthenticated, exact business/submission counts unchanged.
17. **:1512 `bounded_caller_context_reads_keep_application_role_authorized_index_baseline`** — 1,000 contexts/proposals/submissions; real app-role point/replay index plans; five warmups, 30 exact authorized HTTP reads and p95≤250ms; attributable bounded evidence. PG instrumentation may retire explicitly, native authorized read/performance oracle absent.
18. **:1574 `connection_death_after_receipt_insert_before_commit_rolls_back_and_exact_delivery_recovers`** — Uncommitted receipt invisible; terminate blocked proposal connection; unavailable and complete rollback/frozen-context preservation. Reopened exact retry/replay creates one valid receipt/proposal; source unchanged.
19. **:1640 `draft_validation_checks_real_ir_with_auth_origin_and_bounds_without_writes_or_approval`** — Anonymous/invalid token, foreign origin, malformed/oversized body, forged approval and wrong types. Exact unsupported schema/unresolved speaker/unknown field/>2MiB errors. Owner and unrelated actor validate supplied bytes, but foreign evidence cannot save. Runs/submissions/evidence/reviews/source unchanged.
20. **:1787 `studio_editor_import_proposal_edit_accept_review_save_and_reopen_preserve_history`** — Full HTTP import→context→proposal→comparison→edited validation→acceptance→owner review→source→save journey. Stable provenance/work/adaptation/cues/IDs/roles; reader write/review denied; exact stale response, original acceptance/save/review retries, bounded metadata history/reviews. Reopened head/pinned/context/source exact; independent final counts/restart fixture.

## catalog_http_postgres.rs — NEEDS_PORT (#333), 17 tests

Common request helper validates success/error schemas, `no-store`/`nosniff`, JSON media type and 204 behavior. Independent counts cover work/unit/history/member/operation/clock/content rows.

### Native partial mapping, not replacement

- `src/contexts/catalog/domain/tests.rs`: `all_work_kind_and_unit_kind_pairs_obey_the_current_catalog_matrix`, `all_catalog_kinds_allow_units_without_a_script_source_digest_or_cast`, `work_patch_retains_omitted_fields_and_validates_the_resulting_snapshot`, `unit_patch_keeps_pinned_links_until_an_explicit_set_or_clear`, `optional_link_patch_serialization_distinguishes_keep_set_and_clear`, `optional_source_and_script_links_preserve_exact_digests_and_pinned_revision`, `link_admission_refuses_unpinned_revisions_noncanonical_ids_and_wrong_digest_families`, `archived_units_preserve_metadata_and_only_explicit_restore_makes_them_editable`, `archived_parents_block_unit_creation_edit_archive_restore_and_reorder`, `reorder_permutates_existing_active_slots_and_preserves_archived_tombstone_positions`, `reorder_requires_an_exact_active_permutation_and_refuses_duplicates_unknowns_and_archives`.
- `src/contexts/catalog/application/wire.rs::tests`: `creates_accept_all_declared_categories_without_script_or_cast_requirements`, `work_admission_accumulates_errors_without_echoing_private_input`, `unit_admission_accumulates_all_nested_reference_errors`, `metadata_sizes_count_utf8_bytes_and_allow_only_multiline_controls`, `work_patch_preserves_omitted_values_and_rejects_explicit_null_or_empty_patch`, `unit_patch_distinguishes_omitted_links_explicit_clears_and_pinned_replacements`, `archive_requests_validate_both_revision_guards_and_canonical_ids`, `reorder_reports_each_duplicate_or_invalid_entry_and_caps_the_batch`.
- `src/contexts/catalog/application/mod.rs::tests::cursor_requires_matching_actor_query_and_epoch` checks supplied cursor facts, not database pagination/epoch changes.
- `src/contexts/catalog/domain/content_tests.rs`: `equivalent_vietnamese_text_has_equal_content_digest_and_no_identity_change`, `rights_provenance_and_sequence_change_full_revision_but_not_content_binding`, `content_sequence_compare_and_swap_has_exact_conflicts_and_overflow`, `invalid_text_ids_and_declarations_cannot_construct_admitted_values`, `rights_order_does_not_create_an_owner_only_change`.
- `tests/server_turso_barriers.rs::catalog_rows_are_well_formed_keep_their_identity_and_their_history_is_append_only` reaches real-file raw-SQL ID/kind/head/digest/FK/identity/history/replace/upsert barriers, but not Studio mutation/replay/membership/reorder/error mapping.
- `tests/studio_contract.rs::{catalog_literal_acceptance_and_rejection_fixtures_match_the_schema,content_identity_literal_contracts_and_independent_digests_match_admission}` supplies literal schema/digest facts only.
- `tests/surface_routes.rs::{studio_routes_require_authentication_at_the_current_and_legacy_paths,legacy_studio_paths_are_flagged_deprecated_and_current_ones_are_not,session_endpoint_exists_for_studio_only_and_shares_validation_with_the_legacy_alias,origin_guard_covers_every_surface_for_mutations,content_identity_routes_exist_only_in_current_studio_and_never_share_its_session}` supplies limited pre-admission route/origin evidence, not successful Catalog HTTP or durable authority.

**Additional entrypoint dependency:** the tail includes `support/brief_http_postgres.rs` and `support/audio_lifecycle_http_postgres.rs`. Removing the parent also removes these nested legacy module entrypoints. Their oracles require separate inventory; they are not part of the 17 top-level count.

### Per-test ledger

1. **:38 `session_exchange_migrates_overlapping_cookies_and_confirms_actual_access`** — Duplicate cookies denied; current/deprecated login expire opposite-path cookies and preserve HttpOnly/SameSite/path, actor/session introspection and successful works access. Missing/ambiguous/revoked/expired sessions denied. No native successful cookie-migration/business parity.
2. **:494 `http_catalog_creates_all_categories_without_script_cast_and_rejects_wrong_unit_kind`** — Three categories create owned work/unit with correct chapter/episode kind, no script requirement and exact unit GET. Wrong unit kind gives 422 with no effects; exact totals.
3. **:539 `http_retry_returns_original_snapshot_duplicate_ids_and_changed_intents_do_not_commit`** — Create/patch/unit receipts exact after later changes; omitted synopsis retained. Changed intent, duplicate IDs, stale work/unit guards yield exact conflicts/current revision and unchanged independent counts.
4. **:630 `http_authorizes_every_work_and_unit_route_and_keeps_owner_only_mutations_private`** — Reader succeeds versus stranger 404/anonymous 401 on work/history/list/unit. Reader mutation/reorder forbidden; editor metadata allowed but visibility/rights/archive forbidden. Forged owner rejected; Carol creates/lists only own work; no denied effects.
5. **:733 `http_catalog_validates_fields_query_body_and_supported_representation_before_mutation`** — Multiple invalid category/ID/metadata issues. Reject msgpack request/Accept, bad Origin, >128KiB body, limit 0/51/non-number, malformed cursor/boolean query, invalid UTF-8 path, wrong method, anonymous create; zero operations.
6. **:811 `http_pagination_is_bounded_deterministic_scoped_and_stale_after_catalog_changes`** — Exact sorted/repeated work pages; cursor actor/query binding; another actor's write invalidates epoch. Unit page ordering exact; later append stales cursor.
7. **:895 `http_archive_preserves_history_and_tombstone_slots_and_reorder_is_atomic`** — Archived unit keeps slot; duplicate/missing/unknown/archived reorder input atomically rejected. Valid active permutation preserves tombstone position. Work archive current/filter/history exact, archived patch conflicts unchanged.
8. **:1013 `concurrent_http_reorders_have_one_winner_with_exact_revision_and_no_partial_commit`** — Two barrier-queued reorders: one 200, one stale_revision/current=4; whole original/reversed order, exact positions and four operation/work-revision rows.
9. **:1088 `http_waiting_mutation_rechecks_session_and_membership_at_transaction_boundary`** — Expiry/member removal while blocked returns 401/404; only intended membership deletion changes count; owner reads exact prior work.
10. **:1176 `http_optional_links_pin_exact_owned_historical_bytes_and_patch_omission_differs_from_null`** — Own source/script/digest link pins historical export after script advances; omitted links retained; same operation plus null differs/conflicts; fresh null clears. Foreign/bad SHA/export/content-digest links rejected; source/export bytes and digests unchanged.
11. **:1306 `committed_http_catalog_emits_exact_postgres_restart_oracle`** — Real HTTP creates work/unit, two content revisions, production brief and producer grant; emits exact requests/results/counts for restart partner. Native creator fixture absent.
12. **:1385 `catalog_http_recovery_after_server_restart`** — After DB restart/migration replay, exact work/unit, old pinned content/head, original-operation replay/counts, brief read/create replay, producer-grant replay and producer read.
13. **:1513 `content_categories_allow_text_and_recordings_without_script_ir_and_keep_metadata_separate`** — Text/recording accepted across categories without Script IR; NFC/LF and pinned GET exact. Metadata edit cannot move managed content; provenance-only append changes full digest not content digest. Original replay exact; changed/cross-API operation reuse conflicts; bounded metadata-only history; archived append denied but old content readable.
14. **:1647 `content_authorization_replays_and_validation_never_bypass_current_access`** — Editor append allowed; rights change forbidden; stranger hidden; forged owner/blank content rejected unchanged. Downgrade still reads but cannot replay old write; removal/anonymous denied. Body/media/Accept/Origin controls preserved.
15. **:1793 `content_links_retain_authoritative_sources_and_exact_owned_script_exports`** — Own source+script content pin survives later save. Bad checksum/export and foreign links rejected; revision under wrong unit hidden; original source/export bytes/digests unchanged.
16. **:1913 `content_concurrent_append_rollback_and_database_immutability_are_real_constraints`** — Fault after content insertion rolls back all rows. Competing appends have one creation, exact stale response and one head/history row. Owner UPDATE/DELETE/TRUNCATE denied; privileged malformed snapshot produces HTTP corrupt_revision.
17. **:2009 `content_read_baseline_records_actual_authorized_api_and_indexed_postgres_plans`** — 100 revisions; 30 exact head and bounded history reads, next_after=70; real app-role head/history plans and timings recorded. Native Studio read/performance equivalent absent.

## catalog_postgres.rs — NEEDS_PORT (#333), 20 tests

Common helpers count works/history/units/members/operations/clock; seed exact source bytes/evidence and accepted script exports. All count-preservation assertions are meaningful.

### Native partial mapping, not replacement

The Catalog domain/wire/cursor/content and real-file barrier candidates listed above apply here. In particular `src/contexts/catalog/domain/tests.rs::archived_work_permission_revision_retains_its_lifecycle_and_metadata` checks the pure permission-change revision; it does not revoke actual membership. `src/contexts/catalog/storage.rs::tests::{stored_snapshots_round_trip_through_admission,work_restore_rejects_invalid_metadata_and_revision,unit_restore_rechecks_parent_category_and_link_validation,snapshot_decoder_rejects_unknown_fields_duplicate_fields_and_oversize,snapshot_decoder_rejects_noncanonical_entity_ids}` directly decode bytes, not Catalog database/HTTP reads.

`src/platform/turso/tests.rs::{a_new_file_holds_the_registry_metadata_and_exactly_the_migrated_schema,foreign_diverged_newer_other_epoch_and_non_database_files_are_refused_unchanged}` and `business_tests.rs::{the_registry_of_eleven_upgrades_to_the_business_closure_without_rewriting_a_checksum,a_binary_of_eleven_refuses_the_file_of_fifteen_and_a_diverged_slice_is_never_adopted}` protect native registry facts, not old source/script/Catalog receipt-byte equivalence. Catalog Studio memberships, operations, reorder, successful adapter writes and late rechecks remain #333.

### Per-test ledger

1. **:205 `novel_story_and_podcast_accept_metadata_without_script_cast_or_scene`** — Three categories persist owned/category-matched unit without source/script, exact retrieval and counts; bad token denied.
2. **:241 `rejected_kind_invalid_metadata_stale_revisions_and_duplicate_ids_leave_no_rows`** — Exact wrong-kind, duplicate ID, stale expected/current and blank-title errors; all counts unchanged.
3. **:318 `work_patch_fingerprints_intent_and_replays_without_resolving_again`** — Omitted metadata retained; after later edit original operation returns original outcome while head stays later. Explicit additional field changes intent/conflicts; stale patch exact; counts unchanged.
4. **:407 `unit_patch_preserves_or_clears_links_and_replays_original_snapshot`** — Title patch retains source/digest, explicit clear removes, old replay returns original snapshot after clear. Changed intent conflicts; foreign source rejected; original source SHA unchanged.
5. **:524 `editor_work_patch_only_requires_owner_for_effective_protected_changes`** — Editor may title-patch while explicitly resupplying unchanged rights/visibility; actual visibility/rights changes forbidden with no effects.
6. **:592 `archived_work_owner_can_revoke_membership_and_block_history_and_replay`** — Member reads archived work until owner revokes. New permission revision preserves archived metadata/lifecycle and old snapshot bytes. Removed member loses work/unit/history/replay; owner sees six exact history entries; members zero/unit revisions unchanged.
7. **:710 `identical_operation_replays_original_snapshot_but_changed_payload_conflicts`** — Create receipt exact after later edit, current head remains later; changed same-operation title conflicts without rows.
8. **:762 `duplicate_unit_ids_never_reparent_and_foreign_ids_do_not_disclose_existence`** — Own duplicate yields DuplicateId, foreign duplicate NotFound; no reparent/effects. Foreign unit and work existence hidden.
9. **:819 `owner_reader_editor_and_revocation_are_checked_at_application_boundary`** — Stranger read/edit hidden; reader read/write partition; editor create/edit allowed, protected visibility/rights/archive/member changes denied. Revocation removes list/read access; revoked owner session unauthenticated; denied operations change no rows.
10. **:1008 `owner_can_remove_inactive_member_and_reactivation_does_not_restore_access`** — Remove inactive member successfully; exact work revision/receipt/clock/member counts. Reactivation does not restore read/list authority.
11. **:1074 `reorder_is_atomic_stable_identity_and_archival_preserves_history`** — Duplicate reorder rejected unchanged; reverse order/positions/IDs/revisions exact. Unit/work archive filters and gapless work history; archived edit blocked; five immutable unit revisions retained.
12. **:1227 `optional_legacy_links_require_owner_and_exact_pinned_digests`** — Foreign source/script, wrong SHA/export and missing rights rejected atomically. Valid own links stay pinned after later script save; original source/export bytes retained.
13. **:1367 `deterministic_pagination_rejects_stale_epoch_and_cross_actor_cursor`** — Three sorted single-item pages and terminal cursor; wrong actor/filter invalid, write stale, zero limit rejected.
14. **:1426 `concurrent_edit_and_reorder_have_one_winner_without_partial_child_changes`** — Barrier-queued edit/reorder: one success, one exact revision conflict; final child order whole original/reverse, four operations/history rows.
15. **:1510 `expiry_and_membership_revocation_while_waiting_reauthenticate_before_commit`** — Clock/work-row barrier × expiry/member revocation matrix; exact unauthenticated/NotFound, no changes beyond intended member deletion.
16. **:1611 `settled_catalog_records_are_immutable_and_app_has_no_delete_identity_update_or_ddl`** — App cannot delete entities/history, update snapshots/receipts/owner or DDL. Owner history/identity mutation and receipt truncation denied; plain truncate FK error distinct; counts intact. Native barriers cover only part and trust shift must be documented.
17. **:1673 `baseline_replay_preserves_exact_source_script_and_catalog_bytes`** — Repeated migration keeps source bytes/SHA, script export/digests, history snapshots/digests and operation result bytes/digests exact; work intact and source/script links usable afterward.
18. **:1717 `committed_catalog_fixture_survives_new_store_and_emits_server_restart_oracle`** — Reopened Store preserves current work and original create/unit receipts; independent counts and exact partner fixture emitted.
19. **:1775 `catalog_recovery_after_server_restart`** — Partner verifies actual DB restart/migration replay, exact current snapshot, original create/unit replay, unit title and row counts; records attributed result.
20. **:1844 `authorized_read_baseline_is_bounded_and_records_select_plans_at_one_hundred_works`** — 100 works, Alice owns 80; authorized bounded 20-item application page equals raw SELECT IDs. Limit/no disk spill, owner/order indexes and attributed timing/plans. Public listener discovery is not Studio ownership listing.

## delegation_http_postgres.rs — NEEDS_PORT (#333), 4 tests

Common helper validates delegation/content/Catalog/ApiError schemas, JSON/no-store and 204. Independent lookup oracle hashes decoded secret with purpose separator; counts include grants/revocations/lineage/operations/history/clock.

### Native partial mapping, not replacement

`src/contexts/catalog/domain/delegation_tests.rs` actually covers pure `action_labels_are_a_closed_exact_vocabulary`, `action_sets_are_non_empty_and_reject_duplicates_instead_of_merging`, `grant_lifetime_is_positive_bounded_and_overflow_safe`, `an_owner_cannot_delegate_to_themself`, `lookup_hash_matches_an_independent_oracle_and_is_purpose_separated`, `admission_is_exhaustively_exact_over_lifecycle_facts_and_clock_boundaries`, `authorization_requires_the_exact_current_unarchived_work_and_a_granted_action`, `every_action_subset_authorizes_exactly_its_members`, `delegated_content_may_not_bind_sources_or_scripts`, `request_digest_is_canonical_and_binds_every_owner_chosen_field`.

These supplied-fact tests do not prove actual owner issuance, one-time secret handling, stored hash, lineage attribution, live revocation, immutable delegation rows or deterministic write/revoke ordering. No native successful delegation HTTP or durable delegation-table replacement was found; #333 remains the owner.

### Per-test ledger

1. **:355 `delegate_reads_and_appends_only_inside_the_granted_work_under_its_own_identity`** — One-time secret; exact grant attribution/actions/revocation state and stored purpose-separated hash, distinct from session hash. Issuance replay reveals no second secret/row; changed intent conflicts. Own grant/work/unit reads, all other-work/unit/content routes hidden. Append attributed to delegate in content/work history and lineage, never owner; exact increments/replay. Foreign unit/source links/rights changes forbidden; head/history/owner view exact. Mixed/missing/wrong-purpose cookie/Bearer denied; owner grant list exact.
2. **:650 `revocation_expiry_deactivation_and_archive_end_delegated_access`** — Read-only action boundaries; idempotent one-row revocation and current denials/list timestamp. Missing grant/wrong-work revoke hidden. Delegate/grantor deactivation suspends, reactivation restores; DB-clock expiry denied. Work archive ends access/issuance; grant/revocation settled mutation refused.
3. **:855 `only_the_owner_issues_and_every_rejected_issuance_commits_nothing`** — Editor denied, stranger hidden for issue/list. Exact self/missing/inactive/invalid actor, >30-day/past expiry and empty/duplicate action field errors. Unknown action/extra grantor bad request, missing Origin forbidden; counts unchanged. Reused grant ID with new operation conflicts.
4. **:1015 `an_admitted_write_finishes_before_revoke_and_a_waiting_write_sees_the_revocation`** — Two actual deterministic schedules: admitted append commits before queued revoke then later token use denied; revoke commits first, queued append denied, only revocation count changes. Pure admission truth tables do not replace transaction-order evidence.

## Removed Editorial and Ingestion sources

All names in the first column identify the deleted source, recoverable from Git at
`84eea4c3:services/cantos-server/legacy-pg-tests/tests/`. They are not Cargo targets.
The native mappings exercise real Turso =0.8.3 files; Studio business fixtures explicitly use
synthetic development sessions, and do not imply Identity/browser acceptance.

### imports_postgres.rs — removed (9 original tests)

| Original test | Native test or mechanism retirement |
| --- | --- |
| `originals_receipts_failed_extraction_and_registry_survive_reopen_exactly` | `ingestion_turso::originals_receipts_failed_extraction_and_registry_survive_reopen_exactly` |
| `concurrent_duplicates_replay_once_and_changed_operation_is_rejected_atomically` | `test_support::editorial::tests::{competing_imports_preserve_one_receipt_and_reject_changed_operation,different_payloads_racing_one_operation_have_one_winner_and_actor_scoped_receipts}` |
| `actor_deactivation_while_waiting_for_import_lock_is_rechecked_before_insert` | `test_support::editorial::tests::waiting_saves_and_imports_recheck_admission_after_the_ordered_revocation` |
| `denied_sessions_and_wrong_owner_never_read_or_write_sources` | `ingestion_turso::denied_sessions_and_wrong_owner_never_read_or_write_sources` |
| `invalid_admission_and_injected_insert_or_connection_failure_leave_no_partial_registry` | `ingestion_turso::invalid_admission_and_injected_insert_failure_leave_no_partial_registry` now refuses both BEFORE and AFTER INSERT; real rollback plus clean exact retry; PG process-kill syntax retired |
| `imported_sources_are_immutable_tamper_detected_and_legacy_links_reuse_revision_store` | `ingestion_turso::script_links_share_the_preserved_original_but_never_grant_source_ownership`; `editorial_ingestion_storage` settled guards, reference/text/bytes corruption and `corrupt_failed_extraction_receipt_is_never_retrieved_as_an_accepted_import` |
| `baseline_replay_preserves_source_records_and_revisions_without_backfill` | `editorial_recovery_turso::repeated_native_migrations_preserve_business_bytes_and_corrupt_history_refuses_unchanged`; `ingestion_turso::legacy_text_only_sources_remain_readable_without_fabricating_an_import_receipt` |
| `import_restart_fixture_preserves_exact_bytes_receipt_and_same_operation` | `editorial_recovery_turso::killed_server_reopens_exact_revision_and_import_receipts_with_cross_codec_replay` (TXT, failed DOCX and failed UTF-8; exact byte download/receipt/replay after child kill) |
| `bounded_import_point_reads_keep_authorized_postgresql_index_baseline` | `ingestion_turso::bounded_native_import_point_reads_use_indexes_and_preserve_authorized_receipts`: 1,000 × 1KiB, native point/replay plans, 5 warmups/30 exact authorized HTTP reads, local warm p95 ≤250ms; no PG/production speedup claim |

### revisions_postgres.rs — removed (21 original tests)

| Original test | Native test or mechanism retirement |
| --- | --- |
| `editorial_reviews_are_pinned_replayable_owner_only_and_history_is_bounded` | `editorial_turso::editorial_reviews_are_pinned_replayable_owner_only_and_history_is_bounded` |
| `source_bytes_operator_credentials_and_review_rollback_cross_real_boundaries` | `operator_turso::actual_source_and_token_commands_preserve_originals_and_revoke_access_after_reopen`; `test_support::editorial::tests::rejecting_a_final_step_rolls_back_the_provisional_script_and_review`; `editorial_ingestion_storage` settled-row/corrupt-load tests |
| `baseline_replay_preserves_accepted_revision_bytes_without_retired_runtime_tables` | `editorial_recovery_turso::repeated_native_migrations_preserve_business_bytes_and_corrupt_history_refuses_unchanged`; native history is 16 independent versions; PG epoch/count syntax is obsolete |
| `baseline_checksum_mismatch_and_unrecognized_database_are_refused_without_mutation` | Same native replay/checksum test; shared-store `files_that_are_not_ours_are_refused_without_being_written` and `a_changed_migration_or_a_tampered_history_is_detected` (real files) |
| `complete_exports_metadata_retries_and_pinned_history_survive_edits` | `editorial_turso::complete_exports_metadata_retries_and_pinned_history_survive_edits` |
| `backend_denies_anonymous_wrong_owner_reader_revoked_and_expired_sessions` | `editorial_turso::backend_denies_anonymous_wrong_owner_reader_revoked_and_expired_sessions` |
| `invalid_exports_and_foreign_evidence_leave_no_partial_script` | `editorial_turso::invalid_exports_and_foreign_evidence_leave_no_partial_script` |
| `concurrent_stale_saves_and_duplicate_delivery_are_atomic` | `test_support::editorial::tests::competing_saves_have_one_winner_and_duplicate_delivery_has_one_revision` (controlled writer order, exact rows) |
| `killed_connection_before_commit_rolls_back_and_same_key_can_retry` | `editorial_turso::failure_after_a_new_revision_insert_preserves_the_previous_head_and_retries_once`; real AFTER INSERT failure replaces PG backend termination; unknown COMMIT is separately tested in `lost_commit_acknowledgements_reconcile_each_receipt_without_duplicate_effects` |
| `app_privileges_triggers_and_load_checks_guard_immutable_history` | `editorial_ingestion_storage::{settled_editorial_and_ingestion_rows_reject_update_delete_replace_and_upsert,stored_corruption_is_refused_on_revision_history_review_and_import_reads}`; PG roles/TRUNCATE/DDL grants retired with explicit owner/file trust shift |
| `restart_fixture_for_process_and_database_recovery` | `editorial_recovery_turso::killed_server_reopens_exact_revision_and_import_receipts_with_cross_codec_replay` (actual child kill, fresh child, three codecs, persisted counts) |
| `session_exchange_origin_guard_and_malformed_inputs_are_safe` | `editorial_turso::session_exchange_origin_guard_and_malformed_inputs_are_safe` |
| `msgpack_and_json_carry_the_same_revision_facts_on_every_negotiated_route` | `editorial_turso::msgpack_and_json_carry_the_same_revision_facts_on_every_negotiated_route` |
| `a_lost_msgpack_response_replays_in_either_representation_without_a_second_revision` | `editorial_turso::a_lost_msgpack_response_replays_in_either_representation_without_a_second_revision` |
| `refused_representations_write_nothing_and_other_routes_stay_json_only` | `editorial_turso::refused_representations_write_nothing_and_other_routes_stay_json_only` |
| `msgpack_requests_get_the_same_origin_session_and_permission_decisions` | `editorial_turso::msgpack_requests_get_the_same_origin_session_and_permission_decisions` |
| `legacy_studio_paths_remain_compatible_and_sessions_stay_scoped_to_their_mount` | `editorial_turso::legacy_studio_paths_remain_compatible_and_sessions_stay_scoped_to_their_mount` |
| `protobuf_json_and_legacy_msgpack_have_identical_revision_and_validation_facts` | `editorial_turso::protobuf_json_and_legacy_msgpack_have_identical_revision_and_validation_facts` |
| `lost_json_and_protobuf_responses_replay_both_directions_without_another_write` | `editorial_turso::lost_json_and_protobuf_responses_replay_both_directions_without_another_write` |
| `protobuf_refusals_preserve_database_state_and_do_not_expand_other_routes` | `editorial_turso::protobuf_refusals_preserve_database_state_and_do_not_expand_other_routes` |
| `protobuf_authorization_and_legacy_aliases_keep_existing_json_error_taxonomy` | `editorial_turso::protobuf_authorization_and_legacy_aliases_keep_existing_json_error_taxonomy` |

The two transaction fixes are within the existing owner: `record_source` pre-checks immutable
existing evidence and binds the transaction clock for `recorded_at`; import replay states the
partial unique index predicate explicitly for the pinned planner. No registered DDL, schema
version, Script IR bytes, digests or wire error meaning changes. Full-file recovery is native
reopen, not PostgreSQL conversion; #336 still owns real legacy data/prefix recovery.
