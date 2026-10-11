# Verification strategy

> **Scope.** Turn a Cantos invariant into reproducible evidence: the verification ledger,
> independent oracles, edge-case partitions, the escalation ladder and honest reporting. Use
> before writing tests for any behavior change, when an edge space is unclear, or when a test
> suite passes but nobody trusts it. The vocabulary and report format live in the foundation
> [`SKILL.md`](../SKILL.md) and are not repeated here.

## Route by claim

| The claim is about… | Go to |
|---|---|
| a state that should be impossible | [`types-as-proofs.md`](types-as-proofs.md) — prevent it, do not test it |
| a rule tangled with I/O | [`functional-core.md`](functional-core.md) — extract it first |
| a law over a large input space, a state machine, golden fixtures, a reference model | [`property-and-differential-testing.md`](property-and-differential-testing.md) |
| whether assertions kill real defects; a tiny kernel worth model checking | [`mutation-and-formal.md`](mutation-and-formal.md) |
| constraints, transactions, concurrency, migrations | [`persistence.md`](persistence.md) |
| crash, lease, duplicate, timeout and budget behavior | [`fault-injection-testing.md`](../../cantos-production-pipeline/references/fault-injection-testing.md) |
| a parser or decoder of untrusted bytes: panics, hangs, resource blow-ups | [`fuzzing.md`](fuzzing.md) |
| moving a decision out of a handler, loop or component so it can be tested | [`extraction-recipes.md`](extraction-recipes.md) |
| rendered appearance or interaction | the renderer skill's testing reference |

## Start with a ledger

| Claim / invariant | Failure mode | Cheapest oracle | Test level |
|---|---|---|---|
| editing one dialogue regenerates only that dialogue | unrelated clips rebilled | regeneration plan equals `{dialogue-02}` | unit + property |
| stale approval blocks publication | unapproved audio goes public | `Err` contains `ApprovalStale` | unit |
| a lost provider response is reconciled first | duplicate paid synthesis | no second attempt before reconciliation | fault-injected |
| repeated play creates no jobs | listener actions incur cost | job table row count unchanged | integration |

Every test maps to a row; every high-impact row has evidence or a stated gap.

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
3. Fix; run the single test, the module suite, then proportionate broader checks.

## Mocked and live evidence are different rows

Routine implementation handoff uses the [local handoff policy](local-execution.md#local-handoff-and-pr-completion):
local regression tests, mocked integration where a flow crosses modules, and applicable verification
checks. Do not wait for GitHub CI. Mocked integration is adequate for orchestration and contract
behavior against doubles; database semantics and real-provider acceptance require their own evidence.

The work plan requires labeling them separately
([work plan § Shared completion evidence](../../../../docs/work-plan/README.md#shared-completion-evidence)).
A scripted fake TTS provider proves orchestration, not synthesis. A local file proves mixing
code ran, not CDN delivery. A desktop render proves layout code ran, not native playback.
