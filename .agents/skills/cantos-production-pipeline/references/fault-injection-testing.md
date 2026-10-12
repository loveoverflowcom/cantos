# Fault-injection testing

> **Design history (ADR 0018).** This describes fault injection against the removed job runtime. Cantos no longer runs this; do not implement it in this
> repository. It remains the discipline an external producer follows, and a checklist for any
> future component that must survive crashes.

> **Scope.** Prove Cantos crash, retry, duplicate-delivery, timeout, stale-worker, cancellation
> and budget behavior instead of asserting it in prose: named fault points, a data-driven fault
> plan, a clock the test owns, a scripted provider with its own execution ledger, a simulated
> crash that keeps only durable state, real PostgreSQL for the SQL claims, deterministic
> interleavings, a scenario catalog and an invariant checker that stays independent of the code
> under test. Use when adding or reviewing a worker, a provider adapter, an upload, an outbox
> consumer, a budget path, or any claim of the form "a restart is safe".

The product rules are the [business-rule invariants](../../../../docs/product/business-rules.md#product-invariants-to-verify)
("worker dies after dispatch", "provider response is lost", "upload is incomplete") and the
[work plan's shared evidence](../../../../docs/work-plan/README.md#shared-completion-evidence),
which asks for crash and retry behavior to be shown, with mocked and live evidence labeled
separately. The label this reference earns is `fault-injected`
([vocabulary](../../cantos-engineering/SKILL.md#6-never-collapse-evidence-into-the-word-verified)).
Everything below is a **proposal**: no harness, failpoint crate or test database exists. Libraries
(a failpoint crate, proptest, testcontainers or a script that starts PostgreSQL) are candidates
needing a recorded decision and a pinned, verified version.

## 1. What fault injection proves, and what it does not

A fault-injected test says: *when this specific thing went wrong at this specific point, the
system ended in a state that satisfies these invariants.* Three limits keep the claim honest.

- It covers the points you named. An unnamed crash point is untested, so the catalog in § 8 is
  part of the evidence and a reviewer can ask what is missing from it.
- It covers the fakes and stores you ran against. A scripted provider shows orchestration, not
  synthesis; an in-memory table shows nothing about locking ([§ 6](#6-real-postgresql-for-the-sql-claims)).
- It is not a performance, scale or soak claim and not `production-observed`.

Report each scenario as *point → injection → outcome → oracle*. "We tested retries" names no
point, so it is not a result.

## 2. Name every fault point

A fault point is a place in the shell where the world can change underneath the code: between two
statements, around a network call, around a commit. Name it `area.where`, lower snake case, stable
across refactors, and keep the list next to the code that owns it so adding an effect forces an
update.

| Point | What the world does there |
|---|---|
| `claim.after_claim` | worker dies holding a fresh lease before doing anything |
| `reserve.before_commit` | the reservation-plus-intent transaction aborts |
| `dispatch.after_intent_before_send` | crash after the durable intent, before any byte left the process |
| `dispatch.after_send_before_outcome` | crash or timeout when the provider may have executed and billed |
| `dispatch.response_lost` | the provider executed and the response never arrives |
| `observe.after_outcome_before_record` | the response arrived, the process died before the observation row |
| `probe.after_bytes_before_artifact` | bytes were received, no artifact row exists yet |
| `upload.after_put_before_verify` | the object store holds the bytes, the row says nothing |
| `accept.before_commit` | the accepting transaction aborts after the fenced update |
| `accept.after_commit_before_dispatch` | crash right after commit, before the outbox is delivered |
| `heartbeat.skipped` | the worker stalls past lease expiry and wakes later |
| `cancel.during_call` | cancellation is recorded while a provider call is in flight |
| `consume.after_effect_before_ack` | an outbox consumer applied the effect and died before acking |

Publication adds its own points ([`stage.*`, `commit.*`, `republish.*`](../../cantos-publication/references/staged-publication.md#fault-injection-named-crash-points));
they use the same harness and the same naming.

## 3. The harness: a fault plan as data

Failpoints live in the shell, never in the pure core. The shell asks a small port what to do at a
named point; production binds a no-op.

```rust
// Illustrative and proposed. `Faults` is a port with one production adapter that never faults.
pub trait Faults {
    fn at(&self, point: FaultPoint) -> FaultAction;
}

pub enum FaultAction {
    Proceed,
    Crash,                    // abandon the worker task here; no cleanup runs
    Fail(InjectedError),      // the operation returns this error
    Delay(Duration),          // advances the test clock, never sleeps
    Hold(GateId),             // block until the test releases the gate (§ 7)
}

pub struct FaultPlan {
    // Sparse and explicit: which hit of which point does what.
    rules: Vec<(FaultPoint, Hit, FaultAction)>,
}
```

- **A plan is a value.** It can be printed, stored beside a failing seed and replayed. A test that
  needs "the second heartbeat is skipped" says so in one `Hit::nth(2)` rule.
- **Count hits, not time.** A rule fires on the n-th time a point is reached, so the test is
  independent of scheduling.
- **No fault-injection code in production paths by default.** Gate real implementations behind a
  `#[cfg(test)]` or an explicit Cargo feature (proposed) and add a check that a production
  configuration cannot select a faulting `Faults`. The same rule applies to scripted providers
  ([provider adapters § 7](provider-adapters.md#7-credentials-logs-fakes-fixtures-and-live-tests)).
- **Do not hide the shell behind mocks to inject faults.** A fault planted inside a mock proves the
  mock. Plant it at the same seam production crosses: the repository call, the provider call, the
  object-store call.

## 4. Simulate a crash, not an error

An error return and a crash are different events. Returning `Err` runs the caller's cleanup and
`Drop` implementations; a killed process does not. A test that "injects a crash" by returning an
error proves nothing about a half-finished effect.

```rust
// Illustrative and proposed. The worker future is dropped at the fault point; only durable state
// survives, so the restart is built from nothing but the database and the object store.
async fn crash_at(point: FaultPoint, mut scenario: Scenario) -> Restarted {
    scenario.faults.insert(point, Hit::nth(1), FaultAction::Crash);
    let outcome = scenario.run_worker_until_abandoned().await;
    assert!(outcome.abandoned_at(point), "the fault point was never reached");
    scenario.restart_with_fresh_memory()       // new worker, new caches, same database and store
}
```

- Assert that the point **was reached**. A scenario whose fault never fired silently proves the
  happy path and turns green for the wrong reason.
- The restart shares the database, the object store and the provider's ledger, and nothing else:
  fresh worker state, fresh caches, a new worker ID, the same clock.
- A provider call already issued before the crash is not undone. The scripted provider keeps
  executing its side of the story ([§ 5](#5-a-scripted-provider-with-its-own-ledger)).

## 5. A scripted provider with its own ledger

The oracle for "no duplicate paid call" must not be the production code's own record of what it
did. The scripted provider keeps its **own** ledger of what it was actually asked to execute.

```rust
// Illustrative and proposed. Test support only; the production registry rejects it.
pub struct ScriptedSynthesizer {
    ledger: Mutex<Vec<Execution>>,        // what the "provider" really did, by key and fingerprint
    script: Vec<Behavior>,                // consumed in order, one per request
    capabilities: ProviderCapabilities,   // idempotency and lookup support are configurable
}

pub enum Behavior {
    Succeed(SyntheticAudio),
    ExecuteThenLoseResponse,      // executed and billed; the caller sees a timeout
    TimeoutBeforeExecute,         // never executed
    RateLimited { retry_after: Duration },
    MalformedAudio(Truncation),
    Permanent(Reason),
}
```

- It honors idempotency keys and lookups **as configured**: with `IdempotencyKey { retention }` a
  repeated key returns the first result and adds no ledger entry; with no support, nothing does
  and the ambiguous attempt must surface for resolution.
- `ExecuteThenLoseResponse` is the case that matters. It is the only way to show that a timeout
  was not retried blindly, because only the ledger can say the call had already happened.
- Audio is synthesized in code (a known tone or silence), never captured provider output
  ([`audio-mix-and-qc.md`](audio-mix-and-qc.md#7-test-signals-are-generated-in-code)).
- The assertion is on the ledger: `ledger.executions_for(fingerprint).len() == 1`. The same shape
  works for an object store (`puts_for(key)`) and any other effect with an external side.

## 6. Real PostgreSQL for the SQL claims

Lease claims, fencing, write-once acceptance, conditional reservation, unique and foreign-key
constraints and transaction rollback are properties of PostgreSQL, not of Rust. A fake queue, an
in-memory table or SQLite can pass every one of them while the real statement races. These claims
are `integration-tested` only against a real PostgreSQL of the intended major version.

- **Isolation per test.** A fresh database or schema per test, created from the real migrations
  (a template database copy is the fast option). Never share state between tests, never depend on
  test order, and never point a test at a developer's long-lived database.
- **Real connections, really concurrent.** Use at least two connections, each in its own task, for
  claim races and the last-budget race. One connection serialized in a loop cannot race.
- **Clocks.** Pass `now` into the statement (database time in production, a test clock in tests)
  so lease-expiry tests advance a value instead of sleeping
  ([durable jobs § 4](durable-jobs.md#4-claim-heartbeat-accept-the-fenced-protocol)).
- **Assert on rows, then on behavior.** A constraint test inserts the offending row and expects the
  exact database error mapped to the exact domain error; a rollback test asserts that the failed
  transaction left **no** partial rows.
- **Resource honesty.** A container or local server costs memory and time. Check the machine first
  ([`local-execution.md`](../../cantos-engineering/references/local-execution.md#resources)),
  run these suites sequentially on a constrained machine and report an unavailable database as
  `unavailable`, not as a pass. Details of test databases live in
  [`persistence.md`](../../cantos-engineering/references/persistence.md).

The same applies to object storage: upload, conditional write and checksum behavior are
`integration-tested` against a real S3-compatible store, not an in-memory map
([storage and delivery](../../cantos-publication/references/storage-and-delivery.md)).

## 7. Deterministic interleavings

Races are reproduced by controlling order, not by repetition and luck.

- **Gates.** `FaultAction::Hold(gate)` parks a task at a point until the test calls
  `gate.release()`. The stale-worker race becomes a script: worker A claims and holds at
  `heartbeat.skipped`; the test advances the clock past expiry; worker B claims and accepts; the
  test releases A; A's accept returns zero rows and A's observation is still recorded.
- **Never `sleep`.** Wait on an observable condition (a row, a gate, a ledger count) with a timeout
  that fails loudly. A sleep is a flaky test and a slow one.
- **Small spaces are walked, not sampled.** Two workers with a handful of points have a few dozen
  interleavings: enumerate them all with a small scheduler and run the invariant checker on each.
- **Large spaces are generated.** For longer schedules, generate seeded schedules of events (claim,
  heartbeat, stall, advance clock, cancel, deliver twice), shrink the **event list**, print the
  seed and replay it ([property testing](../../cantos-engineering/references/property-and-differential-testing.md)).
  The oracle is the same invariant checker below. That is `property-tested`.

## 8. The scenario catalog

Each row is one test with a stable name. After **every** scenario, run the invariant checker (§ 9).

| Scenario | Point and injection | Required outcome |
|---|---|---|
| worker dies after dispatch | `dispatch.after_send_before_outcome`: crash | the next claim loads the attempt, records `Uncertain`, reconciles or surfaces it; the ledger shows one execution |
| response lost | `dispatch.response_lost` | no repeat before reconciliation; with no support the run pauses with `UncertainAttempt` and keeps the reservation |
| crash before send | `dispatch.after_intent_before_send`: crash | intent exists, the ledger is empty; recovery treats it as uncertain and never sends a second request under a new key |
| duplicate dispatch | the same step claimed twice in a race | exactly one accepted artifact; the loser's accept returns zero rows |
| stale worker wakes | `heartbeat.skipped`, expiry, second worker accepts, first wakes | the first worker's accept is rejected; its observation is recorded; cost settles once |
| outcome unrecorded | `observe.after_outcome_before_record`: crash | recovery finds the provider reference or ledger evidence and does not rebill |
| malformed audio | `Behavior::MalformedAudio` | the artifact is not created; the attempt is retried within the bound; its cost is settled |
| rate limit | `Behavior::RateLimited` | the next dispatch waits at least `retry_after`; the retry bound holds |
| permanent failure | `Behavior::Permanent` | no retry; a recovery action is recorded; the run is `failed`, not looped |
| crash loop | a crash at `claim.after_claim` every time | attempts are counted from history, so the bound still holds and the step ends in a recoverable state |
| budget hit mid-run | the limit falls between two lines | the run pauses with `BudgetExhausted { needed, available }`; accepted lines stay accepted; no new reservation exceeds the limit |
| retry cannot bypass budget | transient failures near the limit | each retry reserves again and is refused when the limit is reached |
| cancellation during a call | `cancel.during_call` | the in-flight call finishes or times out, its outcome and cost are recorded, nothing is accepted into the cancelled run |
| upload interrupted | `upload.after_put_before_verify`: crash | resume or restart yields one artifact with the expected checksum; no unverified artifact is accepted |
| accept aborted | `accept.before_commit` | no accepted pointer and no outbox row; a retry accepts exactly once |
| duplicate outbox delivery | `consume.after_effect_before_ack`: deliver twice | one effect; a repeated idempotent effect is harmless |
| unrelated line edited | edit one dialogue, restart | only that line and its dependent mixes are replanned; every other accepted artifact is reused |

## 9. The invariant checker is independent

After each scenario, a checker reads **durable state and the provider ledger** and asserts the
laws, written independently of the worker's code paths.

```rust
// Illustrative and proposed. Written from the business rules, not from the implementation.
pub fn assert_production_invariants(db: &DurableState, ledger: &ProviderLedger) {
    for step in db.steps() {
        assert!(db.accepted_artifacts(step).len() <= 1);                    // at most one accepted
        assert!(step.accepted_matches_step_fingerprint());                  // right line, right inputs
    }
    for attempt in db.attempts() {
        assert!(db.reservation_for(attempt).is_some());                     // reserved before sent
        assert!(ledger.executions_for(attempt.fingerprint()).len()
                <= db.billable_attempts_for(attempt.fingerprint()));        // never an unrecorded call
    }
    assert!(db.reserved_plus_settled() <= db.budget_limit());               // no overrun of the limit
    assert!(db.cancelled_runs().all(|run| db.accepted_after_cancel(run).is_empty()));
}
```

- It runs after every scenario and inside every generated schedule, so one checker guards hundreds
  of cases.
- It is itself tested: seed a deliberately corrupted state per law and assert that the checker
  names the violated law. A checker that never fails is not an oracle.
- A repeated paid call is detected by the **ledger**, not by a counter in the production code.

## 10. Hygiene, flakes and reporting

- A fault-injection test that sometimes fails is a bug in the code or the test, never a retry
  candidate. Print the seed and the fault plan on failure; do not re-run until green.
- Keep each scenario under a short bounded time; unbounded waits hide deadlocks.
- Keep the catalog small enough to run on a constrained machine, and mark slow groups so the
  default run stays focused ([`local-execution.md`](../../cantos-engineering/references/local-execution.md)).
- Report per scenario: point, injection, outcome, oracle, store used (real PostgreSQL or none),
  provider used (scripted, recorded or live) and the result. Report unrun groups as `not-run`.

## 11. Rule cards

| Rule | Failure mode | Good | Counterexample | Oracle | Status · exception |
|---|---|---|---|---|---|
| Name every fault point and assert it fired | an untested crash point; a green scenario that never faulted | `at(FaultPoint)` and a "was reached" assertion | a fault planted inside a mock | the harness fails when the point is not hit | proposed · none |
| A crash drops the task and keeps only durable state | cleanup code hides a half-finished effect | restart with fresh memory | returning `Err` to "simulate" a crash | scenario `worker_dies_after_dispatch` | proposed · none |
| The duplicate-call oracle is the provider's own ledger | the production code vouches for itself | `ledger.executions_for(fingerprint) == 1` | asserting a counter the worker increments | `ExecuteThenLoseResponse` scenario | proposed · none |
| SQL claims need real PostgreSQL | a fake passes while the statement races | two real connections racing | SQLite or an in-memory queue | the claim-race and last-budget tests | proposed · none |
| Control order, never time | flaky sleeps and missed races | gates and a test clock | `sleep(Duration::from_millis(50))` | the stale-worker script is repeatable | proposed · none |
| One independent checker after every scenario | each test asserts a different subset | `assert_production_invariants` | per-test ad hoc asserts | corrupted-state self-tests | proposed · none |
| Fakes are unreachable from production | a demo reported as provider support | registry rejects fake IDs | `TTS_PROVIDER=fake` accepted | configuration test | proposed · none |

## 12. Evidence

| Claim | Oracle | Honest label |
|---|---|---|
| a restart after each named point leaves the invariants true | the scenario catalog with the checker | `fault-injected` |
| claim, fencing, write-once accept and the budget race hold | concurrent real connections | `integration-tested` |
| no duplicate paid call under any listed fault | the provider's own execution ledger | `fault-injected` against a double; never `provider-live-tested` |
| the invariant checker catches violations | seeded corrupt states | `example-tested` |
| generated schedules never break the invariants | seeded event schedules with shrinking | `property-tested` over the model |
| all interleavings of two workers over a small set of points | bounded enumeration | `example-tested` (state the bound) |
