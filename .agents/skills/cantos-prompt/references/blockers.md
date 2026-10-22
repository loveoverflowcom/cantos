# Blocker branches and deferred work

> **Scope.** How a prompt (and an agent receiving one) keeps working when an environment,
> dependency, contract or decision is missing, and how the gap is recorded. Delivery and
> evidence rules stay with the foundation's
> [local handoff](../../cantos-engineering/references/local-execution.md#local-handoff-and-pr-completion)
> and [independent gates](../../cantos-engineering/references/verification-strategy.md#independent-verification-gates);
> this file only classifies the blocker and names the branch to take.

## Two different environments

The session that reads the repository and writes a prompt (a ChatGPT sandbox, a Codex or Claude
session, a reviewer's laptop) is not the coder's machine (a Mac mini, Linux host, worktree or
CI). Only measurements taken on the target are evidence about it; until then
`TARGET_ENVIRONMENT=UNKNOWN`. The coder checks the checkout, the toolchain and tools the task
needs, free disk for target/cache/temp and the dependencies nearest the task before any heavy
step. Do not ask for PostgreSQL or Podman probes for a pure-domain change or a small UI fix.

## Decide by cause

| Situation | Continue now | Record exactly |
|---|---|---|
| real infrastructure available and the task needs it | real adapter plus a disposable, scoped integration run | the scope and the actual result |
| external service missing, contract clear | core, application and the real adapter where it can be written; contract tests with doubles at the boundary | real-I/O acceptance pending; runtime returns typed unavailable/pending |
| upstream not accepted, contract stable | the consumer on that contract; success/failure fixtures only in tests; split out the integration part | contract version and owner, assumptions, integration condition |
| contract incomplete, technical choice reversible | propose the smallest interface, isolate the boundary, continue work that needs none of the missing semantics | the provisional contract is not an agreed one |
| product or security rule unresolved and outcome-changing | finish independent work first, then ask the exact missing decision | no invented authorization, email linking or durability semantics |
| Kani, SDK, browser or device missing | write the harness, test, UI or code within reach; run the independent checks available | which tool check and which runtime/proof did not run |
| low disk or missing toolchain | light checks; small source edits when writing is safe; fewer concurrent lanes; existing configuration | no RAM-only compile promise; no toolchain/image download; no deleting other people's caches |
| disk not writable at all | review, text patch or a concrete resume plan | files not saved, builds not run; nothing presented as complete |
| a real test `FAIL` | diagnose regression versus pre-existing versus environment with evidence ([`cantos-diagnostics`](../../cantos-diagnostics/SKILL.md)); fix in scope; continue independent work | the `FAIL` unchanged, never relabeled skipped |
| GitHub or the issue unreachable | work from the data at hand; discovery-first prompt | the source gap and `UNKNOWN`; no inferred status |
| CI not run or not green | finish the requested source/PR delivery with local evidence | no merge or production promise; no waiting on or re-running CI by default |

A missing environment never turns the runtime into a mock. A test-only in-memory fake proves no
durability, locking, transaction, filesystem or concurrency semantics of a real database; a test
mailbox sink proves no email delivery; an auth fixture creates no real permission.

When the user accepts code with broken seams, name every target that will not build or run. The
review can still be handed off with acceptance missing; never repair it with fake success or by
hiding a failure. An unaccepted product dependency also follows the work item's
[prototype rule](../../cantos-work-item/references/planning-and-slicing.md#prototypes-before-a-gate).

## Deferred ledger

One record per gap, with a local ID such as `D-001` or a real issue that was read. Never create
an issue to fill the ledger. The record feeds the PR's outstanding-check list required by the
[progressive delivery policy](../../cantos-engineering/references/local-execution.md#progressive-commit-push-and-pr);
it adds no new status.

| Field | Content |
|---|---|
| id / kind | implement, integrate, verify or deploy |
| scope / evidence | symbol, path or contract, and the evidence of why it is blocked |
| can_continue | independent work that can proceed now |
| current_behavior | what the runtime returns; whether the unwired path is reachable |
| test_double | location and isolation, or none; a fake is never called live |
| resume_when | the specific dependency, tool or contract needed |
| next_step | a verified command or a discovery step; no invented syntax |
| acceptance / risk | what remains unproven and its consequence |
| owner | the real owner, or `unassigned — assignment required before acceptance` |

Prefer the PR checklist when only the environment is missing. A source comment marks a gap only
where code really has an unfinished boundary, and follows the foundation's
[clean-code rule](../../cantos-engineering/references/clean-code.md) (no TODO without an issue
link). Do not scatter markers through finished logic because a test could not run.

## No new macros by default

Do not generate `deferred_dependency!`, `fallback_infra!`, `strict_production_check` or similar
unless the task needs such a mechanism and the coder verified its design in the repository. An
opt-in feature does not guarantee production enables it; `compile_error!` probes no network or
database; Cargo feature unification can change the built combination. Reuse the existing
composition boundary or gate. Never fill unfinished work with `allow(dead_code)`, ignored tests
or fixture success.
