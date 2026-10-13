# Local execution, tooling reality and remote compute

> **Scope.** Decide what an agent may run while working on Cantos and how to report it:
> discovering real commands, focused local checks first, resource limits, paid providers,
> remote CI, secrets and generated media. Use before running any build, test, provider call,
> browser or device session, and before pushing or opening a pull request.

## Discover, never assume

Cantos has implemented Script IR, revision and Studio slices. Before citing or running a command, establish separately:

| Fact | Adequate source |
|---|---|
| the toolchain is configured | a manifest (`Cargo.toml`, `build.gradle.kts`, `package.json`) or script in the repository |
| a test or target exists | its source, discovered by the actual runner |
| the executable is available here | a successful `--version` in this environment |
| a result was obtained | attributable output from a run in this session |
| CI runs it | the workflow file and the command it invokes |
| it is a required merge gate | repository rules — workflow YAML alone is not enough |

None implies another. A command written in a skill, a work-plan item or an issue is a proposal
until the repository implements it. Mark unknown capability as unknown.

Repository-level operations go through [`cargo xtask`](../../../../xtask/README.md); scripts and
external tools are its primitives, not competing entrypoints. It runs from any directory:

```sh
cargo xtask doctor                     # which tools, gates and surfaces exist here
cargo xtask verify repository          # documents, links, JSON, skills, Python tests, whitespace
cargo xtask preflight --changed-only   # gates selected from the diff; uncertain impact runs all
cargo xtask preflight --changed-only --dry-run   # the plan, without running it
```

`verify repository` validates documents, links, JSON syntax, text hygiene and skill structure; it
does not build, test or run any application. `doctor` and the gate prerequisites are the
"executable is available here" source in the table above; a missing tool is `UNSUPPORTED` and
the command fails before running anything.

## Focused first, proportionate after

1. Run the single test or check that settles the claim.
2. Then the module or package suite (`cargo xtask verify <suite>`).
3. Then `cargo xtask preflight --changed-only`, which runs its gates sequentially.

Docs-only or read-only work must not start a heavy build merely to "have a baseline". Report any
required gate that remains unrun on the residual-risk line.

## Local handoff and PR completion

Use the foundation's [L1–L5 order](verification-strategy.md#layered-verification) and
[independent gate ledger](verification-strategy.md#independent-verification-gates).
**Core verification pass does not depend on integration acceptance.** When the user requests
commit/push/PR work, deliver L1–L3 once their applicable obligations pass, even if L4–L5 cannot
run. Do not keep independently verified code pending because PostgreSQL, Podman or another
container runtime, an API server, emulator or integration environment is missing. Do not
require installing or starting the full stack to accept a pure-domain change.

Open the requested PR and return its link and local evidence without waiting for GitHub
Actions. Do not poll, wait for, dispatch or re-run remote workflows to supply missing local
evidence. Required tests, merge protection, CI and production acceptance retain their gates.

| Change or claim | Local evidence |
|---|---|
| pure-domain behavior change | L1–L3 obligations, focused deterministic regression, applicable module tests/static checks and changed-only preflight; no infrastructure prerequisite |
| flow spanning application modules or an external adapter | production code exercised together with deterministic mocks/fakes at external boundaries; report `mock-integration-tested` and name the doubles |
| PostgreSQL constraints, SQL, transactions, migrations or concurrent writes | disposable real local PostgreSQL checks to accept those semantics; an unavailable run permits a requested PR only with that verification explicitly incomplete |
| documents or skills only | repository verification and changed-only preflight; no application or integration test required |

For mocked integration, exercise the real handler/application/adapter code and assert observable
results and relevant errors, timeouts or retry behavior. A fabricated successful response or a
test of an isolated fake is not integration evidence. No paid vendor, deployed service, cloud
storage or production environment is required for routine implementation handoff. A requirement
for real provider, device, publication or phase-exit acceptance stays with that capability's owner;
record it as deferred acceptance rather than claiming it from mocks.

### Progressive commit, push and PR

- Keep commits cohesive around reviewable, independently verified layers. Passing L1–L3 is
  sufficient for requested core commit/push/PR delivery while I/O acceptance remains pending.
  L4 results may be delivered as mocked evidence without a real API; adapter/SQL changes whose
  real checks cannot run may also be handed off for review with verification incomplete.
- `FAIL` blocks a clean handoff and a core-verified claim for the affected scope: fix it or
  report the actual failure. Never disguise it as an environment gap or use `--skip` to hide it.
  `NOT_RUN`, `UNSUPPORTED` and zero selected tests do not satisfy required checks.
- Run all applicable checks that can run. xtask prechecks all selected prerequisites, so one
  missing tool can leave independent gates `NOT_RUN`. Record that original summary, then use
  a reasoned `--skip <gate>` or an existing focused suite to run the available gates. Keep the
  skipped gate `NOT_RUN` and retain the blocker; a successful exit is not full acceptance.
- List every outstanding check in the PR with why it is missing (environment, dependency or
  outside this PR's scope), prerequisites, exact command, acceptance criteria, residual risk
  and next owner. If no owner is assigned, state `unassigned — assignment required before
  acceptance`; do not invent an assignee. Use a PR checklist; create no follow-up issue unless
  the user requests one.
- Incomplete code must not expose an unsafe production behavior or API. Keep it unreachable
  or behind existing fail-closed gates and explain the activation/rollout boundary. Never
  bypass required tests, relax branch protection or change CI to manufacture a green result.

The handoff may say `CORE_VERIFIED / IO_ACCEPTANCE_PENDING` **only for a named scope whose
L1–L3 obligations passed**. This is implementation progress, not a new evidence level, xtask
status, merge authorization or production-ready claim. Do not apply it to unverified SQL
transaction semantics. A passed layer never accepts the whole feature; DB/API/runtime and
production/phase-exit acceptance stay explicit until actually exercised.

### Docs-only fallback when xtask cannot start

When Cargo/Rust is unavailable and **every changed path is documentation or a skill**, run the
existing `repository` gate's primitives from the repository root:

```sh
python3 scripts/check_repository.py
python3 -m unittest discover -s scripts -p 'test_*.py'
git diff --check
git diff --cached --check
```

This is a narrow fallback to existing checks, not a second orchestration entrypoint. All four
must pass. Report their actual results and xtask/preflight as unavailable, with Rust formatting
not run and no Rust sources changed. Do not invent a xtask summary. This exception does not cover
code, scripts, fixtures, contracts, workflow changes or uncertain impact; those still need their
applicable local gates. No new regression test is required for wording alone.

## Resources

Before an expensive build, browser or emulator run: check free disk on the filesystems that will
receive artifacts, memory pressure and concurrent work. Run one heavy lane at a time (Cargo,
Gradle, Xcode, browser). Do not kill unrelated processes, delete caches you do not own, or install
SDKs, emulators or toolchains outside the task's scope to get a green result. A resource limit
never authorizes weaker checks or remote compute.

## Paid providers and external services

- Never call a commercial TTS or adaptation provider without an explicit user request, a
  configured cost limit and server-side credentials supplied by the user's secret store.
- Use fake or local providers for tests; label them as such in every report.
- A provider-live run records provider, model, request IDs, measured usage and cost, and the
  sample's rights status.
- Never trigger production publication, CDN purges or storage deletion from an agent task.

## Remote CI and publishing

The repository workflow provisions toolchains and PostgreSQL, then runs `cargo xtask ci --job
<repository|rust|web|postgres>` on pushes and pull requests to `develop`; `cargo xtask ci` runs the
same jobs locally. Pushing a
branch or opening a PR consumes CI and is outward-facing: do it only when the user asks. That
request authorizes any CI automatically triggered by the PR; it does not require waiting for it.
Report remote CI as `not checked` unless its status was actually read. Never debug by repeatedly
re-running remote jobs, relax branch protection or substitute a remote green check for local evidence.

## Secrets, private content and media

- Never commit secrets, provider credentials, signed URLs, private manuscripts, downloaded media
  or generated audio. `.gitignore` already excludes common audio formats, `/artifacts/` and
  `/private-assets/`; keep generated evidence there or under `target/`.
- Do not print environment variables or credential files to logs or reports.
- Redact before an image, log or transcript reaches any external tool.

## Reporting execution

For every check, record: exact command, working directory, source revision plus dirty state,
toolchain version, result (`passed | failed | blocked | not-run | unavailable | zero-selected`)
and where its output lives. When output exceeds a screen, redirect it whole to a file in an
ignored [run record](../../cantos-diagnostics/references/run-record.md) and read it
[summary first](../../cantos-diagnostics/references/progressive-disclosure.md#2-depths); never
pipe a check into `tail` or `grep`, which loses its exit status and full output. `blocked`, `not-run`, `unavailable` and `zero-selected` are never
reported as passing. xtask summaries already use `PASS`/`FAIL`/`NOT_RUN`/`UNSUPPORTED` for
`passed`/`failed`/`not-run`/`unavailable`; an explicit `--skip` is still `not-run`.
