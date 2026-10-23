# Property, state-machine and differential testing

> **Scope.** Build evidence that examples cannot provide for Cantos: laws over generated inputs,
> command sequences against state machines, golden fixtures, small reference models, bounded
> exhaustive loops and cross-platform contract vectors. Use when the input space is large, when a
> reducer or job lifecycle needs sequences, when an optimized implementation needs a slow twin,
> or when Rust and Kotlin must agree on the same contract.

A property test is a **law plus a generator plus a shrinker**; if any is weak, the test is
decoration. `proptest` is a candidate for Rust (good shrinking); Kotlin candidates include
Kotest property testing. Both are proposals until a manifest pins them.

## When to use a property

Use one when all hold: the claim is a law, not an example; the input space is too large to
enumerate; you can generate *reachable* values; and a failure will shrink to something readable.
If the domain is finite and small, a bounded exhaustive loop is cheaper and stronger.

## Laws that pay off in Cantos

| Law | Shape | Cantos target |
|---|---|---|
| Round trip | `decode(encode(x)) == x` | Script IR, manifests, contract DTOs |
| Canonical form | equivalent inputs → identical bytes | script digest, fingerprints (NFC vs NFD text, key order) |
| Idempotence | `f(f(x)) == f(x)` | normalization, migrations, duplicate outbox delivery |
| Sensitivity | changing any effective input changes the fingerprint | speech, mix and episode fingerprints |
| Insensitivity | changing a non-effective field leaves it unchanged | reordering dialogue does not change speech fingerprints |
| Minimal invalidation | regeneration plan ⊆ dialogues whose effective inputs changed | partial regeneration |
| Transactional rejection | `Err(step) ⇒ state' == state` | draft edits, budget reservation, gate decisions |
| Invariant preservation | valid state + accepted command → valid state | job lifecycle, release lifecycle, player reducer |
| Monotonic history | settled records never change across any command sequence | revisions, attempts, approvals, manifests |
| Convergence | any interleaving of duplicate/delayed progress ops reaches the documented outcome | listener progress sync |

## Generators

- Build valid values through public constructors so generated data cannot violate invariants
  the production code relies on; generate raw invalid input separately for rejection laws.
- Bias toward edges: empty scenes, one-character casts, very long Vietnamese lines, duplicate IDs
  in the raw layer, budget equal to the estimate.
- Keep generated domains small enough that collisions and repeats actually happen.
- Persist failing seeds and commit the shrunk counterexample as a named example test.

## State-machine tests

For a registered Cantos lifecycle, read [cantos-behavior](../../cantos-behavior/SKILL.md)
and use its accepted Quint model as executable expectation. Do not add a hand-maintained
Rust/Kotlin/JS reducer beside it. Keep existing native tests and distinct codec/numeric oracles.
For an unmodeled lifecycle, resolve requirements/model ownership before shared scenario work.
Generate command sequences, observe the actual implementation independently and check after every step:

- accepted commands keep the invariant (at most one accepted artifact per step; an active release
  always points at a ready manifest; a paused player never resumes without a play command);
- rejected commands leave state and emitted effects unchanged;
- the implementation and the model agree on acceptance and on the resulting observable state.

Include crashes, duplicates, cancellations and late completions as commands, not just the happy
path. Expected stateful outcomes come from the accepted `.qnt` model and actual observations
come from production seams. A second handwritten reference reducer can mirror both mistakes;
model and assertions also need independent requirement review and sensitive controls.

## Golden fixtures and reference data

- Golden Script IR files, manifests and fingerprints under the owning service's `contracts/` (or a test fixture
  directory) are reviewed artifacts: changing one is a deliberate, explained diff.
- Recorded provider responses are fixtures for adapter contract tests. Redact credentials and
  private content before committing; never commit generated audio.
- Original or permitted sample content only, with rights recorded as in the existing
  [episode fixture](../../../../services/cantos-server/contracts/studio/script-ir/examples/episode-draft.json).

## Bounded exhaustive loops

When a domain is finite — job state × event, player state × event, release state × command,
retry class × attempt count up to the limit — walk all of it. This beats sampling and needs no
dependency.

## Cross-platform contract vectors

Rust and Kotlin must agree on contract meaning (player semantics, progress reconciliation
outcomes, error codes) without sharing runtime code. Keep shared JSON vectors — input sequence
plus expected outcome — derived from the owning specification, and run them in each platform's
test suite. Expected outcomes come from the spec, never from running one implementation and
copying its output. A vector run on one platform proves nothing about the other; report each.

## Reporting

`property-tested` names the law, generator scope and case count. `differentially-tested` names
the oracle (model, fixture set, previous version, other platform). A seed is not a counterexample;
keep the concrete shrunk input.
