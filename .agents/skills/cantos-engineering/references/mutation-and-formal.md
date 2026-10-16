# Mutation testing and bounded model checking

> **Scope.** Two optional, targeted techniques for Cantos pure kernels: mutation testing to
> measure whether assertions kill plausible defects, and bounded model checking to establish a
> small proposition over a symbolic domain. Use only on stable, pure, high-impact modules after
> ordinary tests exist. Never as a coverage target, across the whole repository, or as a
> substitute for an oracle.

## Mutation testing

A mutation tool (cargo-mutants is a Rust candidate; Pitest-style tools exist for the JVM) inserts
small defects and reports which survive the test suite. That measures **assertion strength**,
not correctness: a fully killed module can still implement the wrong specification.

### Good and poor targets

| Good target (pure, stable, dense in decisions) | Poor target |
|---|---|
| Script IR semantic validator and canonical encoder | Axum handlers, workers, adapters, anything async or socket-bound |
| fingerprint composition and regeneration planning | Leptos components and Compose screens |
| retry classification and backoff computation | code that changes weekly |
| budget reservation arithmetic | generated code, `Debug`/`Display` impls |
| publication gate decision and approval staleness | a percentage goal |
| progress reconciliation, player reducer | |

### Running a campaign

1. Scope to one module or one diff; record the source revision and the exact command.
2. Run the module's tests first; a red suite makes mutation results meaningless.
3. Classify every survivor: real gap, equivalent mutant, unreachable, tool limitation or
   low value. The classification is the deliverable, not the score.
4. Turn each real gap into a named test that kills it, then rerun the campaign on that module.

A survivor in `decide_publication` that flips `has_blocking_findings` is a real gap: some
blocker is not asserted. A survivor that changes a log message is low value.

Report `mutation-tested` with scope, tool version, killed/survived/timeout counts and the survivor
classification.

### Classes and blind spots

| Class | Meaning | Action |
|---|---|---|
| real gap | the mutant changes observable behavior and nothing asserts the difference | write a named test; order by blast radius |
| equivalent | the mutated program behaves identically | record why in one line; no test |
| unreachable | the branch cannot execute for any reachable input | record why; ask whether the branch should exist |
| verifier-only | inside code gated for a model checker or fuzzer | exclude by configuration; never "fix" with a test |
| tool limitation | unviable substitution, meaningless `Default` | ignore; do not contort the code |
| low value | trivial accessor or formatting | usually ignore, but read it once |

- **Verifier-only code is mutated too.** Mutation tools parse source, so model-checking and fuzzing
  harness modules (gated by `cfg`) are mutated without ever being compiled into the test build and
  every such mutant survives. Keep harnesses in a clearly named module and exclude it in the tool's
  configuration, then list the mutants again to confirm the count dropped.
- **Read the low-value ones once.** A `Debug` impl that redacts a secret is a security property;
  its surviving mutant means nothing asserts the redaction (provider keys, signed URLs).
- **A timeout is a finding.** A mutant that makes the suite hang usually marks an unbounded loop
  whose termination rests on an unproven invariant (a retry or backoff loop, a claim loop); that
  liveness hazard outweighs several missing accessor tests.

## Bounded model checking

Kani does not directly verify Kotlin/CMP; apply [VER-04/07](../../cantos-cmp-mobile/references/cmp-testing.md)
for independent bounded Kotlin models and any future Kotlin mutation tooling.

Kani is a Rust candidate for proving that a pure function satisfies a proposition for all inputs
in a declared bounded domain. Consider it only when all hold:

- the kernel is small, pure and critical (canonical integer arithmetic, a state transition table,
  a fixed-point loudness or money computation, an offset calculation);
- the domain cannot be walked exhaustively at reasonable cost;
- a defect would be P0/P1 (wrong publication, rebilling, corrupted digest);
- simpler techniques were tried and are insufficient.

Do not require Kani across the backend, asynchronous adapters, Leptos views or Kotlin UI.
Potential Cantos targets include checked size/offset arithmetic, revision/generation exhaustion,
exact approval/input identity and fail-closed publication decisions. A target being important
does not make an unsupported SDK, database or native callback verifiable. Keep those outside
the kernel and name their assumptions at the boundary under
[ERR-04](typed-errors-and-composition.md#err-04--exceptions-and-panics-stay-at-the-outer-edge).

### Harness obligations

Every selected harness must record:

| Field | Obligation |
|---|---|
| Proposition / oracle | named invariant from its owner; assert exact accepted/rejected outcome and refinement preservation/unchanged state as applicable; never compute expected results using the implementation |
| Production reach | exact symbols called by the harness, source revision, features, target architecture/width, build configuration; no copy of the kernel masquerading as production verification |
| Symbolic domain | input types/ranges, lengths, state/event alphabet and maximum trace length; explain every excluded case |
| Assumptions | each `kani::assume` plus its producer/contract justification and the evidence that the real boundary establishes it |
| Bounds | unwind/recursion/allocation/sequence bounds as applicable; include any loops in dependencies; keep unwinding checks enabled |
| Reachability | `kani::cover!` for the contract's meaningful acceptance, rejection and boundary classes; review each cover result, not just assertion success |
| Stubs / limitations | names, replacement behavior and why each preserves this proposition; unsupported calls, concurrency, I/O, floating-point or target differences outside the claim |

Generate raw invalid values when proving factory rejection. Restrict inputs to a refined
predicate only when proving an operation whose caller must already supply that type, and cite
the construction barrier separately. Never assume the result, skip exhaustion/conflict cases,
or use `assume(false)` to obtain a successful run. Unexpected `UNREACHABLE` assertions or
unsatisfied cover goals require investigation: they may reveal an overconstrained model.
An intentionally impossible branch needs an explanation; coverage alone proves no correctness.

Use `#[cfg(kani)]` for verifier-only harness modules and `#[kani::proof]` for harnesses;
`#[kani::unwind(N)]` is a bound, not a permission to disable unwinding assertions. Insufficient
unwind is a failed verification obligation. A timeout/unsupported operation is no success.
Convert meaningful counterexamples into ordinary deterministic regressions, then rerun both
the tests and the affected harness at the corrected bounds. A harness assertion diagnoses a
violated proposition; it is not authorization for panic-driven production control flow.

### Actual execution and repository integration

**Current implemented scope:** `cargo xtask verify kani` registers eleven
production kernels: ten in the API library (`catalog-revision`, `image-bounds`, `wav-payload`,
`submission-window`, `publication-commit`, `listener-access`, `listener-reconcile`,
`byte-range`, `playback-grant` and `public-cover`) and one in the Forge crate
(`forge-attempt-ledger`, the paid-attempt integer kernel). Each kernel names its one package.
Omission selects all eleven; repeat `--harness` for a subset. Registration is not execution:
the ledger records which kernels have an actual attributable run.
The fixed model pins Kani 0.68.0, bundled rustc 1.100.0-nightly (8925ea358,
nightly-2026-08-21), CBMC 6.11.0 and x86_64-unknown-linux-gnu/64-bit usize,
API library/no default features, CaDiCaL and unwind 2. Actual execution, source
identity, covers and residuals are recorded in
[the proof ledger](../../../../docs/assurance/kani.md). Production remains Rust
1.98.1; the verifier has no runtime dependency and no automatic install/setup.

The implemented [xtask command contract](../../../../xtask/README.md#optional-targeted-kani-proofs)
keeps Kani out of normal builds, `verify rust`, ordinary full preflight and CI.
Explicit `preflight --kani` opts in; changed-only selection narrows registered
kernels and broadens on uncertain/shared inputs. Missing/incompatible tools are
UNSUPPORTED, explicit skips/stopped later gates NOT_RUN, and incomplete/zero/
wrong harnesses, failed/undetermined checks, unsatisfied covers, unexpected
unreachability, timeout/output cap or source/config/lock changes FAIL.

The single-harness primitive is owned by xtask, not another orchestration entry.
It invokes an existing compatible absolute bundle driver and never the installer
proxy. Rust validates and binds exact bounded configuration bytes/existence; the
primitive compares that approved snapshot before execution and on proof exit, including
formerly absent Cargo-config candidates. Configuration reads stop at 1 MiB + 1
and reject oversize input. Generic model-source hashing streams bounded-memory
chunks without a total-size cap. Fresh logs and metadata bind source,
Git/dirty hashes, model/configuration and unchanged Cargo.lock (this Kani release
has no --locked option). Endpoint hashes require no concurrent mutate-and-restore
edits; they are integrity evidence, not filesystem isolation. Always inspect the
actual selected proof output; tool probes, parser mocks and ordinary tests are
not model-checking evidence.

Two existing `cfg(kani)` Theatre sources remain **unregistered and NOT_RUN**:
[`live/request.rs`](../../../../apps/cantos-theatre/web/src/live/request.rs)
`completion_requires_current_pending_generation` and
`cancellation_never_wraps_or_accepts_previous_completion`. They call production
ProbeState methods over u64 generations/representative phases, unwind 2 and no
assumptions/stubs, but lack explicit cover goals. Their historical
[unrun record](../../../../docs/evidence/live-client-integration.md#kani-boundary)
is retained. Private fields, source presence and ordinary tests establish no
Theatre formal claim. Any future campaign must justify reachability and execute
its exact model; do not add speculative UI/rendering/native/backend proof lanes.

For any new target, record its concrete gap/production reach, exact verifier /
compiler / solver / target compatibility and configuration before adopting it.
Normal Cantos builds remain Rust 1.98.1. Do not add a runtime proof dependency,
change the production toolchain, install tools just to green a docs change or
register every backend harness by default. Extend the existing typed xtask
registry and gate tests rather than adding competing repository orchestration.
The upstream cargo-kani proxy can bootstrap/install even on a version call;
capability checks must preserve the implemented no-install bundle path.

Only an **actual model-checker run** supports `bounded-model-checked`. Ordinary `cargo test`,
compiling a `cfg(kani)` module, manual reasoning or a written harness does not. Retain output
showing the selected harness was discovered/executed, assertion/unwinding results and all cover
outcomes for the exact source and model. Report zero selected, unsupported, timeout, vacuity and
failures honestly. A satisfied assumption-limited proposition does not prove that production
I/O satisfies the assumptions, that a retry is delivered once, or that the entire feature works.

```text
Kernel / owning invariant / production symbols:
Source revision / dirty state / features / target architecture:
Kani + bundled compiler / solver versions:
Symbolic inputs / assumptions and boundary justification:
Bounds / unwinding checks / stubs:
Exact xtask command / underlying verifier invocation / retained output:
Harnesses selected / actually executed:
Assertions / unwinding / cover outcomes:
Evidence: bounded-model-checked only for the executed model and bounds; otherwise actual status
Counterexamples / ordinary regressions:
Limitations / unverified production assumptions / next owner:
```

Official sources to recheck at adoption: [usage and harness selection](https://model-checking.github.io/kani/usage.html),
[proof/unwind attributes](https://model-checking.github.io/kani/reference/attributes.html),
[cover and vacuity](https://model-checking.github.io/kani/faq.html).
Keep independent passed L1–L3 evidence deliverable under
[local handoff](local-execution.md#local-handoff-and-pr-completion); list any selected but unrun
Kani obligation separately. A model-checker success never substitutes for DB/API/native acceptance.

## What not to do

- Do not add a mutation or model-checking dependency without a recorded decision and a target.
- Do not adopt several proof tools speculatively; a theorem prover or second verifier needs its
  own approved spike with a concrete gap.
- Do not treat a historical campaign as evidence for changed code; results are bound to the
  source revision, tests and tool version that produced them.
