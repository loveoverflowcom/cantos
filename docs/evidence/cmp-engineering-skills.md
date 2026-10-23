# CMP engineering skills audit

Date: 2026-10-13. Documentation/skills only; no production behavior, dependency, fixture, schema
or toolchain changes. Baseline: latest fetched `origin/develop` at
`4f33a901b87be770be7cf5bbdac909408570240d`, isolated branch `codex/cmp-engineering-skills`.
The original checkout/branch was preserved.

## Invariant and owners

An agent working on Kotlin must distinguish type barriers from runtime validation and executed
proof, keep domain decisions pure and effects owned, and preserve existing contracts.
Failure mode: invalid construction, stale/duplicate effects or an overstated native/proof claim.

| File / owner | Responsibility and enforcement |
|---|---|
| [CMP router](../../.agents/skills/cantos-cmp-mobile/SKILL.md) | Mandatory Kotlin/architecture/testing routing for domain/controller work, not only visible UI |
| [Kotlin conventions](../../.agents/skills/cantos-cmp-mobile/references/kotlin-conventions.md) | TYPE-01–05 / FP-01–04; KT ownership/concurrency/style; severity, applicability, Kotlin positive/negative example, oracle and legitimate exception per rule |
| [CMP architecture](../../.agents/skills/cantos-cmp-mobile/references/cmp-architecture.md) | ARC-01/02, actual layer/package mapping, SOLID-S/O/L/I/D, state/effect ownership and reviewer checklist |
| [CMP testing](../../.agents/skills/cantos-cmp-mobile/references/cmp-testing.md) | VER-01–08, independent oracles, reproducible properties/models/races, per-target evidence limits |
| [CMP tooling](../../.agents/skills/cantos-cmp-mobile/references/cmp-tooling.md) | TOOL-01; source-backed candidate evaluation with target/compiler/licence/maintenance/cost gaps; no adoption |
| [Engineering foundation](../../.agents/skills/cantos-engineering/SKILL.md) and type/core/immutability/formal references | Shared principles/vocabulary remain canonical; language-specific definitions route into CMP |
| [Code-review router](../../.agents/skills/cantos-code-review/SKILL.md) and [passes](../../.agents/skills/cantos-code-review/references/review-passes.md) | Apply TYPE/FP/ARC/SOLID/VER to non-UI Kotlin changes read-only |
| [Mobile architecture](../architecture/mobile.md), AGENTS, skill index and [preview reference](../../.agents/skills/cantos-cmp-mobile/references/compose-preview.md) | Remove contradicted README-only/no-Gradle descriptions and cite actual mock hosts/commands |

## Repository inspection

Read the mobile README, Gradle settings/shared/root/Android build configuration, source-set
inventory, platform hosts and [architecture record](../../apps/cantos-theatre/mobile/docs/architecture.md).
The project pins Kotlin 2.2.20, CMP 1.9.0, Material3 1.8.2, Gradle 8.14.3 and AGP 8.11.1;
there is no `gradle/libs.versions.toml`. Pins live directly in the build files.

Read `domain/playback/{PlaybackState,PlaybackEvent,PlaybackReducer}.kt`:

- `PlaybackSource`/`PlaybackIdentity` use private construction, validating nullable factories and
  `@ConsistentCopyVisibility`. `PlaybackSession` and `PlaybackEngineCommand` check relational
  invariants in `init`. Not every primitive is a strong identity; the new rules guide future
  scoped work, not a claim that the current app already meets every new rule.
- `reducePlayback` returns complete immutable state plus ordered effects. Rejection returns
  unchanged state and one typed rejected effect; stale callbacks are equal-state/zero-effect.
  Uint32 generation exhaustion is checked. Speed uses the corpus's finite permille set.
- `PlaybackController` is a serialized event owner **plus separate ordered effect worker** and
  observation collector. Commands do not wait for readiness. Read-only StateFlows expose state/
  notices. `AppContainer` wires independent owners and reset fences, with scoped cleanup.
- `MediaEngine` accepts complete generation/source-fenced commands and observation-only events.
  Mock adapters are not native media integrations.

Inspected reducer/controller tests, AppContainer reset tests and the Kotlin vector decoder/
harness. The shared [corpus](../../apps/cantos-theatre/conformance/playback/v1/vectors/README.md)
contains seven scenarios/51 steps with reviewed hand-authored expected states and effects.
Gradle embeds the JSON bytes after SHA-256 inventory checks; expected outputs are not generated
from an implementation. Rust and Kotlin consumers/hashes remain unchanged.

These observations are source inspection, not new test execution. The mock has no production
listener API/auth, audio, downloads, durable progress or account lifecycle. Desktop tests,
Android APK builds, simulator framework linking, native runtime and screen-reader evidence
remain different claims.

## Rule to verification map

| Rules | Cheapest enforcement / oracle | Current limitation |
|---|---|---|
| TYPE-01/03/04, VER-01 | Compiler identity/visibility/exhaustiveness; construction/import review | No compile-negative/import lint harness; runtime validity is separate |
| TYPE-02/05, VER-02/06 | Factory/boundary partitions, exact rejection/state/effects and reviewed corpus | No new Kotlin tests executed for wording-only edits |
| FP-01/02/03 | Pure signatures/imports, alias/preservation assertions and review | Review rules do not become automated gates by documentation |
| FP-04, KT-STATE/LIFETIME/CONCURRENCY, VER-05 | Controlled scheduler, ordered fake commands, delayed observations and reset/close assertions | No claim of durable idempotency, exactly-once delivery or device concurrency |
| ARC-01/02, SOLID-S/O/L/I/D | Import/owner inventory, validated mapping, shared port behavioral contract tests | Native/production adapters absent; no all-adapter substitutability claim |
| VER-03/04/07 | Generated/shrunk/replayable traces, independent bounded models and meaningful mutants when justified | Kotest/Native model/Kotlin mutation tools are candidates; Kani applies to Rust |
| VER-08, TOOL-01 | Attributable target/version/count/bounds output and explicit missing evidence | Upstream documentation is not a successful local compatibility probe |

## Routing walkthrough

Author walkthrough of [routing cases](../../.agents/skills/routing-cases.md), not an independent
agent evaluation. R10/R35–38 retain host/native and Studio scope distinctions; R39–43 exercise
Arrow restraint, constructor/copy/restore relations, retryable state/effect separation, non-UI
Kotlin review and false iOS/formal claims. The routed references supply the expected decisions.
Independent forward-testing: NOT_RUN. No permission or additional approval gate was added.

## Local execution

Working directory: isolated worktree, baseline above plus the listed documentation changes.
Rust/Cargo: `RUSTUP_TOOLCHAIN=1.87.0`; Python temporary files use `TMPDIR=/private/tmp`.

Use the already bundled Python 3.12.14 on PATH; the host default Python 3.9.6 cannot run the
repository's TOML-dependent tests. Reproduce from this worktree:

```sh
export PATH="/Users/manh.pd1/.cache/codex-runtimes/codex-primary-runtime/dependencies/python/bin:$PATH"
export RUSTUP_TOOLCHAIN=1.87.0
export TMPDIR=/private/tmp
cargo xtask verify repository
cargo xtask preflight --changed-only
```

Repository verification: **PASS**, 438 text files/15 skills and 221 Python tests, plus both
unstaged/staged whitespace checks. Initial attempts found a 303-line router (shortened to the
300-line limit), then failed under Python 3.9; the corrected Python 3.12 run passed.
Changed-only preflight's final gate summary is recorded in the PR, from actual execution.

The optional system skill-creator `quick_validate.py` helper could not start because PyYAML is
absent in both available interpreters; its result is unavailable, not passing. No dependency was
installed to replace the repository's own comprehensive frontmatter/link/reference/structure
validation, which passed. Author routing walkthrough is manual; independent agent evaluation is
NOT_RUN.

Local raw logs are under ignored `artifacts/cmp-engineering-skills/`; no generated media or private
content is committed. GitHub CI is not checked or awaited.

## Residual risk

Evidence for new standards is `documented` and manually reviewed; repository checks establish
skill/link/frontmatter/text hygiene, not application correctness. No application/dependency/test
tooling changes warrant a CMP build, native run, integration, property, model or mutation suite in
this PR. Those checks are NOT_RUN here; candidate all-target compatibility is also NOT_RUN.
No formal proof, native audio verification or iOS runtime acceptance is claimed.
Production/050 gates remain open, and GitHub CI is not checked or awaited for local handoff.
