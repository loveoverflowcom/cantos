---
name: cantos-engineering
description: >-
  Engineering foundation for Cantos (Rust, Axum, PostgreSQL, Leptos, Compose Multiplatform).
  Invariant-first workflow, decoupled module boundaries, immutable values and revisions, types
  as proof barriers, functional core with imperative shell, clean readable syntax, cheapest
  adequate test evidence, PostgreSQL read performance and conditional DBSP evaluation,
  one evidence vocabulary and a mandatory residual-risk report. Use for
  any domain, backend, persistence, contract, test, refactoring or verification change.
---

# Cantos engineering

This skill is a **behavioral contract and router**, not a manual. Load the smallest relevant set
of references, one at a time. Every other Cantos skill composes this one and reuses its order,
vocabulary and report; none of them redefines these.

## Ground rules

- **Read before editing.** [`AGENTS.md`](../../../AGENTS.md), the owning product or architecture
  document, the work-plan item and the code you will touch. Product rules live in
  [`docs/`](../../../docs/README.md); link them, never restate a competing copy.
- **Repository reality.** Script IR, the Axum/PostgreSQL revision backend and minimal Leptos
  Studio exist; mobile and production execution remain planned. Inspect manifests, scripts and
  workflows before citing a command; see [`local-execution.md`](references/local-execution.md).
- **Distinguish three states** in code, docs and reports: *proposed* design, *implemented*
  behavior and *verified* behavior with a named evidence level.
- **Keep the stack.** Rust + Axum + PostgreSQL modular monolith, Leptos Web, CMP mobile with
  native playback, S3-compatible storage. A new crate, service, trait, dependency or framework
  needs a concrete consumer and a stated reason ([decision 0001](../../../docs/decisions/0001-modular-monolith.md)).

## The required order

```text
read the owning doc, the work item and the code
        ↓
name the invariant and its failure mode
        ↓
choose the owner and the boundary (decouple)
        ↓
make invalid states unrepresentable; make settled facts immutable
        ↓
extract the functional core from the imperative shell
        ↓
choose the cheapest adequate evidence (for a bug: a failing test first)
        ↓
implement in small, named, readable steps
        ↓
run the focused test, then proportionate broader checks
        ↓
format and lint
        ↓
report the exact evidence level and the residual risk
```

Skipping a step is a decision you must state. "No core extracted: this adapter only maps types"
is a fine answer; silence is not.

### Database-backed GET, read and fetch work

Load [PostgreSQL read performance and DBSP evaluation](references/postgresql-read-performance.md)
before adding or changing a database-backed read, its query, or a write that feeds a read model.
It owns the mandatory inventory, PostgreSQL baseline, simpler optimization comparison, DBSP
fit decision, differential/failure checks, benchmark record and acceptance checklist. Prioritize
hot read paths; adopt incremental views only when measured benefit and semantics justify them.
Mutations require a separate, workload-specific overhead and consistency decision. This is a
manual working requirement; no DBSP integration or performance gate is implemented here.

## 1. Name the invariant before writing code

State in one sentence the observable behavior or invariant the change alters and what goes wrong
if it is false. Keep a working ledger while implementing:

| Claim / invariant | Failure mode | Cheapest oracle | Evidence level |
|---|---|---|---|

The [business-rule invariants](../../../docs/product/business-rules.md#product-invariants-to-verify)
are the first ledger seeds for any production, publication or listening change: one changed line
regenerates only that line and its dependent mixes; a dead worker resumes without duplicate
accepted output; a lost provider response is reconciled before a repeat; stale approval blocks
publication; an incomplete upload is never listener-visible; repeated play costs nothing; late
offline progress never blindly overwrites newer progress.

Every added test maps to a claim. Every changed high-impact claim gets evidence or an explicit gap.

## 2. Decouple by ownership

```text
pure domain values and rules   ←  application (permissions, transitions, gates)
        ↑                              ↑
   adapters (PostgreSQL, S3, TTS, adaptation AI, audio tools)   ←  HTTP handlers, workers, UI
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
a correction creates a new revision, run, artifact or release and preserves history. In code,
transformations take values and return new values; mutation stays local, private and invisible
until the function returns.

→ [`types-as-proofs.md`](references/types-as-proofs.md),
[`immutability.md`](references/immutability.md),
[`boundary-hardening.md`](references/boundary-hardening.md) when a value crosses serde, SQL rows,
fixtures, AI output or a migration.

## 4. Functional core, imperative shell

```text
world: HTTP · PostgreSQL · S3 · TTS · adaptation AI · audio tools · clock · IDs · randomness
                         ↓
                  imperative shell        resolves facts, executes effects, retries
                         ↓
                 typed plain facts
                         ↓
             pure deterministic core     validate · decide · plan · reduce · fingerprint
                         ↓
              decision / effects as data
```

When a durable decision sits inside a handler, worker loop, adapter, Leptos component or Kotlin
screen, extract it before reaching for mocks. Cantos decisions that belong in the core include
validating a script revision, planning partial regeneration, computing a speech fingerprint,
classifying a provider failure into a retry decision, deciding whether a release may publish and
reconciling listener progress. Each must be callable from a plain `#[test]` with no async runtime,
database or provider.

Do not force this on thin CRUD or an adapter that only maps types.

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

## 7. Verification tiers

| Tier | What runs |
|---|---|
| Every change today | `cargo xtask verify repository` (repository checks, Python unit tests, `git diff --check`) |
| Every code change | the focused test first, then `cargo xtask preflight --changed-only`, which selects fmt, clippy, tests, contracts, Web, mutation and PostgreSQL gates from the diff and broadens when impact is uncertain; plus the cheapest regression per behavior change |
| Pipeline / publication change | fault-injected crash, retry, duplicate-delivery and stale-revision cases at the changed step |
| UI change | the renderer skill's evidence loop: driven flow, inspected captures, viewports, themes, Vietnamese text, focus, text scaling |
| Phase exit | the work plan's [review boundary](../../../docs/work-plan/project-planning.md#phase-exit-and-review-evidence) with live and mocked evidence labeled separately |

For implementation handoff and PR creation, use the [local handoff policy](references/local-execution.md#local-handoff-and-pr-completion):
applicable local tests, mocked integration and verification checks pass; no waiting for GitHub CI.
Real local infrastructure is still needed for claims a mock cannot establish. Production and
phase-exit acceptance remain separate. Docs-only work gets no invented integration requirement.

Repository-level checks go through [`cargo xtask`](../../../xtask/README.md); scripts are its
primitives. Report each gate as xtask printed it (`PASS`, `FAIL`, `NOT_RUN`, `UNSUPPORTED`, which
map to `passed`, `failed`, `not-run`, `unavailable`). Report every slow, skipped or unavailable
gate on the residual-risk line, never by omission.

## 8. Clean syntax and formatting before completion

Write code a reviewer can read once: small functions named for the transformation they perform,
types that carry meaning, exhaustive matches, exact error enums, early returns over nesting, no
`unwrap`/`expect` in production paths, explicit imports, comments that state *why*. Follow
[`clean-code.md`](references/clean-code.md).

After the final edit and before any commit or handoff: list changed paths, run the formatter of
each touched toolchain in check mode, then lint and tests. A green test run does not waive
formatting. Preserve unrelated work in a dirty tree; report files a formatter touched that you did
not edit.

## 9. Completion report

Every field is required, including the last one:

```text
Invariant:
Owner / boundary:
Evidence level:            <from § 6>
Evidence:                  test / property / tool → pass | fail
Command:                   <exact, reproducible>
Provenance:                <source revision, fixtures, live vs mocked, artifacts>
Edge classes covered:
Not covered / residual risk:
```

A report with an empty residual-risk line is a report nobody can act on.

## References — load the relevant set, one at a time

| The work is about… | Reference |
|---|---|
| module boundaries, dependency direction, ports, Narrative Forge, provider neutrality | [`decoupling.md`](references/decoupling.md) |
| separating a durable rule from I/O, effects as data, transactional rejection | [`functional-core.md`](references/functional-core.md) |
| moving a decision out of a handler, worker loop, component or screen, with before/after recipes | [`extraction-recipes.md`](references/extraction-recipes.md) |
| immutable revisions, append-only records, value semantics in Rust and Kotlin | [`immutability.md`](references/immutability.md) |
| making an invalid state or identity unrepresentable | [`types-as-proofs.md`](references/types-as-proofs.md) |
| serde, SQL rows, fixtures, AI output or migrations around a refined type | [`boundary-hardening.md`](references/boundary-hardening.md) |
| PostgreSQL transactions, constraints, concurrency control, migrations, test databases | [`persistence.md`](references/persistence.md) |
| database-backed GET/read/fetch, hot queries, indexes, DBSP incremental views, CDC, read-model invalidation or performance acceptance | [`postgresql-read-performance.md`](references/postgresql-read-performance.md) |
| Axum handlers, DTOs, error mapping, authorization, OpenAPI and client contracts | [`http-api-boundary.md`](references/http-api-boundary.md) |
| naming, function shape, errors, comments, Rust/Kotlin/Leptos style | [`clean-code.md`](references/clean-code.md) |
| choosing an oracle, partitioning edge cases, the verification ledger | [`verification-strategy.md`](references/verification-strategy.md) |
| laws over generated inputs, state machines, golden fixtures, reference models | [`property-and-differential-testing.md`](references/property-and-differential-testing.md) |
| assertion strength, or a small kernel worth bounded model checking | [`mutation-and-formal.md`](references/mutation-and-formal.md) |
| a parser or decoder of bytes nobody on the team controls: DOCX/Markdown import, AI output, provider bodies, manifests, audio | [`fuzzing.md`](references/fuzzing.md) |
| running checks, resource limits, remote CI, secrets, what may be executed | [`local-execution.md`](references/local-execution.md) |
| reviewing an existing PR, range, patch or local diff | [`diff-review.md`](references/diff-review.md), entered through [`cantos-code-review`](../cantos-code-review/SKILL.md) |

## Compose with

Domain owners: [`cantos-script-ir`](../cantos-script-ir/SKILL.md),
[`cantos-production-pipeline`](../cantos-production-pipeline/SKILL.md),
[`cantos-publication`](../cantos-publication/SKILL.md),
[`cantos-listening`](../cantos-listening/SKILL.md). UI: [`cantos-ui-design`](../cantos-ui-design/SKILL.md)
with [`cantos-leptos-web`](../cantos-leptos-web/SKILL.md) or
[`cantos-cmp-mobile`](../cantos-cmp-mobile/SKILL.md). When a UI task changes domain state,
validation, authorization or async decisions, apply this skill first and keep those decisions out
of presentation code.
