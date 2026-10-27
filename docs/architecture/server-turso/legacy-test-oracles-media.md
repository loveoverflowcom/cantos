# Media / Production legacy PostgreSQL oracle retirement ledger

> **Historical source-only review.** NEEDS_PORT and missing-adapter statements below describe
> the original pinned inventory, not current `develop`. The subsequent
> [native retirement mapping](remaining-pg-retirement.md) and [exact oracle ledger](remaining-pg-oracles.json)
> supersede those porting conclusions. Preserve the old assertions and evidence as history.

> Historical, source-bound inventory at `12fb751ec71e20233fe4caaee35e4544fe4a0d4b`.
> Its recommendations and environment observations describe that revision, not the current
> checkout. Current retirement dispositions, real native test boundaries and limitations are in
> [the native retirement record](remaining-pg-retirement.md) and [exact mapping](remaining-pg-oracles.json).

Appendix to the [retained-oracle coverage ledger](legacy-test-oracles.md). Native mappings are source-only, not fresh behavioral acceptance.


Pinned source: `12fb751ec71e20233fe4caaee35e4544fe4a0d4b`.

## Result

**94 tests across 11 listed files; all 11 files NEEDS_PORT. No whole-file deletion is justified by this inventory.** The 11,038 source lines were read in full, including assertions, negative cases, fixtures and support dependencies. Native candidates below are statically Cargo/module-reachable sources, not an executed test listing or a passing run. Native execution is **NOT_RUN** because Cargo is absent from this cloud environment's PATH.

Safety boundary: native foundation/session and listener-reader tests do not replace Studio/Production/Media write acceptance. Actual source still retains PostgreSQL-shaped writer adapters, and the #333 checkpoint explicitly defers those ports. Pure/fake/storage-only/native-listener overlaps are credited only for their stated boundary. PG-only role/lock/SQLSTATE/EXPLAIN mechanics may be explicitly retired during porting without dropping the meaningful business assertions surrounding them.

Counts: audio intake 21; audio lifecycle support 11; Production 11; runtime grants 2; artwork storage HTTP 4; artwork SQL support 10; artwork repository support 1; brief support 8; publication support 12; Theatre catalog-walk support 4; Theatre development-seed support 10.

Scope/paths: archived support sources are under `services/cantos-server/legacy-pg-tests/tests/support/`. Normal package `tests/*.rs` and wired `#[cfg(test)]` modules are native discovery roots; the archived tree is not a Cargo target. This source inventory changes no schema, contract, gate or historical evidence claim.

## Legacy Media audio oracle assertion ledger

BASE: `12fb751ec71e20233fe4caaee35e4544fe4a0d4b`.

Both files must remain **NEEDS_PORT**. No complete test in either file has a demonstrated native replacement.

- `services/cantos-server/legacy-pg-tests/tests/audio_intake_postgres.rs`: **21 tests**.
- `services/cantos-server/legacy-pg-tests/tests/support/audio_lifecycle_http_postgres.rs`: **11 tests**.
- Full sources read, including helpers and module wiring.
- Cargo is absent from PATH; execution and Cargo test listing are **NOT_RUN**. Native discovery below is established structurally from workspace/package manifests, top-level `tests/*.rs`, and wired `#[cfg(test)]` source modules, not an executed `cargo --list`.

### Decisive implementation boundary

- `src/contexts/media/turso.rs` explicitly limits Turso adapters to public-listener reads/indexing; Studio intake, lifecycle and publication writes remain PostgreSQL SQL.
- `src/platform/turso/audio_release_chain.sql` explicitly excludes upload intents, intake receipts, reviews, events/operations and Media write transactions.
- `src/platform/turso/media_audio.sql` remains a reserved placeholder.
- Searching native tests and `src` finds no test calling `create_audio_intent`, `upload_audio_intent`, `load_audio_intent`, `create_audio_submission`, `upload_audio_submission`, `review_audio_submission` or `create_audio_release_candidate`. Their live call sites are handlers and application implementations, not native integration tests.
- Native release-chain fixture seeding and listener reads do not accept the Studio writes that create the chain.

### Native candidate reference key

Paths below are relative to `services/cantos-server/` unless otherwise stated. Every mapping is **partial only**, and every named source was inspected beyond its test name.

#### P: `src/contexts/media/domain/intake.rs`

- `exact_operation_replays_after_revision_advances`
- `operation_cannot_change_any_pinned_binding`
- `new_intent_needs_exact_current_revision_and_digest`

These test the pure prepare decision, not persisted IDs, transactions, authorization or HTTP.

#### I: `tests/audio_intake.rs`

- `four_profiles_accept_actual_mono_ogg_opus_and_derive_exact_byte_identity`
- `actual_wrong_codec_container_channels_profile_and_corruption_are_rejected`
- `profiles_and_byte_admission_do_not_use_filenames_or_mime_claims`

The first two are ignored, tool-backed inspection tests explicitly selected by the audio-inspection gate. They exercise actual media inspection, not DB/HTTP admission.

#### C: `tests/audio_intake_contract.rs`

- `independent_safe_fixtures_conform_to_schema_and_shared_raw_transport`
- `schema_and_raw_consumer_close_each_object_against_forged_approval_or_delivery_facts`
- `profile_ids_are_closed_explicit_and_map_to_exact_codec_envelopes_without_a_lower_floor`
- `private_media_schema_bounds_duration_size_identity_and_observed_channel_rate`
- `historical_aac_proposal_never_becomes_an_accepted_current_intake_profile`
- `schema_refuses_zero_or_unpersistable_revisions_and_noncanonical_pins_or_public_urls`
- `omitted_or_null_receipt_means_only_no_accepted_receipt`
- `audio_rejection_conforms_to_shared_studio_error_schema`

Literal/schema/DTO coverage only.

#### L: `src/contexts/media/lifecycle_tests.rs`, wired by `domain/lifecycle.rs`

- `every_binding_field_and_rights_declaration_is_exact`
- `upload_decision_has_exact_digest_revocation_expiry_stale_and_replay_errors`
- `settled_records_reject_tamper_and_review_operation_id_split`
- `gate_matches_independent_truth_table_and_never_implies_publication`
- `latest_rejection_withdrawal_and_changed_candidate_are_blockers`
- `explicit_state_event_matrix_rejects_terminal_admission`
- `upload_clock_cannot_precede_creation_or_settled_admission`
- `a_settled_rendition_cannot_be_replayed_for_another_submission`
- `submission_windows_admit_only_positive_bounded_lifetimes`
- `submission_window_matches_wider_integer_lifetime_and_membership_oracles`

Pure admitted values and decision facts, not transactions, credentials or real bytes.

#### S: `src/contexts/media/storage.rs`

- `immutable_object_survives_duplicate_and_adapter_restart`
- `concurrent_equal_uploads_preserve_one_complete_object`
- `tampered_existing_bytes_are_rejected_and_never_replaced`
- `missing_truncated_writable_and_symlink_objects_fail_closed`
- `private_port_reads_full_first_middle_final_and_suffix_ranges_in_bounded_chunks`
- `private_port_rejects_missing_wrong_length_and_another_objects_normalized_range`
- `partial_private_staging_orphans_are_never_accepted_or_collected_on_restart`

These cover real private filesystem mechanics with synthetic storage bytes, not persisted Media receipts or owning HTTP authorization.

#### R: `src/contexts/media/storage/registry_tests.rs`

- `changing_active_write_target_keeps_historical_reads_on_the_recorded_target`
- `missing_historical_target_never_falls_back_to_a_healthy_active_target`
- `registry_forwards_the_exact_immutable_key_and_integrity_to_its_selected_adapter`
- `registry_preserves_typed_port_failures_without_retry_or_target_fallback`
- `missing_or_corrupt_accepted_objects_remain_corrupt_revision_failures`

These use named recording doubles. `inspected_image_writes_and_historical_reads_share_the_existing_target_registry` additionally uses actual image/filesystem data, not audio receipt rows.

#### T: `src/adapters/inbound/http/audio_transport/tests.rs`

- `single_ranges_cover_closed_open_suffix_case_and_unsatisfiable_requests`
- `a_valid_multi_range_is_ignored_whole_never_fake_multipart_or_partial_content`
- `head_ignores_range_and_if_range_even_when_duplicate_or_malformed`
- `if_range_requires_exact_strong_tag_and_never_invents_a_timestamp`
- `successful_get_and_range_bodies_are_exact_binary_bytes_without_metadata_negotiation`
- `head_has_whole_length_without_polling_source_and_releases_its_lane`
- `conditional_and_416_metadata_are_private_and_preserve_caller_owned_error_bodies`

Planning/response mechanics with supplied metadata and named source doubles, not DB-derived read authority.

Additional partial readiness coverage:

- `src/contexts/media/read_application.rs::missing_target_cannot_construct_an_authorized_conditional_audio_read`
- `src/adapters/inbound/http/studio/audio_reads.rs::studio_projects_every_transport_error_to_its_own_safe_wire_contract`
- `src/adapters/inbound/http/studio/audio_reads.rs::studio_head_transport_errors_have_no_body`

#### H: `tests/surface_routes.rs`

- `private_audio_reads_require_studio_authentication_and_have_no_other_surface_alias` tests missing/ambiguous cookies and producer Bearer denial on GET/HEAD; API/Theatre/Admin read aliases remain absent. It deliberately decides before store access.
- `origin_guard_covers_every_surface_for_mutations` is generic pre-store origin policy, not successful audio route authorization.

#### B: `tests/server_turso_barriers.rs`

- `the_audio_release_chain_is_digest_checked_bound_and_immutable_and_the_pointer_only_moves_forward`

Actual file-backed Turso guards for selected release-chain rows, including submission/rendition/candidate immutability, duplicate submission token, unknown brief FK and duplicate rendition. It does not cover intents/receipts or reviews. Its guard list also does not exercise the revocation table, despite that table existing in migration 14. PG role privileges/TRUNCATE SQLSTATEs are not transferable native mechanisms.

#### Static partial coverage

- `src/adapters/inbound/http/tests.rs::api_router_serves_no_static_files` exercises production API denial and development editorial-only static success.
- `tests/private_artwork_http.rs::explicit_private_artwork_host_disables_filesystem_static_fallback` exercises one static GET with an explicit artwork host.

Neither tests the legacy audio case's overlapping roots, nested symlink and GET/HEAD matrix around an actual accepted audio object.

### Per-test ledger: `audio_intake_postgres.rs`

All 21 tests retain **NEEDS_PORT**. Candidate references identify partial overlap only.

Shared helpers: every `Harness::request` response asserts `cache-control: no-store` and `x-content-type-options: nosniff`; `owner_audio_request` independently asserts `private, no-store, no-transform` for GET/HEAD, `no-store` for PUT, and `nosniff`. Synthetic encoding asserts successful ffmpeg execution and nonempty encoded bytes. Harness setup asserts the disposable cluster user/database restrictions. Owned lock helpers have bounded waits that assert the intended fault point was reached rather than assuming a race occurred.

#### 1. `intent_is_explicit_revision_bound_and_exact_operation_replays_original_identity` (366)

- Exact script ID/revision/content digest/profile/created actor/operation ID and Studio status/upload paths.
- Exact same operation returns original full intent.
- Changed profile, revision or digest under that operation returns `OperationReused`.
- Reusing operation on another owned script returns `OperationReused`.
- Loaded pending status returns same intent, no receipt, DB counts `(1,0)`.
- Partial P, C. Missing persisted original identity/path/actor, operation namespace across scripts, pending status and row counts.

#### 2. `invalid_or_stale_target_never_creates_an_intent_and_there_is_no_default_profile` (431)

- Empty/default/unknown-version/AAC profile gives HTTP 400 `invalid_request`; missing profile also 400.
- Revision 2 against head 1 gives 409 `stale_revision` with `current_revision: 1`.
- Wrong content digest gives 400 `invalid_request` with `expected_content_digest/content_digest_mismatch` issue.
- All failed requests leave counts `(0,0)`.
- Four allowed profiles create HTTP 200 intents with exact returned profile; counts `(4,0)`.
- Partial P, C, I. Missing real HTTP error mapping, atomic non-insertion and persisted creation per profile.

#### 3. `only_current_owner_can_create_read_or_upload_and_intent_scope_cannot_cross_scripts` (487)

- Missing session gives 401; unrelated actor gives 404 on create/status/upload.
- Reader and editor script members each get 403 on all three operations.
- Both original owner and another owner get 404 if intent ID is used under another script on GET/PUT.
- Counts stay `(1,0)`.
- H does not cover current-owner/member authorization or wrong-script DB scoping.

#### 4. `http_keeps_same_origin_cookie_binary_and_current_studio_surface_boundaries` (562)

- Missing/untrusted Origin gives 403 `forbidden` on valid upload route.
- Duplicate session cookie gives 401.
- Missing content type, application/json and audio/ogg each give 415.
- Unprefixed API, Theatre and Admin aliases give 404 on create/status/PUT.
- Counts stay `(1,0)`.
- Generic origin/H overlap is pre-store and read-only. Audio-specific PUT content-type/error/header and create/status alias matrix remains unproven.

#### 5. `accepted_receipt_contains_observed_media_and_one_private_object_with_no_delivery_url` (640)

- Upload 200 and receipt exact intent/script/revision/digest/profile/operation/accepted actor binding.
- Actual SHA-256 and byte length; ogg/opus/mono/48k; bounded approximately-one-second duration and bitrate; exact profile.
- No object_key/path/playback_url/download_url/published in receipt or media.
- DB object_key equals hash and persisted receipt bytes equal canonical serialization.
- Stored file exactly equals uploaded bytes, Unix mode 0400.
- GET status returns exact receipt; counts `(1,1)`.
- Partial I, C, S. Missing integrated actual-byte admission, canonical durable receipt, omission checks on real responses and status.

#### 6. `same_bytes_replay_exact_receipt_and_changed_valid_media_conflicts_without_replacement` (710)

- Same bytes return exact original HTTP status/body.
- Independently encoded different valid bytes differ and give 409 `operation_reused`.
- GET still returns first receipt; counts `(1,1)`.
- S duplicate-file tests cannot replace DB receipt identity/replay/conflict behavior.

#### 7. `rejected_media_and_body_limit_leave_intent_pending_with_no_accepted_row` (733)

- Empty, nonaudio, half-truncated Opus, WAV, stereo Opus and exactly MAX_BYTES garbage each give 422 `audio_rejected` and counts `(1,0)`.
- MAX_BYTES+1 gives 413.
- GET receipt stays null, counts unchanged.
- Subsequent valid upload succeeds.
- Partial I and audio_intake.rs unit size/spoof validation. Integrated size-limit status, retained pending intent, zero acceptance and recoverable valid retry are absent.

#### 8. `lost_http_response_reconciles_and_fresh_store_replays_durable_receipt` (770)

- Real upload response 200, then response discarded unread.
- Fresh Store loads same intent and committed receipt.
- Same upload on fresh Store returns that exact receipt.
- Durable bytes match canonical receipt; counts `(1,1)`.
- Generic store durability or S restart is not per-use-case lost-response reconciliation.

#### 9. `new_script_revision_blocks_pending_acceptance_and_preserves_historical_accepted_status` (820)

- Pending upload after new script head gives 409 `stale_revision/current_revision: 2`.
- Fresh Store replays original prepare operation as original pending intent despite head advance.
- Upload replay for old accepted intent conflicts, but historical status returns original receipt.
- Historical owner GET/HEAD remains 200 with audio/ogg, full length, exact SHA ETag; GET exact bytes, HEAD empty.
- Counts `(2,1)`.
- P covers pure prepare replay after advance; T/S isolated transport/storage. Missing persisted distinction between stale upload authority and historical read authority.

#### 10. `revoked_expired_or_inactive_owner_cannot_read_replay_create_or_accept` (880)

- For revoked session, expired session and inactive actor independently: pending status/upload, accepted status/upload replay and new create all give 401.
- Previously accepted metadata remains, counts `(2,1)`.
- Native general session tests cannot establish every Media use case re-admits authorization and preserves receipts.

#### 11. `owner_deactivation_after_inspection_is_rechecked_before_receipt_commit` (938)

- Owned lock exposes post-inspection/pre-commit reauthorization point.
- Deactivation after inspection yields `Unauthenticated` and counts `(1,0)`.
- After reactivation and fresh Store, intent has no receipt.
- No native Media writer/failpoint test. Port invariant using Turso serialized-writer boundary, not PG lock syntax.

#### 12. `concurrent_duplicate_intents_and_bytes_commit_one_original_immutable_receipt` (983)

- Concurrent exact prepare calls return same intent; counts `(1,0)`.
- First receipt insert held; duplicate cannot complete before original acceptance commits, both demonstrably wait.
- After release both uploads return identical receipt; counts `(1,1)`.
- S concurrent file creates cover object dedup only, not operation/receipt winner ordering.

#### 13. `receipt_insert_rollback_keeps_pending_intent_and_restart_retry_accepts_once` (1050)

- Trigger failure after private persistence causes `Unavailable`.
- Counts `(1,0)`; fresh Store sees pending/no receipt.
- Remove failure, fresh Store accepts and exact duplicate replays; counts `(1,1)`.
- S orphan coverage does not establish DB rollback, pending status or one durable receipt.

#### 14. `terminated_database_transaction_cannot_commit_acceptance_and_restart_can_reconcile` (1096)

- Held receipt insertion transaction demonstrably terminated.
- Upload yields `Unavailable`; counts `(1,0)`.
- Fresh Store sees no receipt; later retry accepts, identical retry returns same receipt; counts `(1,1)`.
- PG termination technique is implementation-specific; interruption/no-false-acceptance/reconciliation oracle is not obsolete.

#### 15. `settled_intents_and_receipts_reject_update_delete_truncate_even_for_test_admin` (1152)

- Runtime role lacks UPDATE/DELETE/TRUNCATE on each intent and receipt table.
- Each UPDATE/DELETE/TRUNCATE rejected for admin (55000) and app (42501).
- Counts remain `(1,1)`.
- B excludes intents/receipts. PG principals/SQLSTATEs require native mechanism translation, not silent removal of append-only semantics.

#### 16. `database_binding_and_receipt_digest_are_independent_integrity_barriers` (1209)

- Direct wrong script-content digest intent insert rejected with 23514; zero rows.
- Disposable admin bypass corrupts stored receipt bytes without changing digest.
- Fresh Store reports `CorruptRevision`; HTTP gives 500 `corrupt_revision`; rows remain `(1,1)`.
- C schema or pure digest validation does not replace independent DB binding and adapter decode barriers.

#### 17. `tampered_private_object_blocks_status_and_replay_instead_of_claiming_verified_acceptance` (1253)

- Same-length byte mutation followed by 0400 restoration makes fresh Store status and identical upload replay each `CorruptRevision`.
- Deleting object also makes status `CorruptRevision`.
- Receipt/intent rows remain `(1,1)`.
- Strong S/R partial overlap, but no native persisted receipt path re-verifies objects on status and replay.

#### 18. `overlapping_public_static_roots_never_serve_private_accepted_object_bytes` (1302)

- With actual accepted object, private root, objects root and parent as supplied public roots each deny GET/HEAD at object's relative path with 404.
- Counts `(1,1)`.
- Production API no-static test is partial; audio-enabled development router overlap matrix is not demonstrated.

#### 19. `audio_enabled_host_blocks_nested_symlinks_and_regular_static_files` (1346, Unix)

- Confirms public/private roots disjoint.
- Audio-enabled host denies GET/HEAD to nested symlink pointing at accepted objects and ordinary index.html.
- Editorial-only host without audio storage serves GET/HEAD index.html; GET exact marker bytes.
- Counts `(1,1)`.
- Existing static/artwork tests cover isolated pieces, not this matrix. Not obsolete merely because production API has no static fallback: native development router still serves editorial-only static content.

#### 20. `changing_the_write_target_preserves_exact_historical_reads_without_copying_or_fallback` (1500)

- Initial accepted record targets legacy-filesystem and stores canonical original receipt bytes.
- Switch write target; new root initially empty.
- Old owner GET/status work while replacement root remains empty.
- New different bytes target next-filesystem, exist only there; old bytes never copied.
- Old upload replay returns original receipt and unchanged storage-target/key/receipt-bytes/digest record.
- Both owner reads return exact bytes/type/length.
- Remove historical target: old GET gives 503 `unavailable` while new GET remains exact success.
- Original record unchanged; counts `(2,2)`.
- R is routing-double coverage plus an actual image analogue. Missing audio receipt persistence, target transitions, immutable receipt reconciliation and HTTP failure isolation.

#### 21. `baseline_replay_preserves_receipt_bytes_digests_storage_targets_and_reads` (1597)

- Initial registry exactly four historical migrations.
- Two migrate replays preserve receipt storage record and exact version/checksum registry.
- Version 1 appears once.
- Receipt/rendition storage_target_id columns NOT NULL with legacy-filesystem default.
- Fresh Store reloads exact receipt; owner GET length/bytes intact; counts `(1,1)`.
- PG migration-count/checksum mechanics are legacy-specific, but migration replay preserving accepted metadata/targets/bytes is not covered by native script-intent tests. Retain source-bound oracle; never reinterpret as native restore PASS.

### Per-test ledger: `support/audio_lifecycle_http_postgres.rs`

All 11 tests retain **NEEDS_PORT**.

#### Shared helper assertions

- AudioHarness creates real catalog/content/brief records through HTTP using recording or text and exact selected profile.
- Parent Harness JSON calls assert no-store, nosniff, JSON type except 204 and schema conformance. `.success` asserts exact status; `.error` asserts exact status and error code.
- `upload` asserts no-store and validates `AudioRenditionDocument` or `ApiError`.
- `preview` asserts caller-selected cache contract plus nosniff.
- Synthetic encoding asserts ffmpeg success. Owned failpoint waits assert the intended transaction state was reached within a deadline.
- These response/schema assertions must not disappear from a future port ledger.

#### 1. `owner_preview_pins_actual_rendition_bytes_ranges_and_independent_surface_authorization` (246)

- Actual upload hash/length/profile match independently computed facts.
- No-registry and replacement-only-registry hosts return 503 for GET/HEAD, unconditional and with if-none-match *, mismatching if-match or unsatisfiable Range; validators cannot bypass unavailable original reader.
- Full GET exact bytes/hash, audio/ogg, full length, accept-ranges bytes, strong hash ETag, no content-range/content-encoding.
- HEAD ignores invalid range, full metadata, no body/content-range.
- First/middle/suffix/open-final/clamped-final/case-insensitive byte ranges return 206 and exact slices, lengths, content-range/type/ETag.
- Matching If-Range gives 206; unknown range unit or mismatching If-Range full 200.
- Unsatisfiable GET gives 416, bytes */length and invalid_request.
- Valid multipart request intentionally full 200, not fake multipart.
- Filesystem read-access gives 303 to exact owner byte route, no read-expiry header, empty body, no-store.
- Wrong owner 404; absent session 401; upload Bearer without owner cookie 401; pending submission, wrong rendition and wrong unit 404; Theatre alias 404 with exact safe minimal error.
- T/S/H/read-readiness provide extensive partial coverage. No native integration binds preview to admitted rendition rows and owning authority, exercises 303, or proves readiness-before-conditionals end to end.

#### 2. `audio_all_profiles_recording_and_text_admit_real_bytes_replay_and_keep_declarations_separate` (520)

- Four profiles alternate recording/text.
- Prepare awaiting_bytes; duplicate prepare exactly equal.
- Actual upload preserves exact binding and producer declarations separately from observed hash/length/profile.
- No object_key/upload_token leak.
- Exact bytes replay exact rendition.
- Owner status admitted, exact rendition, one row.
- Changed expected SHA under original operation gives 409 operation_reused.
- Fresh Store loads exact status.
- I/C/L only isolated portions. Writer/restart/declaration separation/HTTP replay absent.

#### 3. `audio_wrong_owner_scope_digest_unknown_profile_and_stale_brief_make_no_partial_records` (581)

- Wrong owner 404 and exact unchanged count object.
- Wrong brief ID/content revision ID/content digest/profile/work each 400 invalid_request and unchanged counts.
- Malformed UTF-8 unit path and script path 400.
- PATCH submission collection, PATCH script-intent collection and POST producer-status endpoint each 405 invalid_request; producer error schema checked.
- After prepare, Studio session token cannot act as upload capability (401); wrong byte digest 400.
- Wrong owner and wrong unit status each 404; counts unchanged.
- New brief makes old upload 409 stale_production_inputs; counts unchanged; status stale.
- L covers binding/digest/stale decisions, not route/credential/HTTP/no-partial-write parity.

#### 4. `audio_concurrent_finalize_deduplicates_object_and_rendition_and_failure_retries_cleanly` (698)

- Post-object-persistence rendition insert failpoint gives 503 unavailable, zero rendition rows.
- One complete private orphan remains, not named .pending.
- After removing failure, concurrent same-byte finalizations return equal rendition, one DB rendition and one object.
- Distinct submission with same bytes gets distinct rendition ID, equal verified media, still one object.
- S covers concurrent object dedup/orphans only. Missing rollback/retry and distinct-submission receipt identity.

#### 5. `audio_reviews_candidates_bind_exact_bytes_and_latest_decisions_without_a_public_release` (742)

- QC/rights/editorial approved review writes each 201; exact operation replay equal; changed decision under same operation 409 operation_reused.
- Candidate 201, only public_delivery_not_implemented blocker; exact candidate replay equal.
- Wrong review owner 404; wrong SHA 400.
- Later QC rejection sequence exactly 4; original immutable candidate retained.
- Candidate read blockers exactly qc_not_approved, candidate_reviews_changed, public_delivery_not_implemented.
- Rights withdrawal succeeds; status exposes three latest reviews, DB retains five history rows.
- Four unrelated public/admin/studio-release endpoints 404.
- App DELETE denied 42501 and admin TRUNCATE denied 55000 across submissions/renditions/reviews/candidates/revocations.
- L covers pure blockers/validation; B selected other release rows, not review persistence or revocation guard cases. Missing latest/history persistence, operation receipts, authorizations, exact HTTP blocker sets and complete append-only matrix.

#### 6. `audio_revocation_expiry_and_two_way_read_upload_credential_separation_are_enforced` (890)

- Upload token first: producer read grant using same secret rejected 422 invalid_request.
- Owner revoke 204; upload then 401; exactly one revocation row.
- Read grant first: grant 201; submission with same secret 400 invalid_request; zero submissions.
- Short-lived prepared upload expires: upload 401, zero renditions, owner status expired.
- L expiry/revocation decisions only. No native shared-secret namespace exclusion, both orderings, durable revocation or exact HTTP status coverage.

#### 7. `audio_actual_corruption_missing_and_tampered_objects_fail_closed_and_cleanup_staging` (948)

- Actual nonaudio bytes 422 audio_rejected, zero rendition rows and zero objects.
- Valid second submission admitted.
- Tampered then missing accepted object each causes owner status 500 corrupt_revision.
- Existing rendition row remains one.
- I/S/R partial; missing staging cleanup/non-admission plus DB status integrity check.

#### 8. `audio_expiry_after_rendition_insert_rolls_back_and_preserves_only_private_orphan` (987)

- Owned insert failpoint observably reached within deadline.
- Complete private object exists before commit/authorization completes.
- Permission expires while insertion held.
- Release failpoint: upload 401, zero rendition rows, owner status expired.
- Pure clock/storage orphan tests do not prove post-I/O/post-insert authority recheck and rollback. Needs native deterministic transaction-time failpoint, not PG sleep/lock clone.

#### 9. `audio_read_upload_grant_race_admits_only_one_capability_for_a_secret` (1036)

- Concurrent read-grant and upload-submission creation with same secret have XOR success.
- Total committed grant+submission rows exactly one.
- No native cross-context concurrency/secret-exclusivity test. Same-table token uniqueness in B is insufficient.

#### 10. `audio_restart_fixture_pins_real_private_bytes_receipts_reviews_candidates` (1057)

- Real HTTP submission prepare, actual media upload 200, QC review 201, candidate 201 and owner status 200, all helper headers/schemas.
- Writes database/root/unit/request/rendition/review/candidate request/candidate/status/token data to ignored audio-restart.json and intentionally preserves private root.
- Test itself does not assert restart; downstream runner does.
- Historical fixture prerequisite, not redundant smoke. No native equivalent creates exact chain through owning HTTP.

#### 11. `audio_read_baseline_uses_current_owner_app_role_and_exact_bounded_point_queries` (1096)

- Prepare/admit/review/candidate through real HTTP.
- Twenty status reads return exact rendition and twenty candidate reads exact original candidate.
- Each local warm p95 sample set at most 500ms.
- Actual query connection role cantos_app.
- Executes EXPLAIN (ANALYZE,BUFFERS) for exact bounded submission/rendition/latest-review/candidate queries.
- Writes plan/sample/role/server-version/object-byte/provenance record; explicitly no production SLO/cardinality or DBSP claim.
- PG role/EXPLAIN syntax is historical measurement, but listener benchmarks would lose owner status/candidate consumer and integrity-read workload. Native write prerequisites, exact point-read behavior and attributable baseline are absent.

### Support consumers and deletion hazards

- `catalog_http_postgres.rs:2088-2089` includes this module as `audio_lifecycle_http`.
- `audio_lifecycle_http_postgres.rs:1208-1209` includes `publication_http_postgres.rs`.
- `publication_http_postgres.rs` uses parent AudioHarness, UPLOAD, upload, preview and review_request helpers to construct real private admission/publication fixtures.
- `publication_http_postgres.rs:923` and `:926` nest `theatre_catalog_walk_http_postgres.rs` and `theatre_dev_seed_http_postgres.rs`.
- `scripts/test_postgres.py` consumes audio-restart.json at `:994`; prepare_theatre_fixture consumes it, and audio_recovery is used before/after killed-host plus PostgreSQL restart.
- `scripts/test_postgres.py:1074` compares exact before/after recovery state and writes audio-server-restart.json describing original-operation prepare/PUT/candidate replay, owner GET/HEAD metadata and exact ranges/416/read-access 303.
- `scripts/test_postgres_runner.py` retains the restart fixture path.
- Both PG runner scripts select the audio_lifecycle_http:: subtree.
- `scripts/seed_theatre.py` and `xtask/src/seed.rs` retain nested theatre development seeder test name.
- Evidence docs retain source-bound audio lifecycle/read-baseline and provider-independent storage references. Never rewrite historical PASS as current native results.

### Concrete #333 retirement prerequisites

#### `audio_intake_postgres.rs`

- Port script-intent/receipt schema and current application owner/read/write adapter first; preserve script/editorial prerequisites.
- Add a native Cargo-discovered real-file target, for example **proposed** `tests/audio_intake_turso.rs`, retaining 21 theorem names above or an explicit assertion-level split.
- Required evidence: exact receipt bytes/digests and original target retention; real router authorization/error/header/body matrices; deterministic pre-commit reauthorization; duplicate operation/byte concurrency; rollback/interruption/lost-response restart reconciliation; corruption and migration replay.
- Isolated P/I/C/S/R/T coverage does not discharge these integrated obligations.

#### `support/audio_lifecycle_http_postgres.rs`

- Port lifecycle/review/grant-secret namespace/write transactions and remaining Media DDL, not merely migration 14 reader tables.
- Add native real-file/router target, for example **proposed** `tests/audio_lifecycle_http_turso.rs`, retaining 11 theorem names and helper response contracts.
- Preserve publication/theatre support tree until independent assertion ledgers permit retirement or replacement support root exists.
- Recreate restart fixture/runner assertions under native process/file fixture before claiming fixture obsolete.
- Measure authoritative owner submission/candidate reads separately from historical PG and listener discovery measurements.

### Final dispositions

| File | Test count | Disposition | Action |
| --- | ---: | --- | --- |
| `audio_intake_postgres.rs` | 21 | NEEDS_PORT | Retain |
| `support/audio_lifecycle_http_postgres.rs` | 11 | NEEDS_PORT | Retain with unresolved support consumers |

No deletion is justified by this inventory. Native execution: **NOT_RUN** (Cargo unavailable). This source inspection is not native PASS evidence.


## Production and private artwork: 28 tests in five files

All source paths in this section are relative to `services/cantos-server/`. Legacy paths begin `legacy-pg-tests/tests/`; native references below omit `src/contexts/` only when explicitly introduced as such. Every inventoried legacy file was read in full at base `12fb751ec71e20233fe4caaee35e4544fe4a0d4b`. Findings are source inspection, not execution. Native tests were **NOT_RUN** because Cargo is absent from the cloud PATH.

### Per-file decision and native evidence boundary

| Legacy file | Tests | Disposition | Actual native partial evidence | Remaining prerequisite / gap |
| --- | ---: | --- | --- | --- |
| `production_postgres.rs` | 11 | NEEDS_PORT | `tests/production.rs` pure Production admission/eligibility; `src/contexts/production/domain/commands.rs::tests` pure command decisions | #333 Production settings/rights/snapshot/approval tables and adapters, authorized HTTP, real-file replay/reopen/rollback/corruption and competing writes. |
| `production_grants_postgres.rs` | 2 | NEEDS_PORT (mixed obsolete mechanism + live behavior) | Native owner/process trust shift documented in `docs/architecture/server-turso-porting.md`; listener private visibility is narrower than the Studio journey | PG privilege/Firebase-registration checks are OBSOLETE candidates, not native parity. First test also asserts real Studio work/chapter creation and private Theatre isolation; preserve that meaningful business oracle pending #333 Catalog writes. |
| `private_artwork_storage_postgres.rs` | 4 | NEEDS_PORT | `tests/private_artwork_http.rs` lazy/detached-store refusal probes; `src/contexts/media/image_application/tests.rs` fake metadata/storage L4; `image_application/objects/tests.rs` actual filesystem bridge without DB/auth/HTTP | #333 Catalog owner/write/revision + Media image/artwork schema/adapters; actual authenticated loopback, durable metadata, storage-target pinning, atomic CAS/history and corruption. |
| `support/private_artwork_postgres.rs` | 10 | NEEDS_PORT | `src/contexts/media/domain/artwork_tests.rs` pure history; `image_application/tests.rs` fake repository; `domain/object_storage.rs::tests` target grammar overlap | #333 raw SQL barriers/FKs, checked DB projection restoration, catalog/artwork transaction rollback/race, migration byte preservation. One pure grammar test can be ported without DB; existing grammar witnesses are not assertion-complete. |
| `support/private_artwork_repository_postgres.rs` | 1 | NEEDS_PORT | Fake repository L4 flow and real image/filesystem bridge are separate, narrower evidence | #333 actual repository prepare/finalize/selection receipts and atomic Catalog CAS race. |

No file above is a genuine whole-file REPLACED or wholly OBSOLETE oracle. Preserving a file does not claim it compiles or runs from its archived location.

### Shared helper assertions and discovery/consumers

- Cargo has no legacy `[[test]]` declaration; this tree is outside normal `tests/` discovery. The three top-level files have no Rust include consumer. Retired `scripts/test_postgres.py` names Production targets; those scripts are historical consumers, not active native tests.
- `private_artwork_postgres.rs` and `private_artwork_repository_postgres.rs` have no current `mod`/`#[path]` consumer. The repository support source references the other file's `Harness` through `crate::contexts::media::postgres::artwork::tests`. Orphaned source still preserves unique migration oracles.
- Artwork storage declares `#[path = "support/artwork_storage_http.rs"]`; the helper is retained under active `tests/support/artwork_storage_http.rs`, not the archived relative path. Existing fixture/include paths likewise belong to the former layout. This is archived source, not a runnable native target; do not infer safe removal merely from broken archived paths.
- Production `Harness::json` asserts `Cache-Control: no-store` and `X-Content-Type-Options: nosniff` on every response. Harness setup asserts the disposable admin/database identity.
- Artwork loopback helper `request` asserts `no-store`, `nosniff`, and `Referrer-Policy: no-referrer`; `Reply::success` asserts exact status and JSON content type; `Reply::error` asserts exact error code. `PrivateRoots::snapshot` asserts regular objects and no `.pending-` entries. Setup checks created work ID/revision 1. `recorded_image` checks target/key/hash/size/MIME/2x3 scalar columns against exact persisted document, actual bytes and mode `0400`.
- Native `tests/private_artwork_http.rs` explicitly excludes database/session-owner SQL/object-storage/authorized loopback acceptance. `src/contexts/media/image_application/tests.rs` explicitly uses `FakeMetadataRepository` and `FakePrivateStorage`; `image_application/objects/tests.rs` explicitly excludes DB/owner-session HTTP. They must not be credited as L5 parity.

### `production_postgres.rs`: 11 tests

1. `local_casting_rights_and_snapshot_approval_survive_reopen`: eSpeak provider/adapter/output identities; persisted approval survives reopen with eligible/current inputs, no billable dispatch; Bob gets NotFound; changed voice/settings invalidates approval without another snapshot. Native partial: `tests/production.rs::local_casting_preserves_requested_identity_and_rejects_inherited_unsupported_delivery` and `immutable_snapshot_does_not_follow_changed_draft_rights_versions_or_expiry`. Missing durable adapter/reopen/auth.
2. `mixed_espeak_and_google_casting_freezes_and_approves_only_explicit_neutral_delivery`: HTTP catalog contains eSpeak, Neural2 and Chirp3 with no intensity support; inherited nonneutral delivery saves but review returns 400/invalid_request/unsupported_control; explicit neutral/0 review 200, snapshot 201 with nonempty exact resolved adapter/output/performance; approval/eligibility 200, no dispatch, one snapshot. Native partial: pure local-casting test and `google_casting_pins_its_adapter_and_requires_an_explicit_neutral_delivery`; missing authenticated HTTP/writes/counts.
3. `historical_google_snapshot_reads_and_replays_without_becoming_current_authorization`: seeds historical synthesize-2 document against actual pinned receipts; reopen returns exact historical document and approval but no eligible/current/billable authority; exact ApprovalScopeMismatch path; GET and freeze replay preserve ID/document; new approval 409/production_blocked; UPDATE rejected 55000; raw document/freeze/approval bytes and all counts unchanged. Native partial: `historical_google_inputs_remain_exact_inspectable_history_but_approval_is_stale`, `historical_admission_never_normalizes_forged_effective_inputs_even_with_a_new_digest`; missing storage replay/HTTP/barriers.
4. `immutable_candidates_and_exact_concurrent_replays_survive_reopen`: equal concurrent freeze operations yield one identical snapshot; changed same-operation digest is OperationReused; equal concurrent approvals return identical receipt; new duplicate approval is StaleProductionInputs/no rows; reopen eligibility/document exact; DELETE and TRUNCATE of settings/rights/snapshots/approvals rejected 55000. Pure command tests cover decisions only; no native durable race/replay/barriers.
5. `settings_rights_and_full_export_edits_invalidate_authorization_without_mutating_snapshot`: revocation yields RightsRevoked + RightsChanged, old exact approval still replays; independent voice/performance/pronunciation/budget/rate edits yield SettingsChanged; export title edit yields RevisionChanged; document remains exact. Native partial: immutable-snapshot and `digest_binds_every_scoped_input_including_rights_excluded_from_content_digest`; missing durable mutation/replay.
6. `private_http_and_owner_actions_fail_closed_and_blocked_candidate_is_inspectable`: unreviewed blocked snapshot is inspectable; missing session 401/foreign 404 with empty issues; runtime role cannot deactivate actors or mutate membership (42501); editor read 200 but approval 403/forbidden; owner approval 409/production_blocked/script_unreviewed with unchanged counts; removal revokes read; inactive owner Unauthenticated; every catalog capability has dispatch=false and exactly two are reference-only. Native partial: `a_frozen_blocked_candidate_is_inspectable_but_never_authorized`, catalog pure test; missing real auth/no-effect boundaries. PG role checks require explicit obsolete trust-shift treatment.
7. `expired_rights_unsupported_inputs_and_transaction_failure_leave_no_partial_authorization`: unsupported provider InvalidRequestFields/no rows; expired rights finding and ProductionBlocked approval; injected approval INSERT failure returns Unavailable with all counts unchanged; removing failure and retrying inserts exactly one approval. Native partial: `every_rights_status_term_subject_language_scope_and_restriction_blocks_fail_closed`; no transaction fault/retry port.
8. `permission_revocation_waits_for_actual_production_transaction_and_next_write_denies`: paused settings INSERT and membership DELETE both visibly wait; admitted first write records Bob; subsequent write after revocation is NotFound. Missing native serialized-writer/admission ordering equivalent. PG lock implementation details do not obsolete the policy oracle.
9. `a_self_consistent_forged_snapshot_does_not_replace_actual_pinned_receipts`: raw self-consistent altered budget/new digest tied to original settings must load as CorruptRevision; genuine snapshot remains eligible. Native historical-admission test checks a pinned-settings mismatch in memory, not the persisted scalar/document decoder.
10. `concurrent_settings_updates_and_failed_freeze_cannot_accept_a_stale_preview`: AFTER INSERT fault gives Unavailable/no count change; exact retry succeeds; competing settings writes have one winner/one StaleProductionInputs and settings count 2; new-operation stale preview refused/no effects; original exact freeze replays its ID. Missing real transaction/CAS/receipt port.
11. `missing_lock_capability_during_role_rollout_fails_without_authority_changes`: removing PG lock-function EXECUTE makes production read Unavailable/no writes; restore recovers version 1. Mechanism-specific OBSOLETE candidate under the documented Turso owner/process boundary; retained inside the mixed NEEDS_PORT file, not claimed replaced.

Native pure Production tests are ordinary `tests/production.rs` targets; the command test module is included with `#[cfg(test)]`. Reading the assertions confirms rule overlap, not durable acceptance.

### `production_grants_postgres.rs`: 2 tests

1. `deployed_runtime_grants_alone_serve_studio_and_theatre_requests`: before grants creation must not be 201 (non-vacuity); apply grant file twice idempotently; real Studio work/chapter create 201 and list 200; private title absent from Theatre 200; retired jobs/clips/adaptation relations absent; application role denied admin/identity/registration reads, actor/session INSERT, catalog/bookmark DELETE/TRUNCATE, script-revision UPDATE; bookmarks allow SELECT/INSERT and only tombstone/order/accepted-at/snapshot/digest updates, not immutable binding columns; schema CREATE denied. PG privileges are obsolete mechanism candidates, but the real business success/private-isolation assertions have no complete native Studio replacement. Do not delete the mixed file as “only grants.”
2. `registration_grants_are_separate_minimal_idempotent_and_revocable`: base denies Firebase enrollment/send-start rights; optional file twice allows scoped SELECT/INSERT plus phase/last-sent/person/block updates; binding/lifetime/identity updates, full table UPDATE, DELETE/TRUNCATE, receipts/admin/actors denied; only send-start sequence USAGE; no direct enrollment guard/receipt function EXECUTE or schema CREATE; revoke twice removes optional privilege but preserves base catalog INSERT; retired relations absent throughout. Entire Firebase/PG privilege-specific test is an OBSOLETE candidate under removed provider auth and no PG role model, not native parity. Preserve first test's live business gap.

### `private_artwork_storage_postgres.rs`: 4 tests

Shared `admit` helper asserts independently hashed RGB PNG/RGBA16 PNG/JPEG preparation request binding; exact JSON and Protobuf replay with `Vary: accept`; session stamp; Pending/no asset → Admitted/exact asset; asset/intent/work/owner identity and observed size/hash/MIME/2x3; duplicate upload and metadata read return exact bytes; serialized receipts expose no storage target/key/private root/HTTP(S) URL. `preview` asserts GET/HEAD 200, exact MIME/length/ETag, no content-range/location/set-cookie/CORS, exact GET bytes and empty HEAD even when Range/If-None-Match are sent.

1. `production_http_shared_storage_preserves_three_formats_receipts_targets_and_history_after_reopen`: starts empty revision 1; admits all three formats using shared assertions; actual private bytes and mode 0400; select rev2, replacement rev3, Catalog edit rev4 inherits artwork, clear rev5 tombstones with time; exact rev1..5 history. Reopened Store/new active target keeps historical original target/inode/no copy while new objects use new target; old prepare/set replays are exact with no row/file mutation. Exact counts `(head,history,changes,artwork_ops,intents,assets,clock)=(5,5,3,6,3,3,5)` and three objects. Future revision/missing asset and legacy/Theatre alias 404. Native partial: fake complete flow, actual filesystem bridge, lazy router boundary; no combined real metadata/auth/storage journey.
2. `production_owner_fence_denies_member_foreign_expired_revoked_and_invalid_commands_without_writes`: editor ordinary Catalog read proves non-vacuity, yet missing session/member/foreign/expired credentials deny image/artwork GET/HEAD/create/select/upload with exact 401/404, empty HEAD and no row/file changes. Stale header/query or duplicate Studio fence gets 409/unauthenticated with actual stamp; duplicate cookies 401; cross-work intent/asset/preview/select 404; stale revision 409/current3; wrong digest 422/asset_binding_mismatch; operation reuse 409; omitted artwork 400; octet-stream/gzip 415; MIME mismatch 422/image_binding_mismatch; wrong hash 422/expected_hash_mismatch; foreign origin 403. Revocation denies reads/replay/upload/select with 401/no changes. Native partial: fake owner/mismatch tests, lazy router cookie/origin refusal; missing authorized fences/durable no-effect evidence.
3. `two_independent_production_http_stores_commit_one_catalog_artwork_cas_winner`: real clear and Catalog edit from two stores both wait at shared Catalog clock; release yields exactly one success at rev3 and one 409/stale_revision/current3. Exact head/history/change/artwork-operation/catalog-operation/clock counts depend on winner; current artwork follows winner. Older select replay exact/no mutations; files unchanged. Fake late-Catalog-edit test is not a real-file competing-HTTP CAS replacement.
4. `malformed_actual_images_and_corrupt_recorded_objects_fail_closed_without_repair_or_public_fallback`: unconfigured route 404; matching declared hash/length cannot admit truncated actual PNG (422/malformed_image, unchanged state). Corrupt original private inode plus tempting valid same-key object on new target must yield GET/HEAD preview 500 with no ETag/location; selection/reupload 500/corrupt_revision, no repair or mutation. Metadata remains exact inspectable. Archived Work clear/prepare 409/catalog_conflict/no effects. Native partial fake corruption + actual bridge missing-object tests do not cover combined SQL/HTTP/target authority.

Native partial test references: `image_application/tests.rs::{complete_private_prepare_admit_select_replace_clear_and_history_keep_exact_receipts,owner_only_paths_fail_closed_for_foreign_owner_members_wrong_work_and_missing_session,declared_hash_length_mime_and_actual_format_mismatch_never_persist,admitted_retries_preserve_bytes_and_original_time_after_expiry_archive_and_target_switch,stale_selection_and_late_catalog_edit_never_overwrite_or_append_artwork,verification_corruption_missing_or_unavailable_never_selects_and_replay_verifies_objects,prepare_and_artwork_operation_reuse_and_changed_payload_are_atomic_conflicts}`. Filesystem-only: `image_application/objects/tests.rs::{production_image_bridge_persists_and_reopens_exact_real_filesystem_bytes,restored_location_is_exact_and_never_falls_back_to_an_active_target,accepted_location_with_missing_bytes_is_corruption_on_verify_and_open}`. These are statically reachable via native `cfg(test)` modules but NOT_RUN here.

### `support/private_artwork_postgres.rs`: 10 tests

1. `owner_facade_restores_exact_old_lifecycle_and_denies_members_and_future_revisions`: owner historical rev1 returns exact original draft despite archived current rev3; editor cannot owner-read; future rev4 NotFound. Missing actual Catalog owner/history adapter.
2. `effective_artwork_survives_edits_and_clear_is_a_historical_tombstone`: rev1 none/no timestamp; rev2/3 selected with timestamp; rev4 cleared/time; exactly two persisted change rows. Pure `domain/artwork_tests.rs::set_clear_and_unrelated_edits_reproduce_exact_history_without_resurrection` covers history decisions, not stored query/restoration.
3. `rollback_discards_work_snapshot_head_epoch_change_and_operation_together`: write all effects in transaction then rollback; exact head/history/clock/change/operation snapshot and original Work unchanged. Missing actual cross-context atomic rollback.
4. `checked_asset_restore_rejects_validly_typed_column_document_disagreement`: raw scalar width changed to 2 while document width stays 1; actual loader returns CorruptRevision. Pure restoration is not scalar/document database agreement evidence.
5. `immutable_tables_reject_update_delete_truncate_even_when_dml_is_granted`: intent/asset/artwork-change/operation UPDATE self-assignment, DELETE and TRUNCATE all reject with OBJECT_NOT_IN_PREREQUISITE_STATE despite DML grants. Missing actual Turso per-table barriers plus REPLACE/upsert bypass equivalents required by port guide.
6. `composite_foreign_keys_reject_wrong_owner_work_and_exact_asset_digest`: raw intent wrong owner/work; raw selection cross-work asset; raw selection wrong exact asset digest all ForeignKeyViolation. Domain selection checks do not replace engine FKs.
7. `checked_operation_replay_preserves_original_bytes_and_rejects_rechecksummed_bad_document`: prior response bytes equal exact intent document and restore exact object; invalid zero-length document with recomputed checksum still returns CorruptRevision. Missing actual row restoration/business validation.
8. `baseline_replay_preserves_exact_catalog_snapshot_and_operation_bytes`: migrate twice preserves exact catalog snapshot bytes/digest/recorder, operation fingerprints/result bytes/digests, head/clock, original Work; image/artwork tables stay empty. Foundation upgrade tests do not contain these business records.
9. `artwork_and_catalog_edit_race_accepts_one_cas_and_never_half_advances_a_cover`: actual competing transactions produce one winner, exact loser RevisionConflict(current2,expected1); head/history/clock 2 and artwork change/operation count exactly winner boolean. Missing actual native serialized writer equivalent.
10. `recorded_target_grammar_matches_inspected_private_storage_contract`: accepts `a`, `private-fs`, `a0`, 63×a; rejects empty, `A`, `0a`, `-a`, `a-`, `a_b`, `a/b`, `a.b`, newline, non-ASCII, 64×a. Current `postgres/image.rs::target_valid` delegates exactly to `StorageTargetId::parse`. Native `domain/object_storage.rs::tests::target_identity_is_stable_and_rejects_paths_unicode_and_noncanonical_spelling` overlaps but omits `a0`, `0a`, `a_b`, `a.b` witnesses. This pure test is independently portable; existing native test alone is not assertion-complete. Whole file stays NEEDS_PORT.

### `support/private_artwork_repository_postgres.rs`: 1 test

1. `production_repository_prepare_finalize_select_and_catalog_cas_race_preserve_exact_receipts`: actual production repository prepare replay keeps exact JSON/value without catalog touch; real decoded PNG has independent hash and 2×3; finalize replay on another synthetic target preserves original JSON/value without catalog touch; select rev2 exact and replay with no asset retains original bytes/head. Concurrent repository clear versus Catalog edit has one winner and exact loser RevisionConflict(current3,expected2); exact head/history3 and winner-dependent change/operation/clock counts; old select remains exact after later edit. Fake flow/operation reuse and real filesystem bridge cannot replace production repository transaction/CAS.

### Shared port acceptance required

For every retained database-backed case, #333 must implement the relevant context tables, constraints and actual adapter, then translate SQL fixtures onto run-owned `ServerFixture` files. Preserve exact identities, receipts, hashes, historical bytes and typed errors. Re-express lock policy as serialized-writer ordering with session/owner admission in the writing unit; retain raw barrier/FK/corruption injection, rollback, restart, lost acknowledgement and competing-writer coverage. Add authenticated router/loopback and actual private-storage boundaries wherever the legacy oracle had them. Migration slots containing comments, foundation/session tests and fixture-seeded listener release chains do not satisfy these writer oracles.


## Publication, brief, catalog-walk and seed oracle ledger

Read-only inventory at HEAD/BASE `12fb751ec71e20233fe4caaee35e4544fe4a0d4b`. Read all 4,180 lines of the four requested sources, relevant AGENTS/skills/current checkpoint and #333 owner docs, native candidates, and support consumers. No source edits or test execution. Cargo is absent from PATH, so native execution is **NOT_RUN**. “Discoverable” below means static Cargo/module discovery, not a successful build/test run.

### File decisions

| Legacy support file | Tests | Disposition |
| --- | ---: | --- |
| brief_http_postgres.rs | 8 | NEEDS_PORT |
| publication_http_postgres.rs | 12 | NEEDS_PORT |
| theatre_catalog_walk_http_postgres.rs | 4 | NEEDS_PORT |
| theatre_dev_seed_http_postgres.rs | 10 | NEEDS_PORT |
| Total | 34 | No full-file deletion supported |

None is safely REPLACED or wholly OBSOLETE. Native coverage is meaningful but does not replace each file's assertions at the original boundary. PostgreSQL implementation details can become OBSOLETE during a deliberate port, not the enclosing business oracle.

Strong source evidence:
- `services/cantos-server/src/contexts/media/turso.rs` explicitly limits itself to public-listener reader/index and says Studio intake/lifecycle/publication writes remain PostgreSQL-shaped.
- `docs/architecture/server-turso/cmp-business-api.md` explicitly excludes Studio/publication writes and creator-to-publication acceptance.
- `tests/theatre_business_journey.rs` explicitly excludes creator-to-publication and inspected Opus; its world is seeded using valid domain values.
- `src/contexts/media/publication_storage_tests.rs` explicitly says full Opus promotion is exercised by the PostgreSQL seed acceptance.
- `docs/deployment/theatre-seeded-catalog.md` and `xtask/src/seed.rs` require the presently absent native `catalog_http` seeder. The named persistent-environment lane is retired, but native local seed remains an intended consumer.

### Native evidence key

All paths in this key are relative to `services/cantos-server/`.

- B: `tests/brief_domain.rs`, default integration target.
- P: `src/contexts/media/publication_tests.rs`, included by `domain/publication.rs` under cfg(test).
- PA: `src/contexts/media/publication_application_tests.rs`, included by `publication_application.rs` under cfg(test).
- PS: `src/contexts/media/publication_storage_tests.rs`, included by `publication_storage.rs` under cfg(test).
- R: `src/contexts/media/storage/registry_tests.rs`, storage-registry unit tests; named adapter doubles in the relevant cases.
- T: `src/contexts/media/turso/tests.rs`, included by `turso.rs` under cfg(test), real file-backed listener reads over seeded domain chains.
- A: `src/contexts/media/listener_application.rs::tests`, application tests using reader/delivery doubles.
- L: `src/contexts/media/domain/listening.rs::tests`, pure domain/keyset tests.
- C: `src/contexts/media/listener_cursor.rs::tests`.
- X: `src/contexts/media/listener_projection.rs::tests`.
- H: `tests/server_turso_barriers.rs`, default integration target, raw SQL against real files.
- J: `tests/theatre_business_journey.rs::catalog_to_playback_to_library_over_the_production_composition_with_a_real_identity`, default router journey with real Identity, seeded transport bytes.
- E: `tests/theatre_business_process.rs::the_server_binary_serves_the_cmp_slice_over_loopback_and_a_second_process_reads_the_same_outcomes`, default real-process journey.
- PC: `tests/publication_contract.rs::real_publication_routes_require_studio_auth_and_have_no_legacy_or_listener_alias`.

### Shared assertions

These must not disappear silently from a port:
- Legacy Harness Studio requests assert `no-store`, `nosniff`, JSON content type except 204, response-schema compliance, expected status and exact error code.
- Brief producer helper asserts `no-store`, `nosniff`, ProductionBriefDocument/ApiError schema.
- Publication `theatre_call` asserts `no-store` and JSON decoding, with test-specific exact anonymous envelopes.
- SeedWorld local Theatre calls assert exact Theatre CORS/`no-store`; local media calls assert exact Theatre CORS, credentials=true, `private, no-store, no-transform`.
- Fixture setup requires actual successful Studio content/brief/intake/review/candidate/clearance/approval operations, not direct seeded rows.

### brief_http_postgres.rs — 8 tests, NEEDS_PORT

#### 1. line 91: brief_text_recording_profiles_replay_and_conflicting_creation_are_real_http
- For text/narration and recording: creation 201; freshness=current; exact embedded content; exact replay; NFD-equivalent notes replay identically.
- Same operation with changed profile returns 409 operation_reused; new operation with duplicated brief identity returns the same error.
- Successive 20/32/40-kbps requests advance versions to 2/3/4 and retain exact requested profile.
- Old version retains exact brief and reports guidance_changed.
- Unsupported AAC, Opus v2 and empty profile return 422 invalid_request.
- Native partial: B::prose_and_recording_require_no_script_cast_or_provider; B::explicit_profiles_versions_and_optional_guidance_are_admitted_without_defaults; B::every_instruction_input_changes_the_digest_and_nfc_is_stable; B::stale_comparison_never_retargets_or_changes_the_frozen_brief.
- Gap: real-file creation/head/version/identity conflicts, operation replay and HTTP schema/error integration. **NEEDS_PORT**.

#### 2. line 159: producer_scope_studio_cookie_capabilities_revocation_and_owner_isolation
- Bob cannot issue Alice's grant or read brief, including after becoming Reader: 404.
- Missing narration capability and Studio-session secret as producer secret return 422.
- Created grant permission=read_instructions, response omits token; identical request replays; changed expiry under one operation returns 409 operation_reused.
- Correct bearer exports exact brief; Studio credential as bearer returns 401; correct bearer for wrong unit 404; missing bearer 401; Studio-cookie access to producer path 401.
- Revocation twice returns 204; revoked bearer 401; exactly one persisted revocation and grant; stored producer hash differs from ordinary session token_hash result; frozen snapshot contains no raw token.
- Native partial: B::required_controls_fail_explicitly_instead_of_silent_fallback; B::producer_permission_is_exact_expiring_and_revocable.
- Gap: real grant issuance/replay/owner-role isolation/auth-purpose separation, durable hashed-secret storage, repeated-revocation effects and HTTP behavior. **NEEDS_PORT**.

#### 3. line 291: brief_content_rights_guidance_and_archival_drift_block_producers_preserving_snapshots
- Independently changes content attribution with same effective text, Work rights, guidance version, archival.
- Each producer export returns 409 stale_production_inputs.
- Owner still reads exact original frozen brief with content_changed/rights_changed/guidance_changed/archived as appropriate.
- Original creation replay returns the same stale response.
- Native partial: B::stale_comparison_never_retargets_or_changes_the_frozen_brief covers pure freshness and unchanged document.
- Gap: cross-context current-fact selection, producer denial, persisted snapshot and stale replay integration. **NEEDS_PORT**.

#### 4. line 378: brief_source_and_script_export_pin_owning_contexts_and_preserve_originals
- Real import and saved Script revision feed separate source/script content and brief creation.
- Source instructions contain exact import receipt and nonempty receipt_digest; Script instructions contain exact saved revision.
- Owner GET succeeds for each; original source bytes and saved revision remain exact after export.
- Native overlap is isolated import/Script/core tests, not this owning-context brief export.
- Gap: Production Brief to Ingestion/Editorial exact export binding and preservation over native persistence/HTTP. **NEEDS_PORT**.

#### 5. line 483: source_brief_refuses_legacy_failed_and_oversized_receipts_without_advancing_head
- Legacy operator-only source returns 422 evidence_unavailable.
- Failed DOCX extraction and oversized extracted TXT return 422 invalid_request.
- Content binding is accepted, but each rejected brief leaves zero production_briefs for its unit.
- Native isolated import limits/admission do not replace brief admission/no-side-effects.
- Gap: cross-context evidence rejection and native atomic no-brief/no-head advancement. **NEEDS_PORT**.

#### 6. line 580: brief_atomic_rollback_concurrent_versions_and_sql_immutability
- Injected head-insert failure returns 503 unavailable, zero briefs.
- Concurrent expected_version=0 requests yield exactly one 201 and one 409.
- Raw UPDATE, DELETE, TRUNCATE CASCADE refuse with PG OBJECT_NOT_IN_PREREQUISITE_STATE.
- Deliberately corrupted frozen snapshot yields GET 500 corrupt_revision.
- Native partial: H::the_audio_release_chain_is_digest_checked_bound_and_immutable_and_the_pointer_only_moves_forward checks raw brief update/delete/replace variants.
- Gap: brief transaction rollback, concurrent version race, brief decoder/HTTP corruption. PG TRUNCATE/SQLSTATE mechanism is obsolete, not invariants. **NEEDS_PORT**.

#### 7. line 651: producer_expiry_while_waiting_and_inactive_owner_are_checked_before_export
- Export waiting behind catalog lock returns 401 after grant expiry.
- Fresh grant returns 401 after owner becomes inactive.
- Native partial: B::producer_permission_is_exact_expiring_and_revocable tests exact pure expiry.
- Gap: trusted-clock re-admission after waiting and durable owner-state recheck. Translate PG fixture to native ordered writer/fixed clock. **NEEDS_PORT**.

#### 8. line 708: brief_read_baseline_uses_actual_authorized_export_and_indexed_pins
- Creates 30 versions; producer grant exports exact version-30 document on all 20 reads.
- Actual connection role=cantos_app.
- Records timings and EXPLAIN ANALYZE output. Does NOT assert index selection, latency bound or performance gain.
- No native replacement found.
- PG role/EXPLAIN details become obsolete under owner-process trust, but repeated exact authorized pinned reads remain unported. Resume native measurement under #384 after #333 brief export. **NEEDS_PORT**.

### publication_http_postgres.rs — 12 tests, NEEDS_PORT

#### 1. line 128: publication_flow_replays_restarts_and_republishes_immutable_history
- Readiness blockers empty; publish pointer=1 and rendition binding/media exact.
- Exact request replay gives same response; one release/pointer/operation/event, zero withdrawals.
- Fresh Service/Store reads exact durable active response.
- Republish pointer=2, different release, supersedes prior, same manifest.
- Old stored snapshot exact; raw release UPDATE refuses.
- Partial: P::genuine_admitted_values_create_an_immutable_exact_release; H::the_audio_release_chain_is_digest_checked_bound_and_immutable_and_the_pointer_only_moves_forward.
- Gap: native creator publish/replay/restart/republish receipts/events/history. **NEEDS_PORT**.

#### 2. line 230: publication_observes_the_recorded_target_after_switch_and_never_falls_back
- After active-write target changes, existing rendition permits new approval/readiness/publish/active read on original recorded target.
- Copy identical bytes to new target, delete original: no fallback.
- Readiness ObjectIntegrityMismatch; active Blocked([ObjectUnverified]); stored release exact.
- Partial: R::changing_active_write_target_keeps_historical_reads_on_the_recorded_target; R::missing_historical_target_never_falls_back_to_a_healthy_active_target (registry doubles); P::object_and_delivery_digest_mismatch_are_distinct.
- Gap: native persisted rendition target through publication service and actual filesystem integrity/error integration. **NEEDS_PORT**.

#### 3. line 315: publication_two_publishers_have_one_winner_and_duplicate_requests_one_receipt
- Distinct concurrent operations at pointer 0 yield one 201/one 409, one release.
- Concurrent identical operation yields identical 201 receipts, one event.
- Partial: PA::a_second_publisher_using_the_first_expected_pointer_conflicts is sequential pure preparation, no database race/receipt.
- Gap: actual native race/duplicate commit receipt/event. **NEEDS_PORT**.

#### 4. line 355: publication_stale_qc_rights_revocation_missing_bytes_and_unauthorized_requests_fail_closed
- Bob publish 404, unchanged release/pointer/operation/event/withdrawal counts.
- Withdrawn QC causes 409 production_blocked containing qc_not_approved, unchanged counts.
- Revoked distribution clearance and deleted private bytes independently block with unchanged counts.
- Despite name, no brief/content mutation occurs.
- Partial: P::later_qc_withdrawal_invalidates_candidate_and_publish; P::changed_clearance_invalidates_exact_publication_approval; P::each_storage_observation_and_archival_has_an_exact_blocker. PC tests no-session rejection, not authenticated foreign owner.
- Gap: native current facts, owner scope, real missing private bytes, atomic no-effects for each. **NEEDS_PORT**.

#### 5. line 440: publication_default_delivery_is_deferred_and_withdrawal_does_not_resurrect_prior_release
- Default delivery blocks with delivery_unavailable, unchanged counts.
- Publish twice; active withdrawal pointer=3; exact withdrawal replay; active release=null, two releases retained.
- Partial: PA::unavailable_production_delivery_never_proposes_activation; T::a_withdrawal_or_revocation_fact_denies_even_while_the_pointer_still_names_the_release.
- Gap: native publish/withdraw receipts/pointer/history/no superseded fallback. **NEEDS_PORT**.

#### 6. line 513: publication_new_approval_is_blocked_after_rights_revocation_and_published_release_stops_resolution
- After publish, revoked clearance prevents new approval with rights_revoked.
- Active read hides release but keeps pointer_version.
- Partial: P::approving_rejects_revoked_expired_and_wrong_scope_rights_before_delivery; P::retained_release_obeys_exact_expiry_revocation_withdrawal_and_approval.
- T's revoked fixture revokes submission, not distribution clearance.
- Gap: persisted current clearance, approval HTTP denial, Studio active resolution. **NEEDS_PORT**.

#### 7. line 573: publication_failed_replacement_preserves_prior_active_release_after_qc_changes
- Publish, withdraw QC, replacement blocks; counts unchanged; active response exact prior result.
- Partial: PA::failed_replacement_has_no_pointer_effect_and_preserves_prior_manifest uses pure Pointer/ObjectMissing; P::later_qc_withdrawal_invalidates_candidate_and_publish covers QC.
- Gap: persisted QC transition plus failed transaction plus retained active HTTP response. **NEEDS_PORT**.

#### 8. line 621: publication_withdrawing_superseded_release_preserves_new_active_pointer
- Publish twice; withdraw old release at current expected pointer.
- Pointer remains 2; withdrawn_release_id=old; active response=second; two releases/one withdrawal.
- No equivalent native write workflow found. Read-side fixture supersession is not parity. **NEEDS_PORT**.

#### 9. line 673: publication_sql_failure_rolls_back_release_pointer_event_and_receipt_then_retries
- Final-event insert fault returns 503 unavailable; all five table counts unchanged.
- Remove fault, exact request retry gives pointer 1 and one release/pointer/operation/event.
- Pure preparation/shared-owner rollback tests do not exercise this write boundary.
- Gap: entire native publication multi-table fault/rollback/exact retry receipt. **NEEDS_PORT**.

#### 10. line 732: theatre_public_discovery_manifest_search_replacement_and_withdrawal
- Public Studio publish; anonymous list exactly one Work; record anchor.
- Uppercase Vietnamese + podcast/language filter finds it; unit publication/duration matches release.
- Three manifest reads exact SHA; omit owner_id, rights_refs, candidate_id, review_evidence, object_key, delivery_object_id, url.
- Default playback 503 rendition_unavailable without publication-table changes.
- Republish retains first anchor; old publication 404. Withdraw replacement: empty list, Work detail 404.
- Partial: T::discovery_orders_filters_searches_and_pages_by_immutable_anchor; T::private_staged_withdrawn_revoked_expired_superseded_archived_and_disabled_are_refused; T::playback_is_authorized_for_the_exact_release_and_the_bytes_are_those_it_names; X::manifest_projects_exact_release_identity_and_omits_private_envelope; J catalog/media.
- Gap: same HTTP-created release/index/republish/withdraw and no-write playback assertion. **NEEDS_PORT**.

#### 11. line 824: theatre_private_published_release_and_other_owner_remain_invisible
- Private published Work: anonymous empty WorkPage, next_cursor=null.
- Manifest GET and playback POST both 404 exact {code:not_found,retryable:false}.
- Despite name, there is no distinct second-owner fixture/action.
- Partial: T::private_staged_withdrawn_revoked_expired_superseded_archived_and_disabled_are_refused gives real-file service-level private/unlisted/staged denials.
- Gap: Studio-created private release and exact anonymous HTTP envelopes. **NEEDS_PORT**.

#### 12. line 859: theatre_rejects_copied_approval_snapshot_with_forged_latest_scalar_identity
- Approval withdrawal hides publication, 404.
- Forged newest approval row copies old approved snapshot/digest but changes scalar id/operation/sequence.
- Manifest must return 500 exact {code:unavailable,retryable:false}, not copied approval.
- Partial: T::a_row_that_fails_its_integrity_checks_is_corruption_and_is_never_served tests four different corruptions, not approval scalar/snapshot identity. H does not test forged approval insert.
- Gap: exact current-approval corruption and HTTP projection. **NEEDS_PORT**.

### theatre_catalog_walk_http_postgres.rs — 4 tests, NEEDS_PORT

#### 1. line 202: theatre_units_walk_survives_mid_walk_withdraw_publish_and_republish
- Four prepared units, first three published; page one unit 0, public position 0, no ordinal.
- Presented Work count 3 excludes staged; plain detail omits unit_count.
- Withdraw unseen unit; reverse Studio reorder increments Work revision and moves original Studio position to 3.
- Refresh/publish seen unit and staged unit: fresh public units 0,3,2, positions 0,0,2. Original retains first public position but new publication; new unit adopts Studio position 0.
- Presented walk same order, ordinals 1,2,3 across ties/gaps; detail/list/search counts 3.
- Original cursor retries identical; equal-position UUID text C order; tail 3,2, each once, no withdrawn unit.
- Wrong route/limit/Work cursor uses return 400 exact invalid_cursor without consuming valid cursor.
- Archive last eligible unit through Studio: count 2, rows 0,3, final ordinal 2.
- Partial: T::discovery_orders_filters_searches_and_pages_by_immutable_anchor; T::presentation_is_an_opt_in_with_exact_counts_and_ordinals_over_the_eligible_relation; A::retrying_a_unit_cursor_replays_the_same_continuation; A::empty_first_unit_page_preserves_continuation_and_cursor_scope; L::unit_keyset_matches_sql_c_collation_of_canonical_uuid_text; L::unit_keyset_is_strict_total_order.
- Gap: native same-file mid-walk writes/indexing, tied-position SQL ordering, real reorder/republish anchor, rank-before-cursor, archive recomputation. **NEEDS_PORT**.

#### 2. line 395: theatre_works_walk_rechecks_visibility_and_binds_filter_cursors
- Initial three Works; first page equals full order.
- Withdraw unseen, republish seen, first-publish new Work.
- New Work leads fresh list; seen Work anchor unchanged.
- Original continuation retry identical; tail contains original remaining Work and excludes newly inserted-before-cursor Work.
- Wrong limit/kind/language/query/route returns exact invalid_cursor; original remains usable.
- Filtered search cursor retries; independent query/kind/language changes reject; full filtered walk equals current eligible list.
- Partial: T::discovery_orders_filters_searches_and_pages_by_immutable_anchor; L::stable_work_anchors_page_once_under_insert_delete_and_rendition_replacement (pure model); C::opaque_tokens_bind_query_expire_and_never_encode_internal_values.
- Gap: native same-file mutation during HTTP walk and complete filter bindings. **NEEDS_PORT**.

#### 3. line 493: theatre_work_filters_follow_the_latest_eligible_representative
- Two releases of one Work: old Vietnamese/title, later edited title + English audio; new timestamp >= old.
- Newest eligible title/language is representative.
- Old-title/Vietnamese, Vietnamese-only and wrong-kind/English filters empty; new-title/story/English finds Work.
- Withdraw newest: old-title/Vietnamese returns, English list empty.
- Partial: T::list_and_detail_share_one_representative_release_per_work; T::released_metadata_is_pinned_while_current_privacy_still_governs; X::summaries_use_published_audio_language_and_listing_anchors.
- Native representative case uses same-time/same-metadata releases, not newest mixed-language-before-filter/fallback.
- Gap: exact native differential representative/filter scenario. **NEEDS_PORT**.

#### 4. line 549: theatre_units_walk_rechecks_rights_expiry_on_the_database_clock
- Three publications; unseen middle has short clearance window; assert before expiry and list length 3; capture cursor.
- Bounded actual DB-clock polling, no snapshot rewrites.
- Continuation skips expired middle, reaches final unit, no cursor; expired manifest exact 404 envelope.
- Partial: T::private_staged_withdrawn_revoked_expired_superseded_archived_and_disabled_are_refused seeds already-expired rights; L::expiry_is_half_open is pure.
- Gap: active-to-expired change between cursor reads with trusted native clock/HTTP. Port using FixedClock, not PG polling. **NEEDS_PORT**.

### theatre_dev_seed_http_postgres.rs — 10 tests, NEEDS_PORT

#### 1. line 1275: theatre_development_catalog_seed
- Seven Works: four public plus withdrawn/private/unpublished; novel/story/podcast, Vietnamese/English, 23-episode pagination, actual synthetic Opus, full Studio authoring/intake/review/clearance/approval/publish/withdraw.
- Helper assertions: unapproved candidate blocked and absent from public storage; approved readiness empty blockers and promoted SHA exact; existing grant loses media access immediately after withdrawal.
- Disposable shelf exact four/newest-first/no cursor; limit-2 pages exactly reconstruct shelf.
- Live Studio facts exact title, Work revision=1+2*unit count, unit title/description/revision=1, pointer/current release match receipt.
- Non-public detail/units/manifests exact 404.
- Public detail equals shelf, exact title/language; unit walks preserve order/gaps excluding withdrawn; exact publication/title/description; duration within 100ms.
- 23 podcast episodes: two pages, first 20.
- Manifest exact Work/unit/publication; grant SHA exact.
- Media GET 200 audio/ogg exact length/SHA; HEAD 200 exact length/empty body; Range 206 exact bytes[100..200]; unsatisfiable range 416.
- Local same host private owner Range 206, private/no-store/no-transform, exact Studio CORS/body; no cookie 401; Theatre origin with Studio cookie 403; failures no-store.
- Unknown Work 404; forged cursor 400 exact invalid_cursor.
- Optional receipt preserves roots/session and records seed.
- Partial: J/E/T/PS cover reader/transport/schema/bytes/ranges/some visibility, but seed arbitrary transport bytes, bypass Opus intake/promotion and Studio creation. Private-byte auth/CORS is distinct.
- Gap: native full creator seeder/actual byte promotion, blocked by #333 Catalog/Content/Brief/Media write ports. **NEEDS_PORT**.

#### 2. line 1553: seed_write_ahead_replay_preserves_operation_after_lost_response
- First begin persists operation=first, no response. Reopen after crash ignores new proposal and reuses first. Successful checkpoint reopens as owned response.
- No native SeedJournal equivalent. Generic lost-COMMIT tests do not test client write-ahead journal.
- Preserve pending native replay design. Named persistent PG lane retired, not REPLACED. **NEEDS_PORT**.

#### 3. line 1586: seed_journal_refuses_a_different_environment_before_replay
- Reopen same journal with changed environment panics before replay.
- No native equivalent. Exact persistent target retired.
- Candidate OBSOLETE only with deliberate retirement of persistent-only helper; otherwise preserve target-binding for native design. File remains **NEEDS_PORT**.

#### 4. line 1594: persistent_session_binding_refuses_wrong_database_actor_or_session_before_effects
- Wrong fence SessionMismatch; wrong actor ActorMismatch; absent fence MissingFence; duplicate fence InvalidFence.
- No native equivalent of helper. Other auth tests do not test client/API/DB seed binding.
- Old direct PG/OIDC lookup obsolete, but local replacement needs explicit design, not name-based deletion; preserve unresolved safeguard.

#### 5. line 1624: persistent_http_transport_binds_real_session_and_public_reads_cross_actual_cors_anonymously
- Synthetic loopback handlers assert exact secure Studio cookie/origin/no Authorization; mutation bound fence.
- Anonymous reads omit cookie/auth/origin/fence.
- Producer exact upload Bearer + Studio origin, no cookie/fence.
- Bind actor exact; mutation/public/producer succeed; redirect 307 returned, not followed.
- Real TCP to synthetic handlers, NOT real server authentication despite name.
- Native auth/CORS journeys different scope; no native seeder transport found. Preserve until local contract port or explicit persistent-only retirement.

#### 6. line 1740: persistent_media_refuses_rewriting_a_plaintext_grant_onto_the_api_origin
- Plain HTTP advertised grant rejected before network; no destination rewrite onto API origin.
- No native helper equivalent. Production origin parser tests not same remote-fetch helper.
- Retired named environment does not establish generic safeguard replacement.

#### 7. line 1756: fresh_validated_session_for_same_dataset_reuses_operation_ids_and_responses
- Same owner/target + new validated fence reuses original operation/response; one operation, two session audit records.
- No native client journal equivalent; auth refresh is not replay parity. Preserve pending design/explicit obsolete decision.

#### 8. line 1803: new_session_for_a_different_actor_cannot_replay_an_owned_dataset
- Different validated actor cannot record session/replay owned journal; expected panic.
- No native equivalent; preserve pending seed ownership/replay decision.

#### 9. line 1824: studio_cookie_name_is_closed_and_refuses_cookie_header_injection
- Secure cookie header rejects empty/semicolon/CRLF/space token.
- Name parser allows only __Secure-cantos_studio/cantos_session; rejects empty/arbitrary/semicolon/CRLF names.
- No native SeedCookieName equivalent. Server admission is not helper emission/injection parity.
- Native xtask seed still specifies development cantos_session; preserve safe emission during port.

#### 10. line 1854: configured_studio_cookie_name_sends_one_exact_cookie_without_fallback
- Both allowed names: loopback sees one exact Cookie header/name=value; bind actor exact and subsequent session GET succeeds, no fallback cookie.
- No native seeder equivalent. Preserve or explicitly retire persistent-only secure branch during local lane implementation; not blanket REPLACED.

### Consumer edges and deletion hazards

- `catalog_http_postgres.rs:2085-2086` includes brief support as brief_http; direct brief_request call near 1352 and producer near 1499.
- `support/audio_lifecycle_http_postgres.rs` calls brief_request near 106 and 681.
- Walk and seed children call brief_request.
- `audio_lifecycle_http_postgres.rs:1208-1209` includes publication support as pub(crate) mod publication_http.
- Publication includes both walk/seed children and owns approve_candidate/theatre_call/VerifiedDeliveryFake used by them.
- `PublishedFixture::{listener_world,listener_handles,listener_submission_id,listener_publish,listener_withdraw}` have no callers found in repository. Individually removable dead-support candidates if scoped, not proof other fixtures/tests obsolete.
- Seed source hashes its own include_bytes into journal fingerprint: move/edit changes replay identity; preserve historical source/journal provenance.
- `xtask/src/seed.rs::SEED_TEST` and `scripts/seed_theatre.py::SEED_TEST` name seed path. Native xtask checks presently absent `services/cantos-server/tests/catalog_http.rs` before effects.
- `docs/work-plan/theatre-catalog-state-handoff.md` links walk source/names publication tests; `docs/evidence/theatre-seeded-catalog-integration.md` retains source-bound seed evidence. Repair moved links; never relabel evidence native.

### Exact #333 resumption

1. Native port `src/contexts/production/postgres/briefs.rs` plus Catalog/Content and Ingestion/Editorial export use cases; default-discovered brief HTTP fixtures preserving all 8 ledgers; raw native barriers/fixed clock/ordered writer fault tests.
2. Native port `src/contexts/media/postgres/{lifecycle,publication}.rs`, actual-byte intake and recorded-target observations; restore publication HTTP cases including receipt/event rollback, races, current rights and current approval corruption.
3. Native Studio Catalog/Content reorder/archive plus true publication index writes; exact mid-walk scenarios, not only pure keysets/seeded readers.
4. Ordinary `tests/catalog_http.rs` HTTP seeder matching current xtask contract after preceding ports; preserve actual Opus/promotion/private-public transport/negative fates. Explicitly decide obsolete named persistent-PG helpers versus local replay safeguards. Fixture seeds and J/E do not close this prerequisite.

No newly established PASS or native parity claim. This ledger records source-inspection evidence only.
