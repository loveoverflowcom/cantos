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
branch or opening a PR consumes CI and is outward-facing: do it only when the user asks. Never
debug by repeatedly re-running remote jobs, never relax branch protection, and never treat a
remote green check as a substitute for the local evidence a claim needs.

## Secrets, private content and media

- Never commit secrets, provider credentials, signed URLs, private manuscripts, downloaded media
  or generated audio. `.gitignore` already excludes common audio formats, `/artifacts/` and
  `/private-assets/`; keep generated evidence there or under `target/`.
- Do not print environment variables or credential files to logs or reports.
- Redact before an image, log or transcript reaches any external tool.

## Reporting execution

For every check, record: exact command, working directory, source revision plus dirty state,
toolchain version, result (`passed | failed | blocked | not-run | unavailable | zero-selected`)
and where its output lives. `blocked`, `not-run`, `unavailable` and `zero-selected` are never
reported as passing. xtask summaries already use `PASS`/`FAIL`/`NOT_RUN`/`UNSUPPORTED` for
`passed`/`failed`/`not-run`/`unavailable`; an explicit `--skip` is still `not-run`.
