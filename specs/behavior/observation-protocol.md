# Observation protocols v1

This document owns two registered protocols: the private audio admission pilot (this first
part) and the [native Identity auth protocol](#native-identity-auth-observation-protocol-v1) below.
Both are test protocols for pure-core differential observation, never an API or a production dependency.

## Private admission observation protocol v1

Capability: `cantos.media.private-audio-admission`; schema:
`cantos-private-audio-admission-observation-v1`. Lifecycle remains **draft**. The
[model](media/private_audio_admission.qnt) and [catalog](catalog.json) own expectations/IDs;
[production lifecycle](../../services/cantos-server/src/contexts/media/domain/lifecycle.rs)
owns actual decisions. This is a test protocol, not an API, replacement DSL or production dependency.

## Command and boundaries

`cargo xtask verify behavior --conformance [--capability cantos.media.private-audio-admission]`
first executes all existing model stages. It then consumes that exact bundle, decoding the
pinned Quint 0.33.0 ITF representation. Only replay events and stable IDs enter the ignored
Rust unit-test observer; expected states/outcomes never enter it. No handwritten reducer
computes expected values. The Rust observer calls `decide_admission` at Begin and Complete,
`submission_state` at every checkpoint, and real typed submission/rendition constructors.

The observer retains its own facts/pending requests, counts actual inspection decisions and
successful rendition constructions, and reports full actual immutable documents. Receipt
aliases are projected from the constructed document, never copied from expected checkpoints.
No production function, raw/proof constructor, dependency or contract field changes.

## Exact mapping

| Model input | Real typed fixture |
| --- | --- |
| Digest alias 0 / 1 | 64 lowercase `a` / `b` SHA-256 characters |
| Length alias 0 / 1 | 100 / 101 bytes (separate from digest dimension) |
| Clock class n, 0–4 | 2000 + 1000 × n milliseconds |
| createdAt 1 / expiresAt 3 | Real `[3000, 5000)` submission window |
| Settled ID 1 | Fixed synthetic rendition UUID from the native lifecycle fixture |
| Complete(true) media fact | Synthetic valid mono Opus metadata, 48 kHz, 24 kbps; no inspected bytes |

The strictly increasing time map preserves order/equality, inclusive creation/exclusive expiry,
admitted-at checks and rollback observations. Payload maps preserve equality classes only,
not real hash/size/profile completeness. Binding/rights/owner and synthetic Brief are admitted
by existing typed constructors. Fixture changes outside these mappings fail closed.

## Checkpoints, effects and comparison

Decode every `state`, `before`, `last`, `outcome` checkpoint with contiguous exact indices and
previous-state linkage. Nullary sums carry empty `#tup`; numeric values carry canonical decimal
`#bigint`; Complete carries Boolean; Rejected nests a closed error variant. Reject unknown
fields/actions/variants and malformed or out-of-range input facts.

Assertion is `Assertion`/`NoOutcome` and never calls a decision or constructs another rendition.
Initial NoEvent occurs once. RestoreCurrent changes only recovered current authority over the
same binding, never revocation or history. Compare exact state/outcome/error, pending request,
owner view, receipt identity/admitted time/payload, effect counts and cumulative real decision
calls. Retain full submission/rendition JSON and require immutability across all checkpoints.

Complete(false) still calls the real decision but deliberately reports the model's MediaRejected
harness result. Its raw decision result has no independent oracle in this ITF checkpoint because
Quint masks it. Abandon and external fact updates are also harness effects. Neither is a claim
about actual inspector, storage, cancellation or application reauthorization order. Owner
Admitted means history, never producer permission, publication or listener access.

## Positive selection and sensitivity

Replay all 19 catalog named runs, all 11 original-model reachability witnesses and the one
retained sampled-property trace as **positive** expectations. Witness status `violation` means
it reached a witness by violating its negation, not wrong behavior. The other property samples
have model evidence but only the retained sample is replayed against Rust, not all 500 samples.

Seven deliberately mutated-model counterexamples are **sensitivity-only**, never correct
expected behavior. Decode structurally valid mutant counts/IDs even when they violate original
invariants. Replay growing prefixes through the first semantic state/outcome mismatch against
the correct core. Stop that control there: later mutant actions may be illegal after a correct
rejection. A parser/process/guard failure never kills a control. Record exact mismatch index,
expected mutant checkpoint and actual observation.

Separately replay the original revoke-during-inspection and revoked-replay named scenarios
with a deliberately faulty observer passing revoked=false to the actual admission seam.
Both must semantically mismatch. This detects a concrete authority-fact adapter defect,
not every possible implementation defect or all production mutations.

## Stable scenario coverage

These suffixes belong to the catalog capability prefix above; selection remains catalog-owned,
not this prose table. Every listed run is replayed with its full raw checkpoint sequence.

| Catalog run suffixes | Independently compared observations |
| --- | --- |
| `run.admit`, `run.lost-response-replay` | Actual Inspect/Admitted/Replay; one construction, immutable full documents and no assertion effect |
| `run.wrong-digest`, `run.wrong-length`, `run.contradictory-replay` | Independent digest/length aliases and exact DigestMismatch versus OperationReused, unchanged receipt/effects |
| `run.media-rejection-retry`, `run.abandon-retry` | Pending harness attempt cleared, no rejected settlement, later actual inspection/construction; failed media remains synthetic |
| `run.revoke-during-inspection`, `run.invalidate-during-inspection`, `run.expire-during-inspection` | Actual current decision on completion, exact authority error, no rendition; observer-fault control specifically covers revocation |
| `run.expired-owner-history`, `run.revoked-replay` | Historical receipt retained; owner projection distinct from denied producer replay |
| `run.stale-revocation`, `run.error-precedence` | Exact error precedence and independent owner-view precedence under combined facts |
| `run.before-creation`, `run.before-admission-replay` | Lower creation/admission-time errors with strict increasing alias mapping, including rollback |
| `run.restored-authority-admit`, `run.restored-authority-replay`, `run.revocation-survives-restoration` | Same binding regains current authority, no revoked grant reset, exact admission/replay and unchanged documents |

Original-model witnesses provide additional generated paths for the registered antecedents.
Native wrong-settled-binding, arbitrary numeric/hash/constructor and explicit transition-matrix
regressions remain independently owned; do not fabricate missing model state to label them
conformance. Later API/Leptos/CMP adapters may reuse these IDs only where observations have
these same meanings, retaining their own transport/interaction/device evidence.

## Freshness and evidence

Model `report.json` remains independent and model-only. `conformance.json` records its exact
hash, source revision, model/tool/catalog/config/input identities, observer/comparator/fixture/
compiler-config input hashes, full source snapshots, commands, logs, observations and first
mismatches. All output stays ignored under `target/verification/behavior/`; no bulk/private
content or media is committed. Retained input bytes permit reconstruction of dirty inputs.

Identity, original traces, mutant sensitivity, observer sensitivity and endpoint freshness must
all PASS. Empty/missing/duplicate/reordered traces, zero tests, partial observation, wrong schema
or stale retained/source/tool/config hashes fail closed. Cargo is locked/offline; each owned
observer process has a 180-second bound. Unexpected compiler overrides/ambient replay settings
are refused, including legacy/TOML ancestor/global Cargo configuration, configured tool
selection and target runner/linker overrides. Config absence is also pinned. Exact Rust/Cargo
1.98.1, launchers, toolchain binaries and compiler driver hashes are recorded and rechecked;
Cargo invokes the pinned compiler directly. Sources/HEAD/tools are checked again at completion. Endpoint checks do not isolate
against concurrent mutate-and-restore writes or hostile replacement of both tool and receipt.

## Assurance limits and next boundary

Evidence is bounded sequential `differentially-tested` pure core with synthetic shell effects,
plus explicit sensitivity defects. It is not exhaustive refinement, solver/liveness proof,
production application orchestration, authentication, SQL concurrency/restart, actual bytes,
object storage/provider, public delivery or API/Leptos/CMP/device evidence. Preserve native
constructor, foreign settled binding, clock/numeric/hash and explicit state/event-matrix tests.
Those outside-model claims retain their owners and gates. [Issue 106](https://github.com/loveoverflowcom/cantos/issues/106)
remains open for model review and later shared scenarios. Do not adapt the model to code drift
without reviewed requirement evidence.

## Native Identity auth observation protocol v1

Capabilities `cantos.identity.native-mailbox-proof` (schema
`cantos-native-mailbox-proof-observation-v1`) and `cantos.identity.native-session-admission`
(schema `cantos-native-session-admission-observation-v1`), #334/#335 observed under #106. Both
lifecycles remain **draft**; the catalog and `.qnt` models own IDs and expectations and the
[inventory rows](inventory.md#draft-cantosidentitynative-mailbox-proof-334335-v1-signup) keep
their reported mismatches and needs-decision gaps. Scope is **`pure-core-synthetic-shell`**:
real production decisions plus a small, disclosed synthetic shell. It is not DB, RPC, HTTP,
cookie, mail or end-to-end acceptance, and it does not promote either model.

### Command and boundaries

`cargo xtask verify behavior --conformance --capability ID` runs every model stage, then replays the
exact bundle through the dispatcher's finite allowlist (`adapters()`): one catalog capability, one
observation schema, one exact ignored Rust test, one declared codec and one comparator. A capability
missing from either the catalog or the allowlist fails closed; without `--capability` only catalog-registered
observers are replayed and the other models stay model-only. Process bounds, provenance hashes and the
independent `report.json`/`conformance.json` split are shared with the audio protocol; the audio
observer and codec are unchanged.

| Capability | Test-only observer (`#[cfg(test)]`, `#[ignore]` replay entry) | Exact test run with `cargo test -p PACKAGE --lib --locked --offline … --ignored --exact` |
| --- | --- | --- |
| mailbox proof | [`quint_conformance_tests.rs`](../../services/cantos-identity/src/quint_conformance_tests.rs) in `cantos-identity` (default features, no database) | `quint_conformance_tests::replay_quint_native_mailbox_proof` |
| session admission | [`decision/quint_conformance_tests.rs`](../../services/cantos-server/src/platform/identity_auth/decision/quint_conformance_tests.rs) in `cantos-server` | `platform::identity_auth::decision::quint_conformance_tests::replay_quint_native_session_admission` |

The observers consume only stable trace IDs and replay events plus input facts (clock, Identity record,
roster, fate). Expected states, outcomes and checkpoints never enter them; the declared codec in
[`verify_behavior_conformance_native.py`](../../scripts/verify_behavior_conformance_native.py) only decodes the
pinned ITF representation and evaluates no transition. In Rust, every production decision comes from the real
symbols below; the only rules the observers evaluate themselves are the disclosed synthetic-shell rules, which
restate store/session behavior that has no pure production symbol and are labelled `shell` wherever they decide. Serde and `serde_json` are
`[dev-dependencies]` of `cantos-identity`; no release build links an observer, and a registry test checks the
`#[cfg(test)]` declarations and the dependency tables.

### Production seams and the disclosed synthetic shell

Every observed checkpoint carries `decidedBy` (`core`, `shell`, `core+shell`, `harness`, `none`) and cumulative
production call counters. The label follows the counters: `shell` means no production seam ran in that
checkpoint, `core`/`core+shell` mean one did, and an assertion or input fact must call none. The comparator
rejects a label that disagrees with the counters and a report that never exercised a registered seam.

| Capability | Real production symbols called | Synthetic shell (no production symbol decides it) |
| --- | --- | --- |
| mailbox proof | `ProofId::from_bytes`, `ProofTime::new`, `ProofBinding::new`, `MailboxProof::{issue, plan_confirmation, plan_cancellation, plan_replacement}`, `ProofTransition::{expected, next}` | store epoch check, receipt replay (`Replayed`/`OperationReused`), unknown-operation lookup, secret-class (digest) check, lifetime-unique locator set (`LocatorNotUnique`), the held plan, compare-and-replace against the current snapshot (`StaleSnapshot`), commit fate (`Commit`, `Reject`, `UnknownApplied`, `UnknownLost`), the account counter and the constant no-authority projection |
| session admission | `validate_fact` (Server Identity client `platform::identity_auth::rpc`), `SecurityState::from_response`, `decide_issue`, `decide_admission` | Studio roster membership, the single session slot and generation, an unreachable or absent Identity read, the requested-versus-committed revocation write and logout, the Identity record and roster as input facts |

The synthetic commit is a value replacement in memory. It is not a Turso or PostgreSQL transaction and it
never claims a durable receipt, a session, an authentication fact or a Studio/Admin authority: the core
returns plans only, so the model's `session`/`studio`/`admin` flags are compared as harness constants. The
commit-time window recheck calls the real planner again at the commit instant, as the store does inside its
write transaction; an assertion checkpoint never runs a decision or applies an effect a second time.
Recovery intents (`TheatreRecovery`, `StudioRecovery`) are only wrong-binding inputs, and the observed proof
binding must equal the fixture binding at every checkpoint.

### Exact mapping

| Model input | Real typed fixture |
| --- | --- |
| mailbox locator / command / operation / epoch n | nonzero 128-bit big-endian value n |
| mailbox clock class n | Unix second 1000 + n (order, equality, one-second cooldown and TTL 2 preserved) |
| mailbox recovery classes | `CredentialRecovery` with subject 1, version 1 and purpose Theatre/Studio |
| admission subject / epoch n | sixteen bytes of n; version n is n; operation 1 echoed, 2 not echoed |
| admission fact age Fresh / Stale / Future | 1 s old / one microsecond older than 30 s / one microsecond past the 5 s skew (exact boundaries stay native-tested) |
| admission Identity `Malformed` | a `GetSecurityStateResponse` with security version 0, refused by the real decoder |
| admission `Unreachable` / absent subject | shell facts: no reply / `NOT_FOUND` |

### Selection, sensitivity and observer faults

Positive expectations are the 8 named runs, 8 reachability witnesses and the one retained sampled-property trace of
each capability (17 traces); witness `violation` means it reached its witness, not wrong behavior. Zero selected
traces, a missing/skipped/duplicated scenario, zero or skipped observer tests, a wrong schema or capability, an unordered or
partial observation, and any `decidedBy` label that disagrees with the call counters fail closed. Parse errors, process
crashes and illegal later continuations never count as a kill.

The 9 mailbox and 7 admission deliberately wrong model controls are replayed as growing prefixes through the first
semantic state/outcome mismatch, recorded with the event, the observed origin and the differing fields. A control counts
only when that divergence is the removed rule firing where it is registered: the adapter registers, per control, the event and
observed outcome (and, where the model leaves the outcome unchanged, the state field) it must produce, and its detector.
A **core** kill additionally requires the first divergence at a `core` checkpoint with a differing outcome, so a production
seam made the distinguishing decision. A **shell** or **harness** kill only shows that a disclosed synthetic-shell rule or a
harness constant disagrees with the wrong model: it is a check of the shell and the comparator, never production evidence.
`conformance.json` counts them apart (`mutantKillsByDetector`): mailbox 5 core, 3 shell, 1 harness; admission 4 core, 3 shell.

| Capability | Control (property) → detecting observation [detector] |
| --- | --- |
| mailbox | `explicit-only` → `Prepare`, `Rejected(NotExplicit)` [core]; `consumed-within-bound-window` → `Settle`, `Rejected(Expired)` from the commit-instant planner [core]; `terminal-sticky` → `Replace`, `Rejected(AlreadyCancelled)` [core]; `retired-locator-rejected` → `Prepare`, `Rejected(ProofChanged)` [core]; `plan-has-no-effect` → `Prepare` plan, only `state.durable` differs because the shell holds the plan [shell]; `commit-compares-snapshot` → `Settle`, `Rejected(StaleSnapshot)` [shell]; `single-account-effect` → `Prepare`, `Rejected(AlreadyConsumed)` [core]; `no-authority` → `Settle`, only `state.session` differs [harness constant]; `only-committed-effects` → `Settle(UnknownLost)`, only `state.durable` differs because the shell applies the fate [shell] |
| admission | `issue-needs-fresh-bound-fact` → `Login`, `Rejected(FactNotFresh)` [core]; `admit-only-exact-active` → `Admit`, `Rejected(SecurityChanged)` [core]; `mismatch-requests-revoke` → `Admit`, outcome/`state.pending` differ [core]; `request-is-not-commit` → `Admit`, only `state.session.revoked` differs because the shell keeps the slot until a committed write [shell]; `revocation-sticky` → `Admit`, `Rejected(SessionRevoked)` [shell]; `outage-never-revokes` → `Admit`, `Rejected(IdentityUnavailable)` from the real decoder refusing a malformed read [core]; `session-changes-only-by-transition` → `CommitRevoke`, only `state.session.revoked` differs [shell] |

Each capability also replays original traces with a deliberately faulty observer through an environment-selected defect
(`CANTOS_QUINT_OBSERVER_FAULT`, refused when ambient) and requires every named scenario to mismatch.

| Capability | Fault (adapter drift, not a production defect) | Original runs that must detect it |
| --- | --- | --- |
| mailbox | `inspect-as-explicit`: a client `explicit=false` flag is presented to the core as an explicit action | `run.explicit-confirmation` |
| mailbox | `skip-commit-recheck`: commit applies the held plan without the real planner at the commit instant | `run.binding-and-window` |
| admission | `fresh-for-every-age`: every fact is stamped fresh | `run.fact-binding` |
| admission | `admit-against-current`: the admission decision compares the current read with itself | `run.version-advanced`, `run.version-regressed`, `run.security-changes` |

### Freshness, evidence and limits

`conformance.json` records the model report hash, source revision, tool/compiler/configuration identities, hashes of every
implementation input (the observed crates, the codec, its tests and the lock), the registered production symbols, the
synthetic shell elements, per-checkpoint origin and production-call totals, mutant kills by detector, every mismatch and every command with input,
output and log hashes. It stays independent of the model `report.json`; all output is ignored under
`target/verification/behavior/`.

Evidence is bounded sequential `differentially-tested` pure-core observation with a synthetic shell, plus explicit
sensitivity defects. The commit-snapshot, unknown-fate, revocation-commit and logout properties are checked only against
the shell's restatement here; their production evidence remains the native store and Server tests. It is not exhaustive refinement, a solver or liveness proof, a durable store, SQL isolation or
concurrency, restart, entropy, digest or constant-time comparison, mail, RPC, HTTP/CSRF/cookie transport, a real
Identity read, the Studio roster source, or any front-end/device claim; those keep their native tests and gates. The
mailbox planners' Kani kernel is separate evidence and is not implied by this replay; the Server session decisions have
no kernel under the current lane ([reconciliation](../../docs/assurance/kani.md#reconciliation-with-the-native-auth-quint-observers-106)).
