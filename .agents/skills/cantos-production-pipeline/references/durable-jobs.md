# Durable jobs

> **Design history (ADR 0018).** This describes the PostgreSQL job runtime (leases, fencing, outbox, recovery) of the removed synthesis context. Cantos no longer runs this; do not implement it in this
> repository. It remains the discipline an external producer follows, and a checklist for any
> future component that must survive crashes.

> **Scope.** Build Cantos production runs, steps and provider attempts as durable PostgreSQL
> state: freezing inputs into a witness, three separate state machines, the pure decisions
> (`decide_next_step`, `classify_failure`, `backoff_delay`), a transactional claim with an
> expiring lease and a fencing token, heartbeats, write-once acceptance, outcomes recorded as
> facts, a recovery sweep, the transactional outbox and cancellation. Use when adding or
> reviewing a worker, a job table, a claim/heartbeat/accept query, a retry policy, or cancel,
> resume and replacement behavior.

The product rules live in [pipeline § Freeze production inputs](../../../../docs/product/production-pipeline.md#freeze-production-inputs),
[§ Jobs and failure recovery](../../../../docs/product/production-pipeline.md#jobs-and-failure-recovery)
and [architecture § Data and delivery](../../../../docs/architecture/overview.md#data-and-delivery),
which calls the lease/fencing/outbox model "a proposal to validate in implementation". This
reference is the method for validating it. Every table, column, type and statement below is a
**proposal**: discover the real migrations and modules first and adapt the names.

## 1. Create a run from frozen inputs

A run never reads a draft, a "latest" row or a mutable casting table after creation. It reads one
immutable input document whose type proves that freezing succeeded.

```rust
// Illustrative and proposed. Fields are private: only `freeze_production_inputs` builds one.
pub struct FrozenProductionInputs {
    script: PinnedScriptRevision,      // cantos-script-ir: immutable revision + verified digest
    lines: Vec<FrozenLine>,            // per dialogue: SpokenContent, voice, resolved controls
    cues: Vec<FrozenCue>,              // asset revision, checksum, anchor, gain and fades
    profiles: FrozenProfiles,          // pronunciation, mix, mastering and output revisions
    rights: RightsEvidenceRefs,        // what cantos-publication's evaluator accepted
    budget: BudgetPolicy,
    approval_policy: ApprovalPolicyRevision,
    digest: RunInputDigest,
}

pub fn freeze_production_inputs(
    facts: FreezeFacts,
) -> Result<FrozenProductionInputs, FreezeFindings>;
```

- **Collect every finding.** `FreezeFindings` is a non-empty, sorted list (`VoiceUnresolved`,
  `ControlUnsupported`, `RightsNotCovered`, `CueAssetUnresolved`, `BudgetUnset`, …) so Studio
  shows all blockers at once. Rights are judged by the evaluator in
  [`rights-and-provenance.md`](../../cantos-publication/references/rights-and-provenance.md),
  never re-implemented here; capability findings come from
  [`provider-adapters.md`](provider-adapters.md#2-capabilities-are-data-checked-before-any-paid-call).
- **Persist once.** Insert the canonical input document, its digest and the run row in one
  transaction. Revoke `UPDATE`/`DELETE` on the input table from the application role, or add a
  rejecting trigger, and prove it with an `integration-tested` attempt to update a row.
- **Reload through a validating conversion.** `TryFrom<FrozenInputsRow>` recomputes the digest
  and returns `FrozenInputsError::DigestMismatch` instead of trusting the stored hash
  ([`boundary-hardening.md`](../../cantos-engineering/references/boundary-hardening.md)).
- **Replace, never edit.** A revised script, recast or fixed provider choice creates a new run
  with `replaces: Some(previous_run)`. Unchanged lines are reused through fingerprints
  ([`fingerprints-and-invalidation.md`](fingerprints-and-invalidation.md)), not by copying step
  rows between runs. The previous run keeps its history.

Oracle: an edit-during-run test saves a new draft after freezing and asserts the run's input
digest and every step fingerprint are byte-identical; a module-dependency check (proposed) shows
the worker never imports the draft repository.

## 2. Three state machines, one meaning each

| Machine | States | Changes how | Owner |
|---|---|---|---|
| run execution | `queued`, `running`, `waiting_review`, `paused`, `failed`, `cancelled`, `completed` | `evolve_run` transition function only | this skill |
| step execution | `pending`, `leased`, `succeeded`, `failed`, `cancelled` | fenced SQL (§ 4) | this skill |
| provider attempt | intent → observations → effective outcome | append-only rows, no status column | this skill |
| editorial review of the script | — | — | [`cantos-script-ir`](../../cantos-script-ir/SKILL.md) |
| QC results | bound to an artifact checksum | append-only rows | [`audio-mix-and-qc.md`](audio-mix-and-qc.md) |
| approvals, publication gate, release | — | — | [`cantos-publication`](../../cantos-publication/SKILL.md) |

The state names are the pipeline document's suggested sets. In Rust, give variants their data so
invalid combinations cannot exist; in PostgreSQL, map them to a status column plus reason columns
guarded by a `CHECK` that a reason is present exactly when the status needs one.

```rust
pub enum RunStatus {
    Queued,
    Running,
    WaitingReview,
    Paused(PauseReason),
    Failed(RunFailure),
    Cancelled,
    Completed,
}

pub enum PauseReason {
    BudgetExhausted { needed: Money, available: Money },
    UncertainAttempt { attempt: AttemptId },
    BlockingFinding { finding: FindingId },
    CreatorRequested { by: ActorId },
}
```

```text
queued ──first claim──▶ running ──all steps succeeded──▶ waiting_review ──recorded──▶ completed
running ──budget · uncertain attempt · finding · creator──▶ paused ──explicit resume──▶ running
running ──non-retryable failure──▶ failed ──explicit retry, inputs still valid──▶ queued
queued · running · paused · waiting_review · failed ──cancel──▶ cancelled
cancelled and completed are terminal: continue with a replacement run.
```

`waiting_review` means execution waits on a human record owned elsewhere; the review outcome is
never written into the execution status. A `completed` run is not an approved render, and a
`succeeded` step is not a QC pass. Test `evolve_run` with an exhaustive state × event table with
no wildcard arm (`statically-checked` plus `example-tested`).

Gap to carry: the documented step states have no "awaiting reconciliation". This proposal
releases such a step to `pending` with a non-null `blocked_on_attempt` column that the claim
query excludes, lets the other steps continue, and pauses the run with
`PauseReason::UncertainAttempt` once nothing else is runnable. That adds a column, not a state;
if a step-level state proves necessary, update the pipeline document first.

## 3. The pure core

The shell resolves every fact, including one clock instant and a jitter sample, then calls pure
functions callable from a plain `#[test]` ([`functional-core.md`](../../cantos-engineering/references/functional-core.md)).

### `decide_next_step`

After claiming a step lease, the shell loads the run facts scoped to that step and asks what to
do next.

```rust
pub struct RunFacts {
    pub run: RunStatus,
    pub cancellation: Option<CancelRequest>,
    pub step: StepFacts,               // the leased step: kind, fingerprint, attempt count
    pub attempts: Vec<AttemptFacts>,   // its append-only history, effective outcomes resolved
    pub reuse: ReuseDecision,          // from fingerprints-and-invalidation.md
    pub budget: BudgetFacts,
    pub retry_policy: RetryPolicy,
    pub now: Timestamp,
}

pub enum StepDecision {
    Cancel,
    Reuse { artifact: ArtifactId },
    Reconcile { attempt: AttemptId, plan: ReconciliationPlan },
    AwaitResolution { attempt: AttemptId },
    Fail(StepFailure),
    WaitUntil(Timestamp),
    Pause(PauseReason),
    Dispatch(DispatchPlan),            // includes the reservation amount to request
}

pub fn decide_next_step(facts: &RunFacts) -> StepDecision;
```

Precedence is the contract; write it as a table and test it as one:

| Order | Fact | Decision | Why it outranks what follows |
|---|---|---|---|
| 1 | cancellation recorded | `Cancel` | cancellation forbids every new piece of work |
| 2 | `ReuseDecision::Reuse` | `Reuse` | reuse sends nothing and spends nothing |
| 3 | an attempt with no definitive outcome | `Reconcile` or `AwaitResolution` | never repeat a call that may have been paid |
| 4 | last failure non-retryable, or attempts exhausted | `Fail(recovery action)` | retries are bounded |
| 5 | backoff not yet elapsed | `WaitUntil` | spacing comes from the jittered schedule |
| 6 | the reservation would exceed the budget | `Pause(BudgetExhausted)` | retries cannot bypass limits |
| 7 | otherwise | `Dispatch` | — |

The relevant facts reduce to seven two-valued dimensions: 128 combinations. Enumerate all of them
and compare against a hand-written table of expected decisions — a bounded exhaustive check that
beats sampling ([`verification-strategy.md`](../../cantos-engineering/references/verification-strategy.md)).
The budget answer in row 6 is advisory: the conditional reservation update in
[`cost-and-budget.md`](cost-and-budget.md#3-reserve-in-the-same-transaction-as-the-attempt) is
authoritative, and a lost race maps back to `Pause(BudgetExhausted)` after re-reading facts.

### `classify_failure` and bounded backoff

Adapters map vendor errors into provider-neutral `AttemptFailure` classes
([`provider-adapters.md`](provider-adapters.md#3-classify-the-send-boundary-honestly)); the core
decides what happens next. No wildcard arm: a new class must force a decision.

```rust
pub fn classify_failure(
    failure: &AttemptFailure,
    attempt: AttemptNumber,
    policy: &RetryPolicy,
    jitter: Jitter,
) -> RetryDecision {
    use RecoveryAction::{FixInput, Investigate, Recast};
    match failure {
        AttemptFailure::Ambiguous(_) => RetryDecision::Reconcile,
        AttemptFailure::InvalidInput(defect) => RetryDecision::Stop(FixInput(defect.clone())),
        AttemptFailure::Unsupported(finding) => RetryDecision::Stop(Recast(finding.clone())),
        AttemptFailure::Permanent(reason) => RetryDecision::Stop(Investigate(reason.clone())),
        AttemptFailure::NotSent(_)
        | AttemptFailure::Transient(_)
        | AttemptFailure::MalformedAudio(_) => retry_within_bound(attempt, policy, jitter, None),
        AttemptFailure::RateLimited { retry_after } => {
            retry_within_bound(attempt, policy, jitter, *retry_after)
        }
    }
}

fn retry_within_bound(
    attempt: AttemptNumber,
    policy: &RetryPolicy,
    jitter: Jitter,
    provider_floor: Option<Duration>,
) -> RetryDecision {
    if attempt >= policy.max_attempts() {
        return RetryDecision::Stop(RecoveryAction::RetryLimitReached { attempts: attempt });
    }
    let delay = backoff_delay(attempt, policy, jitter);
    RetryDecision::RetryAfter(provider_floor.map_or(delay, |floor| delay.max(floor)))
}

/// Equal-jitter exponential backoff: half of the ceiling is fixed, half comes from `jitter`.
pub fn backoff_delay(attempt: AttemptNumber, policy: &RetryPolicy, jitter: Jitter) -> Duration {
    let doublings = (attempt.get() - 1).min(policy.max_doublings());
    let ceiling = policy.base().saturating_mul(1 << doublings).min(policy.cap());
    let half = ceiling / 2;
    half + jitter.scale(half)
}
```

`AttemptNumber` is non-zero, `RetryPolicy::new` rejects `max_doublings > 31` and a cap below the
base, and `Jitter` is a per-mille value the shell draws from an injected, seeded RNG. Because the
domain is small (attempts × 1 001 jitter values), walk all of it: delay ≤ cap, delay ≥ half the
ceiling, ceilings never decrease, identical inputs give identical output. The attempt counter
lives in the append-only attempt history, so a re-claim cannot reset it.

### Cancellation semantics

- A cancel request is a fact: append `cancel_request (run, actor, reason, at)` and set
  `cancel_requested_at` in one transaction; a repeated request is a no-op that returns the first.
- Cancellation fences: the same transaction moves every non-terminal step to `cancelled` and
  increments its `lease_token`, so an in-flight worker's accept fails through the ordinary fence.
  The claim query skips cancelled runs; `decide_next_step` returns `Cancel` as a second guard.
- In-flight calls finish or time out; never drop the future mid-request, because dropping it
  turns a clean outcome into an ambiguous one. Record the outcome and settle its cost.
- A late output is never accepted into the cancelled run. Whether it becomes an unaccepted,
  reusable artifact or is discarded is the documented policy decision; record which.
- `cancelled` is terminal and blocks publication of anything from the run; the gate itself
  belongs to [`approvals-and-gates.md`](../../cantos-publication/references/approvals-and-gates.md).

## 4. Claim, heartbeat, accept: the fenced protocol

A lease is not a lock. Never hold a transaction or row lock across synthesis: a dropped
connection silently releases it and a second worker repeats the paid call. Each statement below
commits on its own; the `lease_token` (fencing token) is the only thing that links them.

```sql
-- PROPOSAL: illustrative names; validate against the real schema and PostgreSQL version.
-- $1 worker id, $2 lease ttl, $3 now (database clock in production, test clock in tests).
WITH candidate AS (
    SELECT s.id
    FROM production_step AS s
    JOIN production_run AS r ON r.id = s.run_id
    WHERE r.status IN ('queued', 'running')
      AND r.cancel_requested_at IS NULL
      AND s.blocked_on_attempt IS NULL
      AND (   (s.status = 'pending' AND s.not_before <= $3)
           OR (s.status = 'leased'  AND s.lease_expires_at < $3))
    ORDER BY s.not_before, s.id
    LIMIT 1
    FOR UPDATE OF s SKIP LOCKED
)
UPDATE production_step AS s
SET status           = 'leased',
    lease_owner      = $1,
    lease_token      = s.lease_token + 1,
    lease_expires_at = $3 + $2,
    heartbeat_at     = $3
FROM candidate
WHERE s.id = candidate.id
RETURNING s.id, s.lease_token, s.lease_expires_at;
```

```sql
-- PROPOSAL: heartbeat. Zero rows means the lease is lost: stop accepting, keep recording facts.
UPDATE production_step
SET lease_expires_at = $4 + $3, heartbeat_at = $4
WHERE id = $1 AND lease_token = $2 AND status = 'leased'
RETURNING lease_expires_at;
```

```sql
-- PROPOSAL: write-once acceptance under the fence, in the same transaction as its outbox rows.
UPDATE production_step
SET status = 'succeeded', accepted_artifact_id = $3,
    lease_owner = NULL, lease_expires_at = NULL
WHERE id = $1
  AND lease_token = $2
  AND status = 'leased'
  AND accepted_artifact_id IS NULL
RETURNING id;

-- The accepted artifact must have been produced for this step's fingerprint
-- (needs UNIQUE (id, fingerprint) on artifact).
ALTER TABLE production_step
    ADD CONSTRAINT accepted_artifact_matches_fingerprint
    FOREIGN KEY (accepted_artifact_id, input_fingerprint)
    REFERENCES artifact (id, fingerprint);
```

- Zero rows from the accept is `AcceptError::LeaseLost`, `AlreadyAccepted` or `StepCancelled`;
  re-read the row to say which. Never treat it as success.
- The single `accepted_artifact_id` column plus the composite foreign key make "at most one
  accepted artifact per step, matching its fingerprint" a database fact rather than a hope.
- Take `$now` from the database clock in production (one clock for every worker) and from a
  test clock in tests, so lease-expiry tests never sleep.
- Keep the heartbeat interval at most a third of the lease TTL, and bound the provider timeout so
  a call cannot outlive the attempt record's meaning.
- `SKIP LOCKED` prevents two workers from claiming one row at the same instant; only the token
  protects against the stale worker that wakes later. Test both on real PostgreSQL
  ([`fault-injection-testing.md`](fault-injection-testing.md#6-real-postgresql-for-the-sql-claims)).

## 5. Outcomes are facts; acceptance is fenced

A stale worker's provider call really happened and may have cost money. Fence acceptance, never
the record of what happened. Observations are append-only and keyed so a retried write is
idempotent:

```sql
-- PROPOSAL: unfenced and idempotent. A stale worker still records its paid call.
INSERT INTO attempt_observation
    (attempt_id, observation_key, kind, provider_request_id, artifact_id, usage, observed_at)
VALUES ($1, $2, $3, $4, $5, $6, $7)
ON CONFLICT (attempt_id, observation_key) DO NOTHING;
```

The effective outcome is a pure fold over observations, so the recovery sweep's "uncertain" can
be superseded by a definitive result that arrives later, and contradictions surface instead of
being overwritten:

```rust
pub fn resolve_attempt(observations: &[AttemptObservation]) -> EffectiveOutcome {
    // Definitive beats uncertain; two different definitive outcomes are a Conflict finding
    // for a human, never "last write wins".
}
```

Test the fold with every ordering of uncertain, succeeded, failed and not-executed observations:
a small finite domain, so enumerate it.

## 6. Recovery sweep

When a claim takes over an expired lease (token + 1), load that step's attempts first:

1. An attempt intent with no definitive observation gets an appended
   `Uncertain(NoOutcomeAfterLeaseExpiry)` before anything else happens. An intent row means
   "may have been sent"; there is no cheaper truth.
2. A `Succeeded` observation whose artifact is uploaded and checksum-verified is accepted under
   the new token without calling the provider again.
3. Anything else goes back through `decide_next_step`, whose precedence sends uncertain attempts
   to reconciliation before any new dispatch.

Revalidate accepted artifacts on restart (checksum, availability) instead of rerunning the run.

## 7. Transactional outbox

If the next piece of work is a database row (the scene mix step after its last line is
accepted), insert it in the accepting transaction: that is already atomic. Use the outbox only
for effects outside PostgreSQL — publication indexing, cache purges, notifications.

- Insert the event row in the same transaction as the state change it announces. Events are
  immutable; only delivery bookkeeping (`claimed_by`, `claim_expires_at`, `delivered_at`,
  `delivery_attempts`) changes.
- Lease outbox rows exactly like steps. Do not hold `FOR UPDATE` across a network call.
- Consumers are idempotent by event ID: `INSERT INTO consumed_event (consumer, event_id) … ON
  CONFLICT DO NOTHING` in the same transaction as the consumer's own write; an external effect
  must itself be idempotent (purging twice is harmless; sending a notification twice is not).

## 8. Operation IDs and idempotency keys

- The shell mints an `OperationId` per attempt intent and persists it, with the provider
  idempotency key derived from it, **before** sending. A reconciling worker reuses the stored
  key; it never derives a fresh one.
- A deliberate new attempt after a definitive failure gets a new operation ID and key.
- Record the provider's documented key retention window. Past it, resubmitting with the old key
  protects nothing, so the attempt is uncertain again.
- Studio commands (start, cancel, resume, resolve, amend budget) carry client operation IDs per
  [`http-api-boundary.md`](../../cantos-engineering/references/http-api-boundary.md).

## 9. Rule cards

| Rule | Failure mode | Good | Counterexample | Oracle | Status · exception |
|---|---|---|---|---|---|
| A run reads only `FrozenProductionInputs` | in-flight audio mixes text from two revisions | worker loads the frozen document by run ID | `script_repo.latest(episode)` inside a worker | edit-during-run test; dependency check | proposed · none |
| Execution status is not editorial or publication status | a completed job reads as approved | `RunStatus` and approval records in different modules | `status = 'approved'` on the run row | type review; exhaustive match | proposed · none |
| Claim with an expiring lease and increment the token | crashed worker's step never resumes | claim SQL in § 4 | `SELECT … FOR UPDATE` held during synthesis | real PostgreSQL crash test | proposed · none |
| Every accepted-result write is conditional on the token | stale worker overwrites a newer result | `WHERE lease_token = $2` | `UPDATE … WHERE id = $1` | stale-token accept returns zero rows | proposed · none |
| Record outcomes unfenced and idempotently | real cost of a stale call disappears | § 5 insert | discarding a response after `LeaseLost` | stale worker's observation persists | proposed · none |
| No attempt counter reset on re-claim | retry bound bypassed by crashes | count from attempt history | `attempts = 0` in the claim | crash-loop scenario stays bounded | proposed · none |
| Cancelled is terminal | resurrected run publishes stale audio | replacement run | `UPDATE run SET status = 'running'` after cancel | `evolve_run` table | proposed · none |
| Never claim exactly-once synthesis or billing | false guarantee in docs and UI | "at-least-once execution, idempotent acceptance" | "exactly-once TTS" in a PR | review | manual · a provider contract that proves it, cited |

## 10. Evidence

| Claim | Oracle | Honest label |
|---|---|---|
| decision precedence | exhaustive 128-row table | `example-tested` (state the domain) |
| backoff bounds and determinism | exhaustive attempts × jitter loop | `example-tested`; `property-tested` if generated |
| outcome fold | every ordering of observations | `example-tested` |
| claim, heartbeat, fencing, write-once accept | concurrent connections on real PostgreSQL | `integration-tested` — never from an in-memory store |
| recovery after a crash at a named point | [`fault-injection-testing.md`](fault-injection-testing.md) | `fault-injected` |
| run, step and attempt interleavings | model-based state-machine test | `property-tested` over the model |
