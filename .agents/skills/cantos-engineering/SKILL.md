---
name: cantos-engineering
description: >-
  Engineering foundation for Cantos (Rust, Axum, Leptos, Compose Multiplatform).
  Invariant-first workflow, decoupled module boundaries, immutable values and revisions, types
  as proof barriers, functional core with imperative shell, clean readable syntax,
  layered verification and progressive delivery, persistence and measured Turso read optimization,
  native Turso/identity refactoring with legacy PostgreSQL boundaries, structured runtime events,
  one evidence vocabulary and a mandatory residual-risk report. Use for any domain, backend,
  persistence, contract, test, refactoring or verification change.
---

# Cantos engineering

This skill is a **behavioral contract and router**. Load relevant references one at a time.
Every Cantos skill composes its order, vocabulary and report without redefining them.

## Ground rules

- **Read before editing.** [`AGENTS.md`](../../../AGENTS.md), the owning product/architecture doc,
  work item and touched code. Product rules live in [`docs/`](../../../docs/README.md); link, never duplicate them.
- **Repository reality.** Script IR, editorial/catalog/content/brief and private audio cores,
  native Server/Identity foundation and Leptos Studio exist. Business SQL/schema porting is
  incomplete; opening embedded Turso is not business HTTP acceptance. Read the current
  [platform implementation map](../../../docs/architecture/repository-layout.md#platform-scope)
  and actual source for Theatre Web/CMP owners and their separate runtime acceptance; a preview never supplies it.
  API TTS/workers were removed under [ADR 0018](../../../docs/decisions/0018-external-audio-production.md).
  Inspect commands and runtime capabilities; see [local execution](references/local-execution.md#runtime-portability).
- **Platform scope.** Follow the [platform matrix](../../../docs/architecture/repository-layout.md#platform-scope):
  Studio is web-only. Exclude Studio Mobile from implementation, scaffolding and build/test/preview automation unless explicitly requested in a future roadmap; retain Theatre Web/Mobile and shared libraries.
- **Behavior SSOT impact.** When a task can change a state, guard, outcome, permission, lifecycle,
  retry/recovery step or its oracle, even with no `.qnt` in the diff, run the
  [impact procedure](../cantos-behavior/SKILL.md#ssot-impact-procedure) before choosing a write and
  report it in § 9; presentation-only work records the reason. Keep model and implementation evidence separate.
- **Distinguish three states** in code, docs and reports: *proposed* design, *implemented*
  behavior and *verified* behavior with a named evidence level. An owner-selected target is not a runtime result.
- **Keep accepted boundaries.** Rust/Axum, Leptos Web, CMP native playback and S3-compatible
  storage remain. The [native Turso decision](../../../docs/architecture/native-turso-refactor.md)
  explicitly authorizes epic #331's Server/Identity split and replacement of PostgreSQL/Kanidm/container
  dependencies. Older stack descriptions remain the current/legacy baseline, not a veto on that
  selected target or a claim the replacement already exists. Outside the scoped refactor, follow
  the actual accepted stack. A new crate, service, trait, dependency or framework still needs a
  concrete consumer and stated reason; use [service-runtime](references/service-runtime.md).

### Native identity and embedded-database refactoring

For #331 read the owning [architecture](../../../docs/architecture/native-turso-refactor.md),
then [persistence](references/persistence.md), [service runtime](references/service-runtime.md),
[identity/session boundaries](references/identity-and-session-boundaries.md) and
[Turso read performance](references/turso-read-performance.md) as each boundary is touched.
Read only the boundary being changed. The [v1 owner amendment](../../../docs/architecture/native-turso-refactor.md#owner-amendment-v1-authentication-and-parallel-integration)
owns factor scope, transitional integration and security-state choices. These references qualify
the pinned engine, file ownership, private RPC, revocation and measurable resource claims.
Do not infer Turso behavior from SQLite, assume a ready-made DBSP connector, or start a container
for native acceptance. Preserve legacy data and source-bound evidence; production effects still
require explicit authority. Comprehensive active-doc/command migration belongs with implementation.

Selected high-risk pure authentication/persistence kernels require targeted Kani work through
[the existing formal method](references/mutation-and-formal.md#bounded-model-checking).
That does not make crypto, database I/O, RPC or concurrency formally verified; their actual
integration/fault evidence remains separate. Do not add a parallel skill, verifier or session engine.

Image boundary work follows the [runtime image policy](../../../docs/design/runtime-images.md) and its Media transition map.
Inventory bytes/consumers first; preserve immutable receipts and use new assets for conversion.
A legacy positive fixture is migration debt, never evidence that the new format is admitted.

## The required order

Follow **Layered Verification + Progressive Delivery**: verify the pure core before the I/O shell.

| Order | Obligation |
|---|---|
| L1 — Abstraction & Architecture | name the invariant/failure mode, module owner and inward dependency direction; review DDD boundaries, SOLID and justified ports |
| L2 — Domain, Types & Error Handling | immutable values, fallible constructors, closed outcomes, pure decisions and atomic rejection |
| L3 — Unit & Property Verification | exact semantic regressions and applicable laws, focused tests, format, lint and static checks without infrastructure |
| L4 — Application & I/O Adapters | only after L1–L3 pass: use cases, ports, effects and typed error mappings exercised with deterministic fakes |
| L5 — DB, API & Integration | real constraints, transactions, HTTP/auth contracts, storage/platform integration and scoped E2E/runtime acceptance |

The [verification strategy](references/verification-strategy.md#layered-verification) owns the
layer obligations and independent gate ledger. State inapplicable steps; adapter-only work with
no domain rule may proceed directly to L4, with a reason. For a bug, reproduce failure first.
The [local handoff policy](references/local-execution.md#local-handoff-and-pr-completion) owns
commit/push/PR eligibility: deliver independently verified layers when requested, explicitly
defer infrastructure acceptance and report exact evidence and residual risk.

### Database-backed GET, read and fetch work

Before changing a database-backed read/query or a write feeding a read model, load
[Turso SQL/index/cache measurement](references/turso-read-performance.md).
It owns inventory, actual authoritative-engine baseline, SQL/index comparison, selective cache,
differential/failure checks, benchmark record and acceptance. Turso is the selected #331 target;
PostgreSQL remains an explicit legacy baseline until migration acceptance. Incremental engines remain DEFER under #384; prioritize measured hot paths.
Mutations retain atomicity and admission/revocation rechecks. DBSP code/feature/gate are retired;
[ADR 0017](../../../docs/decisions/0017-experimental-dbsp-job-summary.md) preserves historical reasoning.

## 1. Name the invariant before writing code

State the altered observable invariant and failure mode in one sentence; keep a working ledger:

| Claim / invariant | Failure mode | Cheapest oracle | Evidence level |
|---|---|---|---|

Use the current [business rules](../../../docs/product/business-rules.md#product-invariants-to-verify)
with ADR 0018: exact immutable content/brief bindings, scoped/revocable producer access,
duplicate upload/lost-finalize reconciliation, actual-byte integrity, stale approval blocking,
private incomplete uploads, playback without production and revision-aware offline progress.
Provider attempts/regeneration belong to external producers, never a new backend worker.

## 2. Decouple by ownership

```text
pure domain values and rules   ←  application (permissions, transitions, gates)
        ↑                              ↑
   adapters (database, private storage, read-only inspection)   ←  HTTP/RPC handlers, UI
```

Dependencies point toward pure domain values. Script IR and domain rules never import Axum,
Leptos, Compose, SQL clients, provider SDKs or Narrative Forge internals. Web and CMP share
contracts and design semantics, never a UI runtime or Rust memory layout. A port exists only at a
real boundary with a real second implementation (production adapter plus test fake counts).

→ [`decoupling.md`](references/decoupling.md)

## 3. Prefer prevention: types and immutability

Before testing that a state cannot happen, ask whether a type can refuse to represent it: private
newtypes with fallible constructors, enums instead of flag/`Option` combinations, evidence values
instead of booleans (`FrozenProductionInputs`, `ApprovedRelease`), `#[serde(try_from = …)]` so the
wire path cannot forge what the constructor rejects. A compiler claim names a misuse that fails
for that reason; `must_use`, tokens, `Drop` and lifetimes prove no durable effect ([limits](references/types-as-proofs.md#what-a-type-does-not-prove)).

Settled facts are immutable. An accepted script revision, frozen production inputs, a provider
attempt, an accepted artifact, a QC result, an approval and a release manifest are written once;
a correction creates a new revision, artifact or release and preserves history. External producer
attempt history belongs to its producer. In code, transformations take values and return new
values; mutation stays local, private and invisible until the function returns.

→ [`types-as-proofs.md`](references/types-as-proofs.md),
[`immutability.md`](references/immutability.md),
[`boundary-hardening.md`](references/boundary-hardening.md) when a value crosses serde, SQL rows,
fixtures, AI output or a migration.

Load [typed errors and composition](references/typed-errors-and-composition.md) for fallible flow and containment at concrete outer boundaries.

## 4. Functional core, imperative shell

```text
world: HTTP/RPC · database · private storage · read-only audio tools · clock · IDs · randomness
                         ↓
                  imperative shell        resolves facts, executes effects, retries
                         ↓
                 typed plain facts
                         ↓
             pure deterministic core     validate · decide · plan · reduce · fingerprint
                         ↓
              decision / effects as data
```

Extract durable decisions from handlers, worker loops, adapters and components before mocking.
Core decisions include script validation, brief binding, intake/replay classification, publication
gates and listener progress reconciliation. Each runs from a plain unit test without an async
runtime, database or provider. Kotlin uses the same method;
`reducePlayback` is the current pure-core example. Do not force this on thin CRUD or an adapter
that only maps types. Logging is a shell effect too: the core returns typed outcomes and the
shell emits [structured events](references/structured-observability.md) from them.
Purity follows responsibility, not folders; local `mut` is not impurity; transitions keep rejection, no-op and effects distinct; consumers emit typed intents, never open mutable callbacks.
→ [`functional-core.md`](references/functional-core.md) ([transitions](references/functional-core.md#transition-semantics), [shell order](references/functional-core.md#the-imperative-shell-executes-in-a-fixed-order))

## 5. Every behavior change gets the cheapest deterministic regression evidence

1. Identify the observable behavior or invariant.
2. Add or update the cheapest deterministic test that would fail if it regressed.
3. Assert the exact error variant or the exact resulting state — never only `is_err()`.
4. Pick the smallest method that answers the claim's question ([by question](references/verification-strategy.md#choose-the-method-by-the-question));
   a test of the decision alone does not show the entrypoint reaches it ([reach](references/verification-strategy.md#production-reach-entrypoint-core-and-effect)).

```text
types / exhaustive match
→ focused example / table test with exact errors
→ bounded exhaustive loop over a finite domain
→ property or state-machine test
→ replay / differential / golden-fixture oracle
→ fault injection at named crash points (jobs, uploads, publication)
→ integration against the real selected database / object storage / provider
→ fuzzing of untrusted-input parsers (a different oracle: crash, hang, blow-up)
→ mutation testing (orthogonal: do the assertions kill defects?)
→ bounded model checking for a small critical kernel
```

Exemptions: formatting, comments, pure renames, metadata-only edits, CSS-only polish covered by
inspected visuals, or an existing test that already fails for this regression — **cite it**.
"Too small", "manually checked" and "it compiles" are not exemptions.

Name tests like theorems (`edit_of_one_dialogue_invalidates_only_its_scene_mix`), keep the oracle
independent of the implementation, and for a bug make the test fail first.

→ [`verification-strategy.md`](references/verification-strategy.md),
[`property-and-differential-testing.md`](references/property-and-differential-testing.md),
[`mutation-and-formal.md`](references/mutation-and-formal.md),
[`fuzzing.md`](references/fuzzing.md) for import, provider, manifest and audio parsers

## 6. Never collapse evidence into the word "verified"

Use the canonical [evidence vocabulary](references/verification-strategy.md#evidence-vocabulary)
and its live/mock, file-durability, renderer and performance qualifications. Every report names
the level and actual production boundary exercised; a document or build cannot accept runtime
behavior. This anchor remains the shared entrypoint for existing consumers.

## 7. Verification tiers

The [verification cadence](references/verification-cadence.md) owns timing: DEV focused checks,
CHECKPOINT module/renderer evidence, HANDOFF final-diff preflight and CRITICAL targeted evidence.
Edits and intermediate commits do not each trigger preflight. Reuse valid evidence and avoid
checks already covered by the handoff plan. Other skills reference this policy's schedule.

For handoff, follow the [local handoff policy](references/local-execution.md#local-handoff-and-pr-completion)
and report [independent gates](references/verification-strategy.md#independent-verification-gates).
Core verification is independent of integration acceptance; a layer's acceptance never establishes
production or the work plan's [phase-exit acceptance](../../../docs/work-plan/project-planning.md#phase-exit-and-review-evidence).
Pipeline/publication and UI evidence remain the owner's obligations; docs-only needs no integration test.

Repository-level checks go through [`cargo xtask`](../../../xtask/README.md); scripts are its
primitives. Report each gate as xtask printed it (`PASS`, `FAIL`, `NOT_RUN`, `UNSUPPORTED`, which
map to `passed`, `failed`, `not-run`, `unavailable`). Report every slow, skipped or unavailable
gate on the residual-risk line, never by omission.

## 8. Clean syntax and formatting before completion

Write code a reviewer can read once: small functions named for the transformation they perform,
types that carry meaning, exhaustive matches, exact error enums, early returns over nesting, no
`unwrap`/`expect` in production paths, explicit imports, comments that state *why*. Follow
[`clean-code.md`](references/clean-code.md).

At checkpoint/handoff, list changed paths and ensure format (check mode), lint and test evidence
covers each touched toolchain's final inputs under the [cadence policy](references/verification-cadence.md).
Use preflight for included checks; intermediate commits do not require rerunning the set.
A green test run does not waive formatting. Preserve unrelated work in a dirty tree; report
files a formatter touched that you did not edit.

## 9. Completion report

Every field is required, including the last one:

```text
Invariant:
Owner / boundary:
Behavior SSOT impact:      <capability · lifecycle · class · authority · delta | none: reason>
Evidence level:            <from § 6>
Evidence:                  test / property / tool → pass | fail
Command:                   <exact, reproducible>
Provenance:                <source revision, fixtures, live vs mocked, artifacts>
Performance (if affected): <baseline/delta, samples, conditions and provenance; unrun limits>
Layer gates / verified scope: <independent status, evidence level and covered claims per layer>
Deferred acceptance:       <unrun DB/API/I/O checks; reason; prerequisites, command, criteria, owner>
Execution blockers / failures: <environment or dependency gap vs actual failed check>
Edge classes covered:
Not covered / residual risk:
```

A report must state actionable residual risk.

## References — load the relevant set, one at a time

| The work is about… | Reference |
|---|---|
| module boundaries, dependency direction, ports, Narrative Forge, provider neutrality | [`decoupling.md`](references/decoupling.md) |
| separating a durable rule from I/O, effects as data, transactional rejection | [`functional-core.md`](references/functional-core.md) |
| moving a decision out of a handler, worker loop, component or screen, with before/after recipes | [`extraction-recipes.md`](references/extraction-recipes.md) |
| immutable revisions, append-only records, value semantics in Rust and Kotlin | [`immutability.md`](references/immutability.md) |
| making an invalid state or identity unrepresentable | [`types-as-proofs.md`](references/types-as-proofs.md) |
| expected/domain/validation errors, infrastructure failures, unexpected faults, typed composition, cancellation, retry and translation | [`typed-errors-and-composition.md`](references/typed-errors-and-composition.md) |
| Kotlin identities/factories/ADTs, pure reducers, ownership, SOLID or concurrency verification | [CMP Kotlin rules](../cantos-cmp-mobile/references/kotlin-conventions.md), [architecture](../cantos-cmp-mobile/references/cmp-architecture.md), [testing](../cantos-cmp-mobile/references/cmp-testing.md); load all three for a Kotlin decision |
| serde, SQL rows, fixtures, AI output or migrations around a refined type | [`boundary-hardening.md`](references/boundary-hardening.md) |
| Turso/legacy PostgreSQL transactions, file ownership, constraints, concurrency, migrations and real database tests | [`persistence.md`](references/persistence.md) |
| native processes, UDS/files, supervisors, no-container acceptance or explicit legacy runtime operations | [`service-runtime.md`](references/service-runtime.md) |
| v1 authentication, mailbox proof, deferred factors/recovery, private RPC, session/revocation boundaries | [`identity-and-session-boundaries.md`](references/identity-and-session-boundaries.md) |
| database-backed GET/read/fetch, hot queries, indexes, selective cache, read-model invalidation or performance acceptance | [`turso-read-performance.md`](references/turso-read-performance.md) |
| Axum handlers, DTOs, error mapping, authorization and client contracts | [`http-api-boundary.md`](references/http-api-boundary.md) |
| JSON/versioned Protobuf or legacy MessagePack on a route, `Content-Type`/`Accept` negotiation, a client codec switch, wire goldens, codec benchmarks, extending the codec to Theatre or CMP | [`wire-codec.md`](references/wire-codec.md) |
| naming, function shape, errors, comments, Rust/Kotlin/Leptos style | [`clean-code.md`](references/clean-code.md) |
| choosing an oracle, partitioning edge cases, the verification ledger | [`verification-strategy.md`](references/verification-strategy.md) |
| laws over generated inputs, state machines, golden fixtures, reference models | [`property-and-differential-testing.md`](references/property-and-differential-testing.md) |
| assertion strength, or a small kernel worth bounded model checking | [`mutation-and-formal.md`](references/mutation-and-formal.md) |
| a parser or decoder of bytes nobody on the team controls: DOCX/Markdown import, AI output, provider bodies, manifests, audio | [`fuzzing.md`](references/fuzzing.md) |
| running checks, resource limits, remote CI, secrets, what may be executed | [`local-execution.md`](references/local-execution.md) |
| when to run checks, milestones, risk, evidence reuse and invalidation | [`verification-cadence.md`](references/verification-cadence.md) |
| emitting or reviewing logs, diagnostic events, levels, redaction, correlation IDs or telemetry | [`structured-observability.md`](references/structured-observability.md); investigating a failure or log goes through [`cantos-diagnostics`](../cantos-diagnostics/SKILL.md) |
| reviewing an existing PR, range, patch or local diff | [`diff-review.md`](references/diff-review.md), entered through [`cantos-code-review`](../cantos-code-review/SKILL.md) |

## Compose with
Domain owners: [`cantos-script-ir`](../cantos-script-ir/SKILL.md),
[`cantos-production-pipeline`](../cantos-production-pipeline/SKILL.md),
[`cantos-publication`](../cantos-publication/SKILL.md),
[`cantos-listening`](../cantos-listening/SKILL.md). UI: [`cantos-ui-design`](../cantos-ui-design/SKILL.md)
with [`cantos-leptos-web`](../cantos-leptos-web/SKILL.md) or [`cantos-cmp-mobile`](../cantos-cmp-mobile/SKILL.md). When a UI task changes domain state,
validation, authorization or async decisions, apply this skill first and keep those decisions out
of presentation code.
