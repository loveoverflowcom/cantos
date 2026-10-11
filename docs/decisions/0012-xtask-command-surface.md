# ADR 0012: cargo xtask as the single repository command surface

- Status: accepted
- Date: 2026-10-11
- Decision owner: Cantos maintainers
- Related issue or PR: `chore: unify developer workflow behind cargo xtask` (targets `develop`)
- Supersedes / superseded by: N/A

## Context

Building, running and verifying Cantos had grown into independent entrypoints: Cargo commands
copied into five READMEs, `cd apps/... && NO_COLOR=true trunk build --locked`, Python runners in
`scripts/`, and a GitHub workflow with its own command lists. They had drifted. CI linted the
WASM crates but never ran `trunk build`, never ran the WAV oracle, and its PostgreSQL job ran three
of the four ignored database suites against a service container instead of the documented
disposable-cluster runner, so `production_postgres` and the HTTP restart checks never ran in CI.
Nothing selected checks from a change set, and nothing distinguished an unrun check from a pass.

Known: VOT Workspace solves the same problem with a Rust `xtask` crate behind a Cargo alias, a
typed build contract, a pure changed-path selector and one CI contract. Cantos is much smaller:
one backend, two Leptos clients, no mobile Gradle project, Kanidm as configuration only, and no
images, releases or deployments.

## Decision

Add `xtask/` as a workspace member, invoked as `cargo xtask` through `.cargo/config.toml`.
Repository-level operations go through it; `scripts/` and external tools remain implementation
primitives that it invokes unchanged (one script gains the new member in its disposable copy).

- `build`, `run` and `app` cover `cantos-api`, `cantos-studio --web`, `cantos --web` and `all`,
  with `dev` and `--release` profiles. `--mobile` is a reserved surface that reports `UNSUPPORTED`.
- `run`/`app` supervise their own process groups, check ports and readiness, and never stop
  a process they did not start. They never provision PostgreSQL; they use an explicit
  `DATABASE_URL` or the cluster retained by `verify postgres --keep`.
- A gate catalog owns every check; `verify <suite>`, `preflight [--changed-only]` and
  `ci [--job]` select from it. Uncertain impact broadens to every gate. Skips are explicit and
  reported as `NOT_RUN`.
- GitHub Actions only provisions environments (pinned Rust 1.87.0, Trunk 0.21.14, PostgreSQL 17
  binaries) and runs `cargo xtask ci --job <job>`.

The authoritative contract is [`xtask/README.md`](../../xtask/README.md).

## Alternatives

| Option | Benefits | Costs and limitations | Reason selected or rejected |
| --- | --- | --- | --- |
| Keep README commands and workflow YAML | No new code | Lists drift; CI already diverged; no change-based selection or outcome vocabulary | Rejected: the drift is the problem |
| `cargo-make`, `just` or a Makefile | Familiar task runners | Another tool to install; logic in strings; hard to unit-test selection and process handling | Rejected: VOT moved off `cargo-make` for these reasons |
| A Python orchestrator in `scripts/` | Python is already required | A second language for the same contract; no type checks on plans | Rejected: Rust plans are testable values beside the code they build |
| Copy VOT xtask wholesale | Fast start | Images, releases, deploy, mobile runners and Kanidm automation that Cantos does not have | Rejected: only the architecture and patterns are inherited |

## Consequences

- Product and user impact: none; no domain logic, API, migration, TTS authorization or production
  behavior changes.
- Compatibility and migrations: N/A. The workspace gains a member; `Cargo.lock` only adds clap,
  signal-hook and their dependencies, resolved for Rust 1.87; anyhow, serde, serde_json and libc
  are reused at their locked versions.
- Operations, storage, and production cost: CI now builds both Web clients with Trunk, runs the
  WAV oracle and runs the full disposable-cluster PostgreSQL suite with PostgreSQL 17, adding job
  time (installing Trunk from source and PostgreSQL from the PGDG repository). No paid service is
  called.
- Implementation and recovery requirements: a new script or check is added as a gate, classified
  for preflight and, when required, placed in a CI job.
- Deferred questions: mobile build/run once a Gradle project exists; Kanidm integration; release
  and deployment commands; whether `mutation` should join CI.

## Validation and revisit conditions

The [xtask evidence](../evidence/unified-xtask.md) records which commands ran here, which did not
and why. Revisit when a mobile project, identity integration, a second deployable or a release
process appears, or when changed-only selection misses a regression that CI catches.
