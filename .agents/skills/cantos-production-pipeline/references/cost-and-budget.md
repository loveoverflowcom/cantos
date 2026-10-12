# Cost and budget

> **Current scope (ADR 0018):** Provider token spending, reservations, charges and retries belong to the independent AI producer. Cantos stores provenance/cost claims but never makes or bills a TTS provider call. Historic examples below are for external tool implementers, not backend job code.


> **Scope.** Model production cost as distinct, append-only typed records — estimate,
> reservation, release, provider-reported usage, settlement, billing adjustment and cache reuse —
> with money as integer minor units and a currency type; reserve budget in the same transaction
> as the attempt; pause or fail with an explanation when a limit is reached; keep retries inside
> the limit; show an unavailable estimate as unavailable. Use when touching estimates, budgets,
> reservations, provider usage, billing reconciliation, cost display, concurrency limits or any
> arithmetic on money.

The product rules live in [business rules § Production, caching and costs](../../../../docs/product/business-rules.md#production-caching-and-costs)
(rules 17–18: distinguishable costs, enforced budgets, retries never bypass limits) and
[pipeline § Cost and operational visibility](../../../../docs/product/production-pipeline.md#cost-and-operational-visibility).
Budget amounts, currencies and estimate sources are undecided; [`.env.example`](../../../../.env.example)
proposes `PRODUCTION_COST_LIMIT_MINOR` and `PRODUCTION_COST_CURRENCY`, which already implies the
integer-minor-unit model below. Everything here is a **proposal**.

## 1. Seven kinds of number, seven types

A single `cost` column cannot answer "why did this run pause?" or "what did the failed retries
cost?". Each kind is its own record, and none is ever edited.

```rust
// Illustrative and proposed.
pub enum CostEntry {
    Estimate(CostEstimate),            // before execution, per step and per run
    Reservation(Reservation),          // held against the budget before dispatch
    Release(ReservationRelease),       // returned after definitive non-execution
    ReportedUsage(ProviderUsage),      // what the provider says it consumed, per attempt
    Settlement(Settlement),            // what we believe is owed for an attempt
    Adjustment(BillingAdjustment),     // later invoice or billing data; corrects by appending
    CacheReuse(CacheReuse),            // zero incremental cost; references the source artifact
}

pub enum CostEstimate {
    Known { amount: Money, basis: EstimateBasis },
    Unavailable { reason: EstimateGap },
}

pub enum SettlementBasis {
    ProviderReported(ProviderUsageId),
    ProvisionalFromEstimate,           // shown as provisional until an adjustment arrives
}
```

Reports group settled cost by the attempt's effective outcome — accepted, superseded (succeeded
but not accepted), failed, held for an uncertain attempt — so "spent on retries" is a query, not
a guess. Credits are adjustments with a negative amount and their own reason, never a reduced
settlement.

## 2. Money is integers with a currency

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Money {
    minor: i64,
    currency: Currency,               // ISO 4217 code with its minor-unit exponent
}

impl Money {
    pub fn checked_add(self, other: Money) -> Result<Money, MoneyError> {
        if self.currency != other.currency {
            return Err(MoneyError::CurrencyMismatch { left: self.currency, right: other.currency });
        }
        let minor = self.minor.checked_add(other.minor).ok_or(MoneyError::Overflow)?;
        Ok(Money { minor, currency: self.currency })
    }
}
```

- No `Add`, `Default`, `From<f64>` or cross-currency `PartialOrd`: each would need a panic or a
  silent assumption. Comparison is `checked_cmp`, returning the same mismatch error.
- The exponent comes from the currency, never from an assumption of two decimals: `VND` has 0,
  `USD` has 2. Formatting (`18.500 ₫`) is presentation, owned by
  [`cantos-ui-design`](../../cantos-ui-design/SKILL.md); the core returns `Money`, not strings.
- Provider prices are often smaller than one minor unit. Store them as
  `UnitPrice { per_million_units_minor: u64, unit: UsageUnit, currency }`, compute in `u128`,
  round estimates and reservations **up**, and convert back with a checked conversion.
- Record the billing unit exactly as the provider defines it. `Ngày mai, mình có diễn tiếp không?`
  is 34 code points and 42 UTF-8 bytes in NFC, but 42 code points and 50 bytes in NFD; an
  estimate that counts the wrong unit is wrong for every Vietnamese line. Test it with that line.
- A provider billing in another currency than the budget is refused with
  `BudgetFinding::CurrencyMismatch` unless a recorded conversion policy exists — an open decision.

Database columns are `bigint` minor units plus a currency code column with a `CHECK` against the
supported set; never `numeric` without a stated scale, and never `real`/`double precision`.

## 3. Reserve in the same transaction as the attempt

The reservation is the authorization to spend. It is taken by a conditional update that the
database serializes, in the same transaction that inserts the attempt intent:

```sql
-- PROPOSAL: illustrative names. Zero rows means the budget cannot cover this attempt.
UPDATE production_budget
SET reserved_minor = reserved_minor + $2
WHERE run_id = $1
  AND currency = $3
  AND reserved_minor + settled_minor + $2 <= limit_minor
RETURNING reserved_minor, settled_minor, limit_minor;

-- Same transaction: the reservation row, then the attempt intent that cannot exist without it.
INSERT INTO budget_reservation (id, run_id, step_id, amount_minor, currency, created_at)
VALUES ($4, $1, $5, $2, $3, $6);

INSERT INTO attempt_intent (id, step_id, lease_token, operation_id, reservation_id, created_at)
VALUES ($7, $5, $8, $9, $4, $6);           -- reservation_id is NOT NULL and a foreign key
```

- `reservation_id NOT NULL REFERENCES budget_reservation` turns "no attempt without a
  reservation" into a database fact. A local provider with no bill writes a zero-amount
  reservation so the invariant stays uniform.
- `decide_next_step` checks the budget from facts it was given; that answer is advisory. The
  conditional update is authoritative, and a lost race re-reads facts and pauses
  ([`durable-jobs.md`](durable-jobs.md#decide_next_step)).
- Retries go through exactly this path. A retry is a new attempt with a new reservation; there is
  no code path that dispatches without one.
- If several budget scopes apply (run, creator, deployment), reserve against all of them in one
  fixed lock order inside the same transaction. Which scopes exist is an open product decision.
- An unavailable estimate blocks paid dispatch with `BudgetFinding::EstimateUnavailable`, unless
  the frozen budget policy sets a per-attempt ceiling, which is then reserved. It is displayed as
  unavailable, never as zero.

## 4. What each outcome does to its reservation

| Effective outcome | Reservation | Ledger entries | Budget counters |
|---|---|---|---|
| `NotSent`, documented non-billing `RateLimited` | released | `Release` | `reserved −= r` |
| succeeded, usage reported | settled | `ReportedUsage`, `Settlement(ProviderReported)` | `reserved −= r`, `settled += actual` |
| succeeded, no usage reported | settled provisionally | `Settlement(ProvisionalFromEstimate)` | `reserved −= r`, `settled += r` |
| failed after send, billed per provider documentation | settled | as above | as above |
| uncertain | **held** until resolution | none yet | unchanged |
| later invoice data | — | `Adjustment` | `settled += delta` |

Each transition is idempotent by reservation ID: settling or releasing twice changes nothing.
An uncertain attempt keeps its money held, so a run with unresolved paid calls cannot quietly
spend the same money again on a repeat.

Actual spend can exceed the limit only when a provider charges more than was reserved for calls
already in flight. That is a fact to record, not to clamp: there is deliberately no `CHECK
(settled ≤ limit)`. Append a `BudgetOverrun` finding and pause the run. The precise invariant is:
**no dispatch is ever authorized unless its reservation fits under the limit at commit time, and
every overrun is recorded and pauses further dispatch.**

## 5. Exhaustion pauses with an explanation

```rust
pub struct BudgetExplanation {
    pub limit: Money,
    pub settled: Money,
    pub reserved_in_flight: Money,
    pub held_for_uncertain: Money,
    pub next_attempt: CostEstimate,
    pub remaining_steps: usize,
}

pub fn explain_budget_pause(budget: &BudgetFacts, remaining: &RemainingWork) -> BudgetExplanation;
```

Studio renders the numbers; it does not compute them. A rendered message might read
`Đã tạm dừng: câu thoại tiếp theo cần 18.500 ₫, ngân sách còn 12.000 ₫`, with the held amount for
uncertain calls shown separately. Pause or fail is the frozen budget policy's choice.

Raising a limit is an appended `budget_amendment (run, new_limit, actor, reason, at)` written in
the same transaction as the new `limit_minor`. It changes no synthesis input and no fingerprint.
Gap: the pipeline document freezes the production budget with the run but does not say whether a
paused run may resume under an amended limit or needs a replacement run; record the decision.

## 6. Concurrency limits

Per-provider or per-creator concurrency is a lease on a slot, not a count: a claim that counts
leased steps can be passed by two concurrent claims that see the same count. Provision N slot
rows per provider and take one with the same `FOR UPDATE SKIP LOCKED` + lease + token pattern as
steps. Rate-limit responses feed `classify_failure`, which honors `Retry-After` as a floor.

## 7. Rule cards

| Rule | Failure mode | Good | Counterexample | Oracle | Status · exception |
|---|---|---|---|---|---|
| Money is `i64` minor units plus `Currency` | float drift; VND shown with two decimals | `Money { minor: 18_500, currency: Vnd }` | `cost: f64`, `amount: 185.0` | boundary tests at `i64::MAX`; mismatch error test | proposed · none |
| Each cost kind is its own record | an estimate presented as a settled charge | `CostEstimate::Known` vs `Settlement` | one `cost` column overwritten as data arrives | type review; report groups by kind | proposed · none |
| Reserve before dispatch, in the attempt's transaction | crash leaves spend with no authorization | § 3 | reserve in one transaction, insert attempt in another | crash between them leaves neither | proposed · none |
| Retries reserve again | retry loop spends past the limit | every attempt has its own reservation | retry reuses the first attempt's reservation | property: reservations = attempts | proposed · none |
| Uncertain attempts hold their reservation | repeat spends money already possibly spent | held until resolution | release on timeout | fault-injected timeout-after-send | proposed · provider-documented non-billing |
| Unavailable is not zero | free-looking runs that are not free | `CostEstimate::Unavailable` | `unwrap_or(0)` | display test; type review | proposed · none |
| Overruns are recorded, never clamped | actual cost hidden to satisfy a constraint | `BudgetOverrun` finding + pause | `settled = min(actual, limit)` | scripted over-charge scenario | proposed · none |

## 8. Evidence

| Claim | Oracle | Honest label |
|---|---|---|
| money arithmetic is exact and rejects mismatches | table tests at boundaries | `example-tested` |
| reservations + settlements ≤ limit when reported usage ≤ reservation | random interleavings over a fake provider | `property-tested` |
| two concurrent reservations for the last budget: exactly one wins | two connections on real PostgreSQL | `integration-tested` |
| every attempt has a reservation | foreign key + insert-without-reservation test | `integration-tested` once the schema exists |
| a budget hit mid-run pauses with the right numbers | scenario in [`fault-injection-testing.md`](fault-injection-testing.md) | `fault-injected` |
| real provider usage matches its invoice | billing reconciliation against provider data | `provider-live-tested` for the sample; `production-observed` beyond it |
