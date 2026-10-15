---
name: cantos-engineering
description: >-
  Engineering foundation for Cantos (Rust, Axum, PostgreSQL, Leptos, Compose Multiplatform).
  Invariant-first workflow, decoupled module boundaries, immutable values and revisions, types
  as proof barriers, functional core with imperative shell, clean readable syntax,
  layered verification and progressive delivery, PostgreSQL read performance and conditional DBSP evaluation,
  structured runtime events, one evidence vocabulary and a mandatory residual-risk report. Use for
  any domain, backend, persistence, contract, test, refactoring or verification change.
---

# Cantos engineering

This skill is a **behavioral contract and router**. Load relevant references one at a time.
Every Cantos skill composes its order, vocabulary and report without redefining them.

## Ground rules

- **Read before editing.** [`AGENTS.md`](../../../AGENTS.md), the owning product/architecture doc,
  work item and touched code. Product rules live in [`docs/`](../../../docs/README.md); link, never duplicate them.
- **Repository reality.** Script IR, Axum/PostgreSQL editorial/catalog/content/brief and private
  audio lifecycle slices, and Leptos Studio exist. Theatre Web is a session-only mock; Theatre
  Mobile has a Gradle CMP mock with Android/iOS hosts and pure playback/controller tests.
  Production listener API, native audio and delivery remain planned. Backend TTS/workers were
  removed under [ADR 0018](../../../docs/decisions/0018-external-audio-production.md).
  Inspect commands and runtime capabilities; see [local execution](references/local-execution.md#runtime-portability).
- **Platform scope.** Follow the [platform matrix](../../../docs/architecture/repository-layout.md#platform-scope):
  Studio is web-only. Exclude Studio Mobile from implementation, scaffolding and build/test/preview automation unless explicitly requested in a future roadmap; retain Theatre Web/Mobile and shared libraries.
- For registered stateful expectations/scenarios, compose [cantos-behavior](../cantos-behavior/SKILL.md); keep model and implementation evidence separate.
- **Distinguish three states** in code, docs and reports: *proposed* design, *implemented*
  behavior and *verified* behavior with a named evidence level.
- **Keep the stack.** Rust + Axum + PostgreSQL modular monolith, Leptos Web, CMP mobile with
  native playback, S3-compatible storage. A new crate, service, trait, dependency or framework
  needs a concrete consumer and a stated reason ([decision 0001](../../../docs/decisions/0001-modular-monolith.md)); runtime work follows [service-runtime](references/service-runtime.md).

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
[PostgreSQL read performance and DBSP evaluation](references/postgresql-read-performance.md).
It owns inventory, PostgreSQL baseline, simpler optimization comparison, DBSP fit,
differential/failure checks, benchmark record and acceptance. Adopt incremental views only
when measured benefit and semantics justify them; prioritize hot paths.
Mutations need a separate overhead/consistency decision. One experimental, feature-gated DBSP read
model exists ([ADR 0017](../../../docs/decisions/0017-experimental-dbsp-job-summary.md)); production adoption does not.

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
   adapters (PostgreSQL, private storage, read-only inspection)   ←  HTTP handlers, UI
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
wire path cannot forge what the constructor rejects.

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
world: HTTP · PostgreSQL · private storage · read-only audio tools · clock · IDs · randomness
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

→ [`functional-core.md`](references/functional-core.md)

## 5. Every behavior change gets the cheapest deterministic regression evidence

1. Identify the observable behavior or invariant.
2. Add or update the cheapest deterministic test that would fail if it regressed.
3. Assert the exact error variant or the exact resulting state — never only `is_err()`.
4. Prefer ordinary deterministic tests; escalate only when the claim justifies it.

```text
types / exhaustive match
→ focused example / table test with exact errors
→ bounded exhaustive loop over a finite domain
→ property or state-machine test
→ replay / differential / golden-fixture oracle
→ fault injection at named crash points (jobs, uploads, publication)
→ integration against real PostgreSQL / object storage / provider
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

This is the canonical vocabulary for every Cantos report:

```text
documented                a document or comment says so
type-enforced             the type system rejects the alternative
statically-checked        a lint, exhaustive match or architecture check enforces it
example-tested            specific inputs are asserted
property-tested           a law holds over generated inputs
differentially-tested     an independent oracle or golden fixture agrees
fault-injected            a crash, timeout or duplicate was injected at a named point
mock-integration-tested   production modules exercised together with named mocked external dependencies
mutation-tested           the assertions kill inserted defects
bounded-model-checked     a proposition holds over a stated symbolic domain and bound
fuzz-tested               a coverage-guided fuzzer ran a named target under resource bounds with no crash,
                          hang or blow-up (robustness only; says nothing about correct output)
integration-tested        crossed a real PostgreSQL, object store or service boundary
provider-live-tested      called a real TTS or adaptation provider under a cost limit
audio-measured            loudness/peak/duration measured by a named tool and profile
audio-listened            a named reviewer listened to the named render
compiled                  it builds — the weakest claim there is
DOM-tested                structure or text asserted from a live DOM
semantics-tested          assertions on a rendered Compose semantics tree; name the renderer
interaction-tested        a real flow was driven by input events
accessibility-checked     semantics + keyboard + focus (never screen-reader proof)
screen-reader-walked      a named TalkBack/VoiceOver/NVDA walkthrough actually happened
screenshot-captured       image files exist
screenshot-inspected      someone opened the rendered pixels and judged them
cross-viewport-inspected  inspected at each listed viewport
cross-theme-inspected     inspected in light and dark
device-tested             ran on a named Android/iOS device, emulator or simulator
production-observed       observed in production, not proven
```

A fake provider alone is `example-tested`; a production flow across modules with external doubles
is `mock-integration-tested`. Neither is `provider-live-tested`; an in-memory store is never
`integration-tested`. A desktop or JVM host render is never `device-tested`.
A document check never proves that an app builds. Agent-authored JSON or assertions are not
verifier output; report only what a tool actually printed.

Performance claims reuse this vocabulary with comparable baseline/delta, sample distribution,
source/build/fixture/tool/host provenance and named cache conditions; report unrun measurements.
Numbers add no evidence level. For Web, use [performance and caching](../cantos-leptos-web/references/performance-and-caching.md).

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
| PostgreSQL transactions, constraints, concurrency control, migrations, test databases | [`persistence.md`](references/persistence.md) |
| database-backed GET/read/fetch, hot queries, indexes, DBSP incremental views, CDC, read-model invalidation or performance acceptance | [`postgresql-read-performance.md`](references/postgresql-read-performance.md) |
| Axum handlers, DTOs, error mapping, authorization and client contracts | [`http-api-boundary.md`](references/http-api-boundary.md) |
| JSON or MessagePack on a route, `Content-Type`/`Accept` negotiation, a client codec switch, wire goldens, codec benchmarks, extending the codec to Theatre or CMP | [`wire-codec.md`](references/wire-codec.md) |
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
