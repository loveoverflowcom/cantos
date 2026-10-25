# Cantos Bearer lifecycle (#253)

Module-owned inventory of [`cantos.identity.bearer-lifecycle`](../../../../specs/behavior/catalog.json). The [root inventory](../../../../specs/behavior/inventory.md) keeps the shared authority framing; the registry owns the stable ID and lifecycle.

This live pure-core/native capability remains executable. The removed provider-era
`bearer_postgres.rs` is [historical source](https://github.com/loveoverflowcom/cantos/blob/c3acd81bfa94fe29bf4944dcafd5520dd0d50cdd/services/cantos-server/tests/bearer_postgres.rs),
not a native acceptance binding. Current catalog refs name the live SessionPolicy/rotation core,
Identity-session adapter and native journal/lifecycle tests. Properties, runs, witnesses and
negative controls are unchanged; no native acceptance is inherited from the old test.

`cantos.identity.bearer-lifecycle` remains **draft**. Approved direction is owner-selected
rotation, bounded authority, durable native cleanup, single-flight refresh and late-result/account
isolation; concrete policy is in [Bearer v1](../../contracts/auth/bearer/v1/README.md). The API
projection models one family, generations 0–3, active/expired authority and one immutable
authentication timestamp.

The Rust/API oracle tests previously mapped here to `run.refresh`, `run.replay`, `run.logout` and
`run.expiry` no longer exist: the SessionPolicy reconciliation (dfe5697f) and the provider
authentication retirement (cf4fa221) removed them. The remaining pure-core tests in
[`bearer::core`](../../src/platform/bearer/core.rs) cover issuance
deadlines, inactive/replay/expiry precedence and rotation without rebasing the absolute
deadline; Identity-auth session tests exercise the reused `rotate`. A run-by-run remap is
**pending owner review**. Until then, no API run claims a named Rust oracle, and model runs
remain model-only evidence.

Web is no longer a consumer of the Bearer refresh-family lifecycle: it holds an HttpOnly cookie
session that renews in place. `cantos-web-auth` core/owner/refresh tests cover the Web session's
exact use/renew/re-read edges, renewal against its dispatch snapshot, unknown renewal,
single-flight, cancelled-login dispatch, the 429 hold and late-waiter/logout/account-switch fences
through the production owner and a scripted port. The retired `verify web-bearer` mock-browser fixture existed only for the removed popup
flow. These are client implementation evidence, not model trace conformance, a Web cookie model
or real Identity/Server proof.

The same capability also has a finite native shell projection: Active/Refreshing/Cleared journal
classes, two actor aliases, intent classes 0–3 and at most two modeled dispatches across the
history. These are equality/history bounds, not production account IDs, counter ranges or a new
refresh policy. A recorded journal write, API confirmation/revocation and fresh authorization
are supplied facts; the model does not execute those adapters. `pendingPair` is an in-process
point between response and durable replacement, not a persisted record. Prior access can remain
admitted during its one refresh flight; a restart discards that in-process admission. An
interrupted Refreshing record or cleanup barrier permits cleanup only, never another refresh.
The Cleared marker must be durable before the cleanup barrier can clear. Marker deletion and
missing/corrupt storage, byte decoding, entropy, clocks, full-pair validation and platform atomicity
remain independent native tests. Reaching intent class 3 permits cleanup but no new modeled login;
it is a finite terminal abstraction, not acceptance of real counter exhaustion.

The following stable runs map to actual [Identity v1 lifecycle tests](../../../../apps/cantos-theatre/mobile/shared/src/commonTest/kotlin/com/cantos/theatre/application/identity/NativeIdentityV1LifecycleTest.kt).
Mappings express shared requirements; these tests do not consume model traces or prove model
conformance. Source presence does not report that any native test ran.

| Stable run | Property / reachable witness | Independent native implementation oracle |
| --- | --- | --- |
| `run.native-journal-before-refresh` | `property.native-journal-before-refresh` / `witness.native-refresh-dispatched` | `refreshing_write_failure_stops_before_effect_and_replacement_failure_revokes` |
| `run.native-single-flight` | `property.native-single-flight` / `witness.native-joined` | `timer_and_private_waiters_share_refresh_and_waiter_cancellation_does_not_spend_twice` |
| `run.native-unknown-refresh` | `property.native-unknown-cleanup-only` / `witness.native-unknown-stopped` | `all_refresh_failures_revoke_known_family_and_never_replay_a_spent_secret` |
| `run.native-pair-write-failure` | `property.native-persist-before-admission` / `witness.native-pair-write-refused` | `refreshing_write_failure_stops_before_effect_and_replacement_failure_revokes` |
| `run.native-interrupted-refresh` | `property.native-restart-needs-confirmation` / `witness.native-interrupted` | `restart_at_refreshing_active_pending_and_cleared_never_replays_refresh_or_password` |
| `run.native-restore-confirmation` | `property.native-restart-needs-confirmation` / `witness.native-interrupted` plus explicit successful confirmation checkpoints | `restore_is_api_confirmed_or_safely_refreshes_expired_access`; `restore_requires_the_exact_actor_family_and_deadline_from_the_api` |
| `run.native-logout-late-result` | `property.native-late-result-fenced` / `witness.native-late-ignored` | `logout_hides_and_marks_synchronously_then_a_late_refresh_cannot_restore_authority`; `late_restore_session_A_cannot_admit_over_explicit_login_B` |
| `run.native-durable-cleanup` | `property.native-marker-before-barrier-clear` / `witness.native-durable-cleanup` | `offline_logout_hides_synchronously_and_restart_retries_only_exact_family_cleanup`; `restart_after_cleared_before_barrier_clear_finishes_without_network`; `missing_corrupt_foreign_and_unavailable_barriers_cannot_be_erased_by_login` |

[Native journal tests](../../../../apps/cantos-theatre/mobile/shared/src/commonTest/kotlin/com/cantos/theatre/adapters/identity/NativeCredentialJournalTest.kt)
independently exercise full-pair/Cleared records, decoding, readback, exact API-origin and protocol binding, retired-record recognition and
storage failure outcomes. They do not establish Android KeyStore/iOS Keychain behavior on a device.
The catalog's seven new negative controls respectively inject dispatch without a Refreshing
journal, duplicate flight, unknown-result retry, adoption after failed pair write, admission after
restart, disabled actor/intent response fencing and barrier clearing before the durable marker.

The catalog owns named runs, witnesses and one sensitive negative control per property.
This capability has **no registered trace-observer conformance adapter**. Model execution is sampled
model evidence only. It does not establish SQL isolation, native controller/transport/storage
conformance, actual process death, browser/issuer acceptance, Library/media isolation, native
playback or installed Android/iOS secure storage. Those claims require their own attributable
implementation, integration and named-device evidence.

## Legacy scope: `cantos.identity.bearer-lifecycle`

The two native capabilities, [mailbox proof](../../../cantos-identity/behavior/native_mailbox_proof.md)
and [session admission](native_session_admission.md), **do not replace** any Bearer lifecycle ID,
and its meaning is unchanged. Its consumers are live: [`bearer::core`](../../src/platform/bearer/core.rs)
(whose `rotate` the Identity-auth native refresh also reuses) and the Theatre
Mobile `NativeAccountController`/`NativeCredentialJournal`, so it is not deprecated. Mapping:
rotation, replay, logout and expiry of a refresh family stay in `bearer-lifecycle`;
the security-state recheck that runs *before* rotation, and mailbox proof consumption, are the
new capabilities. Whether `bearer-lifecycle` runs also describe `ci1_` Identity-auth families
is a review decision this change does not make.
