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

[Typed errors and composition](typed-errors-and-composition.md) owns fallible flow: exact
expected outcomes in the core, typed failures through application/ports, and foreign exception
containment at concrete outer boundaries. An error value alone does not make an effectful
pipeline pure or atomic.

The test is simple: a domain rule is callable from a plain `#[test]` with no async runtime,
database, provider, fixture server or mock.

### Purity follows responsibility, not the folder

A directory name does not make code pure. An `application/` module may legitimately hold both
**pure decisions** (facts in, decision/transition out) and **use-case orchestration** that calls
ports, awaits and commits; the second is imperative shell even when it lives beside the first.
Classify each function by its dependencies and effects, not its path, and do not move every use
case into `infrastructure/` to satisfy a folder rule.

A decision or transition kernel MUST NOT read the clock, RNG, environment, database, network,
browser globals (`window`, storage, `crypto`) or a process-global counter, and MUST NOT name
Axum, Leptos, a Turso connection, `web_sys` or an SDK type in its signature. Time, generated
IDs and adapter results enter as typed facts. Two acceptable shapes for time and identifiers:

| Shape | Use when | Example |
|---|---|---|
| a pre-sampled/pre-minted value parameter | the decision needs a known number of instants or IDs | `rotate(policy, now, absolute, idle, active, used)` in [`platform/bearer/core.rs`](../../../../services/cantos-server/src/platform/bearer/core.rs) takes the instant `now` as a fact |
| an injected generator owned by the shell | how many IDs are needed depends on the decision | Theatre `finish_page_with_followup(…, next_operation_id: impl FnOnce() -> String)` in [`account_view.rs`](../../../../apps/cantos-theatre/web/src/application/account_view.rs); a test passes a deterministic closure |

Counterexample at baseline `d02b2dc8`: the same file's `finish_session` calls
`library_sync::generate_uuid_string()` directly, which reads `window.crypto` on wasm and a global
atomic counter on the host. The transition is therefore not deterministic even though the module
is documented as pure. That is a runtime follow-up, not a reason to weaken this rule.

Thin CRUD, accessors, type-mapping adapters and a local UI toggle (a menu's open flag) need no
kernel or state machine.

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

## Transition semantics

A stateful transition answers three independent questions. Its result type MUST keep them
distinguishable:

1. Was the command **rejected** (`Err`, a domain refusal), or accepted?
2. If accepted, did the owned **state change**, or is it a valid no-op?
3. Which **effects**, in which order, must the shell interpret?

```rust
// Illustrative contract, not a repository-wide framework or a required type name.
enum StateChange<S> {
    Unchanged,
    Replace(S),
}

struct Transition<S, E> {
    change: StateChange<S>,
    effects: Vec<E>, // ordered; the interpreting shell is named by the owning module
}

fn decide(state: &State, command: Command, facts: &Facts)
    -> Result<Transition<State, Effect>, DomainError>;
```

An existing closed outcome enum (`SessionCompletion`, `LibraryCompletion`, `Rotation`,
`reducePlayback`'s state plus ordered effects) is equally valid when it expresses the same three
facts. Do not rename a struct "Writer" or "State monad": the name proves no composition law.

| Rule | Level | Failure it prevents | Oracle |
|---|---|---|---|
| A shared-borrow (`&State`) input is never mutated; an owner's `&mut self` method finishes every fallible check before its first write; `Err` leaves no partial domain mutation | MUST | a refused logout half-clears state, or a refused edit leaves a moved cursor | compare the full state before/after every rejection class |
| `Unchanged` does not imply "no effects" | MUST | a valid no-op that must still flush a queued intent or re-announce status silently drops it | table test asserting effects for each no-op class |
| `Replace` does not imply that every projection changed | MUST | consumers assume a changed aggregate means their row or heading changed | projection equality test; reactive no-op test in the Web owner |
| A by-value API (`fn apply(self, …)`) states who owns the state on success, no-op **and** rejection; rejection returns the original | MUST | the caller loses the state on `Err` | rejection test asserting the returned state equals the input |
| No placeholder or `Default` observable state is swapped in to move data out of a holder (a signal, a mutex) | MUST | subscribers briefly observe an empty account or Library | review; reactive test that no intermediate value is published |
| Effects are ordered data with one named interpreter | MUST | two shells both send, or a persist runs after a send it should precede | ordered-effects assertion in the core test |
| An outcome from the outside world (timeout, network failure) re-enters as an **event** and MAY change state to display it | exception | treating "a failed request must leave state untouched" as a rule for events | the event's transition test asserts the visible failure state |

The last row is not an exception to atomic rejection: rejection concerns a *command* the
decision refuses; a reported failure is a fact the machine is expected to absorb.

`Err` with fail-closed private clearing is a contract decision, not an accident. Theatre
`AccountState::leave` clears scope and projection before its checked generation increment can
return `Exhausted`; if that ordering is intended, the owning contract must say exhaustion
fails closed by hiding private data, and a test must assert it. Otherwise compute the next
generation first. Record which one applies rather than assuming either.

## Close the mutation boundary

Consumers emit intents the owner defines; they do not receive a mutable handle to its
internals. An API such as

```rust
// Counterexample shape at baseline d02b2dc8 (apps/cantos-theatre/web/src/application/account_view.rs).
pub fn with_sync_mut<R>(
    &mut self,
    f: impl FnOnce(&mut LibrarySyncState, Option<&mut LibraryProjection>) -> R,
) -> Option<(RequestFence, R)>
```

lets every caller (runtime, Library, player, public work page) run arbitrary code against the
sync coordinator and projection while the owner can only re-run reconciliation afterwards.
Prefer typed capability methods or a closed per-owner command enum (`request_saved(work, desired,
operation_id)`), each returning the owner's outcome. The owner keeps fence admission,
reconciliation and invariant checks in one place, and tests enumerate the finite intents.

This is not a ban on mutation:

- local `mut` over owned data, private builders and `&mut self` implementation methods inside
  the owner are fine; purity is about observable effects, not the keyword;
- Leptos `.with()`/`.update()` closures in the reactive shell remain the publication mechanism
  (see the Web owner's [reactive publication rules](../../cantos-leptos-web/references/web-architecture.md#reactive-state-ownership-and-publication));
- do not replace every method with one application-wide mega `Command` enum. Group intents by
  the state owner whose invariant they touch.

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

## The imperative shell executes in a fixed order

```text
1 receive input and resolve external facts (clock, IDs, verified identity facts, rows)
2 call the pure decision / transition once
3 commit the state change at its consistency boundary (transaction, signal write, file)
4 publish only the reactive changes that actually happened
5 interpret the ordered effects; route completion events back to the same owner, fenced
```

A domain `Effect` value (data the core returns) is not `leptos::Effect` (a reactive side-effect
runner). Name domain effects for what they request (`PersistJournal`, `SendIntent`) and keep the
reactive runner for external synchronization only.

| Effect | Owner that executes it | Owning method |
|---|---|---|
| Turso/file/transaction, durable receipt and outbox | persistence adapter in the service that owns the file | [persistence](persistence.md) |
| HTTP, private Identity RPC, email, password hashing/crypto | concrete adapters; FE calls only the public Server API | [identity and sessions](identity-and-session-boundaries.md), [HTTP boundary](http-api-boundary.md) |
| browser storage, history, location, media, timers, animation frames | Web `platform/` helpers | [DOM and media interop](../../cantos-leptos-web/references/dom-and-media-interop.md) |
| reactive publication and component lifecycle | the presentation shell | [Web reactive publication](../../cantos-leptos-web/references/web-architecture.md#reactive-state-ownership-and-publication) |

Shell rules that the core cannot discharge:

- MUST NOT hold a signal borrow/guard or a database lock/transaction across a network `.await`
  or RPC; read a snapshot, release, await, then re-admit the completion.
- MUST revalidate mutable facts (revision, actor/session fence, security version, lease) at the
  commit/admission boundary; a pure decision on an earlier read does not remove TOCTOU.
- MUST preserve expected revision, actor/session identity, operation IDs and the commit outcome
  through the round trip; a completion that no longer matches its fence is ignored, not applied.
- MUST NOT assume an in-memory transition, a durable commit and a remote side effect are atomic
  together. Persist an intent before sending it when the protocol requires replay
  (for example Theatre `persist_sync_journal` before a mutation send).
- MUST NOT drop a pending durable operation because of a timeout or local storage failure; an
  unknown outcome reconciles through receipts/status before any new effect, reusing the original
  operation ID ([ERR-06](typed-errors-and-composition.md#err-06--retry-is-a-pure-decision-execution-is-an-effect)).
  Receipt replay must not repeat an effect already completed.
- MUST NOT advertise cross-service revocation as instantaneous or exactly-once unless the owning
  contract proves it ([identity boundaries](identity-and-session-boundaries.md#private-rpc-and-separated-databases)).
- Test doubles replace external dependencies only inside a test or explicitly configured
  preview lane; live mode never falls back to mock data.

Keep the v1 authentication scope where its owners put it: email/password with verification,
Web HttpOnly/Secure cookies and native opaque Bearer per
[identity auth v1](../../../../packages/contracts/identity-auth-v1.md). An implemented backend
contract does not mean a frontend consumer migrated; follow the
[Web auth migration](../../cantos-leptos-web/references/web-architecture.md#web-authentication-migration)
for that status. Deferred factors, recovery or provider migration are not prerequisites for a
pure-core test or an unrelated Web task.

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
- Time, IDs and adapter results enter as facts or a shell-owned generator; no hidden global.
- Rejected input leaves state exactly as it was; `Unchanged`, `Replace` and effects stay distinct.
- Consumers emit typed intents; no open mutable callback crosses the owner's boundary.
- I/O happens only after the core returns.
- No framework, SQL or provider type appears in the core's signature or imports.
- Tests assert behavior and laws, not mock call counts.
- Core tests ran before new I/O wiring; shell/DB acceptance is recorded separately.
