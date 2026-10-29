# Verification strategy

> **Scope.** Turn a Cantos invariant into reproducible evidence: the verification ledger,
> independent oracles, edge-case partitions, the escalation ladder and honest reporting. Use
> before writing tests for any behavior change, when an edge space is unclear, or when a test
> suite passes but nobody trusts it. The canonical vocabulary is [below](#evidence-vocabulary);
> the report format stays in the foundation [`SKILL.md`](../SKILL.md#9-completion-report).

## Route by claim

| The claim is about… | Go to |
|---|---|
| a state that should be impossible | [`types-as-proofs.md`](types-as-proofs.md) — prevent it, do not test it |
| a rule tangled with I/O | [`functional-core.md`](functional-core.md) — extract it first |
| typed errors, composition short-circuit/accumulation, refinement preservation, cancellation or exhaustive boundary mapping | [`typed-errors-and-composition.md`](typed-errors-and-composition.md) — exact outcomes first |
| a law over a large input space, a state machine, golden fixtures, a reference model | [`property-and-differential-testing.md`](property-and-differential-testing.md) |
| whether assertions kill real defects; a tiny kernel worth model checking | [`mutation-and-formal.md`](mutation-and-formal.md) |
| constraints, transactions, concurrency, migrations | [`persistence.md`](persistence.md) |
| crash, lease, duplicate, timeout and budget behavior | [`fault-injection-testing.md`](../../cantos-production-pipeline/references/fault-injection-testing.md) |
| a parser or decoder of untrusted bytes: panics, hangs, resource blow-ups | [`fuzzing.md`](fuzzing.md) |
| moving a decision out of a handler, loop or component so it can be tested | [`extraction-recipes.md`](extraction-recipes.md) |
| rendered appearance or interaction | the renderer skill's testing reference |

## Choose the method by the question

Each technique answers one question about one scope. Pick the smallest technique whose answer
covers the claim; add a second only when it answers a question the first leaves open.

| Technique | Answers | Leaves open | Applies when |
|---|---|---|---|
| type, private constructor, exhaustive `match` | can the value or state exist outside its barrier? | whether the runtime check is right; whether every path goes through it | an invalid combination exists and a type can exclude it ([types](types-as-proofs.md)) |
| static lint or architecture check | is the pattern absent from the code this command linted? | other targets, features and `cfg`; lints not enabled | the lint is enabled at deny level in a gate that covers the code ([lint claims](types-as-proofs.md#compiler-and-lint-claims-need-a-sensitive-misuse)) |
| exact regression (example or table) | does this input give this exact outcome? | inputs not listed | the default for a behavior change or a bug |
| bounded exhaustive loop, property, state machine | does a law hold over a finite domain, generated inputs or event sequences? | inputs the generator cannot reach | the partition is too large to list, or order matters ([properties](property-and-differential-testing.md)) |
| Kani | does this production symbol satisfy the proposition for every input within the bounds and assumptions? | I/O, the shell, inputs the assumptions exclude | a small pure critical kernel ([selection](mutation-and-formal.md#bounded-model-checking)) |
| Quint model and conformance | does the model hold on its runs; does the pure core reproduce the checkpoints of the traces actually replayed? | traces not replayed, the shell, storage; a draft model's authority | a catalog capability ([`cantos-behavior`](../../cantos-behavior/SKILL.md)) |
| real adapter (Turso file, HTTP, browser, device) | does the real boundary keep the semantics? | boundaries it did not cross | transactions, durability, auth contracts, rendering, platform |

- **No duplicate evidence.** A second test with the same oracle at the same layer adds nothing.
  Do not test at runtime that a type-excluded state cannot occur; test the constructor and the
  bypass paths instead. A native test shares a model run's ID only when it answers a different
  question (transport or constructor versus model semantics).
- **Decide by the claim, not the shape.** The number of Booleans, the presence of a `Vec`, a
  diff's length or a file's size select nothing. Ask which state is invalid, who can produce
  it, which input reaches it and what a failure costs ([flags](types-as-proofs.md#model-states-not-flag-combinations),
  [critical scope](verification-cadence.md#critical-and-platform-obligations)).
- There is no quota for fewer tests or more formal coverage; a removed test is a changed claim
  that cites the evidence now answering its question.

## Production reach: entrypoint, core and effect

A claim about what a route, RPC, worker or screen does has three legs: the entrypoint reaches
the decision with real facts, the decision returns the outcome, and the shell executes the
effect. Evidence for one leg says nothing about the others:

| Evidence | Establishes | Leaves open |
|---|---|---|
| plain test or Kani proof calling the decision | the decision leg | whether the entrypoint calls it, with which facts, and whether the effect happens |
| entrypoint test with named deterministic fakes | the branch is reached and its outcome mapped (`mock-integration-tested`) | the real store's write, transaction and durability |
| real adapter test | the effect against the real boundary | entrypoints that test did not drive |

Example at `cf3ab8ed`: a unit test shows `identity_auth::decision::decide_admission` returns
`Admission::Revoke(VersionRegressed)` for a lower security version. The Server's `confirm`
reaches it only after a fresh security-state read and calls `store.revoke` best effort.
[Identity auth v1](../../../../services/cantos-server/contracts/auth/identity-auth/v1/README.md)
denies the request either way, makes revocation permanent only once the write commits and redoes
a failed write at the next admission. "This request is denied" needs an entrypoint test;
"the family stays revoked" needs a real Turso store test; the decision test supplies neither,
and the best-effort write is the contract, not a defect. Likewise the `image-bounds` proof covers
`check_image_bounds`; that the AVIF parser calls it for every color, alpha and tile plane is a
separate claim for the `image_intake` tests, outside the proof.

Record which entrypoint each piece of evidence drove, or "decision only".

## Layered verification

Use [verification cadence](verification-cadence.md) for DEV/CHECKPOINT/HANDOFF timing, risk and
evidence reuse. The layers below define claim obligations, not suites to rerun after every edit.

Apply the foundation's [required order](../SKILL.md#the-required-order) to each changed claim.
Choose the cheapest adequate oracle within each layer; these layers organize work and do not
replace the evidence vocabulary or add a runner.

| Layer | Design, implement and verify |
|---|---|
| L1 — Abstraction & Architecture | identify the module owner, invariant and failure mode; review DDD bounded contexts, SOLID responsibilities, functional core/shell and import/manifests pointing inward. Domain imports no PostgreSQL client, Axum, Leptos, Compose or platform SDK. Add a trait/port/abstraction only for a concrete consumer and real boundary ([decoupling](decoupling.md)). |
| L2 — Domain, Types & Error Handling | model immutable facts with newtypes, closed enums or Kotlin sealed types, fallible constructors and exhaustive matching; make invalid states unrepresentable where practical. Pure functions return `Result` or a typed success/rejection outcome; test propagation of exact errors, invariant-preserving transitions and complete unchanged state on rejection ([types](types-as-proofs.md), [functional core](functional-core.md), [typed errors](typed-errors-and-composition.md)). |
| L3 — Unit & Property Verification | write core unit tests before DB/API wiring: ordinary Rust `#[test]`, Kotlin `commonTest` where applicable, and plain Rust decisions outside Leptos. Use exact table-driven/regression assertions and property/state-machine tests where the input space warrants them; run focused tests, format, lint and applicable static checks. Pure domain tests need no database, network, container, emulator or async runtime. Discover configured commands; report missing format/lint tooling honestly. |
| L4 — Application & I/O Adapters | after L1–L3 pass, implement use cases, narrow ports and adapters. Exercise production orchestration with deterministic fakes, including typed error mapping, ordered effect execution and rejection behavior. Distinguish an isolated fake test from production modules exercised together; name every double. Adapter-only type mapping with no domain rule may start here if that exception is recorded. |
| L5 — DB, API & Integration | run real authoritative-engine migrations/constraints/transactions/concurrency checks (current PostgreSQL or accepted Turso), HTTP request/response/DTO/authorization/error contracts, storage/network/platform integration, scoped E2E and runtime acceptance. A handler tested with a fake store is L4 evidence for that contract, never proof of real database/service semantics. Required L5 checks stay required when unavailable. |

Keep independent runnable core tests even when the shell cannot start. Formatting, linting and
static checks apply to the touched toolchain; a missing mobile tool or emulator does not erase
Rust/JVM results, and a JVM run does not establish Native/device acceptance. Thin CRUD need not
gain a speculative domain layer. Docs-only work follows the existing repository gates.

## Independent verification gates

Use one row per distinct claim scope, splitting error handling, mocked orchestration and real
infrastructure when their evidence differs. Record layer, status, evidence level, exact command
or review artifact, covered scope and gaps. The following is an **illustrative** ledger, not
results from this repository:

| Layer / claim scope | Status | Evidence held |
|---|---|---|
| L1 Architecture / abstraction | PASS | documented owner/dependency review |
| L2 Domain / type safety | PASS | type-enforced construction paths, with named compiler output |
| L2 Error handling | PASS | example-tested exact errors and unchanged rejected state |
| L3 Unit / property tests | PASS | example-tested / property-tested, named executed suites |
| L4 Application / mocked tests | NOT_RUN | none; orchestration not exercised |
| L5 Database integration | UNSUPPORTED | none; PostgreSQL tools missing |
| L5 API integration | NOT_RUN | none; real server not exercised |
| L5 E2E / runtime | NOT_RUN | none; runtime flow not driven |

`PASS` covers only the stated obligations actually satisfied. A manual architecture review is
`documented` evidence; it is not a compiler/static check. Preserve xtask's printed statuses;
an explicit `--skip` is `NOT_RUN` even when the reason is a missing prerequisite. A missing tool
reported by xtask is `UNSUPPORTED`; a selected gate not reached is `NOT_RUN`. A failed test or
command is `FAIL`. Zero selected tests cannot satisfy a gate. Inapplicable scope is stated with
a reason, never promoted to `PASS`. No `NOT_RUN`, `UNSUPPORTED` or `FAIL` becomes `PASS` because
another layer passed, mocks passed or a PR opened.

Keep four report fields distinct:

- **Verified scope:** exact claims and layers supported by actual evidence.
- **Deferred acceptance:** required DB/API/I/O/runtime checks still outstanding, with the
  prerequisites, reproducible command, acceptance criteria, residual risk and next owner.
- **Execution blocker:** missing environment/tool/dependency that prevented execution.
- **Actual failure:** a check ran or failed to start with an error; name it and fix or report it.

Delivery decisions belong to [local handoff](local-execution.md#local-handoff-and-pr-completion).
An infrastructure gap does not invalidate passed core evidence or waive required acceptance.

## Start with a ledger

| Claim / invariant | Failure mode | Cheapest oracle | Test level |
|---|---|---|---|
| editing one dialogue regenerates only that dialogue | unrelated clips rebilled | regeneration plan equals `{dialogue-02}` | unit + property |
| stale approval blocks publication | unapproved audio goes public | `Err` contains `ApprovalStale` | unit |
| a lost provider response is reconciled first | duplicate paid synthesis | no second attempt before reconciliation | fault-injected |
| repeated play creates no jobs | listener actions incur cost | job table row count unchanged | integration |

Every test maps to a row; every high-impact row has evidence or a stated gap.

### Rows for risky claims

Expand a row only for a high-impact claim: P0/P1 failure, a [critical scope](verification-cadence.md#critical-and-platform-obligations),
or a type, lint or formal claim. Ordinary rows keep the four columns above.

```text
Invariant:           <one sentence> — <requirement source and status>
Entrypoint:          <route, RPC, worker or screen relying on it> | decision only
Invalid-state paths: <constructors, serde, rows, Default, mutable methods, fixtures> — closed by … | open
Technique:           <row of the method table> — the question it answers here
Applicability:       REQUIRED | NOT_APPLICABLE: <reason>
Oracle:              <independent expectation: literal, law, model run, real boundary>
Execution:           PASS | FAIL | NOT_RUN | UNSUPPORTED — <command, source identity, output>
Deferred:            <reason, owner, prerequisite, next step> | none
Residual risk:       <what can still fail>
```

Applicability, execution and deferral are separate axes. `NOT_APPLICABLE` never stands in for
`NOT_RUN`; a `REQUIRED` row that did not run keeps its actual status and its deferral.

## Keep the oracle independent

Never compute the expected value by calling the implementation through another path. Use:

- a hand-written expected value for a small example;
- a deliberately naive reference model with a different structure;
- an algebraic law (round trip, idempotence, invariant preservation);
- a golden fixture produced and reviewed separately;
- a previous compatible version (differential);
- a domain invariant checked without re-running the algorithm.

## Partition the edge space

Inspect each category; keep only what the change can reach:

- empty, single, typical, maximal; just below / at / just above every limit;
- duplicates, reorderings, ties, stable vs unstable ordering;
- malformed, truncated, unknown-version and unknown-field inputs;
- Vietnamese text in NFC and NFD, combining marks, long names, emoji in titles;
- every state and transition, including terminal and repeated commands;
- unauthorized actors, other tenants, replayed evidence for another resource;
- retries, duplicate delivery, cancellation, timeouts, lease expiry, crash at each step;
- stale revisions, concurrent edits, late responses overwriting newer state;
- old schema versions and migrations;
- budget exactly reached, exceeded mid-run, unavailable estimate.

## Design semantic tests

- Assert structured outcomes and exact error variants, never `is_err()` alone.
- Assert that a rejection changed nothing (state, rows or canonical bytes).
- Name tests as theorems; keep arrange/act/assert within one screen.
- Prefer pure `#[test]` for rules; exercise production modules together with external fakes for
  `mock-integration-tested` claims; use real local infrastructure for `integration-tested` claims.
- Do not assert mock call counts unless the count is the contract (for example, "exactly one
  provider call per fingerprint").
- Fixed clocks, sequence IDs and seeded randomness; wait on observable conditions, never sleep.

## Escalate only when needed

```text
type / exhaustive match
→ example and table tests
→ bounded exhaustive loop (finite domain: walk all of it)
→ property / state-machine test
→ golden fixtures, reference model, differential
→ fault injection at named points
→ fuzzing of untrusted-input parsers (crash, hang, blow-up)
→ integration with real PostgreSQL / object storage
→ provider-live test under a cost limit
→ mutation testing on a stable pure module
→ bounded model checking of a tiny critical kernel
```

These are different scopes, not a ranking. Mutation testing checks assertion strength; fuzzing
checks robustness; integration checks the shell; a provider-live test checks the vendor contract
on one day.

## Selection matrix

Pick the cheapest technique that fits the risk's shape; escalate only when the claim justifies it.

| Risk or shape | First choice | Escalate when |
|---|---|---|
| small pure rule (a blocker, a classification) | table test with exact variants | the input partition is large → property test |
| algebraic transform (canonical bytes, fingerprint) | property: round trip, idempotence, sensitivity | no obvious oracle → metamorphic or differential |
| finite state × event domain | bounded exhaustive loop against a hand-written table | the domain explodes → state-machine property |
| lifecycle or reducer (run, release, player, download) | command-sequence property against a tiny model | a small critical kernel remains → bounded model checking |
| parser or decoder of untrusted input | table + round trip | hostile bytes → [fuzzing](fuzzing.md) |
| cross-platform contract (Rust and Kotlin) | shared reviewed vectors | an independent oracle is also needed → differential |
| optimized code with a slow twin | differential against the reference model | |
| SQL claim (claim, fence, reservation, constraint) | two real connections on PostgreSQL | crash at a named point → [fault injection](../../cantos-production-pipeline/references/fault-injection-testing.md) |
| external call with an ambiguous outcome | scripted provider with its own ledger | a real vendor contract → recorded fixtures, then a cost-capped live run |
| shared mutable state or atomics inside one process | focused test | schedule-sensitive → a schedule-exploring tool such as loom (candidate) |
| `unsafe` or FFI | safety invariants | broader native risk → Miri or a sanitizer (candidate); justify with a real boundary |
| assertion strength of a pure module | mutation run on that module | survivors classified, then tests that kill the real gaps |

Loom, Miri and sanitizers apply only if the code has such a boundary. The domain core has none by
design, so do not adopt them speculatively.

## For a bug

1. Reproduce it with the smallest failing test at the owning layer.
2. Keep the minimized input as a named regression.
3. Fix; rerun the focused regression, then use the cadence policy for affected checkpoint and
   handoff checks. Expand sooner only for an uncovered risk or changed boundary.

## Mocked and live evidence are different rows

Routine implementation handoff uses the [local handoff policy](local-execution.md#local-handoff-and-pr-completion):
record local regression tests, mocked integration where a flow crosses modules and applicable
checks in independent rows. Deliver verified layers when requested and defer unavailable
infrastructure acceptance explicitly. Mocked integration supports orchestration and contracts
against doubles; database semantics and real-provider acceptance require their own evidence.

The work plan requires labeling them separately
([work plan § Shared completion evidence](../../../../docs/work-plan/README.md#shared-completion-evidence)).
A scripted fake TTS provider proves orchestration, not synthesis. A local file proves mixing
code ran, not CDN delivery. A desktop render proves layout code ran, not native playback.

## Evidence vocabulary

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
integration-tested        crossed a real database, object store or service boundary
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
is `mock-integration-tested`. Neither is `provider-live-tested`; an in-memory repository double is
never real-database integration evidence. An actual memory-only database test does not establish
file durability/recovery. A desktop or JVM host render is never `device-tested`.
A document check never proves that an app builds. Agent-authored JSON or assertions are not
verifier output; report only what a tool actually printed.

Performance claims reuse this vocabulary with comparable baseline/delta, sample distribution,
source/build/fixture/tool/host provenance and named cache conditions; report unrun measurements.
Numbers add no evidence level. For Web, use [performance and caching](../../cantos-leptos-web/references/performance-and-caching.md).
