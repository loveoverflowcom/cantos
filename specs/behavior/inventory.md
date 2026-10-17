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
| Implemented, independently of this model | [Pure lifecycle](../../services/cantos-api/src/contexts/media/domain/lifecycle.rs) `decide_admission` and `submission_state`; [application](../../services/cantos-api/src/contexts/media/lifecycle_application.rs) invokes them before inspection and again before settlement. [Existing native tests](../../services/cantos-api/src/contexts/media/lifecycle_tests.rs) cover digest/replay, lower clock bounds and foreign settled binding. | These are real PR B observation seams. PR A calls Quint, not these functions. Existing tests stay. |
| Implemented boundary, excluded here | `AudioSubmission::create/restore`, `AudioRendition::create/restore`, ownership, immutable full bindings, profile admission and same-receipt transaction arbitration. | The pilot starts with one valid prepared submission. It cannot certify constructors, authentication, foreign/corrupt database rows, actual bytes or concurrent commits. |
| Implemented external fact | [Brief freshness](../../services/cantos-api/src/contexts/production/domain/brief.rs) is recomputed from content, rights, latest guidance and archive status; existing [Work](../../services/cantos-api/src/contexts/catalog/domain/work.rs) and [Unit](../../services/cantos-api/src/contexts/catalog/domain/unit.rs) archive/restore can restore current authority for the same immutable binding. | `restoreCurrent` supplies that recovered fact; named admission/replay runs show it does not clear revocation or replace the settled receipt. No real archive/API/DB action runs. |
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
| Implemented, independently of this model | [Pure kernel](../../services/cantos-api/src/contexts/media/domain/playback_grant.rs) `grant_expiry`/`decide_media_fetch`; [application](../../services/cantos-api/src/contexts/media/listener_media.rs) admits before planning and again after object open; [authorization](../../services/cantos-api/src/contexts/media/listener_application.rs) re-resolves after sealing. Native example/property/tamper tests and the route mock integration stay authoritative for their boundaries. | `admit` and `issuedExpiry` mirror those seams; no Rust observer is registered (`--conformance` does not apply). |
| Model assumption | One immutable publication (identity 1); another unit's release is identity 2. Time classes 0–6 with publishedAt=1, rightsUntil=5, TTL=2, including rollback. `authentic` abstracts the HMAC result; tokens, keys, URLs and bytes are not modeled. Eligibility loss (withdraw/revoke/supersede/owner disable) is one irreversible fact. | No hash, Range, header, SQL isolation, restart or concurrency claim; HTTP transport is native-tested. |
| Model assumption | `Opening` is shell history between admission and the post-open recheck; `completeFetch` abstracts verified object bytes. | Covers withdrawal/expiry while the object opens; not lane bounds or body streaming. |

## Draft: `cantos.media.public-cover-projection` (#171, proposal for #157)

| Classification | Authority / fact | Model consequence |
| --- | --- | --- |
| Proposed, not accepted | [ADR 0033](../../docs/decisions/0033-release-pinned-public-cover.md#decision) recommends the release-pinned cover plus an explicit per-image disclosure; #157 has not accepted it. | `draft`; no wire field or delivery follows from a PASS. |
| Implemented, independently of this model | [Pure kernel](../../services/cantos-api/src/contexts/media/domain/public_cover.rs) `project_public_cover`/`cover_outcome` over existing [artwork history](../../services/cantos-api/src/contexts/media/domain/artwork.rs) `artwork_at`. No production shell supplies a disclosure; it is always `Undecided`. | `effectiveAt`, `disclosure` and `coverView` mirror those seams; no observer is registered. |
| Model assumption | Two synthetic images; asset binding is exact (binding/corruption partitions are native tests). Release eligibility is one flag; a publish pins the current head revision. Time classes 0–6. | No SQL, storage, wire, cache or client claim. |

## Deferred capabilities

| Stable capability ID | Approved / implemented | Assumed / needs-decision | Why deferred |
| --- | --- | --- | --- |
| `cantos.publishing.lifecycle` | [Publication v1](../../packages/contracts/publication-v1.md) and [domain](../../services/cantos-api/src/contexts/media/domain/publication.rs) implement readiness, clearance/approval, immutable releases, active pointers and withdrawal. | Real public delivery/provider acceptance remains separate; choose an exact candidate/pointer/observation scope before making a model authoritative. | Too many independent gate and delivery dimensions for this foundation pilot. No `.qnt` or conformance claim. |
| `cantos.listening.progress` | [Listener Library v1](../../packages/contracts/listener-library-v1.md) and [reconcile core](../../services/cantos-api/src/contexts/listening/domain/reconcile.rs) implement exact-publication conditional updates, retained history and stale conflict classification. | Cross-client offline/native lifecycle and complete replay scope need review; UI mocks do not accept those semantics. | Keep the current kernel/native tests; establish publication/media access acceptance before cross-stack scenarios. |

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
| Implemented independently: SMTP | [Application](../../services/cantos-api/src/platform/theatre_registration/application.rs), [SQL fences](../../services/cantos-api/src/platform/theatre_registration/postgres.rs) and [current schema](../../services/cantos-api/schema/0001_current.sql) retain phase/effect history. | The model's create/setup/mail phases abstract this path. Named expectations are not a Rust observer or PostgreSQL/runtime conformance claim; SQL acceptance requires a disposable real cluster. |
| Implemented independently: default-disabled Firebase (#206) | [ADR0036](../../docs/decisions/0036-firebase-enrollment-proof.md#decision) accepts the code/test direction with one-shot setup continuation. The existing registration owner now includes the [Firebase application](../../services/cantos-api/src/platform/theatre_registration/firebase_enrollment/application.rs), [SQL fences](../../services/cantos-api/src/platform/theatre_registration/firebase_enrollment/postgres.rs) and [current schema](../../services/cantos-api/schema/0001_current.sql); [runtime evidence](../../docs/evidence/firebase-enrollment-runtime.md) separates local checks from unaccepted live activation. | This draft model does not represent the complete Firebase phase machine, dedicated UID/action binding or RAM-only handoff. Native tests own those implemented boundaries; there is no registered registration observer or Firebase runtime-conformance claim. The model's setup-mail phase does not establish Firebase setup delivery. |
| Abstract provider-status expectation | Historical provider status, including Firebase `emailVerified=true`, is not possession of the actual email action proof and grants no claim, provisioning, setup or session authority. | `observeProviderStatus` preserves the entire registration state before and after abstract proof consumption, completion or blocking. This expectation originated in the test-only prototype and remains narrower than the independently implemented runtime; simulation accepts no provider adapter or proof handoff. |
| Finite assumptions | One operation; four distinct proof epochs; expiry is an input fact; exact positive person acknowledgement and committed SQL fences are abstract facts. Rejected and uncertain effects both enter the non-retryable blocked class. | Native tests own parsing, quotas, race/clock/SQL semantics, transport, actual mailbox and issuer acknowledgement shape. No model action creates an account, sends mail, establishes credential completion or proves effective least privilege. |
