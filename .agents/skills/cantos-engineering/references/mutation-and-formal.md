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

Every harness states, in a comment block: the proposition, the symbolic domain and bounds, the
assumptions, which production functions are reached, and what is stubbed. Watch for:

- `assume` that removes the hard case or makes every assertion vacuous — add `cover!` for the
  branches the contract distinguishes;
- an oracle that calls the function under test;
- silently narrowed bounds or removed unwinding checks.

Report `bounded-model-checked` **for the stated domain and bound**, never "proven". Ordinary
`cargo test` does not run these harnesses; their evidence needs the model checker's own output.

## What not to do

- Do not add a mutation or model-checking dependency without a recorded decision and a target.
- Do not adopt several proof tools speculatively; a theorem prover or second verifier needs its
  own approved spike with a concrete gap.
- Do not treat a historical campaign as evidence for changed code; results are bound to the
  source revision, tests and tool version that produced them.
