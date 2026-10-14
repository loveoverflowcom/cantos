# Failure triage, hypotheses and temporary instrumentation

> **Scope.** Decide what kind of failure this is before touching code, keep one discriminating
> hypothesis per iteration, instrument only what existing signals cannot decide and stop loops
> that produce no new evidence. Statuses stay the ones xtask prints; layers stay the
> foundation's L1–L5. This reference adds a diagnostic classification, not a status.

Contents: [classification](#1-classify-before-you-patch), [signature](#2-failure-signature),
[baseline](#3-clean-baseline-comparison), [ledger](#4-hypothesis-ledger),
[instrumentation](#5-temporary-instrumentation), [no-progress rule](#6-no-progress-rule).

## 1. Classify before you patch

| Class | Typical signature | Oracle that establishes it | Report and next owner |
|---|---|---|---|
| **environment** | xtask `UNSUPPORTED`; a step `FAIL`s before running code (missing tool, wrong `rustc`, locale or temporary-path error, port taken, empty offline Cargo cache, full disk) | `cargo xtask doctor`, `--version` probes and the error text name the environment, not a source path; the same command proceeds past that point once a per-command setting such as `RUSTUP_TOOLCHAIN=1.98.1` or `TMPDIR=/private/tmp` is applied | keep the printed status; name the prerequisite and ask the user for anything that installs or starts software; never a code change |
| **pre-existing** | the same signature on a clean base | [baseline comparison](#3-clean-baseline-comparison) reproduces it without the change | still `FAIL` for that gate; "not introduced by this change" with the baseline command; fixing it needs the user's scope |
| **flaky** | identical command, unchanged tree, different outcomes | bounded reruns (at most three, each captured) plus a named nondeterminism source: wall clock, ordering, shared ports or database names, timeouts, parallel tests | report every run (`FAIL`, then `PASS`, …) with frequency; a later pass never replaces the earlier `FAIL`; the source of nondeterminism is a defect to route |
| **regression** | fails with the change, passes on the clean base in the same environment | baseline comparison plus the smallest reproduction at the owning layer | `cantos-logic-fix`, `cantos-ui-fix` or `cantos-work-item` with this diagnosis |
| **test-defect** | the test's expected value contradicts the owning contract | a quote from the owning document or contract, not convenience | route to the owner; never edit an expected value merely to match new output |
| **unknown** | evidence cannot yet separate the classes | the discriminating check still needed | Diagnosis report with the open checks; ask or escalate |

An environment or pre-existing classification explains a red gate; it does not make the gate
`PASS`, does not hide an actual regression elsewhere in the same run and does not license
`--skip` to manufacture green output. Under the
[local handoff policy](../../cantos-engineering/references/local-execution.md#local-handoff-and-pr-completion)
a reasoned `--skip` stays `NOT_RUN` and the blocker stays visible.

Proposals elsewhere may say `BLOCKED` or `SKIPPED`. In Cantos reports, a missing tool printed by
xtask is `UNSUPPORTED`, an explicit skip or a step stopped by another failure is `NOT_RUN`, and a
manually identified prerequisite gap is the foundation's `blocked`. Never invent an xtask status.

## 2. Failure signature

A signature lets two runs, two machines or two agents agree that they see the same failure:

```text
<gate or step> · <test path or file:line> · <error kind or code> · <first meaningful message>
```

Normalize volatile parts before comparing: absolute checkout paths, temporary directories,
ports, timestamps, durations, process IDs, generated UUIDs and cluster directory suffixes.
Example: `rust-tests · contexts::catalog::storage::tests::work_restore_rejects_invalid_metadata_and_revision ·
assertion · left: InvalidMetadata, right: StaleRevision`. A changed signature after a fix
attempt is progress worth recording, even when the gate is still red.

## 3. Clean-baseline comparison

Run the identical command on a detached checkout of the base, with its own target directory so
neither build invalidates the other:

```sh
base="$SCRATCH/cantos-base"          # a scratch directory outside the checkout
git fetch origin develop
git worktree add --detach "$base" origin/develop
(cd "$base" && CARGO_TARGET_DIR="$base/target" cargo xtask verify rust) >"$RUN/logs/NN-baseline-verify-rust.log" 2>&1
git worktree remove "$base"          # cleanup: only the worktree this run created
```

- Use a full worktree; a partial `git archive` copy creates its own missing-file errors.
- Keep the environment identical: same toolchain, exported variables and tool versions.
- A baseline costs a full build. Run it when the classification changes the next action, not
  for a failure whose source path is inside the change.
- Record base SHA and the baseline signature in the run record. Remove the worktree afterwards;
  never remove a worktree or directory this run did not create.

## 4. Hypothesis ledger

Keep the ledger in the run record's `run.md` (or the report when no record exists):

| # | Hypothesis | If true we would see | Discriminating check | Result | Status |
|---|---|---|---|---|---|
| H1 | a conflicting override selects a compiler different from the production pin | `rustc --version` differs from `rust-toolchain.toml` | rerun the step with `RUSTUP_TOOLCHAIN=1.98.1` | compiler identity matches the pin; reclassify any remaining failure separately | confirmed, environment |
| H2 | the rejection mutates the revision before returning | the stored revision differs after `Err` | pure test asserting the unchanged value | value unchanged | refuted |

- A check is **discriminating** only when the hypotheses predict different outcomes for it.
- One new check per iteration; record its command and log pointer.
- **Confirmed** requires a check whose outcome this hypothesis predicted and its rivals did
  not. Co-occurrence in a log, a plausible story or another agent's finding is not confirmation.
- A refuted hypothesis stays in the ledger so the next session does not retry it.

## 5. Temporary instrumentation

Prefer, in this order:

1. **An existing signal at a higher depth**: a backtrace, `--nocapture`, a retained cluster's
   `postgres.log`, the browser console or network panel.
2. **A probe through a test.** A pure core needs no logging: call the production function from
   a test with the suspect input and assert or print its returned value. A probe that fails
   for the right reason becomes the regression for the repair.
3. **A marked temporary event in the shell** on the suspect path: handler, application
   orchestration, adapter, runner or effect interpreter. Never inside the domain, Script IR or a
   pure reducer, which stay observable through their return values.

Every temporary line carries the marker `CANTOS_DIAG` (distinct from the skill name, so the
oracle cannot match ordinary references to `cantos-diagnostics`) and the run ID, emits one line of
`event=` plus `key=value` fields, and contains no secret, token, connection URL, manuscript,
dialogue or audio content:

```rust
eprintln!("CANTOS_DIAG 2026-10-13T0415Z-intake-profile event=profile_checked bitrate_kbps={} channels={}", kbps, channels);
```

```kotlin
println("CANTOS_DIAG 2026-10-13T0415Z-progress event=transition_rejected from=$from event_kind=${event::class.simpleName}")
```

Leptos uses the same one-line format through the browser console. Do not add a logging
dependency, change a public signature, a wire contract or a test's expected value to
instrument.

**Cleanup oracle**, required before handoff and recorded in the report:

```sh
git grep -n 'CANTOS_DIAG' -- . ':!.agents/skills'   # must print nothing
git diff --stat                                     # no instrumentation-only hunks remain
```

When the working tree already holds the user's uncommitted edits, instrument in a separate
worktree or remove exactly the marked lines; never restore a whole file with `git checkout` or
`git restore` over someone else's changes. Instrumentation is never committed.

## 6. No-progress rule

| Signal | Required action |
|---|---|
| the same command on an unchanged tree is about to run again | only for the bounded flakiness check; otherwise choose a discriminating check |
| two fix attempts left the same failure signature | stop patching; revert speculative edits; return to the ledger with new evidence or escalate |
| three iterations confirmed or refuted nothing | write the Diagnosis report with root cause `unknown` and the checks still needed; ask the user |
| a read exceeds its depth budget | narrow the filter; do not read more |
| the next step needs installing, provisioning or starting infrastructure | stop and name the prerequisite; that action belongs to the user |

Count commands, identical reruns, fix attempts per signature and the deepest depth in the
report's loop-health line. These counts make an unproductive loop visible to the next reader.
