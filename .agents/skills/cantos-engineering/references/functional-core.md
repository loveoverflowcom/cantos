# Functional core, imperative shell

> **Scope.** Keep Cantos domain decisions pure, deterministic, total and isolated from I/O:
> facts in, decisions and effects out, transactional rejection, and a thin shell that resolves
> facts and executes effects. Use when a rule sits inside an Axum handler, worker loop, provider
> adapter, SQL query, Leptos component or Kotlin screen; when tests need mocks to reach a rule; or
> when code reads the clock, generates IDs or retries inside a decision.

## The shape

For Kotlin, [FP-01–04](../../cantos-cmp-mobile/references/kotlin-conventions.md) owns the language
examples; [CMP architecture](../../cantos-cmp-mobile/references/cmp-architecture.md) owns SOLID,
layer mapping and effect ownership. The current `reducePlayback` returns state/ordered effects,
including unchanged-state rejection; its controller interprets them in a separate ordered worker.
Provider-oriented examples below apply to external producer workflows under ADR 0018, never
authorization to restore backend synthesis jobs.

```text
raw input → parse → validate → typed facts
                                   │
                      decide / plan / reduce / fingerprint
                                   │
             decision value · next state · effects as data
                                   │
                           imperative shell
                 (persist, call provider, upload, respond, retry)
```

The **core** owns validation, business rules, state transitions, gate decisions, conflict
resolution, retry classification, cost arithmetic, fingerprints and planning.

The **shell** owns HTTP, PostgreSQL, object storage, providers, audio tools, clocks, randomness,
ID generation, environment, logging, lease heartbeats and the execution of effects.

The test is simple: a domain rule is callable from a plain `#[test]` with no async runtime,
database, provider, fixture server or mock.

For new behavior, complete [L1–L3](verification-strategy.md#layered-verification) before wiring
the shell: types and pure decisions, then exact unit/table/regression tests and applicable laws.
Kotlin uses plain `commonTest` decisions; Leptos keeps the same rules outside components. An
existing-shell extraction first preserves behavior with characterization tests below. Missing
infrastructure never requires moving a rule into an adapter; use the
[local handoff policy](local-execution.md#local-handoff-and-pr-completion) to deliver its verified core.

## Pass facts, not services

```rust
// Illustrative. The shell resolved every fact; the core only decides.
pub fn decide_retry(
    failure: &ProviderFailure,
    attempts_so_far: AttemptCount,
    policy: &RetryPolicy,
    jitter: JitterSample,
) -> RetryDecision {
    match failure.class() {
        FailureClass::Transient if attempts_so_far < policy.max_attempts() => {
            RetryDecision::RetryAfter(policy.backoff(attempts_so_far, jitter))
        }
        FailureClass::Transient => RetryDecision::GiveUp(GiveUpReason::AttemptsExhausted),
        FailureClass::OutcomeUnknown => RetryDecision::Reconcile,
        FailureClass::Permanent(reason) => RetryDecision::GiveUp(GiveUpReason::Permanent(reason)),
    }
}
```

The shell samples the clock and RNG, reads the attempt count from PostgreSQL and executes the
decision. The core never sleeps, never reads `SystemTime::now()`, never calls `rand`.

## Cantos decisions that belong in a core

| Decision | Inputs (facts) | Output (data) |
|---|---|---|
| Validate a script revision | parsed Script IR draft | `Result<ValidatedRevision, Vec<ScriptIssue>>` |
| Plan partial regeneration | previous render manifest, new revision, casting | `RegenerationPlan` (dialogues to synthesize, scenes to remix) |
| Compute a speech fingerprint | effective text, voice/provider/model revision, settings | `SpeechFingerprint` |
| Classify a provider failure | typed provider outcome | `RetryDecision` |
| Reserve budget | budget, reservations, estimate | `Result<Reservation, BudgetBlocked>` |
| Decide publication | rights, QC, approvals, asset verification | `Result<PublishableRelease, Vec<PublicationBlocker>>` |
| Reconcile listener progress | server progress + revision, local candidate | `ProgressOutcome` |
| Reduce player state | `PlayerState`, `PlayerEvent` | `(PlayerState, Vec<PlayerEffect>)` |

Not every function needs this. Thin CRUD, adapters that only map types and one-shot glue stay
as they are.

## Return decisions as data

Return what should happen; let the shell do it. An enum of outcomes is usually clearer than a
generic effect framework:

```rust
// Illustrative.
pub enum StepDecision {
    Synthesize(SynthesisPlan),
    ReuseArtifact(ArtifactId),
    WaitForReconciliation(AttemptId),
    Blocked(StepBlocker),
}
```

Do not build a shared `Effect`/`Command`/`Event` framework speculatively. A typed decision per
use case, interpreted by the one shell that owns it, is enough until real duplication appears.

## Make rejection transactional

An `Err` must leave no partial mutation — in memory or in PostgreSQL.

- Validate every fallible condition and build the accepted value before mutating anything.
- Compute an immutable decision first, then commit it in one transaction.
- Build a candidate state and replace the original only after validation succeeds.

Never "mutate and roll back" in application code. Test every rejection class by comparing the
full state (or canonical bytes, or the database rows) before and after.

A pure decision does not remove time-of-check/time-of-use races. The shell must revalidate the
facts under the right guard at commit: a conditional `UPDATE … WHERE revision = $expected`, a
lease token check, or a row lock. See [`persistence.md`](persistence.md).

## Total and deterministic

- Exhaustive `match` over closed enums; no `_ =>` arm that hides a new variant.
- Structured errors for caller-controlled input; no `unwrap`, `expect`, unchecked indexing,
  `unreachable!` or panics in core paths.
- Logical time, IDs and randomness are parameters.
- No output-affecting `HashMap` iteration order, no floats in canonical decisions or digests, no
  hidden global state, no order-dependent parallel reductions.
- Local `mut` inside a function is fine when it is encapsulated and invisible until return.
  Purity is about effects, not about avoiding `mut`.

## UI state follows the same rule only where a durable rule exists

| Worth a pure core | Leave as ordinary local state |
|---|---|
| player state and user intent, sleep timer, resume prompt | a menu's open flag |
| script editor draft, dirty/saving/conflict states | a controlled input's text |
| production progress projection, regeneration scope preview | hover and pressed visuals |
| download lifecycle, progress outbox | scroll position, focus within a list |

Where a rule exists, write `reduce(state, event) -> (state, effects)` in plain Rust (Web) or plain
Kotlin (CMP) with no framework types, and keep the component a thin interpreter.

## Extraction sequence

Worked before-and-after recipes for a handler, a validate-then-commit change, a worker loop and a
UI component: [`extraction-recipes.md`](extraction-recipes.md).

1. Freeze current behavior with characterization tests if coverage is weak.
2. Name the facts the rule consumes and the decision it produces.
3. Introduce input, output and error types without moving behavior.
4. Extract the pure function; keep the existing shell as its only caller.
5. Add exact semantic tests and the relevant law ([`property-and-differential-testing.md`](property-and-differential-testing.md)).
6. Delete mocks that no longer assert an adapter contract.
7. Move modules only after behavior is stable.

Do not combine extraction with an API redesign, dependency upgrade or unrelated cleanup; each
independent axis makes a regression harder to localize.

## Completion checklist

- The rule has one owner and a named invariant.
- The core takes values, not services, and returns a decision.
- Rejected input leaves state exactly as it was.
- I/O happens only after the core returns.
- No framework, SQL or provider type appears in the core's signature or imports.
- Tests assert behavior and laws, not mock call counts.
- Core tests ran before new I/O wiring; shell/DB acceptance is recorded separately.
