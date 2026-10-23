# Security and schema PostgreSQL oracle inventory

Appendix to the [retained-oracle coverage ledger](legacy-test-oracles.md). Admin/schema rows retain source-only status; the retired auth/actor rows below map to executed native files and HTTP tests at base `84eea4c3` plus this change.


Source inventory at `12fb751ec71e20233fe4caaee35e4544fe4a0d4b`: 4,203 legacy-source lines, including shared assertions, compared with native implementations and tests.

**Retained: two files / 28 tests** (`admin_postgres`, 15; `schema_postgres`, 13).
Three auth/actor suites (38 original tests) and their PG-only world helper are removed after
native replacement. Original cloud execution was NOT_RUN; that is not the outcome of this
change's native campaign. Exact commands/results belong to the PR.

## Important findings

- Admin is not merely obsolete Kanidm coverage. Its synthetic verifier exercises lasting atomic audit, immutable receipts, late authorization rechecks, concurrency and restart recovery.
- Admin native migration version 6 remains comments only in `src/platform/turso/admin.sql`. `src/platform/admin/postgres.rs` still calls `admin_identity_facts` / `admin_append_audit` / `admin_operation` / `admin_revoke_access`. Pure role tests and wire fixtures do not replace these effects.
- The preserved PostgreSQL epoch/checksum and prefix-helper assertions remain migration/recovery inputs. Native fresh-file schema tests cannot establish PostgreSQL data-conversion parity.

## Evidence key

Paths are relative to `services/cantos-server/` unless stated otherwise.

- **R** = `src/platform/identity/turso/tests.rs`.
- **S** = `src/platform/turso/tests.rs`.
- **T** = `src/platform/identity_auth/turso/tests.rs`.
- **D** = `src/platform/identity_auth/decision/tests.rs`.
- **C** = `src/platform/identity_auth/credential.rs` tests.
- **G** = `src/adapters/inbound/http/identity_auth/tests.rs`.
- **J** = `tests/identity_auth_candidate.rs:real_identity_actor_session_journey`.
- **H** = `tests/theatre_business_journey.rs:catalog_to_playback_to_library_over_the_production_composition_with_a_real_identity`.
- **HL** = `tests/theatre_business_journey.rs:logout_ends_the_session_and_a_later_write_changes_nothing`.
- **P** = `tests/theatre_business_process.rs:the_server_binary_serves_the_cmp_slice_over_loopback_and_a_second_process_reads_the_same_outcomes`.
- **A** = `src/platform/admin/tests.rs`.
- **W** = repository-root `packages/contracts/identity-rpc/tests/wire.rs`.
- **U** = repository-root `packages/store-turso/tests/owned_database.rs`.

“Partial” below means source counterparts exist, but not sufficient to delete the complete legacy test or file. All execution remains **NOT_RUN**. Test locations in the following sections are under `legacy-pg-tests/tests/`.

## admin_postgres.rs — NEEDS_PORT, 15 tests

The Admin schema/effect gap belongs to **#333**. The pure native role matrix and DTO fixtures do not establish storage, HTTP or audit parity. PostgreSQL role grants, `SECURITY DEFINER`, lock observers, SQLSTATEs and `TRUNCATE` are mechanisms that may become obsolete; the policy, durable-effect, receipt, audit and ordering obligations must be preserved through explicit native replacements. Recovery acceptance also belongs to **#336**.

### 1. :237 admin_capability_is_existing_registry_actor_with_explicit_source_and_audited_role

- Preserves valid dedicated Admin credential → existing actor; schema-valid capability, explicit `identity_operator`/source; one allowed audit with actor/surface/action/correlation; audit excludes token/fixture/issuer; unknown subject creates neither actor nor session.
- Partial: `tests/admin_contract.rs:admin_wire_fixtures_match_schema_and_shared_dtos` verifies fixture/schema/role only; **A:only_allowlisted_groups_map_to_closed_platform_roles** verifies role mapping.
- Gap: actual native capability admission, safe audit persistence, no implicit actor/session creation. The exact old introspection source label is provider-specific, not a reason to delete lasting assertions.

### 2. :298 valid_admin_resource_credential_never_authorizes_private_studio_rendition_reads

- First establishes a genuinely valid Admin capability, then rejects both `bytes`/`read-access` for Admin Bearer, three cookie relabelings and mixed credential; exact 401/no-store/JSON body, no redirect/cookie/read-expiry; all authority/media state and Admin verifier call count unchanged; valid Admin still works afterward.
- Partial: `tests/surface_routes.rs:private_audio_reads_require_studio_authentication_and_have_no_other_surface_alias` and surface-policy negatives.
- Gap: valid dedicated Admin proof, unchanged stored authority/media and verifier non-use. An absent verifier’s 503/negative routing is not parity. Requires Admin plus private-media HTTP coverage.

### 3. :426 credentials_roles_disable_revocation_and_other_surfaces_fail_closed_with_audit

- Rejects Studio/Theatre cookies and opaque producer/user credentials; missing/unknown groups forbidden; auditor capabilities allowed but revoke forbidden; stale group observation, provider-disabled and actor-disabled fail closed; no revocation effect; Admin credential cannot enter Studio/Theatre.
- Partial: **A:only_allowlisted_groups_map_to_closed_platform_roles** and **complete_small_role_action_activity_revocation_time_matrix_is_fail_closed**; normal surface-policy negatives.
- Gap: native verifier/application/HTTP and audit/effect persistence. Group/introspection mechanics may retire; surface separation and fail-closed decisions persist.

### 4. :521 revocation_and_audit_are_atomic_replayable_and_append_only_under_runtime_role

- Same operation replays identical response; one receipt/revocation and separate request audits; operation rebound to another target conflicts; revoked Admin denied without disabling actor/resource sessions.
- UPDATE/DELETE/TRUNCATE of audit/operations/revocations fail with exact trigger error and preserve full row snapshots. Runtime role cannot read/delete audit or forge revocation.
- No native effect counterpart: version 6 is empty.
- PG grants/TRUNCATE/SQLSTATE are obsolete mechanisms only. Atomic receipt, append-only rows, target binding and authority isolation need real native tests.

### 5. :640 required_audit_failure_and_effect_failure_never_commit_partial_success

- Injected audit failure makes revoke and anonymous capability unavailable with zero audit/effect/receipt; injected operation failure rolls back revocation/receipt while recording one failure audit.
- No native counterpart. Preserve fault injection/atomicity; translate trigger injection rather than delete.

### 6. :682 concurrent_duplicate_delivery_recovers_one_receipt_after_application_restart

- Concurrent duplicate revokes resolve same receipt; one operation, two audits; new Store replays same operation and adds only another audit.
- No native Admin counterpart. Generic **U** transaction tests are not Admin receipt parity.

### 7. :711 local_admin_revocation_committed_while_waiting_for_lock_is_rechecked

- Observes actual admission lock wait; commits local Admin revocation before release; waiting capability fails `Revoked` and creates no operation.
- **A** pure matrix covers already-revoked inputs only.
- Gap: native writer-ordered concurrent late fact recheck. PG lock observer obsolete, invariant lasting.

### 8. :741 actor_disable_committed_while_waiting_for_lock_is_rechecked_and_audited

- Observed lock wait, actor disable commits, waiting revoke returns `InactiveActor`, no operation/revocation; exact denied audit retains known actor/action/target.
- Partial: **A** matrix, not persistence/interleaving/audit.
- Gap: native ordered admission and exact durable denial.

### 9. :807 receipt_read_failure_after_verified_admission_is_audited_without_an_effect

- Missing receipt-function privilege after verified admission → `Unavailable`, zero effect/receipt, audit with known actor/unavailable.
- No native counterpart. REVOKE is obsolete injection mechanism; post-admission storage-failure audit invariant persists.

### 10. :838 proof_expiring_during_required_audit_wait_rolls_back_effect_and_records_denial

- Holds required audit lock until proof expiry; final clock recheck rejects `StaleRole`, rolls back effects/receipt, persists denied audit with known actor.
- Partial: **A** matrix checks time boundaries without waiting.
- Gap: native clock/writer-ordering equivalent and rollback/audit observation. Do not claim trusted-clock foundation alone replaces Admin late expiry.

### 11. :884 bounded_admin_point_lookup_baseline_uses_current_runtime_role_and_primary_keys

- 2,000 exact bindings, 20 successful runtime-role point reads, observed EXPLAIN/buffers; reports timing without latency threshold.
- No native Admin lookup/EXPLAIN counterpart found.
- Role/function mechanism can retire; bounded exact lookup and attributable performance baseline need native work under **#333/#384**.

### 12. :925 final_clock_read_failure_rolls_back_success_and_audits_unavailable_with_known_actor

- Final clock SQL unavailable → rollback, no revocation/receipt; one unavailable audit with known actor.
- No Admin counterpart. **U:the_clock_is_read_after_the_writer_is_held_and_a_clock_fault_begins_nothing** covers different, earlier clock failure.
- Preserve final-decision failure and safe audit meaning.

### 13. :959 admin_restart_fixture_preserves_block_receipt_and_audit_without_credentials

- Successful revoke writes database/receipt-only restart fixture, never persists verifier/credential. This is the setup half of the next test, not independently sufficient restart evidence.
- No native Admin counterpart. Retain together with recovery test.

### 14. :982 admin_recovery_after_database_restart

- After actual PG restart: migration replay preserves one block, receipt, audit; fresh verifier/operator binding replays exact original receipt, no second operation, one replay audit.
- No native Admin restart counterpart.
- Native owner backup/reopen tests do not qualify Admin application recovery.

### 15. :1075 verifier_outage_fails_closed_with_unavailable_audit

- Capability and revoke unavailable, no operation/revocation, two anonymous unavailable audits.
- Partial: ordinary routes fail closed with no verifier; **A** pure policy tests cannot establish audit.
- Preserve native verifier-outage response plus durable audit/no-effect semantics.

## schema_postgres.rs — NEEDS_PORT, 13 tests

Native **S/U** verify registry/schema creation, refusal, suffix migration, rollback and immutable barriers. They do not substitute for exact preserved PG epochs/SQL/checksums, legacy data conversion or the retained PG prefix-upgrade helper. The incomplete #333 context schema and #336 Tier B migration/recovery boundary require conservative retention. PG-only role/definer/search_path/sequence mechanics may be labeled obsolete for the native implementation, but that does not retire legacy operating/recovery obligations or establish native parity.

### 1. :50 fresh_epoch_has_only_current_tables_and_replay_preserves_authoritative_rows

- Exact required business table presence, obsolete generation/adaptation tables/columns absent; migration replay preserves actor and four-entry registry.
- Partial: **S:a_new_file_holds_the_registry_metadata_and_exactly_the_migrated_schema** checks schema against replayed registry; **S** upgrade tests preserve actors.
- Gap: native registered slots 4–11 still incomplete, so this cannot mean complete business schema parity. PG-only removed provider tables need explicit obsolete classification individually, not whole-test deletion.

### 2. :121 legacy_registry_is_rejected_without_touching_checksums_data_or_ddl

- Legacy versions 1/28 rejected by migrate/verify; registry/actor unchanged, no epoch marker created.
- Partial: **U:files_that_are_not_ours_are_refused_without_being_written** and **S:foreign_diverged_newer_other_epoch_and_non_database_files_are_refused_unchanged**.
- Gap: actual retained PG migration-history distinction/data-conversion prerequisite. Native file refusal is a new-engine boundary, not proof of PG source handling.

### 3. :150 unmarked_objects_and_empty_runtime_startup_never_create_registry

- Empty runtime verification refuses without creating marker; unmarked table/schema/function/enum each prevents migration adoption without registries.
- Partial: **U** unmarked table/garbage refusal, registry pure tests absent/empty Migrate-only, **S** missing-file refusal.
- Gap: exact old object classes are PG-only; preserve lasting non-adoption/no-autobootstrap and document native sqlite_schema/file equivalent. No need to reproduce PG functions/enums as native features.

### 4. :179 changed_checksum_and_registry_gaps_are_refused_without_repair

- Tampered checksums reject migrate/verify unchanged; emptied registry refuses and remains empty.
- Strong partial: **U:a_changed_migration_or_a_tampered_history_is_detected**; **S** divergent/newer refusal; `packages/store-turso/src/registry.rs:divergent_or_newer_history_is_refused_before_any_write`.
- Exact gap: actual-engine empty/gapped history mutation cases versus only checksum divergence, plus retained PG checksum history obligations.

### 5. :203 foreign_epoch_and_unexpected_version_are_refused

- Wrong epoch refused both intents; unexpected migration 5 refused and retained.
- Strong native equivalents: **S:foreign_diverged_newer_other_epoch_and_non_database_files_are_refused_unchanged** and **U:another_services_file_a_different_epoch_and_a_newer_history_are_refused**.
- Native semantic counterpart exists; PG epoch/checksums still stay immutable migration inputs. Execution NOT_RUN.

### 6. :231 malformed_registry_values_return_typed_errors_without_panicking_or_mutation

- Text/NULL epoch, bigint version, text checksum produce exact typed errors and identical table/epoch snapshots before/after both migrate/verify.
- Partial: checked native observation decoders in `packages/store-turso/src/database.rs:observe_connection`; registry shape tests.
- Gap: actual native malformed-history/header/column-type/NULL vectors with typed refusal/no-byte-mutation. Correct decoder source alone is not a replacement test.

### 7. :269 failed_baseline_rolls_back_registry_and_ddl_then_allows_clean_retry

- SQL language denied after initial DDL causes unavailable and zero public objects; restoring privilege permits clean migrate/verify/four checksums.
- Strong native semantic counterpart: **U:a_failed_creation_leaves_an_empty_unmarked_file_that_can_be_created_again** asserts failed later migration leaves no objects/marker then clean retry; **a_failed_upgrade_rolls_back_only_its_own_migration** covers suffix.
- PG role-language denial mechanism obsolete; transactional DDL rollback/retry remains represented. Execution NOT_RUN.

### 8. :310 baseline_keeps_security_definers_pinned_and_public_without_execution_privileges

- Exact list of 15 SECURITY DEFINER functions, each `search_path=pg_catalog`, zero PUBLIC EXECUTE.
- Pure PG mechanism obsolete for native Turso; no definer/grant equivalent should be fabricated.
- But includes legacy protected-function inventory and retained-runtime security assertions. Cannot discard as a native parity claim while preserved legacy runtime/recovery remains; record explicit scope/trust shift. #333 and separately authorized legacy retirement.

### 9. :365 identity_schema_rejects_every_unicode_whitespace_only_issuer_and_subject

- Both key parts reject 20 explicit whitespace inputs, including U+2028/U+2029 and a mixed whitespace string; no rows; legitimate multiword Vietnamese identity inserts.
- Strong partial: **S:external_key_checks_admit_exactly_what_the_domain_constructor_admits** exercises every named single scalar, repeated whitespace, controls/neighbors, Vietnamese letters and both fields through real engine.
- Exact source gap: independent mixed-space string and full legitimate multiword Vietnamese acceptance are not literal vectors there; native expectation is derived from domain constructor. Preserve independent vectors rather than claiming names prove exact parity.

### 10. :432 concurrent_bootstrap_commits_one_epoch_and_read_only_app_check_needs_no_ddl

- Two concurrent PG migrate calls succeed exactly once in registry; SELECT-only app can verify but cannot update marker/create table.
- Partial: **S:a_second_owner_is_refused_while_the_file_is_open**; **U:one_owner_per_file_and_a_private_directory**; **every_connection_enforces_foreign_keys_and_readers_cannot_write**.
- Native deliberate one-owner exclusion changes concurrency contract, not “both migrations succeed.” PG app role is obsolete; explicit startup no-DDL and owner/reader boundary need correctly scoped replacement/mapping.

### 11. :469 bearer_expansion_preserves_exact_baseline_and_refuses_runtime_upgrade

- Actual PG baseline SQL/checksum 1 and actor; runtime refuses pending suffix unchanged; migrator appends, exact first checksum/actor preserved.
- Strong native suffix counterpart: **S:an_actor_registry_file_upgrades_through_every_later_slice_and_keeps_its_rows**; **U:serving_refuses_pending_migrations_and_migrating_applies_only_the_suffix**.
- Gap: exact retained PG registered SQL/checksum source preservation and conversion path are still #333/#336 inputs, not established by native file upgrade.

### 12. :508 prefix_helper_fingerprints_rows_and_sequence_state_and_detects_drift

- Actual `infra/vps/bin/cantos-prefix-upgrade` helper runs psql against rows/sequence; replay fingerprint unchanged; actor mutation detected; sequence `is_called`-only change detected.
- No native replacement found.
- This is retained PG prefix-upgrade/recovery tooling evidence, not native ORM behavior. **BLOCKED for deletion** pending legacy recovery retirement/acceptance; #333/#336 and separate operator cutover. Do not drop merely because native has no sequences.

### 13. :583 listener_bookmark_expansion_is_the_exact_suffix_after_registry_one_two

- Actual PG SQL/checksums [1,2], bookmark absent; runtime refuses upgrade; migrator appends exact 3/4 SQL hashes, preserves first checksums/actor; replay identical.
- Partial: same native suffix tests as preceding expansion, plus `src/platform/turso/business_tests.rs:the_registry_of_eleven_upgrades_to_the_business_closure_without_rewriting_a_checksum`.
- Gap: native migration versions have different meaning; native upgrade is not literal preserved PG suffix/history or source conversion. Keep registered SQL/checksum oracle.

## Removed auth and actor sources

The original source is recoverable from Git at `84eea4c3`. Actor adapter tests below are in
`src/platform/identity/turso/tests.rs`. The shared native HTTP world is now
`tests/support/identity_world.rs`: real `ServerFixture`, production `api_router`, OS-peer-
authenticated UDS RPC, declared socket peer, independent rows and development sessions disabled.
Its old role configurations and network-DB provisioning are PG_SPECIFIC_OBSOLETE.

### identity_password_facts_postgres.rs — removed (14 original tests)

| Original test | Native replacement / retirement rationale |
| --- | --- |
| `a_stale_future_or_misbound_fact_never_becomes_a_session` | `identity_password_facts_turso::a_stale_future_or_misbound_fact_never_becomes_a_session` |
| `a_security_change_between_the_fact_and_the_issue_is_refused_and_cuts_no_session` | `identity_password_facts_turso::a_security_change_between_the_fact_and_the_issue_is_refused_and_cuts_no_session` |
| `every_security_change_revokes_durably_and_a_matching_snapshot_never_resurrects` | `identity_password_facts_turso::every_security_change_revokes_durably_and_a_matching_snapshot_never_resurrects` |
| `a_slow_or_confused_identity_fails_closed_without_revoking_anything` | `identity_password_facts_turso::a_slow_or_confused_identity_fails_closed_without_revoking_anything` |
| `a_login_that_outlives_its_deadline_is_an_unknown_effect_never_a_session_or_a_retry` | `identity_password_facts_turso::a_login_that_outlives_its_deadline_is_an_unknown_effect_never_a_session_or_a_retry` |
| `only_the_configured_peer_uid_is_trusted_as_identity` | `identity_password_facts_turso::only_the_configured_peer_uid_is_trusted_as_identity` |
| `an_operator_bound_actor_is_reused_and_an_inactive_one_is_never_logged_into` | `identity_password_facts_turso::an_operator_bound_actor_is_reused_and_an_inactive_one_is_never_logged_into` |
| `concurrent_first_logins_of_one_subject_bind_exactly_one_actor` | `identity_password_facts_turso::concurrent_first_logins_of_one_subject_bind_exactly_one_actor` |
| `a_studio_secret_is_accepted_only_from_its_own_secure_cookie` | `identity_password_facts_turso::a_studio_secret_is_accepted_only_from_its_own_secure_cookie` |
| `legacy_studio_session_logout_really_ends_an_identity_session` | `identity_password_facts_turso::legacy_studio_session_logout_really_ends_an_identity_session` |
| `an_unknown_subject_at_admission_is_a_durable_security_fact_not_an_outage` | `identity_password_facts_turso::an_unknown_subject_at_admission_is_a_durable_security_fact_not_an_outage` |
| `concurrent_refresh_of_one_secret_lets_one_win_and_revokes_the_family` | `identity_password_facts_turso::concurrent_refresh_of_one_secret_lets_one_win_and_revokes_the_family` |
| `renew_racing_logout_always_ends_revoked_and_never_fails_with_a_server_error` | `identity_password_facts_turso::renew_racing_logout_always_ends_revoked_and_never_fails_with_a_server_error` |
| `the_runtime_role_cannot_weaken_or_rewrite_a_session_whatever_its_privileges` | `identity_password_facts_turso::native_session_guards_preserve_binding_consumption_and_permanent_revocation`; actual native guards/checks replace PG-only role grants, SQLSTATE and registration-function syntax. `server_turso_barriers` retains shape/binding negatives |

### identity_password_postgres.rs — removed (15 original tests)

| Original test | Native replacement / retirement rationale |
| --- | --- |
| `signup_through_the_mailed_link_then_password_login_gives_an_httponly_cookie_session` | `identity_password_turso::signup_through_the_mailed_link_then_password_login_gives_an_httponly_cookie_session` |
| `wrong_credentials_and_unknown_accounts_get_one_generic_denial_without_a_session` | `identity_password_turso::wrong_credentials_and_unknown_accounts_get_one_generic_denial_without_a_session` |
| `signup_completion_is_replay_safe_and_never_issues_a_session` | `identity_password_turso::signup_completion_is_replay_safe_and_never_issues_a_session` |
| `web_and_native_credentials_of_one_account_resolve_to_the_same_actor` | `identity_password_turso::web_and_native_credentials_of_one_account_resolve_to_the_same_actor` |
| `native_bearer_session_renews_rotates_and_a_spent_refresh_revokes_the_family` | `identity_password_turso::native_bearer_session_renews_rotates_and_a_spent_refresh_revokes_the_family` |
| `a_native_bearer_write_needs_no_origin_and_is_admitted_as_the_account` | `identity_password_turso::a_native_bearer_write_needs_no_origin_and_is_admitted_as_the_account` |
| `native_logout_is_durable_and_a_wrong_kind_of_credential_is_refused_everywhere` | `identity_password_turso::native_logout_is_durable_and_a_wrong_kind_of_credential_is_refused_everywhere` |
| `cookie_mutations_need_the_exact_origin_and_browser_signals_never_reach_native_routes` | `identity_password_turso::cookie_mutations_need_the_exact_origin_and_browser_signals_never_reach_native_routes` |
| `a_cookie_of_one_surface_is_never_authority_on_another_or_with_provider_cookies` | `identity_password_turso::a_cookie_of_one_surface_is_never_authority_on_another_or_with_provider_cookies` |
| `a_late_request_from_the_previous_account_is_fenced_and_a_new_login_retires_the_old_cookie` | `identity_password_turso::a_late_request_from_the_previous_account_is_fenced_and_a_new_login_retires_the_old_cookie` |
| `studio_login_needs_the_server_roster_and_a_theatre_session_never_enters_studio` | `identity_password_turso::studio_login_needs_the_server_roster_and_a_theatre_session_never_enters_studio` |
| `web_renewal_extends_idle_but_never_the_absolute_deadline_and_expired_secrets_die` | `identity_password_turso::web_renewal_extends_idle_but_never_the_absolute_deadline_and_expired_secrets_die` |
| `identity_outage_fails_protected_requests_closed_but_logout_stays_local_and_recovery_resumes` | `identity_password_turso::identity_outage_fails_protected_requests_closed_but_logout_stays_local_and_recovery_resumes` |
| `a_subject_disabled_in_identity_loses_every_session_and_is_never_resurrected` | `identity_password_turso::a_subject_disabled_in_identity_loses_every_session_and_is_never_resurrected` |
| `an_http_development_origin_uses_plain_cookie_names_without_secure` | `identity_password_turso::an_http_development_origin_uses_plain_cookie_names_without_secure` |

### identity_postgres.rs — removed (9 original tests)

| Original test | Native replacement / retirement rationale |
| --- | --- |
| `baseline_replay_preserves_ownership_sessions_and_immutable_history` | `editorial_recovery_turso::repeated_native_migrations_preserve_business_bytes_and_corrupt_history_refuses_unchanged`; shared actor/schema Unicode barriers; native registry checksums are independent of PG history, whose version-count assertions are obsolete here |
| `identity_concurrent_registration_and_acknowledgement_loss_have_no_orphan_actor` | Actor adapter `concurrent_registrations_of_one_key_commit_one_binding_and_no_losing_actor`, `a_lost_acknowledgement_after_a_real_commit_reconciles_by_exact_key_replay` and exact issuer/case lookup tests |
| `identity_linking_collision_and_disable_are_fail_closed` | Actor adapter `rejections_and_inactive_or_missing_targets_change_no_row`, `registration_lookup_and_linking_follow_the_port_contract_on_a_real_file` |
| `identity_mutations_recheck_disable_after_actor_lock_wait` | Actor adapter `waiting_registration_replay_rechecks_a_committed_actor_disable` plus `a_held_writer_orders_competing_requests_and_readers_never_see_uncommitted_bindings` (waiting link) |
| `identity_constraints_immutability_rollback_and_http_role_isolation_hold` | `server_turso_barriers` exact-key/FK/Unicode and immutable insert/update/delete/REPLACE/upsert barriers; actor fault/rollback tests; PG roles and definer functions are obsolete, private-file/single-owner tests qualify the changed boundary |
| `identity_adapter_rolls_back_actor_when_binding_insert_fails` | Actor adapter `a_failed_step_after_the_actor_insert_rolls_back_the_speculative_actor`, extended with removal of the fault and clean retry |
| `identity_candidate_collision_across_distinct_keys_has_one_winner` | Actor adapter `distinct_identity_keys_competing_for_one_candidate_have_exactly_one_winner` (both requests pending behind one writer, exact Created/ActorIdCollision and rows) |
| `identity_operator_commands_consume_the_real_registry_without_issuing_a_session` | `operator_turso::actual_operator_registry_replays_bind_one_actor_and_never_issue_a_session` (actual CLI, exact stdout and two bindings, zero sessions/tokens) |
| `identity_exact_lookup_uses_primary_key_without_an_incremental_view` | Actor adapter `exact_lookup_uses_the_native_primary_key_on_a_bounded_registry`: 2,000 bindings, actual native EXPLAIN plan and 20 exact lookups plus case/issuer misses; PG EXPLAIN syntax retired, no cross-engine speedup claim |

The 15 real-Identity tests earn credentials through mailbox signup/password login with actual
Argon2id, a private Identity Turso file and run-owned SMTP sink. The 14 scripted tests inject
stale/misbound/slow/restored/disabled replies and check real Server rows and HTTP errors; they
remain mock integration for RPC. Security-change revocation is rechecked after reopening the
Server file. These are not browser/device, guarded Linux/Kani, deployed or legacy-conversion
acceptance. PG roles have no Turso equivalent; a file owner can alter schema, while HTTP authority
remains confined to the admitted production application and tested guards.
