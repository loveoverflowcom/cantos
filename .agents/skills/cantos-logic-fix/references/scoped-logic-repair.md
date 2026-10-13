# Scoped logic repair and delivery

Contents: [repair seam](#repair-at-the-invariant-owner), [verification](#verification-scope-and-progressive-delivery),
[escalation](#escalation), [handoff](#handoff-contract).

## Repair at the invariant owner

Write the plan as invariant → failure mode → cause evidence → data flow/owning seam → minimal
change → failing regression oracle → affected consumers. Different failures share a fix only
with evidence of a common cause. Prefer the existing function/type/state owner and no unrelated
refactoring or dependency upgrades. Select the owner's method rather than copying its rules:

| Defect or seam | Method owner |
|---|---|
| business decision buried in a handler/component/controller | [functional core](../../cantos-engineering/references/functional-core.md), [extraction recipes](../../cantos-engineering/references/extraction-recipes.md) when extraction is needed |
| invalid identity/state or constructor/decoding bypass | [types](../../cantos-engineering/references/types-as-proofs.md), [boundary hardening](../../cantos-engineering/references/boundary-hardening.md) |
| state replacement, accepted revision or rejected partial mutation | [immutability](../../cantos-engineering/references/immutability.md), [transactional rejection](../../cantos-engineering/references/functional-core.md#make-rejection-transactional) |
| exact errors, exhaustive matching or panic on input | [clean code](../../cantos-engineering/references/clean-code.md), owning domain error contract |
| reducer laws, event sequences or independent regression oracle | [verification strategy](../../cantos-engineering/references/verification-strategy.md), [property/differential testing](../../cantos-engineering/references/property-and-differential-testing.md) |
| SQL, guarded writes, idempotency, transactions or concurrency | [persistence](../../cantos-engineering/references/persistence.md), the domain owner; [read-performance acceptance](../../cantos-engineering/references/postgresql-read-performance.md) when relevant |
| request validation, authorization or response error mapping | [HTTP boundary](../../cantos-engineering/references/http-api-boundary.md), the existing application/domain owner |
| Leptos async state or effect shell | [Leptos async](../../cantos-leptos-web/references/leptos-async.md), [Web architecture](../../cantos-leptos-web/references/web-architecture.md) |
| Kotlin reducer/controller/concurrency | [Kotlin conventions](../../cantos-cmp-mobile/references/kotlin-conventions.md), [CMP architecture](../../cantos-cmp-mobile/references/cmp-architecture.md), [CMP testing](../../cantos-cmp-mobile/references/cmp-testing.md); load all three |

For a pure-domain defect, make the decision callable from the actual pure core with typed facts
and explicit state/error/effects as data. Keep I/O, clocks, IDs, randomness and effect execution
in the existing shell; do not force extraction on a thin mapping adapter. Check immutable value
replacement, ownership and exhaustive handling through the owner above. A rejected operation's
full state and effects must match the contract; do not validate only `is_err()` or mock calls.

The failing test must exercise production code, assert an independently specified exact state
or error and demonstrate the defect before the patch. Do not modify the expected output merely
to make the patched implementation pass. Run the focused case after the minimal repair, then
applicable edge classes and properties with their seed/bounds and independent oracle recorded.
Foundation exemptions require their stated evidence; they are not permission to skip a logic
regression. Do not replace domain tests with mocked integration.

A pure decision does not close a time-of-check/time-of-use race. For a database defect, trace
the transaction and commit guard, then use the persistence owner and a disposable **real
PostgreSQL** regression with controlled interleavings and row assertions. No arbitrary sleeps,
unbounded retries or in-memory success can prove isolation, locking or atomic commit semantics.
If that regression actually fails, repair it or report `FAIL`; unavailable infrastructure only
explains an unrun acceptance check.

## Verification scope and progressive delivery

L1–L5 are independent **claim scopes**, not five mandatory execution steps or new evidence
levels. Select applicable gates through the [foundation strategy](../../cantos-engineering/references/verification-strategy.md)
and the owner; preserve the foundation vocabulary and acceptance requirements.

| Layer | Scope to report |
|---|---|
| L1 — Architecture & Abstraction | invariant owner, dependency/data-flow boundary, concrete consumer and minimal scope |
| L2 — Domain, Types & Error Handling | exact contract, typed construction/errors, exhaustive mapping, immutable state and rejection atomicity |
| L3 — Unit / Property / Regression Tests | smallest failing-before/passing-after regression; applicable table, property or state-sequence checks |
| L4 — Application & Mocked Integration | production modules exercised together, named doubles and the real application/error/effect flow |
| L5 — Real I/O, DB, API, Device / Runtime | actual PostgreSQL/service/runtime/target, controlled race/commit checks and remaining acceptance |

For each layer state applicability, result, command/oracle, evidence level and limitation.
Report executed gates as printed: `PASS`, `FAIL`, `NOT_RUN`, `UNSUPPORTED`. For irrelevant scope,
explain why it is not applicable; never invent a passing command. Keep unit, mocked and real
evidence distinct, and name selected/executed/skipped tests. Zero selected tests is not a pass.

When relevant pure-core invariants/tests actually pass and required I/O cannot run, deliver that
named core result as `CORE_VERIFIED / IO_ACCEPTANCE_PENDING`. List the actual core evidence and
the precise deferred I/O scenario/owner/blocker. Do not hold a requested commit/push/PR solely
for an absent database, server or emulator; mark verification incomplete where a required local
gate could not run. Never call database concurrency pure-domain verified, a host test
device-tested, or a partial result full-feature verified/production-ready. An actual failing
regression stays `FAIL`, not deferred acceptance.

Before a requested commit/PR run focused checks, format/lint touched toolchains,
`cargo xtask verify repository` and `cargo xtask preflight --changed-only`. Follow
[local handoff](../../cantos-engineering/references/local-execution.md#local-handoff-and-pr-completion)
for missing gates and the narrow docs-only fallback. Do not wait/poll for remote CI. This method
grants no commit, push, upload, PR or other outward-action authority.

## Escalation

Report large architecture changes, database migrations/breaking API, authorization/security
model changes, important publication/rights/durability semantics, multiple independent defects
or UX/domain-model redesign to [cantos-work-item](../../cantos-work-item/SKILL.md) and the owning
skill. State evidence, impact, proposed next boundary and required acceptance before expanding.
A bounded transaction/concurrency repair still composes persistence and real acceptance; a
small diff is not permission to change a security/durability contract.

## Handoff contract

Emit one short report; populate every field, using unknown or not observed instead of an
unsupported claim. Include the [foundation fields](../../cantos-engineering/SKILL.md#9-completion-report)
in Evidence once, with relevant [Web](../../cantos-leptos-web/SKILL.md#web-report-additions) or
[CMP](../../cantos-cmp-mobile/SKILL.md#evidence-report-additions) additions when their seam changed.

```text
Issue / Input source:
Surface / Module:
Observed:
Expected:                  owning rule/contract
Root cause:                confirmed + evidence | hypothesis + confidence | unknown
Invariant:
Owner / boundary:
Scope:                     defect, affected consumers, exclusions/escalation
Files changed:
Verification:
  L1:                      applicability · result · evidence level · oracle/command
  L2:
  L3:
  L4:
  L5:
Evidence:                  exact commands/results; source/build/dirty state; fixtures
                           live/mocked dependencies; failing/passing regression; edge classes
                           exact state/errors; rejection checks; seed/bounds when applicable
Deferred acceptance:       required real boundary/target/scenario and blocker; no FAIL hidden here
Residual risks:            what remains unproven and next proportional action
Commit / PR:               requested publication result or not requested
```
