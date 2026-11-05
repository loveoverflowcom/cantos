---
name: cantos-maintain-verification
description: >-
  Maintain the path from a Cantos claim to a reproducible check: reconcile the AGENTS.md
  verification table, the xtask command contract, the gate, preflight-selection and CI
  catalogs, assurance boundaries, behavior catalog links and evidence reading paths with the
  current code, tests and command owners. Pins the snapshot, enumerates what is actually
  implemented, ledgers each mismatch, classifies it (stale description, command drift, catalog
  drift, missing assertion, vacuous selection, unsupported environment, flaky tooling or
  product regression) and fixes only its owning layer without weakening a gate or rewriting
  historical evidence. Use when asked to refresh or audit verification guidance, investigate
  stale coverage or a check that no longer answers its claim, or improve a repeated checking
  workflow. Routine feature work uses cantos-engineering's verification references; a failing
  gate is diagnosed by cantos-diagnostics first.
---

# Cantos maintain verification

Keep the route from a claim (“this change is verified by X”) to a command that actually checks
it accurate and current. Own only the reconciliation workflow and its Verification maintenance
report. Product rules, the evidence vocabulary, the L1–L5 layers, the cadence and every gate
keep their current owners; this skill adds no status, layer, gate or schedule.

Invoking it is task-driven maintenance. It does not create a recurring job or monitor, and it
does not require a full-stack run for a documentation change.

## Keep one owner for each kind of information

| Information | Owner |
|---|---|
| Affected scope → required check at a milestone | the Verification table in [`AGENTS.md`](../../../AGENTS.md#verification) |
| Commands, flags, suites, gates, prerequisites and the `PASS`/`FAIL`/`NOT_RUN`/`UNSUPPORTED` vocabulary | [`xtask/README.md`](../../../xtask/README.md), with the code as truth: [`cli.rs`](../../../xtask/src/cli.rs), [`gate.rs`](../../../xtask/src/gate.rs), [`retired.rs`](../../../xtask/src/retired.rs) |
| What preflight selects for a changed path and which assurance obligations it raises | [`preflight/select.rs`](../../../xtask/src/preflight/select.rs), [`preflight/assurance.rs`](../../../xtask/src/preflight/assurance.rs), [preflight contract](../../../xtask/README.md#preflight) |
| Which gates CI runs and where | [`ci.rs`](../../../xtask/src/ci.rs), [`repository-checks.yml`](../../../.github/workflows/repository-checks.yml), [`.circleci/config.yml`](../../../.circleci/config.yml); branch protection lives outside the repository |
| Which evidence a claim needs, when checks run, evidence reuse and delivery | [verification strategy](../cantos-engineering/references/verification-strategy.md), [cadence](../cantos-engineering/references/verification-cadence.md), [local execution](../cantos-engineering/references/local-execution.md) |
| Stateful expectations, capability IDs and model/conformance runs | [`specs/behavior/`](../../../specs/behavior/README.md) and [`cantos-behavior`](../cantos-behavior/SKILL.md) |
| Assurance boundaries of Kani, native isolation, OCI qualification and editor acceptance | [`docs/assurance/`](../../../docs/assurance/domain-invariants.md) |
| Source-bound observations and their reading paths | [`docs/evidence/`](../../../docs/evidence/README.md) |
| Skill structure, index, symlinks and Codex metadata | `scripts/check_repository.py` through the `repository` gate; [skill index](../README.md) |

Every guide is an index into source, tests and commands, never a second command manual or a
permanent green board. A test's presence is coverage intent; only a run on identified inputs is
run evidence. When code and a document disagree about what a command does, the code wins for
current behavior and the document is the stale side; when a document and a product owner
disagree about what *should* be checked, the product owner wins and the code may be the defect.

## Reconcile a bounded slice

1. **Pin the scope.** Resolve `BASE_REF` per [CONTRIBUTING](../../../CONTRIBUTING.md#branch-and-pull-request-workflow),
   fetch it and record the SHA plus local changes. Name the slice: one feature's checks, one
   gate, one suite, one owner document or one unreliable check. A general refresh is several
   slices, each reported separately.
2. **Enumerate what exists before reading claims about it.** Read the command and gate
   catalogs from code (`cli.rs` subcommands, the `Gate`/`GateKind` enums, `CiJob::ALL`,
   retired commands, the selector rules) and use the read-only discovery the contract offers:
   `cargo xtask --help`, `cargo xtask verify --help`, `cargo xtask doctor` and
   `cargo xtask preflight --changed-only --base "$BASE_REF" --dry-run`. Do not derive
   implemented checks from the work plan, an ADR, a proposal or an older evidence record.
3. **Trace each claim to an assertion.** For every row in the slice, follow claim → command →
   gate → test target → the assertion that would fail if the claim broke. Confirm package,
   test and fixture names, feature flags, prerequisites and the selector rule that schedules
   it. Distinguish a pure-core test, a mocked boundary, a real-file/HTTP check, a renderer or
   preview observation and device or deployed acceptance; they stay separate rows.
4. **Classify each mismatch** in a ledger (claim, owner, documented, actual, class, action):

   | Class | Meaning | Usual action |
   |---|---|---|
   | stale description | prose, counts, links or examples no longer match the implemented command | fix the owning document |
   | command drift | a documented command, flag or suite is absent, renamed or a retired refusal | fix the document; never re-add a retired lane to satisfy it |
   | catalog drift | `gate.rs`, the README tables, the AGENTS table, the selector and `ci.rs` disagree | fix the stale side; changing which gates run is a tooling change |
   | missing assertion | the named test runs but does not assert the claim, or asserts it vacuously | add the assertion through the claim's owner, or narrow the documented claim |
   | vacuous selection | the gate exists but preflight never selects it for the inputs that affect it | fix `select.rs` with a regression, or document the explicit lane |
   | unsupported environment | the check needs a tool, OS or service absent here | record `UNSUPPORTED` with the prerequisite; install nothing unasked |
   | flaky tooling | the same inputs pass and fail | hand the failure to [`cantos-diagnostics`](../cantos-diagnostics/SKILL.md); never retry until green |
   | product regression | the check is right and the product is wrong | report it and route the repair to its entrypoint; never edit the expectation to match the bug |

5. **Fix the owning layer within scope.** Update only the owner the ledger names, plus links
   that point at the changed text. Keep the intended claim; narrowing a claim is legitimate
   only when the stronger claim was never implemented, and the narrowing is reported.

## Never trade correctness for a green summary

- Do not remove, skip, weaken or move a gate, required check or CI job to make a run pass, and
  do not change CI, required checks or `--skip` usage as part of reconciling text.
- Do not mark a requirement, decision, capability or acceptance row complete because its guide
  is now accurate.
- Evidence records are immutable history. Never rewrite an old result, revision or environment
  to look current; add a new attributable record or a reading-path note. Age alone is not a
  reason to delete evidence, and an old `PASS` never establishes current-code status.
- Registered migration SQL, retained PostgreSQL oracles and checksum history are inputs, not
  drift to clean up.
- Remote-write commands (`distribute`, `deploy`, `secrets`, release publication) never become
  gates, preflight selections or CI steps, and are never run to “check” documentation.

## Improve tooling only for a concrete repeated need

Inspect existing `xtask` commands, gates and script primitives first; documentation often
resolves a discovery problem. When repeatable automation is necessary and authorized, extend
`xtask` along its [capability path](../../../xtask/README.md#adding-a-capability): a gate in
`gate.rs`, classified in `preflight/select.rs` and, only if CI must run it, a job in `ci.rs`.
Never add a parallel runner, a standalone script entrypoint or a new test framework, and never
add a command merely to validate skills.

For an actual tooling change, keep plans as values with exact-command tests, actionable errors,
stable exit status, bounded waits and the existing outcome vocabulary. Make fixture and resource
ownership and cleanup explicit, keep credentials, manuscripts and generated audio out of
diagnostics, and update `xtask/README.md` and the AGENTS table with the changed contract in the
same change.

## Verify proportionately

Follow the [cadence](../cantos-engineering/references/verification-cadence.md):

- A documents/skills-only change needs link and structure review plus the handoff
  `cargo xtask preflight --changed-only --base "$BASE_REF"`, or the
  [docs-only fallback](../cantos-engineering/references/local-execution.md#docs-only-fallback-when-xtask-cannot-start)
  when Cargo cannot start. It refreshes no runtime evidence.
- A selector, gate or command change needs `cargo test -p xtask --locked` (part of
  `verify rust`), a regression that fails without the change, a failure-path check and the
  handoff preflight on the final diff.
- A changed routing or ownership statement in a skill also walks the relevant
  [routing cases](../routing-cases.md).

Report each gate exactly as printed, and keep browser, device, deployed and live-provider
acceptance separate from source checks.

## Report

Write a **Verification maintenance report**:

1. Scope: slice, `BASE_REF` and SHA, local changes, owners inspected.
2. Ledger: each mismatch with class, owner and action (fixed, narrowed, routed or deferred).
3. Changes: files edited and the owner each belongs to.
4. Checks: commands actually run with their inputs and printed statuses; `NOT_RUN` and
   `UNSUPPORTED` with their reasons.
5. Open claims: each unresolved or deferred row with its owner and the next check that would
   settle it; product regressions with their routed entrypoint.

A current guide is not a passing product. Keep historical results tied to their original
revision and environment.
