# Security and schema PostgreSQL oracle inventory

Appendix to the [retained-oracle coverage ledger](legacy-test-oracles.md). Native mappings are source-only, not fresh behavioral acceptance.


Source inventory at `12fb751ec71e20233fe4caaee35e4544fe4a0d4b`: 4,203 legacy-source lines, including shared assertions, compared with native implementations and tests.

**Recommendation: retain all six files as NEEDS_PORT. None is wholly REPLACED or OBSOLETE.** Several individual assertions have strong native source counterparts, but important HTTP, concurrency, migration-history and Admin durability assertions remain unmatched.

Native execution/discovery listing: **NOT_RUN**, because `cargo` is absent on PATH. Native tests below are structurally connected to ordinary Cargo discovery, not verified execution: workspace members, default `tests/*.rs` discovery and `cfg(test)` module declarations are present.

## Important findings

- Admin is not merely obsolete Kanidm coverage. Its synthetic verifier exercises lasting atomic audit, immutable receipts, late authorization rechecks, concurrency and restart recovery.
- Admin native migration version 6 remains comments only in `src/platform/turso/admin.sql`. `src/platform/admin/postgres.rs` still calls `admin_identity_facts` / `admin_append_audit` / `admin_operation` / `admin_revoke_access`. Pure role tests and wire fixtures do not replace these effects.
- `identity_auth_candidate.rs` has real Identity/Turso integration but explicitly no router or cookie transport. Its obsolete “serving still composes PostgreSQL” comment is not current runtime truth.
- `theatre_business_journey.rs` provides genuine partial HTTP coverage, including real Web cookie login, native authorization, mixed/kind confusion refusal, cookie CSRF refusal, session fencing, outage and logout. It does not reproduce the full retained auth matrix.
- The preserved PostgreSQL epoch/checksum and prefix-helper assertions remain migration/recovery inputs. Native fresh-file schema tests cannot establish PostgreSQL data-conversion parity.
- `support/identity_world.rs` is still referenced by both retained password suites. Its removal would discard their harness contract.
- Retained password suites reference `support/fake_identity.rs` and `support/identity_process.rs`, which are now absent from the legacy support directory; current counterparts are under `tests/support`. Broken archived paths do not make their assertions obsolete.

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

## identity_password_facts_postgres.rs — NEEDS_PORT, 14 tests

Native **T** tests provide substantial checked-fact, durable session, concurrency and failure semantics. **D/W/C/G** supply pure validation/transport components. Several inner clusters have near-direct replacements, but the archived suite’s scripted RPC → production HTTP → stored-row/cookie observations are not completely rebound. Remaining integration belongs to **#333/#335**, with restore/security recovery under **#336**. The final role test must explicitly record the native process/file trust shift instead of inventing table-role equivalence.

### 1. :119 a_stale_future_or_misbound_fact_never_becomes_a_session

- 29-second-old fact can issue; 31-second-old, six-second-future, wrong/zero operation, wrong purpose, missing assurance and zero subject each return 503, no cookie, zero sessions.
- Partial: **D:a_fact_becomes_observed_security_only_inside_the_exact_age_window**; **W:a_fact_is_accepted_only_for_its_operation_purpose_and_assurance** and **malformed_facts_are_rejected_before_any_binding_decision**.
- Gap: whole scripted-RPC → native HTTP → zero stored session/no cookie for every mutant.

### 2. :150 a_security_change_between_the_fact_and_the_issue_is_refused_and_cuts_no_session

- Version advance/regression, disabled, epoch change → 401; misbound/unknown state → 503; no cookies/sessions.
- Strong inner replacement: **T:a_security_change_between_the_fact_and_the_issue_binds_and_cuts_nothing** reproduces all six mutations for Theatre/native and asserts no actors/bindings/sessions.
- Remaining assertion layer: HTTP exact statuses/no cookie. Retain whole-file oracle until rebound.

### 3. :193 every_security_change_revokes_durably_and_a_matching_snapshot_never_resurrects

- Advance/regress/disable/epoch change each deny next request, persist exact reason, refuse resurrection when original state returns.
- Strong inner replacement: **T:every_security_change_revokes_the_turso_family_durably_and_never_resurrects** covers all four plus missing subject.
- Remaining: production protected-HTTP path/status.

### 4. :243 a_slow_or_confused_identity_fails_closed_without_revoking_anything

- Delayed and unknown-state replies yield 503, never revoke; restored good answer resumes; state-call count proves each request checks Identity.
- Partial: **T:a_slow_identity_fails_closed_without_revoking_and_the_session_resumes** covers delay/recovery/no revocation; **D** decoding rejects unknown state.
- Gap: combined malformed-state/no-revocation HTTP behavior and explicit no-verdict-cache call-count assertion.

### 5. :270 a_login_that_outlives_its_deadline_is_an_unknown_effect_never_a_session_or_a_retry

- 14-second auth delay → 503/no cookie/zero sessions.
- Partial: client deadline implementation; **T:a_lost_acknowledgement_is_unknown_and_reconciles_without_a_retry** is a storage-COMMIT fault, not auth-RPC timeout.
- Gap: actual scripted auth timeout through native HTTP with persisted-row assertion.

### 6. :282 only_the_configured_peer_uid_is_trusted_as_identity

- Wrong configured Identity UID → 503/no cookie/zero sessions, zero state calls.
- Partial: `services/cantos-identity/tests/rpc_authorization.rs` tests missing caller credentials and UID allowlist, the opposite caller boundary.
- Gap: Server authenticates its Identity peer under wrong-UID actual socket, no session effects.

### 7. :295 an_operator_bound_actor_is_reused_and_an_inactive_one_is_never_logged_into

- Operator-created/link-bound actor reused on login, one actor; deactivation denies existing protected session and new login without creating another session.
- Partial: **R** registration/linking contract; **T:wrong_subject_actor_or_channel_and_an_inactive_actor_never_admit** covers actor disable/access/renew/rotate/issue/bind and unchanged rows.
- Gap: real operator binding consumed by IdentityAuth login/HTTP and exact session count after denial.

### 8. :339 concurrent_first_logins_of_one_subject_bind_exactly_one_actor

- Three successful HTTP logins same ActorId, exactly one actor and three sessions.
- Strong inner replacement: **T:concurrent_first_logins_of_one_subject_bind_exactly_one_actor** covers mixed Web/native, one actor/binding/three sessions.
- Remaining: HTTP status/body behavior.

### 9. :381 a_studio_secret_is_accepted_only_from_its_own_secure_cookie

- Valid Studio cookie authenticates to missing script (404); legacy/plain/other secure names with identical secret return 401.
- Partial: **C** credential/cookie tests; **G** gates; **J** Studio channel/roster.
- Gap: genuine issued Studio HTTP cookie plus exact wrong-name matrix. **H** only Theatre kind confusion.

### 10. :411 legacy_studio_session_logout_really_ends_an_identity_session

- DELETE `/api/v1/studio/session` with actual issued Identity cookie succeeds, persisted logout reason.
- No equivalent native legacy-Studio logout journey found.
- Generic route existence or SessionStore logout is not shared-handler dispatch parity.

### 11. :436 an_unknown_subject_at_admission_is_a_durable_security_fact_not_an_outage

- Identity NotFound → 401, persisted `subject-missing`, never resurrects on return.
- Strong inner replacement: **T:every_security_change_revokes_the_turso_family_durably_and_never_resurrects** includes this fifth mutation.
- Remaining: protected HTTP distinction from outage.

### 12. :456 concurrent_refresh_of_one_secret_lets_one_win_and_revokes_the_family

- Concurrent refresh statuses exactly one 200/one 401; winner’s new access also dies; reason `replay`.
- Strong inner replacement: **T:concurrent_refreshes_spend_once_and_the_loser_revokes_the_family** also proves exactly one spent token/new pair.
- Remaining: real HTTP refresh status/body wiring.

### 13. :487 renew_racing_logout_always_ends_revoked_and_never_fails_with_a_server_error

- Four races: renew 200 or 401, logout 204, next access 401, logout reason.
- Strong inner replacement: **T:renew_racing_logout_always_ends_revoked_and_never_fails_unavailable**.
- Remaining: native HTTP renew/logout race/statuses.

### 14. :513 the_runtime_role_cannot_weaken_or_rewrite_a_session_whatever_its_privileges

- Denies deletion/rebinding/time/security-snapshot/hash mutation through role grants; rejects idle shrink, spending Web token, unspending refresh, clearing/changing revocation or extending revoked family; malformed subject cannot register.
- Strong native schema counterparts: **S:identity_session_rows_admit_only_bound_well_formed_values** and **identity_session_rows_never_move_resurrect_shrink_or_disappear** cover binding/shape/idle/consumption/revocation and additional REPLACE/upsert barriers.
- PG role/GRANT/SQLSTATE/TRUNCATE semantics are obsolete as mechanisms. Document owner/private-file trust shift, not table-role equivalence. Retain file for other tests; this individual lasting schema cluster has strong source mapping.

## identity_password_postgres.rs — NEEDS_PORT, 15 tests

**J/T** implement real Identity plus native adapter semantics; **H/HL/P** give genuine partial production HTTP/process evidence; **C/G** test cookie serialization and admission gates. They do not replace the complete authentic-cookie, Studio, late account-switch, renewal, exact generic-denial and disable/recovery HTTP matrix. Remaining HTTP rebinding belongs to **#335**; data-plane gaps to **#333**, restore/disable recovery acceptance to **#336**.

### 1. :185 signup_through_the_mailed_link_then_password_login_gives_an_httponly_cookie_session

- Pre-signup login denied; real mail link/completion; exact cookie name/tag/length, HttpOnly/Secure/Strict/host-only/path; no secret in body, no-store, session response fence; protected/session views same actor/family; only 32-byte digest persisted; logout clears cookie, durable logout.
- Partial: **J** real signup/login/binding/logout; **H** real Web login/cookie admission; **C:cookies_are_host_only_http_only_strict_and_path_scoped_per_surface**; **T:stored_rows_hold_only_digests_of_the_secrets**.
- Gap: combined real HTTP cookie attributes/body/header/session-view/clear-cookie/durable logout assertions.

### 2. :265 wrong_credentials_and_unknown_accounts_get_one_generic_denial_without_a_session

- Wrong password and nonexistent account yield same 401 body, no cookies, zero sessions.
- Partial: **J** wrong password; Identity application tests own verifier decisions.
- Gap: both real Server HTTP responses compared, exact zero sessions/no cookies.

### 3. :286 signup_completion_is_replay_safe_and_never_issues_a_session

- Exact completion replay says `replayed`/no cookie; altered password denied; absent explicit confirmation bad-request; one Identity binding.
- Partial: **J** completed/replayed real command; Identity signup tests.
- Gap: changed-password and explicit-confirmation HTTP DTO/outcome/no-cookie/binding-count matrix.

### 4. :329 web_and_native_credentials_of_one_account_resolve_to_the_same_actor

- Same account Web/native → same actor, distinct families; second actual account → different actor.
- Partial: **J** same actor Web/native; **H** actual two-account native distinction and Web login, but does not compare Web’s actor/family to native’s here.
- Gap: full explicit real HTTP comparisons.

### 5. :348 native_bearer_session_renews_rotates_and_a_spent_refresh_revokes_the_family

- Native receives no cookie; token_type/prefixes, policy windows, session view; rotation keeps actor/family/authentication/absolute time, changes access; old denied/new allowed; spent refresh kills winner with replay reason; spent refresh cleanup remains 204.
- Partial: **J** and **T:issue_access_renewal_rotation_and_logout_follow_the_port_contract_on_a_real_file** cover core/real Identity rotation; **H/P** establish real HTTP native login.
- Gap: native HTTP refresh/revoke/view/response token shape/timing/no-cookie matrix.

### 6. :445 a_native_bearer_write_needs_no_origin_and_is_admitted_as_the_account

- Authenticated originless write passes admission; anonymous same write is refused.
- Genuine stronger source replacement: **H** performs accepted Library writes as actual native account without Origin, checks rows and anonymous/unusable credential refusal; **HL** repeats accepted write then denial after logout; **P** exercises actual binary.
- This individual invariant has strong native HTTP source parity. Whole file remains NEEDS_PORT; execution NOT_RUN.

### 7. :485 native_logout_is_durable_and_a_wrong_kind_of_credential_is_refused_everywhere

- Mixed cookie/Bearer; cookie/refresh as Bearer; access/refresh as cookie; wrong endpoint kind; Origin on native all denied; native logout invalidates only its family, Web stays live.
- Partial: **H** covers mixed/cookie-as-Bearer/access-as-cookie/refresh-as-Bearer; **G** exact kind and browser signals; **HL/P** native logout.
- Gap: complete endpoint/kind matrix, refresh-as-cookie, native logout leaves same-account Web family live.

### 8. :571 cookie_mutations_need_the_exact_origin_and_browser_signals_never_reach_native_routes

- Missing/evil/sibling/downgraded/wrong-port Origin on login forbidden/no cookie; renew requires Origin; forged logout does not end session; cookie Library write without Origin denied; native login rejects Origin/cookie/fetch-site/fetch-mode; unknown DTO field bad-request/no-store.
- Partial: **G:web_admission_requires_the_exact_origin_for_mutations_and_refuses_mixed_credentials**; **native_admission_refuses_every_browser_signal_and_exactly_one_bearer_of_its_own_kind**; **H** cookie mutation CSRF negative.
- Gap: actual issued-cookie login/renew/logout and strict HTTP DTO matrix, including no adverse logout effect.

### 9. :667 a_cookie_of_one_surface_is_never_authority_on_another_or_with_provider_cookies

- Theatre secret under Studio name, wrong channel tag, plain-name HTTPS, duplicate same-name cookies and mixed old-provider cookie all denied.
- Partial: **C** tag/cookie unit tests, **G**, surface-policy ambiguity negatives; **H** kind confusion.
- Gap: exact authentic-secret, cross-surface/name/tag/duplicate/provider-mix HTTP cases. Old provider login is obsolete, rejecting confusing old cookie remains meaningful.

### 10. :734 a_late_request_from_the_previous_account_is_fenced_and_a_new_login_retires_the_old_cookie

- A→B browser login retires A family; B cookie with A fence returns conflict and B id; late A logout cannot end B; honest B accepted.
- Partial: **H** explicitly tests native wrong-family fence/conflict/no-effect.
- Gap: real Web account-switch previous-cookie retirement, late logout protection and B survival.

### 11. :814 studio_login_needs_the_server_roster_and_a_theatre_session_never_enters_studio

- No roster → route 404; wrong subject 403/no Studio session; wrong password 401; allowed subject Studio cookie tag/path and same actor; data-plane/fence header; no cross-surface authority; removed roster denies existing session.
- Partial: **J** Studio roster, same actor, no-roster admission and channel isolation; **C** cookie shape.
- Gap: real Studio HTTP route/status/cookie/data-plane/fence/count/removal behavior. Remaining Studio context data plane belongs to #333.

### 12. :904 web_renewal_extends_idle_but_never_the_absolute_deadline_and_expired_secrets_die

- Renewal moves aged idle bounded by absolute, same cookie with refreshed age; expired token denied on read and renewal.
- Partial: **J** Web renewal; **T:every_deadline_is_exclusive_and_no_renewal_moves_the_absolute_one** and **issue_access_renewal_rotation_and_logout_follow_the_port_contract_on_a_real_file**.
- Gap: actual HTTP renewal preserves exact cookie/Max-Age and rejects expired HTTP renewal.

### 13. :958 identity_outage_fails_protected_requests_closed_but_logout_stays_local_and_recovery_resumes

- Real Identity stopped: cookie and Bearer protected access/login 503/no login cookie; native logout still 204 locally; same Identity DB restarts, surviving Web resumes, logged-out native stays dead with durable reasons.
- Partial: **J** outage/recovery and local logout separately; **H** real HTTP native outage/no state mutation/public reads survive.
- Gap: combined Web/native outage + local logout + same-instance HTTP recovery and row outcomes.

### 14. :1020 a_subject_disabled_in_identity_loses_every_session_and_is_never_resurrected

- Actual Identity owner stopped; disable through its library; restart; both Web/native next HTTP access denied, durable disabled reason; next login generically denied.
- Partial: **T** scripted disabled-state revocation; Identity persistence disable tests.
- Gap: actual Identity disable consumed across service boundary through Server HTTP for both channels. Scripted facts are not this evidence.

### 15. :1074 an_http_development_origin_uses_plain_cookie_names_without_secure

- Real signup/login with loopback HTTP emits plain `cantos_id_theatre` without Secure, retaining HttpOnly/Strict.
- Partial: **C** cookie serializer tests.
- Gap: real configured HTTP-origin signup/login transport/cookie response.

## identity_postgres.rs — NEEDS_PORT, 9 tests

**R/S** provide substantial real-file registry, exact-key, binding, failure and schema evidence, but this file mixes those assertions with unported editorial/session migration data preservation, a distinct-key candidate race, real operator CLI, and attributable lookup-plan evidence. Those gaps prevent whole-file deletion. Porting is **#333**; legacy data conversion and recovery are **#336**; measured lookup baseline is also **#384**. PG role isolation is a documented trust shift, not a native table privilege claim.

### 1. :76 baseline_replay_preserves_ownership_sessions_and_immutable_history

- Seed legacy actor/session/evidence, save accepted script, replay migrations twice, link identity; exact canonical bytes/accepted timestamp/accepted_by/digest/load/session authority preserved; four legacy migrations, first version and 32-byte checksums; invalid Unicode issuer/subject rejected.
- Partial: **S** upgrade preserves actors/bindings and native history; **S** external-key barriers; native registry tests.
- Gap: legacy authoritative script bytes/timestamps/digests/session/evidence across migration and exact legacy registry history. These are #333 editorial/session-conversion and #336 Tier B inputs, not obsolete PG details.

### 2. :166 identity_concurrent_registration_and_acknowledgement_loss_have_no_orphan_actor

- Concurrent same-key candidates → one Created/one Existing, identical actor; fresh-pool replay after discarded acknowledgement resolves winner; exact issuer/case separation; row counts exclude orphans.
- Strong source counterparts: **R:concurrent_registrations_of_one_key_commit_one_binding_and_no_losing_actor**; **a_lost_acknowledgement_after_a_real_commit_reconciles_by_exact_key_replay**; **registration_lookup_and_linking_follow_the_port_contract_on_a_real_file**.
- Lasting invariant substantially represented on native files. Execution NOT_RUN; keep whole file for uncovered tests.

### 3. :217 identity_linking_collision_and_disable_are_fail_closed

- Actor collision/missing/inactive target; no premature binding; link replay; second issuer; no transfer; disabled bound actor cannot register; lookup reports inactive and rows remain exact.
- Strong source counterparts: **R:rejections_and_inactive_or_missing_targets_change_no_row** and **registration_lookup_and_linking_follow_the_port_contract_on_a_real_file**.
- Native assertions directly check unchanged stored rows and exact variants. Execution NOT_RUN.

### 4. :271 identity_mutations_recheck_disable_after_actor_lock_wait

- Runs both existing-key registration replay and new-key linking behind observed actor lock; concurrent disable commits; both reject inactive, no new binding/orphan.
- Partial: **R:a_held_writer_orders_competing_requests_and_readers_never_see_uncommitted_bindings** tests waiting link against deactivation and waiting register against binding commit/rollback.
- Exact gap: waiting registration replay of a bound identity whose actor becomes disabled; not the same as the already-disabled sequential case. Preserve this branch for #333.

### 5. :358 identity_constraints_immutability_rollback_and_http_role_isolation_hold

- Duplicate/FK/empty values; immutable UPDATE/DELETE/TRUNCATE; explicit transaction rollback; HTTP role cannot read/write registry and unauthorized registry request fails with no effects.
- Strong native schema partial: **S:bindings_are_exact_byte_keys_reference_an_actor_and_never_change**; **external_key_checks_admit_exactly_what_the_domain_constructor_admits**, including REPLACE/upsert.
- Native **R** rollback tests cover failed-write atomicity. PG roles/SQLSTATE/TRUNCATE have no native equivalent and must be recorded as trust shift, not claimed preserved table privilege. Application/OS boundary still requires scoped evidence.

### 6. :411 identity_adapter_rolls_back_actor_when_binding_insert_fails

- Trigger refuses binding; adapter unavailable; zero actors/bindings; remove fault then same candidate succeeds exactly once.
- Strong partial: **R:a_failed_step_after_the_actor_insert_rolls_back_the_speculative_actor** injects real trigger failure and validates no changed rows.
- Small exact gap: clean retry after removing the injected binding fault is not performed in that native test. Do not silently erase this recovery assertion.

### 7. :433 identity_candidate_collision_across_distinct_keys_has_one_winner

- Two different subject keys race on same proposed actor; exactly one Created and one ActorIdCollision; one actor/binding.
- Partial: **R** same-key multi-candidate race and sequential actor collision are different partitions.
- Gap: actual distinct-key/same-candidate native race and exact one-winner error/counts.

### 8. :466 identity_operator_commands_consume_the_real_registry_without_issuing_a_session

- Actual operator binary register replay stable stdout; explicit link Linked then Existing; one actor/two bindings; zero sessions.
- Partial: `src/platform/operator.rs` uses TursoIdentityRegistry; its only unit test checks input syntax.
- Gap: real native operator CLI register/link replay/output, stored binding counts and no session issuance.

### 9. :516 identity_exact_lookup_uses_primary_key_without_an_incremental_view

- 2,000 bindings, actual PG EXPLAIN uses exact PK; 20 correct active lookups; subject case miss; reports warm timings.
- Partial: **R** exact lookup and case/issuer semantics, **S** byte-key schema.
- Gap: native authoritative-engine lookup plan and attributable workload baseline. No incremental engine is needed; retain performance oracle for #333/#384.

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

## support/identity_world.rs — NEEDS_PORT/support, no standalone tests

### Direct consumers

- `legacy-pg-tests/tests/identity_password_facts_postgres.rs`.
- `legacy-pg-tests/tests/identity_password_postgres.rs`.

### Responsibilities and embedded assertions

- `Database::new` validates disposable operator principal/database, creates unique DB, explicitly migrates, applies production `runtime-grants.sql`, then separates operator and runtime configurations.
- `store()` disables development sessions, ensuring tests cannot accidentally pass using a development credential.
- `App::new` composes production `api_router` with real `IdentityAuth` socket/peer UID/DirectPeer policy/Studio roster and HTTPS-derived cookie mode.
- `send()` constructs genuine method/header/JSON requests, inserts declared socket peer, dispatches router, captures status/headers/body.
- `Reply::cookie` asserts exactly one Set-Cookie and parses the actual browser name/value.
- Provides real DB access for independent stored-state assertions and operator-vs-runtime testing.

### Native partial counterparts and gap

- `test_support::ServerFixture` gives run-owned file-backed owner/SQL/restart facilities.
- `tests/support/identity_process.rs` and `fake_identity.rs` provide real/scripted RPC services.
- **H** compose/send/enroll builds production `server_router` and a real Identity, but uses different origin/storage wiring and mostly native accounts.
- **G** handles pure transport gates, not whole HTTP/session fixture parity.

Required port: build a shared native auth-HTTP world using `ServerFixture`, production router, DirectPeer UID, explicit cookie/origin/Studio roster configuration, no-development-session guarantee and independent row inspection. Replace role/grants harness responsibilities with documented process/file trust boundaries. Preserve exactly-one-cookie assertion and request peer identity. Retain this helper until its two remaining consumers are ported or deliberately split.

## Conclusion

All **66 legacy test functions** plus support responsibilities inventoried against actual native source. Native runtime results, full HTTP rebinding, preserved PG data equivalence and Admin effects remain unestablished. No inventoried whole file meets the safe deletion threshold.
