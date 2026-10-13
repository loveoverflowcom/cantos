# Podman service runtime evidence

- Date: 2026-10-13; local Mac mini/macOS ARM64 host.
- Base: fresh `origin/develop` 4f33a901b87be770be7cf5bbdac909408570240d; final rebase below.
- Scope: [ADR 0021](../decisions/0021-podman-service-runtime.md),
  [draft PR77](https://github.com/loveoverflowcom/cantos/pull/77).
- Final base: `3a78e77a2190cd443401ed95d58c5933e302724e`; clean rebase preserves PR73 and PR76.
- Rust: 1.87.0. Private sources/TLS/passwords/media are not committed.

## Invariant and boundary

Only explicit lifecycle requests start local container infrastructure. Exact Cantos manager,
project, instance and service labels authorize container access; inspected/returned immutable
IDs authorize effects. No command removes persistent volumes, performs application migrations,
changes host security labels, provisions a machine or creates cloud resources. Native API run
and disposable PostgreSQL verification remain separate and byte-unchanged.

Owner: xtask local process/runtime boundary. Domain, persistence and API contracts are unchanged.
Evidence level: documented, statically-checked, example-tested and mock-integration-tested;
compiled local Rust command surface. No live Podman integration or TLS/persistence claim.

## Local command evidence

Use `TMPDIR=/private/tmp RUSTUP_TOOLCHAIN=1.87.0` for these commands. The existing local Cargo
artifact directory was reused because free disk started below 1 GiB; no tool was installed.
The exact target override was
`CARGO_TARGET_DIR=/Users/manhblue/projects/loveoverflow/cantos/target`.

| Command | Actual result / boundary |
| --- | --- |
| `cargo xtask verify repository` (checkpoints 1/2) | PASS; 221 Python tests and repository/whitespace checks |
| `cargo test -p xtask --locked` | PASS; 130 tests including 25 services regressions; local loopback fixture tests ran with sandbox escalation |
| `cargo xtask verify rust` | rust-format PASS, workspace rust-lint PASS, workspace rust-tests FAIL: no disk space during compilation; not a clean full-suite pass |
| `cargo xtask verify repository` (final implementation) | PASS; typed services validation plus 221 Python tests and hygiene |
| `cargo clippy -p xtask --all-targets --locked -- -D warnings` | PASS; scoped command boundary lint |
| `cargo xtask preflight --changed-only` | UNSUPPORTED: missing ffmpeg/ffprobe and Gradle; broad selection due xtask changes; all other selected gates NOT_RUN before execution |
| `cargo xtask verify contracts` | audio-inspection UNSUPPORTED (ffmpeg/ffprobe); script-ir-reference/contract-tests/wav-reference NOT_RUN |
| `cargo xtask verify postgres` | UNSUPPORTED: missing ffmpeg/ffprobe; no cluster started |
| `cargo xtask services list` / `services validate` | PASS; inventory/static command exercised |
| `cargo xtask services status` | runtime UNSUPPORTED: Podman absent; no containers or machine touched |

The initial restricted xtask test attempt had loopback permission failures in existing probe/
process tests. The authorized host rerun passed. Workspace Rust tests exhausted disk while
writing their compilation outputs; only the six exact incomplete incremental sessions produced
by this task were removed. Unrelated caches, other worktrees, ports, processes and data were
preserved. Subsequent focused checks use `CARGO_INCREMENTAL=0` to reduce that disk pressure.

## Deterministic and mocked evidence

Production parser/config/planner/lifecycle modules run against a scripted Podman command boundary,
not a copy of lifecycle logic. Tests exercise inventory/schema/version, unknown fields, required
cycles, official image pins, CLI target/readiness constraints, private path/mode/symlink rejection,
loopback port argv, secret-file-only arguments, Kanidm origin/configtest separation, config drift,
foreign containers/volumes, supported/unsupported nocreate errors, immutable IDs, actual mounts,
creation-before-start, exit-before-readiness, deadlines, observed status, both log streams,
cancellation across calls and unchanged retained container/volume selection on restart.
The real process boundary also executes literal metacharacter argv, a bounded stuck child and
cancellation with task-owned shell fixtures. These are CLI/process observations, not a live
Podman engine, TLS session or database durability test.

An independent read-only reviewer found deadline/cancellation/creation-ID/volume-race/SELinux
labeling/error-classification defects in the intermediate draft. They were fixed and regression
cases added. Final source review found no remaining actionable runtime P1/P2 finding; docs were
aligned separately. Review does not replace the listed executed tests.

## Unobserved runtime and residual risk

Podman is absent from PATH; no install, machine initialization/start, connection attachment,
password/certificate creation or live container lifecycle was attempted. Actual Linux/macOS
readiness, restart/stop, volume persistence and upstream Kanidm configtest/TLS remain UNSUPPORTED.
The official image/version/CLI contracts were researched; patch tags are explicit pins, not
immutable digest locks. A server supporting the documented nocreate option is required;
older versions such as 5.8 lack it and require separate operator-approved setup/update.

Operators supply private TLS and any preapproved SELinux labels. Kanidm 1.10.5 health verifies
TLS connectivity to `/status` but does not validate the HTTP status/body. OIDC, identity bootstrap,
production backups/upgrades/deployment and public delivery remain unimplemented/unobserved.
Ownership labels are not a same-user administrator security boundary; concurrent force-removal,
forged labels or changing a named connection definition are unsupported operator intervention.
Do not claim phase-exit/production acceptance or a complete green preflight from these checks.

## Final rebase and handoff

After rebasing the three ordered commits onto the final base, `cargo xtask verify repository`,
`cargo fmt --all -- --check`, `cargo clippy -p xtask --all-targets --locked -- -D warnings` and
`cargo test -p xtask --locked` all passed again (130 tests, including 25 service cases; 221
Python tests). Only the evidence record was edited after those source checks. Final preflight
and service status retain their unavailable prerequisite/runtime results above.

Independent scope coordination: source changes are confined to `xtask`, service inventory/
examples/guides, policy/engineering docs, architecture/work queue/evidence and the two reused
parser/digest dependencies in Cargo manifests. No files in `services/cantos-api`, migrations,
API/domain contracts, native run/process modules or disposable PostgreSQL runner changed.
The independent assurance/Kani work may overlap `xtask/src/{cli,main,gate}.rs`,
`xtask/src/preflight/select.rs`, `xtask/README.md`, Cargo manifests and engineering/work-plan
docs; reconcile those in its own branch. No Kani command or assurance policy was changed here.
Remote CI is not checked; no workflow was polled or dispatched. No automatic merge occurred.
