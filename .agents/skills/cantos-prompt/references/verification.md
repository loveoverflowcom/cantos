# Verification in a prompt

> **Scope.** How a prompt selects proportional checks and asks for honest evidence. The
> foundation owns the layers, vocabulary, risk tiers, cadence and registered tooling; a prompt
> cites them and never builds a competing gate system. Keep planned checks, written test source
> and passing runs apart.

## Owners to cite

| Need | Owner |
|---|---|
| L1–L5 obligations and the independent gate ledger | [verification strategy](../../cantos-engineering/references/verification-strategy.md#layered-verification) |
| DEV, CHECKPOINT and HANDOFF timing, reuse and invalidation | [verification cadence](../../cantos-engineering/references/verification-cadence.md) |
| what may run, resources, docs-only fallback, reporting | [local execution](../../cantos-engineering/references/local-execution.md) |
| Kani selection, harness obligations and actual execution | [bounded model checking](../../cantos-engineering/references/mutation-and-formal.md#bounded-model-checking) |
| Quint models, scenarios, non-vacuity and conformance | [`cantos-behavior`](../../cantos-behavior/SKILL.md) |
| validated types and construction barriers | [types as proofs](../../cantos-engineering/references/types-as-proofs.md) |

## Evidence by scope

| Scope | Appropriate evidence |
|---|---|
| wording, docs or skills | link, structure and repository hygiene; no Kani or application build |
| pure Rust domain | meaningful unit, boundary, error or property tests; validated constructors for important rules |
| high-risk pure Rust transition | Kani when repository policy selects it: property, assumptions, bounds and the production symbol bound |
| stateful lifecycle, retry, revocation, publication | a scoped Quint model; conformance separately when a registered behavior is touched |
| Kotlin/CMP logic | deterministic Kotlin tests per module; never Kani on Kotlin |
| Web/UI wiring | state, error and session tests and the build needed; browser and accessibility at the right scope |
| database, adapter, migration, locking | tests on a real disposable instance of the target engine to accept those semantics |
| external transport | production code with boundary doubles for mock integration; real boundary acceptance separately |
| native platform or player | real device/runtime observation for platform claims; a preview never substitutes |

Add no test that only repeats the implementation or a marker string. For a bug the oracle must
expose the old failure; for a security rule, check rejected, unauthorized, replayed and stale
paths as relevant. Do not demand timeout, deadlock or disk-full coverage from a task that does
not touch them.

Ask for a newtype or typestate only where it protects a concrete invariant (a deadline, a
security version, a positive quantity, a state transition, identity versus authority). Runtime
validation is not compile-time proof; raw primitives are not banned everywhere.

A written `#[kani::proof]` is not a run; a model-check `PASS` speaks only for the model, its
assumptions and scope. Model-only, pure-core conformance and real integration are separate rows.
Do not create a model to label a color or docs change.

## Cadence and resources

Discover commands from `xtask/README.md`, the source and the relevant `--help`. Typical menu
entries, chosen by milestone rather than all run for every task:

```sh
cargo xtask doctor
cargo xtask preflight --profile local --changed-only --base "$BASE_REF" --dry-run
cargo xtask preflight --profile local --changed-only --base "$BASE_REF"
cargo xtask preflight --changed-only --base "$BASE_REF" --dry-run
cargo xtask preflight --changed-only --base "$BASE_REF"
```

Probe lightly first; `doctor` and Cargo can build or write to disk. Under low resources do not
start a heavy runner just to learn tool status. The local profile is never the full handoff,
the base is never reset to HEAD to empty the diff, and a handoff runs once per content with
valid evidence reused. A skipped gate stays `NOT_RUN`; a failure is never skipped away. Do not
invent a fast gate or orchestration script, install Kani/SDK/containers, delete global caches or
use remote CI to make up for missing local resources.

## Evidence ledger in the handoff

Per check: claim; exact command; working directory; SHA and dirty state; important features and
toolchain; actual result; output location; remaining acceptance. Never log secrets.
Code written, compile pass, unit pass, mock integration, real database, Kani, Quint and live E2E
never collapse into "verified". When nothing ran, say the code is unverified and still hand
off the work done in scope instead of holding it back.
