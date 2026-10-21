# Capability authority inventory

The [catalog](catalog.json) owns stable IDs and model lifecycle. This inventory separates
**approved requirement**, **implemented code**, **model assumption** and **needs-decision**.
A model PASS neither accepts a draft requirement nor demonstrates implementation conformance.
The [decision](../../docs/decisions/0027-quint-executable-behavior.md) adopts Quint for reviewed
executable stateful expectations while preserving Protobuf and native test boundaries.

## Selected pilot: `cantos.media.private-audio-admission`

| Classification | Authority / fact | Pilot consequence |
| --- | --- | --- |
| Approved, narrow development scope | [ADR 0020](../../docs/decisions/0020-studio-private-audio-intake.md#decision) selects exact revision-bound private intake, actual byte inspection, immutable receipts, exact replay and commit-time rechecks. [ADR 0018](../../docs/decisions/0018-external-audio-production.md) keeps all audio production external. | No generation, encoding, public delivery, paid provider or new product rule. |
| Approved contract expectation | [Audio Lifecycle v1](../../packages/contracts/audio-lifecycle-v1.md#state-reviews-and-candidate-blockers): exact duplicate PUT returns the same rendition; contradictory settled bytes conflict; revoked/expired/stale producer replay fails; owner precedence is stale, revoked, admitted, expired, awaiting_bytes. | Named success/rejection runs and checkpoint invariants cover only this subset. |
| Implemented, independently of this model | [Pure lifecycle](../../services/cantos-server/src/contexts/media/domain/lifecycle.rs) `decide_admission` and `submission_state`; [application](../../services/cantos-server/src/contexts/media/lifecycle_application.rs) invokes them before inspection and again before settlement. [Existing native tests](../../services/cantos-server/src/contexts/media/lifecycle_tests.rs) cover digest/replay, lower clock bounds and foreign settled binding. | These are real PR B observation seams. PR A calls Quint, not these functions. Existing tests stay. |
| Implemented boundary, excluded here | `AudioSubmission::create/restore`, `AudioRendition::create/restore`, ownership, immutable full bindings, profile admission and same-receipt transaction arbitration. | The pilot starts with one valid prepared submission. It cannot certify constructors, authentication, foreign/corrupt database rows, actual bytes or concurrent commits. |
| Implemented external fact | [Brief freshness](../../services/cantos-server/src/contexts/production/domain/brief.rs) is recomputed from content, rights, latest guidance and archive status; existing [Work](../../services/cantos-server/src/contexts/catalog/domain/work.rs) and [Unit](../../services/cantos-server/src/contexts/catalog/domain/unit.rs) archive/restore can restore current authority for the same immutable binding. | `restoreCurrent` supplies that recovered fact; named admission/replay runs show it does not clear revocation or replace the settled receipt. No real archive/API/DB action runs. |
| Model assumption | One synthetic submission; digest/size aliases 0=exact and 1=conflicting; they are equality classes, not literal hashes or byte lengths. Time classes 0–4 use created_at=1, expires_at=3 and include backwards observations. | No numeric-range/hash/profile completeness or monotonic SQL-clock assumption. The real constructor still owns the 24-hour/positive-size bounds. |
| Model assumption | `Inspecting` is bounded simulator/shell history, not a new persisted state. `complete(true)` supplies abstract independently verified media/storage facts and rechecks current authority. `abandon` drops only that harness attempt. | `Inspect` is not admitted bytes. No actual inspection/storage, cancellation, restart, SQL isolation or request-lane claim. |
| Implemented bounded observation | Exact ITF codec and event-only test observer call the existing production admission/projection and typed constructors, with independent full receipts and decision/effect counts. | [`--conformance` protocol](observation-protocol.md) maps fixed aliases, rejects stale identity and compares original traces separately from semantic sensitivity controls. Synthetic shell facts do not establish L4 application or L5 IO acceptance. |

### Exact pure decision boundary

The model preserves `decide_admission` precedence: revoked, expiry upper bound, creation
lower bound, stale Brief, payload mismatch (`DigestMismatch` before settlement;
`OperationReused` after), settled admission lower time bound, exact replay or inspection.
Foreign settled identity is outside the valid one-submission instance and remains covered by
native tests. Owner history uses a separate projection: a settled record survives upload
expiry, and stale plus revoked stays visibly stale. Never infer producer access from that view.
The independent explicit `transition(state, event)` matrix is retained in native tests;
observing a later clock is not its explicit `Expire` event.

## Draft: `cantos.media.public-playback-delivery` (#170)

| Classification | Authority / fact | Model consequence |
| --- | --- | --- |
| Proposed, not accepted | [ADR 0032](../../docs/decisions/0032-theatre-origin-playback-delivery.md#decision) recommends origin-mode Range delivery with sealed 120 s grants and per-fetch rechecks; [ADR 0038](../../docs/decisions/0038-publication-streaming-evidence-gates.md) keeps its evidence gates. | Lifecycle stays `draft`; a PASS is model-only and accepts nothing. |
| Implemented, independently of this model | [Pure kernel](../../services/cantos-server/src/contexts/media/domain/playback_grant.rs) `grant_expiry`/`decide_media_fetch`; [application](../../services/cantos-server/src/contexts/media/listener_media.rs) admits before planning and again after object open; [authorization](../../services/cantos-server/src/contexts/media/listener_application.rs) re-resolves after sealing. Native example/property/tamper tests and the route mock integration stay authoritative for their boundaries. | `admit` and `issuedExpiry` mirror those seams; no Rust observer is registered (`--conformance` does not apply). |
| Model assumption | One immutable publication (identity 1); another unit's release is identity 2. Time classes 0–6 with publishedAt=1, rightsUntil=5, TTL=2, including rollback. `authentic` abstracts the HMAC result; tokens, keys, URLs and bytes are not modeled. Eligibility loss (withdraw/revoke/supersede/owner disable) is one irreversible fact. | No hash, Range, header, SQL isolation, restart or concurrency claim; HTTP transport is native-tested. |
| Model assumption | `Opening` is shell history between admission and the post-open recheck; `completeFetch` abstracts verified object bytes. | Covers withdrawal/expiry while the object opens; not lane bounds or body streaming. |

## Draft: `cantos.media.public-cover-projection` (#171, proposal for #157)

| Classification | Authority / fact | Model consequence |
| --- | --- | --- |
| Proposed, not accepted | [ADR 0033](../../docs/decisions/0033-release-pinned-public-cover.md#decision) recommends the release-pinned cover plus an explicit per-image disclosure; #157 has not accepted it. | `draft`; no wire field or delivery follows from a PASS. |
| Implemented, independently of this model | [Pure kernel](../../services/cantos-server/src/contexts/media/domain/public_cover.rs) `project_public_cover`/`cover_outcome` over existing [artwork history](../../services/cantos-server/src/contexts/media/domain/artwork.rs) `artwork_at`. No production shell supplies a disclosure; it is always `Undecided`. | `effectiveAt`, `disclosure` and `coverView` mirror those seams; no observer is registered. |
| Model assumption | Two synthetic images; asset binding is exact (binding/corruption partitions are native tests). Release eligibility is one flag; a publish pins the current head revision. Time classes 0–6. | No SQL, storage, wire, cache or client claim. |

## Deferred capabilities

| Stable capability ID | Approved / implemented | Assumed / needs-decision | Why deferred |
| --- | --- | --- | --- |
| `cantos.publishing.lifecycle` | [Publication v1](../../packages/contracts/publication-v1.md) and [domain](../../services/cantos-server/src/contexts/media/domain/publication.rs) implement readiness, clearance/approval, immutable releases, active pointers and withdrawal. | Real public delivery/provider acceptance remains separate; choose an exact candidate/pointer/observation scope before making a model authoritative. | Too many independent gate and delivery dimensions for this foundation pilot. No `.qnt` or conformance claim. |
| `cantos.listening.progress` | [Listener Library v1](../../packages/contracts/listener-library-v1.md) and [reconcile core](../../services/cantos-server/src/contexts/listening/domain/reconcile.rs) implement exact-publication conditional updates, retained history and stale conflict classification. | Cross-client offline/native lifecycle and complete replay scope need review; UI mocks do not accept those semantics. | Keep the current kernel/native tests; establish publication/media access acceptance before cross-stack scenarios. |

Private storage target/version/hash/range policies retain their
[own authority](../../docs/architecture/object-storage.md); a successful model does not accept a
provider. Catalog, script visibility, saved items, offline synchronization, wider API/UI/CMP
scenarios and artwork admission are outside this pilot. Their existing code/contracts/tests
remain authoritative at their current boundaries until an explicitly reviewed model migration.

## Draft: `cantos.tooling.hosting-deploy` (#199)

| Classification | Authority / fact | Model consequence |
| --- | --- | --- |
| Reviewed narrow request | [ADR 0012](../../docs/decisions/0012-xtask-command-surface.md#decision) and the [Hosting command contract](../../docs/deployment/firebase-hosting.md) require explicit one-app intent, a validated bundle, durable intent before one never-retried remote write and honest uncertain outcomes. | The lifecycle stays `draft`; this model does not accept production Hosting or any new origin/auth choice. |
| Implemented, independently of this model | [Coordinator](../../xtask/src/deploy/flow.rs) and [receipt](../../xtask/src/deploy/receipt.rs) preserve release outcome separately from smoke and local persistence; native mocked coordinator tests remain the implementation oracle. | No Rust observer is registered; model PASS is not implementation conformance. |
| Finite assumption | One run and one selected app/bundle; preparation and persistence are abstract booleans, and remote outcomes are nondeterministic facts. | Covers intent-before-effect, no repeat write and confirmed-effect retention after failed smoke/local receipt. It does not parse config/URLs, hash bytes, execute Firebase or prove permissions, transport, crash durability or actual deployment. |

## Draft: `cantos.production.forge-attempt` (#203)

| Classification | Authority / fact | Model consequence |
| --- | --- | --- |
| Reviewed narrow safety direction, not runtime activation | [Issue #203](https://github.com/loveoverflowcom/cantos/issues/203) section 5's durable safety requires an operation and a budget reservation before a billable provider effect; the same key with the same request returns the record, a changed request conflicts, an explicit regeneration is the only new attempt, and an uncertain outcome is never silently replayed. [ADR 0022](../../docs/decisions/0022-cantos-forge-producer-cli.md) accepts product direction, not runtime packaging; [ADR 0018](../../docs/decisions/0018-external-audio-production.md) keeps provider cost and attempts outside the API. | The capability stays `draft`. A PASS accepts neither #203's runtime, authorization and storage implementation nor any provider. |
| Implemented independently of this model | The pure [attempt core](../../services/cantos-forge-functions/src/domain/attempt.rs) (`Ledger`, `submit`, `regenerate`, `settle`, replay), the [request fingerprint](../../services/cantos-forge-functions/src/domain/generation.rs) and the [use cases](../../services/cantos-forge-functions/src/application/produce.rs) over the `AttemptStore` and `Provider` ports with a [deterministic mock provider](../../services/cantos-forge-functions/src/adapters/mock_provider.rs). Every registered run has one native mirror test marked `Quint run:`; [`behavior_binding.rs`](../../services/cantos-forge-functions/tests/behavior_binding.rs) keeps the two lists equal. | No Rust observer is registered: `--conformance` does not apply and the mirror tests are traceability, not an observation of the model's traces. |
| Native storage-boundary hardening | A heterogeneous first-reservation race rechecks the winner's fingerprint; lost-outcome reconciliation requires full valid history, the original replayed prefix and exact event. Corrupt/foreign histories fail closed for generate, regenerate and recover. | The thirteenth named run covers the sequential conflict while open. Fault-injected mocks exercise the actual use cases, not a concurrent or durable store; corrupt prefix/suffix and valid later regeneration remain native boundary tests. |
| Finite assumption | One operation; two request classes (equality classes, not hashes); each attempt costs 1 or 2 units against a budget of 3 and at most 3 attempts. `dispatch` abstracts that the reservation winner started the one provider call; a rejection may be recorded with no call; uncertain outcomes are nondeterministic facts; the operator's acknowledgement is an integer naming an attempt (0 = none). | Covers reserve-before-call, one call per attempt, replay/conflict/refusal changing nothing, acknowledged-only regeneration and bounded spend. It does not model a provider, the store's conditional write, a clock, authorization or billing. |
| Needs decision, not modeled | (1) Reconciling an uncertain attempt from provider evidence or adopting a result a vanished owner later produced; (2) the lease or deadline rule that decides an open attempt's owner is gone (today only an explicit `recover`); (3) the durable producer-side store (filesystem, object store or a native record store) and its verified conditional-write semantics; (4) cost beyond the budget at the first reservation (assumed to fit; the native tests and the Kani kernel cover it). | These stay open in #203; no model action decides them and no outcome is promoted by simulation. |

The integer kernel behind these rules (`Ledger`: attempt count, reserved spend, phase) also has
a registered bounded proof, `forge-attempt-ledger`; see the
[Kani ledger](../../docs/assurance/kani.md#forge-attempt-ledger-kernel-203). Sampled model
properties, the bounded proof and the example/property/mock tests are separate evidence.
`callsNeverExceedReservations` is an aggregate bound. The model's per-attempt dispatch guard
and native call-by-attempt assertions carry the distinct no-second-call expectation; none
establishes a real adapter's cross-process fencing or provider behavior.

## Draft: `cantos.presentation.locale-preference` (#169)

| Classification | Authority / fact | Model consequence |
| --- | --- | --- |
| Bounded implementation contract | [Preference controller](../../apps/cantos-theatre/mobile/shared/src/commonMain/kotlin/com/cantos/theatre/application/preferences/PreferencesController.kt), its [native tests](../../apps/cantos-theatre/mobile/shared/src/commonTest/kotlin/com/cantos/theatre/application/preferences/PreferencesControllerTest.kt) and [repair ledger](../../docs/evidence/issue169-cmp-locale-acceptance.md) specify saved canonical locale over host seed/default, atomic refusal, retry and theme isolation. | `draft`; the model documents this slice and selects no library/catalog/formatting policy under #167. |
| Finite assumption | Two canonical UI locales, two themes and abstract typed native read/write results; reopening receives a supplied read result, not a proof that disk survived process death. | Named runs, reachability and controls cover the reducer/controller expectation only. No Kotlin observer is registered; native implementation/renderer tests, asynchronous storage, Android/iOS/runtime and accessibility acceptance stay separate. |

## Draft: `cantos.identity.theatre-registration` (#174)

| Classification | Authority / fact | Model consequence |
| --- | --- | --- |
| Approved narrow direction, not activation | [ADR 0034](../../docs/decisions/0034-theatre-signup-and-provisioned-studio.md#decision) accepts explicit email proof before new-person creation, positive exact acknowledgement before credential setup, no reset of existing accounts, no registration-issued session/Studio/Admin authority and stop/reconcile after uncertain effects. | `draft`; no policy defaults, live permissions, issuer restriction or deployment acceptance are promoted by simulation. |
| Implemented independently: SMTP | [Application](../../services/cantos-server/src/platform/theatre_registration/application.rs), [SQL fences](../../services/cantos-server/src/platform/theatre_registration/postgres.rs) and [current schema](../../services/cantos-server/schema/0001_current.sql) retain phase/effect history. | The model's create/setup/mail phases abstract this path. Named expectations are not a Rust observer or PostgreSQL/runtime conformance claim; SQL acceptance requires a disposable real cluster. |
| Implemented independently: default-disabled Firebase (#206) | [ADR0036](../../docs/decisions/0036-firebase-enrollment-proof.md#decision) accepts the code/test direction with one-shot setup continuation. The existing registration owner now includes the [Firebase application](../../services/cantos-server/src/platform/theatre_registration/firebase_enrollment/application.rs), [SQL fences](../../services/cantos-server/src/platform/theatre_registration/firebase_enrollment/postgres.rs) and [current schema](../../services/cantos-server/schema/0001_current.sql); [runtime evidence](../../docs/evidence/firebase-enrollment-runtime.md) separates local checks from unaccepted live activation. | This draft model does not represent the complete Firebase phase machine, dedicated UID/action binding or RAM-only handoff. Native tests own those implemented boundaries; there is no registered registration observer or Firebase runtime-conformance claim. The model's setup-mail phase does not establish Firebase setup delivery. |
| Abstract provider-status expectation | Historical provider status, including Firebase `emailVerified=true`, is not possession of the actual email action proof and grants no claim, provisioning, setup or session authority. | `observeProviderStatus` preserves the entire registration state before and after abstract proof consumption, completion or blocking. This expectation originated in the test-only prototype and remains narrower than the independently implemented runtime; simulation accepts no provider adapter or proof handoff. |
| Finite assumptions | One operation; four distinct proof epochs; expiry is an input fact; exact positive person acknowledgement and committed SQL fences are abstract facts. Rejected and uncertain effects both enter the non-retryable blocked class. | Native tests own parsing, quotas, race/clock/SQL semantics, transport, actual mailbox and issuer acknowledgement shape. No model action creates an account, sends mail, establishes credential completion or proves effective least privilege. |

## Cantos Bearer lifecycle (#253)

`cantos.identity.bearer-lifecycle` remains **draft**. Approved direction is owner-selected
rotation, bounded authority, durable native cleanup, single-flight refresh and late-result/account
isolation; concrete policy is in [Bearer v1](../../packages/contracts/bearer-v1.md). The API
projection models one family, generations 0–3, active/expired authority and one immutable
authentication timestamp. Independent Rust/API implementation mappings:

| Stable run | Executable production-code oracle |
| --- | --- |
| `run.refresh` | `core::tests::access_is_short_and_refresh_never_exceeds_initial_authority`; `all_surfaces_issue_rotate_revoke_with_exact_scope_and_stable_session` |
| `run.replay` | `core::tests::replay_revokes_and_logout_expiry_cannot_be_renewed`; `lost_response_and_simultaneous_refresh_revoke_every_generation` |
| `run.logout` | `all_surfaces_issue_rotate_revoke_with_exact_scope_and_stable_session`; `cancelled_or_lost_exchange_cannot_restore_and_does_not_cancel_another_account` |
| `run.expiry` | `expiry_disable_wrong_binding_provider_outage_and_recent_auth_fail_closed` |

Web-specific independent oracles: `cantos-web-auth` core/refresh tests cover exact expiry,
unknown refresh, single-flight and late-waiter/logout/account fences; the explicit `verify
web-bearer` fixture drives actual Web modules against mocked API/issuer HTTP. These are client
implementation evidence, not model trace conformance or real issuer/SQL proof.

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

The following stable runs map to actual [NativeAccountController tests](../../apps/cantos-theatre/mobile/shared/src/commonTest/kotlin/com/cantos/theatre/application/identity/NativeAccountControllerTest.kt).
Mappings express shared requirements; these tests do not consume model traces or prove model
conformance. Source presence does not report that any native test ran.

| Stable run | Property / reachable witness | Independent native implementation oracle |
| --- | --- | --- |
| `run.native-journal-before-refresh` | `property.native-journal-before-refresh` / `witness.native-refresh-dispatched` | `failed_rotation_journal_write_stops_before_spending_refresh` |
| `run.native-single-flight` | `property.native-single-flight` / `witness.native-joined` | `concurrent_waiters_spend_once_and_cancelled_waiter_does_not_cancel_rotation` |
| `run.native-unknown-refresh` | `property.native-unknown-cleanup-only` / `witness.native-unknown-stopped` | `unknown_refresh_revokes_old_family_and_never_replays` |
| `run.native-pair-write-failure` | `property.native-persist-before-admission` / `witness.native-pair-write-refused` | `failed_pair_replacement_revokes_family_and_never_adopts_new_access` |
| `run.native-interrupted-refresh` | `property.native-restart-needs-confirmation` / `witness.native-interrupted` | `interrupted_refresh_record_is_cleanup_only_after_restart` |
| `run.native-restore-confirmation` | `property.native-restart-needs-confirmation` / `witness.native-interrupted` plus explicit successful confirmation checkpoints | `restore_requires_exact_api_confirmation_before_private_authority`; `wrong_actor_family_or_deadline_cannot_restore_local_record` |
| `run.native-logout-late-result` | `property.native-late-result-fenced` / `witness.native-late-ignored` | `logout_hides_and_marks_synchronously_then_late_refresh_cannot_restore`; `cancelled_restore_late_A_cannot_replace_B` |
| `run.native-durable-cleanup` | `property.native-marker-before-barrier-clear` / `witness.native-durable-cleanup` | `cleanup_retries_exact_family_and_cleared_record_recovers_delete_failure`; `orphan_barrier_missing_or_corrupt_storage_never_starts_new_authorization` |

[Native journal tests](../../apps/cantos-theatre/mobile/shared/src/commonTest/kotlin/com/cantos/theatre/adapters/identity/NativeCredentialJournalTest.kt)
independently exercise full-pair/Flow/Cleared records, decoding, readback, API/issuer binding and
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

## Draft: `cantos.identity.native-mailbox-proof` (#334/#335, v1 signup)

One native Identity Theatre-signup mailbox proof, from issued challenge to the single commit that
consumes it and creates the account. Model: [native_mailbox_proof.qnt](identity/native_mailbox_proof.qnt).

| Classification | Authority / fact | Model consequence |
| --- | --- | --- |
| Approved requirement | [ADR 0045](../../docs/decisions/0045-v1-email-password-sign-in.md#decision) (v1 email/password with a mailed proof, no session from signup); [Identity RPC](../../packages/contracts/identity-rpc-v1.md#one-command-setup-decision) one-command `CompleteTheatreSignup`: the proof presenter chooses the password, consumption and account/credential creation are one storage transaction, receipts/polls/`REPLAYED` are history, not authority; [Identity auth v1](../../packages/contracts/identity-auth-v1.md#email-link-and-signup): the fragment link means opening/prefetch consumes nothing, completion issues no session. | Properties `explicit-only`, `single-account-effect` and `no-authority`. Lifecycle stays `draft`. |
| Approved storage contract | [Mailbox store v1](../../packages/contracts/identity-mailbox-store-v1.md#secrets-and-atomic-transitions): binding = operation + instance epoch + intent; the epoch is checked even before receipt replay; exact expiry second excluded; clock before issuance refuses; Consumed/Cancelled terminal; replacement needs a lifetime-unique locator/secret and retires the old one; a plan is applied only by comparing the complete expected snapshot; `Rejected` and `Unknown` commit faults stay distinct and an unknown outcome is reconciled with the identical command. | Properties `consumed-within-bound-window`, `terminal-sticky`, `retired-locator-rejected`, `plan-has-no-effect`, `commit-compares-snapshot`; runs `stale-plan`, `lost-response`, `lost-before-commit`. |
| Implemented independently of this model | Pure [`MailboxProof`](../../services/cantos-identity/src/domain/mailbox_proof.rs) `plan_confirmation`/`plan_cancellation`/`plan_replacement` returning a `#[must_use]` `ProofTransition`; the [proof store](../../services/cantos-identity/src/persistence/proofs.rs) (epoch → receipt replay → explicit action → binding → locator → secret digest → plan → apply → receipt) and [`complete_signup`](../../services/cantos-identity/src/persistence/signup.rs) composing confirmation with subject/email/credential/completion rows in one write transaction. | The model's rejection precedence follows that store order. The public RPC projects every denial to a generic `PERMISSION_DENIED`; precedence is not a wire contract. No Rust observer is registered: `--conformance` does not apply. |
| Finite assumption | One workflow (operation 1, epoch 1, Theatre signup); locators 1–3 as equality classes; a secret is the class of the locator it was minted for (0 = forged); time classes 0–5 with issue 1, TTL 2, cooldown 1, and backwards clock observations; `explicit` is the adapter-established action. Plan and commit are separate steps so an interleaved cancel/replace can make a plan stale; because the implementation computes the plan inside the committing transaction, `settle` re-evaluates the window at the commit instant. | No entropy, digest, constant-time, SQL isolation, concurrent writer, restart, mail, rate budget or transport claim. `UnknownApplied`/`UnknownLost` abstract a lost commit acknowledgement. |
| Gap / not modeled | (1) `CredentialRecovery` intents exist in the core and store, but recovery is deferred by ADR 0045 §3: recovery classes appear only as wrong-binding inputs. (2) Cancellation has no caller authorization or transport yet (store: "its future shell"); `cancel` is an owner-internal step. (3) Detecting a restored Identity file needs future epoch rotation ([security store](../../packages/contracts/identity-security-store-v1.md#startup-and-mutations)); the model covers only a presented epoch that differs from the store's. (4) `StartTheatreSignup` budgets, existing-address handling and `EmailTaken` remain native tests. | None of these is decided or promoted by simulation. |

Independent native oracles. These share requirements; they do not consume model traces, and listing them reports nothing about whether they ran:

| Stable run | Property / witness | Native implementation tests |
| --- | --- | --- |
| `run.explicit-confirmation` | `explicit-only`, `plan-has-no-effect`, `no-authority` / `inspected-pending`, `consumed-explicitly` | `mailbox_proof::tests::polling_and_prefetch_cannot_plan_consumption_in_any_state`; `proof_file_tests::wrong_secret_inspect_and_changed_binding_cannot_consume_or_commit_receipt`; `signup_file_tests::reading_and_polling_state_never_consumes_a_pending_proof` |
| `run.binding-and-window` | `consumed-within-bound-window` / `expired-at-commit` | `every_operation_rejects_a_stale_or_rebound_workflow`; `confirmation_includes_issuance_but_excludes_the_exact_expiry_second`; `clock_before_issue_and_exact_expiry_refuse_but_last_valid_second_consumes`; `changed_epoch_refuses_new_and_replayed_commands_before_writer_access` |
| `run.cancelled-terminal`, `run.consumed-terminal` | `terminal-sticky` / `cancelled-replace-refused` | `consumed_and_cancelled_challenges_are_terminal_for_every_mutator`; `cancellation_at_expiry_is_durable_and_terminal_for_new_commands`; `wrong_expired_cancelled_and_replaced_proofs_never_create_anything` |
| `run.replacement` | `retired-locator-rejected` / `retired-locator-refused` | `replacement_revokes_the_old_locator_and_keeps_exact_binding`; `replacement_checks_freshness_cooldown_clock_and_window_before_any_change`; `replacement_has_exact_cooldown_and_retires_old_locator_and_digest`; `expired_pending_can_replace_and_cannot_reuse_any_issued_material` |
| `run.stale-plan` | `commit-compares-snapshot` / `stale-commit-refused`, `plan-without-effect` | `the_plan_exposes_exact_typed_storage_facts_without_an_authority_value`; `confirmation_and_cancellation_race_has_one_terminal_winner`; `concurrent_completions_of_one_proof_have_exactly_one_winner` |
| `run.lost-response` | `single-account-effect` / `exact-retry-replayed` | `lost_confirmation_response_replays_historical_receipt_without_new_effect`; `a_response_lost_after_commit_reconciles_after_reopen_without_another_effect`; `a_replayed_command_with_changed_content_is_refused`; `identity_password_postgres::signup_completion_is_replay_safe_and_never_issues_a_session` |
| `run.lost-before-commit` | `single-account-effect` / `consumed-explicitly` | `receipt_failure_rolls_back_confirmation_and_replacement_issue_reservation`; `a_failure_in_the_final_write_rolls_back_the_consumed_proof_and_the_account` |

## Draft: `cantos.identity.native-session-admission` (#334/#335, v1 sessions)

Turning a native Identity `AuthenticationFact` into one Server session and re-admitting that
session against a fresh `GetSecurityState`. Model: [native_session_admission.qnt](identity/native_session_admission.qnt).

| Classification | Authority / fact | Model consequence |
| --- | --- | --- |
| Approved requirement | [Identity auth v1](../../packages/contracts/identity-auth-v1.md#freshness-and-revocation-quantified): a fact is used only if `0 ≤ now − authenticated_at ≤ 30 s` (5 s ahead tolerated), else `503`; a fresh read at every login, renewal and protected admission; admit only Active with equal `instance_epoch` and `security_version`; any difference (disabled, advanced, **lower**, other epoch, unknown subject) revokes; once that write commits it is never resurrected; an outage fails closed **without** revoking; logout stays local. [Identity RPC](../../packages/contracts/identity-rpc-v1.md#authentication-fact): a fact is not a session; operation/purpose binding is validated first. [ADR 0045](../../docs/decisions/0045-v1-email-password-sign-in.md#decision): the Studio roster is checked at login and every admission and grants nothing else. | Properties `issue-needs-fresh-bound-fact`, `admit-only-exact-active`, `mismatch-requests-revoke`, `revocation-sticky`, `outage-never-revokes`. Lifecycle stays `draft`. |
| Implemented independently of this model | Pure [`decide_issue`/`decide_admission`](../../services/cantos-server/src/platform/identity_auth/decision.rs) (precedence: subject, age, active, epoch, version; versions compare both ways); the [shell](../../services/cantos-server/src/platform/identity_auth/mod.rs) `login` (fact validation, roster, read, issue) and `confirm` (roster, read, `NOT_FOUND` → `subject-missing`, decision, then a best-effort [`revoke`](../../services/cantos-server/src/platform/identity_auth/postgres.rs) `UPDATE … AND NOT revoked` whose failure is ignored); `access` refuses a revoked family before Identity is asked. | `admit` stands for `confirm`, which protected admission, Web renewal and native refresh all call. `request` (decision) and `commitRevoke` (durable write) are separate steps: property `request-is-not-commit`. No Rust observer: `--conformance` does not apply. |
| Finite assumption | One registered subject (1; subject 2 is never present); epochs 1–2 and versions 0–3 as order classes; fact age as Fresh/Stale/Future classes (exact second boundaries are native-tested); Identity reachability as Answer/Unreachable/Malformed; one session slot whose generation increases on each login. | No HTTP status, cookie/Bearer, CSRF, 409 session fence, admission lease, 15 s bound, token hashing, rotation, SQL locking or RPC deadline claim. |
| Mismatch (reported, not modeled) | The contract lists an "other subject" read as a revoking difference. The implementation's `SecurityState::from_response` refuses a reply naming another subject as `MalformedState` → `503` with **no** revocation, so `RevokeReason::SubjectMismatch` is unreachable through the shell. | The model has no well-formed other-subject reply. Owner decision needed: revoke (contract) or protocol failure (code). |
| Needs decision (unconstrained) | (1) A revoke write that fails is "redone at the next admission" only if the next read still differs; if Identity matches again first (e.g. a restored file is put back), the implementation admits the unrevoked session. The contract does not decide that case. (2) Removing a subject from the Studio roster denies admission without revoking; whether re-adding it may re-admit the same session is not stated. | No property or run asserts either outcome. `run.failed-revoke-write` asserts only the redo while the difference persists; `run.studio-roster` asserts only the denial. |

| Stable run | Property / witness | Native implementation tests |
| --- | --- | --- |
| `run.issue`, `run.fact-binding` | `issue-needs-fresh-bound-fact`, `admit-only-exact-active` / `issued`, `admitted`, `stale-fact-refused` | `decision::tests::a_fact_becomes_observed_security_only_inside_the_exact_age_window`; `issue_refusals_have_a_stable_precedence_and_exact_variants`; `identity_password_facts_postgres::a_stale_future_or_misbound_fact_never_becomes_a_session`; `a_security_change_between_the_fact_and_the_issue_is_refused_and_cuts_no_session` |
| `run.version-advanced`, `run.version-regressed`, `run.security-changes` | `mismatch-requests-revoke`, `request-is-not-commit`, `revocation-sticky` / `epoch-change-requested`, `regression-requested`, `sticky-after-restore` | `admission_over_the_whole_finite_grid_matches_an_independent_oracle`; `every_security_change_revokes_durably_and_a_matching_snapshot_never_resurrects`; `an_unknown_subject_at_admission_is_a_durable_security_fact_not_an_outage`; `identity_password_postgres::a_subject_disabled_in_identity_loses_every_session_and_is_never_resurrected` |
| `run.outage` | `outage-never-revokes` / `outage-denied` | `a_slow_or_confused_identity_fails_closed_without_revoking_anything`; `identity_password_postgres::identity_outage_fails_protected_requests_closed_but_logout_stays_local_and_recovery_resumes` |
| `run.failed-revoke-write` | `request-is-not-commit` / `request-without-commit` | **None.** No native test injects a failed revoke write; this is a coverage gap, not evidence. |
| `run.studio-roster` | `admit-only-exact-active` | `identity_password_postgres::studio_login_needs_the_server_roster_and_a_theatre_session_never_enters_studio` |

### Legacy scope: `cantos.identity.bearer-lifecycle`

The two native capabilities above **do not replace** any Bearer lifecycle ID, and its meaning is
unchanged. Its consumers are live: [`bearer::core`](../../services/cantos-server/src/platform/bearer/core.rs)
(whose `rotate` the Identity-auth native refresh also reuses), `bearer_postgres` and the Theatre
Mobile `NativeAccountController`/`NativeCredentialJournal`, so it is not deprecated. Mapping:
rotation, replay, logout and expiry of a refresh family stay in `bearer-lifecycle`;
the security-state recheck that runs *before* rotation, and mailbox proof consumption, are the
new capabilities. Whether `bearer-lifecycle` runs also describe `ci1_` Identity-auth families
is a review decision this change does not make.
