# Forge generation attempts: verification record (#203, first slice)

This is the source-bound first-slice record. The later
[race/readback and pinned-gate follow-up](forge-attempt-reconciliation.md) preserves these
historical results and records the subsequent fixes and current tool evidence separately.

This records what was and was not verified for the first slice of
[#203](https://github.com/loveoverflowcom/cantos/issues/203): the pure rules, Quint capability,
Kani kernel and mock-provider tests for **when a billable provider call may start**. It adds no
real provider, persistent attempt store, CLI command, Studio control, authorization or Firebase
hosting, and it claims no deployed or live behavior. Rules and layout:
[Forge README](../../apps/cantos-forge/README.md#generation-attempts).

## Invariant and owners

A provider call needs a durable reservation and one attempt makes at most one call. A retry of
the same request returns the record; a changed request under the same key conflicts. An
uncertain outcome (timeout, lost response, unwritable result, vanished owner) is never silently
replayed: a new attempt follows only an explicit regeneration whose decision names exactly that
attempt, inside the operator's attempt and spend limits.

| Owner | Role |
| --- | --- |
| [`specs/behavior/production/forge_attempt.qnt`](../../specs/behavior/production/forge_attempt.qnt), catalog capability `cantos.production.forge-attempt` (draft) | expected behavior: 7 properties, 12 named runs, 9 witnesses, 7 negative controls |
| [`domain/attempt.rs`](../../apps/cantos-forge/src/domain/attempt.rs), [`domain/generation.rs`](../../apps/cantos-forge/src/domain/generation.rs) | pure decisions, the `Ledger` integer kernel, the request fingerprint |
| [`application/produce.rs`](../../apps/cantos-forge/src/application/produce.rs) | reserve, then call once, then record, over the `AttemptStore` and `Provider` ports |
| [`adapters/mock_provider.rs`](../../apps/cantos-forge/src/adapters/mock_provider.rs) | deterministic, scriptable, no network, no spend |
| Kani kernel `forge-attempt-ledger` ([ledger](../assurance/kani.md#forge-attempt-ledger-kernel-203)) | bounded proof of the kernel |

## Evidence ledger

Base: `develop` `178def7`. Layers are independent; none stands in for another.

| Layer | Evidence level | Command | Result |
| --- | --- | --- | --- |
| L2/L3 pure core, fingerprint | example-tested, property-tested, differentially-tested (fingerprint golden computed by an independent Python implementation of the documented layout) | `cargo test -p cantos-forge --locked` | PASS: 194 library tests, 6 architecture, 3 audio-profile contract, 2 `behavior_binding`, 4 CLI and 1 doctest |
| L3 `Ledger::reserve` against a widened specification | property-tested (boundary-biased integers, exact fit, last attempt, wrapping sums) | same | PASS |
| L4 use cases with the mock provider and fault-injecting history | mock-integration-tested, fault-injected (refused, lost-response and raced writes at the reservation and the outcome; unreadable and corrupt history; unusable provider answers), property-tested over random fault schedules | same | PASS: 12 mirrors of the Quint runs and 13 native-only boundary and fault-schedule tests |
| Quint catalog and native mirrors | statically-checked traceability (`tests/behavior_binding.rs`: each registered run has exactly one `Quint run:` mirror, no unknown claim) | same | PASS |
| Quint model | example-tested (12 named runs) and sampled property/witness/control campaign, **development signal only** | `quint test` and a script reproducing the gate's simulation stage, Quint 0.33.0 from the locked `package-lock.json` (`npm ci`) | 12/12 runs; 7 properties hold over 300 samples x 20 steps; 9/9 witnesses reached; 7/7 controls produce their own violation |
| Kani kernel | bounded-model-checked, **development signal only** (not the registered model) | `cargo kani --harness domain::attempt::kani_proofs::attempt_ledger_traces_never_overspend_or_blindly_retry --exact --unwind 2 --solver cadical` in a scratch crate that includes the production `domain/` sources by path | PASS: 0 of 296 checks failed, 9/9 covers satisfied, no unreachable or failed check |
| Proof sensitivity | mutation-tested (manual, on a copy) | the same command on six deliberate defects | all six FAIL as intended (attempt limit off by one, budget bound too strict, wrapping spend, stale decision accepted, open attempt reservable, outcome settled twice) |
| Assertion strength of the Rust tests | mutation-tested | `cargo mutants --package cantos-forge` on `attempt.rs`, `produce.rs`, `generation.rs`, `mock_provider.rs`, excluding the `cfg(kani)` module, in a scratch copy | see [Mutation testing](#mutation-testing) |
| xtask registration and Python primitive | example-tested with synthetic fixtures | `cargo test -p xtask --locked`; `python3 -m unittest discover -s scripts -p test_verify_kani.py` | PASS: 404 and 107 tests. These are not a Kani execution |
| Format, lint | statically-checked | `cargo fmt --all -- --check`; `cargo clippy -p xtask -p cantos-forge --locked --all-targets -- -D warnings` | PASS |

## Provenance and tool differences

* Pinned gates are **UNSUPPORTED** on this authoring host (`cargo xtask doctor`): the Kani
  0.68.0/CBMC 6.11.0 bundle is absent (installed: Kani 0.67.0, CBMC 6.8.0,
  nightly-2025-11-21) and the pinned Node 24.19.0/npm 11.9.0 are absent (installed: 24.22.0 and
  11.19.1). Nothing was installed system-wide; Quint 0.33.0 came from the repository lockfile
  into a scratch directory. The development signals above therefore use different tool versions
  than the registered models, and **`cargo xtask verify behavior` and `cargo xtask verify kani`
  were not run**.
* Kani: the scratch crate includes `apps/cantos-forge/src/domain/mod.rs` by path, unchanged.
  `attempt.rs` SHA-256 `9068b79f21011caafb781051b5de5a12c47286f1930e02e76e84bf0383716cab`;
  `Cargo.lock` `d3ea35c31ce34cc3b979ead0e5822cfedbff0e473ce683b15e43c6bf874b65b6` (not
  modified). Full-width symbolic `u32` attempt limit, `u64` budget and costs, symbolic outcomes
  and decisions; no assumptions or stubs. A symbolic zero cost is mapped to one only after the
  factory's exact zero refusal is asserted. Four unrolled steps, unwind 2, no loops.
* Quint: model `forge_attempt.qnt` `7cad0c9f…01e4`, runs `forge_attempt_tests.qnt` `0d859a61…5e52`,
  catalog `52d89318…65dc` (`sha256sum` prefixes and suffixes), seed `203`. Dev simulation used the
  catalog's budgets (300 samples x 20 steps for properties, 2,000 samples for witnesses and
  controls).
* No raw verifier bundle is retained in Git; the gates' own evidence directories
  (`artifacts/agent-runs/`, `target/verification/behavior/`) are written only when the pinned
  gates run.

## Mutation testing

`cargo mutants` first reported four real survivors outside the unbuilt Kani module (the 32 MiB
provider limit constant, the `Display` of `AttemptNumber` and the `Debug` of
`RequestFingerprint`); each got a pinning test and the run was repeated on the final tree. The
mutants inside `#[cfg(kani)]` code are not compiled by `cargo test`, so that module is excluded
from this measurement and is covered instead by the Kani sensitivity runs above.

Final result on the final tree (`cargo-mutants` 27.1.0, `cargo mutants --in-place --package
cantos-forge --file apps/cantos-forge/src/domain/attempt.rs --file
apps/cantos-forge/src/application/produce.rs --file apps/cantos-forge/src/domain/generation.rs
--file apps/cantos-forge/src/adapters/mock_provider.rs --exclude-re kani_proofs`, run in a
scratch copy of the tree): **89 mutants, 52 caught, 0 missed, 37 unviable** (they do not compile).
Unviable mutants are not kills. The run used the default 20-second test timeout; no mutant timed
out. The measurement covers these four files only.

## Deferred acceptance (not run) and what remains

| Check | Why not run | Command and criteria | Owner |
| --- | --- | --- | --- |
| Registered Quint gate | pinned Node/npm and the locked Quint install are absent here | `cargo xtask verify behavior --setup` once, then `cargo xtask verify behavior --capability cantos.production.forge-attempt`; expect PASS with all 7 properties, 12 runs, 9 witnesses and 7 controls (the draft capability has no `--conformance` observer) | next operator with the pinned tools |
| Registered Kani gate | pinned Kani 0.68.0 bundle absent | `cargo xtask verify kani --harness forge-attempt-ledger`; expect PASS, 9/9 covers, no unreachable check (none is allowed; any compiler-inserted unreachable check must be reviewed, not silently allowed) | same |
| PostgreSQL, API, Studio, Functions, provider | none exists for this slice | n/a | later #203 slices |

Residual risk:

* No persistent `AttemptStore` exists, so the write-once, in-order contract (conditional create
  that fences concurrent invocations) is specified by the port and checked only against an
  in-memory fake. A real adapter must be tested against the real medium before any claim of
  fencing across processes or instances.
* Deciding that an open attempt's owner is gone is today only the explicit `recover` call. A
  wrong call makes the live owner's outcome write fail; the bytes it produced stay only as a blob
  under their digest (orphan handling and adopting a late result are open decisions in the
  [inventory](../../specs/behavior/inventory.md)).
* A lost reservation response leaves an open attempt that nobody can distinguish from a raced
  duplicate; the design deliberately refuses to call and requires `recover` plus an
  acknowledged regeneration. That costs an operator step; it never costs a second billed call.
* The Quint model is small (one operation, two request classes, budget 3, three attempts) and
  its campaign is sampled, not exhaustive. The Kani proof covers four steps of the integer
  kernel on a 64-bit target only, not the fingerprint, the use cases or a provider.
* The mock returns bytes that are not audio. Provider output is recorded as raw bytes; encoding
  to a Cantos profile, inspection and handoff are later slices, and a mock success is never
  provider-live evidence.
