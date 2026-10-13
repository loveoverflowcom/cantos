# Cantos xtask

`cargo xtask` is the repository-level command surface for building, running and verifying Cantos.
It is a workspace member (`xtask/`) invoked through the alias in [`.cargo/config.toml`](../.cargo/config.toml),
and it finds the repository root from its own manifest, so it behaves the same from any directory.

> **Single Surface Rule.** Repository-level operations go through `cargo xtask`. Scripts and
> external tools are implementation primitives, not competing orchestration interfaces.

The architecture follows VOT Workspace's xtask (typed targets, plan-then-execute, a pure
changed-path selector, one CI contract), scaled down to what Cantos actually implements.
The decision is recorded in [ADR 0012](../docs/decisions/0012-xtask-command-surface.md).

## Ownership boundaries

| Layer | Owns |
| --- | --- |
| Cargo, rustup, Trunk, Python 3, Git, PostgreSQL binaries | Primitive tool execution |
| `scripts/*.py` | Repository checks, independent oracles, the mutation runner, the disposable PostgreSQL runner and the test-cluster Studio host. Their logic is reused, not rewritten |
| `cargo xtask` | Targets, profiles, surfaces, gate catalog, prerequisite checks, changed-path selection, process supervision and the CI job contract |
| GitHub Actions | Runner lifecycle and environment: the pinned toolchain, Trunk, PostgreSQL 17 binaries. Each job then calls `cargo xtask ci --job <job>` |

xtask does not install toolchains, initialize/start Podman Machine, change connections, call
providers, create cloud resources, publish or deploy. Only explicit `services up` starts local
container infrastructure; build/test/run/preflight never do.

## Commands

```sh
cargo xtask build cantos-api [--release]          # cargo build --locked -p cantos-api --bins
cargo xtask build cantos-api --dbsp-experimental # adds --features dbsp-experimental (opt-in incremental core; no consumer)
cargo xtask build cantos-studio --web [--release]  # trunk build --locked in apps/cantos-studio/web
cargo xtask build cantos-theatre --web [--release]         # trunk build --locked in apps/cantos-theatre/web
cargo xtask build all [--release]                  # the API and both Web clients

cargo xtask run cantos-api [--test-cluster] [--web-origin URL]
cargo xtask run cantos-studio --web [--test-cluster] [--open]
cargo xtask run cantos-theatre --web [--open]
cargo xtask app cantos-studio --web [--open]
cargo xtask app cantos-theatre --web [--open]

cargo xtask ui-preview leptos build|run|verify       # package-local Leptos gallery
cargo xtask ui-preview leptos run --open             # loopback browser preview on 8083
cargo xtask ui-preview cmp build|run|verify          # package-local CMP desktop preview
cargo xtask ui-preview cmp android-build             # compile the isolated Android preview library

cargo xtask verify repository|rust|web|contracts|mutation|dbsp
cargo xtask verify postgres [--keep]

cargo xtask preflight [--changed-only] [--base REF] [--skip GATE]... [--dry-run]
cargo xtask ci [--job repository|rust|web|postgres] [--dry-run]
cargo xtask doctor
cargo xtask services list|validate|status
cargo xtask services [--instance NAME] up postgresql|kanidm [--timeout-seconds 60]
cargo xtask services [--instance NAME] up --all
cargo xtask services [--instance NAME] logs postgresql|kanidm
cargo xtask services [--instance NAME] down postgresql|kanidm|--all
cargo xtask compose-preview cantos-theatre|ui-system-cmp --file PATH --composable NAME \
  [--label SCENARIO] [--pid PID] [--android-cli PATH] [--timeout-seconds 60] [--dry-run]
```

`cargo xtask <command> --help` documents every flag.

### Targets, surfaces and profiles

`cantos-api` is the Axum/PostgreSQL backend. `cantos-studio` (creators) is web-only: pass
`--web`; `--mobile` is rejected as a usage error (exit 2), not a reserved implementation.
`cantos-theatre` (listeners, alias `cantos`) supports `--web` and `--mobile`; one must be named.
Theatre Mobile has an isolated mock Gradle project. Production `--mobile` remains
`UNSUPPORTED` because native listening adapters are absent; use `theatre-demo` for mock evidence.
Studio Mobile is excluded from all build/run/test/preview plans, even if files appear in its
out-of-scope directory. `build all` covers the API and both Web clients and notes that Theatre
Mobile and `services/kanidm` (configuration only) are not built. See the
[platform matrix](../docs/architecture/repository-layout.md#platform-scope).

`dev` is the default unoptimized build. `--release` passes `--release` to Cargo or Trunk and
produces an optimized local artifact. It is not a production image, a signed bundle or a
deployment; none of those exist yet.

Trunk runs with `NO_COLOR=true`, matching the documented commands (Trunk 0.21 rejects an
inherited `NO_COLOR=1`).

### run and app

| Command | Starts | Requires |
| --- | --- | --- |
| `run cantos-api` | `cantos-server` on 127.0.0.1:8080 after building it | `DATABASE_URL` for a migrated local app-role database, or `--test-cluster` |
| `run cantos-studio --web` | the API with `CANTOS_WEB_ORIGIN=http://127.0.0.1:8081`, then `trunk serve` on 8081 (its `Trunk.toml` proxies `/api/` to 8080) | as `run cantos-api` |
| `run cantos-theatre --web`, `app cantos-theatre --web` | `trunk serve` for the Theatre skeleton on 8082 | nothing; no listener API exists |
| `app cantos-studio --web` | only `trunk serve` on 8081 | an API already listening on 8080, for example `run cantos-api --web-origin http://127.0.0.1:8081` |

`--test-cluster` serves the API through `scripts/serve_studio.py` against the disposable cluster
retained by `cargo xtask verify postgres --keep`; that script refuses any other database. xtask
never reads, prints or invents a database URL. It sets `CANTOS_ENV=development` and the loopback
bind address; the server itself refuses anything else.

Process lifecycle:

- Preconditions are checked before any build: required variables and files, tools, and that
  every port is free. A taken port fails with an explanation; xtask never stops a process it did
  not start, and never looks processes up by port or name.
- Each service starts in its own process group with inherited output. Readiness is a TCP accept
  for the API and an HTTP 200 containing the app's `<title>` for Trunk (up to 15 minutes for the
  first WASM build). A process that exits before readiness fails the command.
- Ctrl-C, SIGTERM or SIGHUP stops every started group, newest first: SIGINT (graceful for Axum and
  Trunk), then SIGKILL after 10 seconds. If any service exits by itself, the others are stopped and
  the command fails.
- If xtask itself is killed with SIGKILL its children survive; the next run finds their ports taken
  and refuses to start. `--open` uses `xdg-open` or `open` after readiness.

The Trunk development proxy is a convenience. The browser walkthroughs recorded in
`docs/evidence/` used the API serving `apps/cantos-studio/web/dist` same-origin; do not cite a
`trunk serve` session as that evidence.

### Explicit local service infrastructure

[Services](../services/README.md) and [ADR 0021](../docs/decisions/0021-podman-service-runtime.md)
are the ownership contract. `list` and `validate` require no Podman, secret, TLS or container.
Repository/CI gates call `validate` using the existing xtask executable path through Cargo;
there is no separate orchestration script or new workflow. Validation covers schema/version,
runtimes, dependencies/cycles, official patch image pins, ports, fixed mounts/health argv and
public Kanidm/environment examples. It does not run upstream `configtest` or prove TLS validity.

`up` manages container infrastructure only. Native API lifecycle stays under `run`; external
storage is inventory-only. `up --all` includes required PostgreSQL and excludes optional Kanidm.
`down --all` stops any already-owned Kanidm and PostgreSQL containers. Secrets/configuration
are exported explicitly using the service `env.example` guides; no dotenv file is loaded.
`--instance` precedes the action, defaults to `local` and scopes resource names/labels. Use a
unique instance and unused port for task fixtures; defaults are shared across worktrees.

Linux commands force local rootless Podman. macOS requires an existing default loopback SSH
connection to a running rootless Podman Machine. The command pins the existing connection name
for this invocation, ignores remote/cloud environment overrides and observes rootless state
before effects. It does not install Podman, attach/change a connection, initialize or start a
machine. Missing runtime/connection, rootful or remote/cloud targets are `UNSUPPORTED`.

The Podman server must support the documented named-volume `nocreate` option. Older releases
such as 5.8 lack it; an explicit invalid-option result is `UNSUPPORTED` with no automatic update.
The [current Podman volume contract](https://docs.podman.io/en/latest/markdown/podman-run.1.html)
allows this safety fence; actual version compatibility is unverified on this host.

Startup validates private caller-owned inputs outside Git, inspects ownership and image labels,
creates/reuses only the expected persistent volume, and checks configuration signatures.
Kanidm `configtest` uses network-disabled temporary data. Server creation is stopped first:
its returned ID, actual mount and volume labels are rechecked before starting that ID.
This container reference prevents ordinary removal of its volume. `nocreate` prevents a
missing volume from silently becoming empty data. No `--replace`, prune, migration, deletion or
security-label mutation is performed. On enforcing SELinux, operator-prepared input labels
must already permit container read access; xtask never relabels files or disables isolation.

Every Podman call has a 60s bound (connection/info 10s), at most 4 MiB per captured output stream,
private temporary logs and an owned CLI process group. Readiness after start has one 1–300s
budget covering all inspections/probes (default 60s), with 5s container health checks. Ctrl-C,
SIGTERM and SIGHUP persist cancellation across calls and stop only the owned CLI group.
Remote interruption/timeouts can leave created/running state: inspect status/logs before retry;
xtask never deletes retained data or guesses that an ambiguous operation did not happen.

`status` reports inspected state/health/exit code and ABSENT only after the documented exists
result. Native/external entries are `NOT_RUN`/NOT_OBSERVED. `logs` reads at most 200 lines from
both streams of an inspected owned ID. `down` verifies stop on that same ID and retains its
container/volume. Names alone never authorize effects or log reads. Drift fails closed; review
recreation/upgrades separately. Ownership labels are an operational guard, not isolation from a
same-user administrator: do not concurrently edit connection definitions, forge labels or force
remove containers/volumes. Backups and production deployment remain operator-owned.

### Package-local UI-system previews

`ui-preview leptos` and `ui-preview cmp` are independent playgrounds for the shared UI system.
They start no product app, API, PostgreSQL cluster, identity provider or audio adapter.

| Renderer | `build` | `verify` | `run` |
| --- | --- | --- | --- |
| `leptos` | Trunk builds `packages/ui-system/leptos/preview.html` to `target/ui-foundation-preview` | native `cantos-ui` SSR package tests, WASM clippy with CSR `preview` and warnings denied, then the same Trunk build | Only Trunk serves `preview.html` on `127.0.0.1:8083`; readiness requires HTTP 200 with the gallery's own title |
| `cmp` | Installed Gradle runs `compileKotlinDesktop` in `packages/ui-system/cmp` | Gradle runs `desktopTest` | Gradle runs `run`; xtask supervises the foreground task and its process group until Ctrl-C or a successful task exit |

Leptos `run --open` opens the browser after readiness. CMP opens its own desktop window; no
window-readiness, Android/iOS-device or accessibility evidence is implied by launching its task.
Both use the existing process-group supervisor, which signals only groups this invocation owns.
CMP `run` additionally needs POSIX `/bin/sh`; other hosts report `UNSUPPORTED`. Its fixed exec
chain supplies the launcher PID, task-local JVM options, in-process Kotlin compilation and
disabled configuration caching. The Gradle project rejects a missing or detached run JVM before
task execution. `--no-daemon` alone can still create a detached single-use JVM; successful task
launch does not prove window readiness or complete process cleanup. Record actual shutdown
observations separately.

CMP requires a real package-local Gradle project (both settings and build files), an installed
Gradle 8.14.3 and a compatible JDK 17 or newer on `PATH`. All Gradle invocations use
`--no-daemon` and `-Dorg.gradle.java.installations.auto-download=false`: the JDK must already be
available. Gradle resolves declared project dependencies normally. xtask never invokes a
downloading wrapper, installs Gradle or provisions a JDK. A missing or incompatible prerequisite
is `UNSUPPORTED` before any build or launch; an actual Gradle compilation/test failure is `FAIL`.
The Theatre `--mobile` surface remains reserved and is separate from this desktop preview.
Shared CMP previews do not create a Studio mobile client.

`ui-preview cmp android-build` compiles `compileDebugKotlinAndroid` in the isolated
`packages/ui-system/cmp/android-preview` library with the same installed Gradle 8.14.3/JDK
checks. That project consumes the shared CMP source and requires a locally configured Android
SDK. The command starts no IDE, emulator, app or device process. Its build is optional and
separate from the desktop build/test gate, so desktop users do not acquire an Android SDK
requirement. `android-build` applies only to CMP; Leptos refuses it as a usage error.

### Verification gates

Every check is a gate with declared prerequisites; `verify`, `preflight` and `ci` select from the
same catalog in [`gate.rs`](src/gate.rs).

| Gate | Commands | Needs |
| --- | --- | --- |
| `repository` | typed `services validate`; `scripts/check_repository.py`; `python3 -m unittest discover -s scripts -p 'test_*.py'`; `git diff --check`; `git diff --cached --check` | cargo, python3, git |
| `rust-format` | `cargo fmt --all -- --check` | rustfmt |
| `script-ir-reference` | `scripts/reference_script_ir.py` (independent Python c1 oracle) | python3 |
| `rust-lint` | `cargo clippy <workspace or -p …> --all-targets --locked -- -D warnings` | clippy |
| `rust-tests` | `cargo test <workspace or -p …> --locked` | cargo |
| `dbsp` | `cargo clippy -p cantos-api --features dbsp-experimental --all-targets --locked -- -D warnings`; `cargo test -p cantos-api --features dbsp-experimental --locked` | clippy |
| `contract-tests` | `cargo test -p cantos-contracts`; `cantos-api` Script IR, Studio/import/production/audio-intake wire, JSON/MessagePack wire, diff, CLI admission, WAV and ordinary audio-intake test targets | cargo |
| `wav-reference` | `cargo build -p cantos-api --bin inspect-wav`; `scripts/reference_wav_inspection.py` | cargo, python3 |
| `audio-inspection` | `cargo test -p cantos-api --test audio_intake --locked -- --ignored` | cargo, trusted installed ffmpeg and ffprobe |
| `web-lint` | `cargo clippy -p cantos-ui -p cantos-studio -p cantos-theatre --all-targets --target wasm32-unknown-unknown --features cantos-ui/preview --locked -- -D warnings` | clippy, wasm32 target |
| `web-build` | `trunk build --locked` per Web client | trunk, wasm32 target |
| `ui-preview` | `trunk build preview.html --locked --dist ../../../target/ui-foundation-preview` in the shared Leptos package | trunk, wasm32 target |
| `cmp-ui-preview` | installed Gradle `desktopTest` in the shared CMP package, with no persistent daemon or JDK provisioning | compatible gradle, java, real CMP Gradle project |
| `theatre-cmp-demo` | installed Gradle `:shared:desktopTest` in the isolated Theatre mock | compatible gradle, java, real Theatre mock project |
| `mutation` | `scripts/mutate_script_ir.py` (nine bounded mutants in a disposable offline copy) | cargo, python3 |
| `postgres` | `scripts/test_postgres.py [--keep]` (new disposable cluster; never an inherited URL) | cargo, python3, initdb, pg_ctl, psql, ffmpeg, ffprobe |

| `verify` suite | Gates |
| --- | --- |
| `repository` | repository |
| `rust` | rust-format, rust-lint (workspace), rust-tests (workspace) |
| `web` | web-lint, web-build for Studio and Theatre, ui-preview |
| `contracts` | script-ir-reference, contract-tests, wav-reference, audio-inspection |
| `mutation` | mutation |
| `dbsp` | dbsp |
| `postgres` | postgres |

The mutation and PostgreSQL runners use `cargo --offline`; run `cargo fetch --locked` once on a
new machine. On macOS setups affected by `/var` versus `/private/var`, prefix Python-heavy runs
with `TMPDIR=/private/tmp`.

The `audio-inspection` gate explicitly runs the ignored real-media tests that ordinary
`cargo test` skips. Tests encode owned synthetic signals into temporary files at runtime,
then exercise the actual bounded read-only inspection adapter at all four explicit Opus
profiles and its invalid-media cases. No audio is committed to Git. This test-only fixture
encoding does not add an encoder, TTS provider or production worker to the backend.
The adapter probes and decodes to a discarded output; it never converts or publishes media.
`ffmpeg -version` and `ffprobe -version` must succeed before any selected gate runs.
Missing tools produce `UNSUPPORTED`; skipped or unreachable inspection remains `NOT_RUN`.
xtask never installs these tools. The Rust and PostgreSQL CI jobs provision the distribution's
trusted `ffmpeg` package, which supplies both executables. The PostgreSQL suite also needs them
for its actual-byte private-intake/storage/replay tests.

### Outcome vocabulary

Multi-step commands end with a summary:

| Status | Meaning | Exit |
| --- | --- | --- |
| `PASS` | ran and succeeded | success |
| `FAIL` | ran, or could not start, and failed | failure |
| `NOT_RUN` | selected but not executed: skipped with `--skip`, stopped by an earlier failure, or blocked by another step's missing prerequisite | success only when explicitly skipped and another step passed |
| `UNSUPPORTED` | cannot run here: a tool is missing, or the target has no implementation (mobile) | failure |

Prerequisites of every selected step are checked before the first one runs, so a long run never
ends with a surprise missing tool. Execution stops at the first failure. In the engineering skill's
reporting vocabulary, `PASS` is `passed`, `FAIL` is `failed`, `NOT_RUN` is `not-run` and
`UNSUPPORTED` is `unavailable`. Usage errors exit with status 2.

### preflight

`cargo xtask preflight` runs every gate. `--changed-only` compares the working tree with the merge
base of `--base` (default `origin/develop`): committed, staged, unstaged and untracked paths, with
both ends of a rename. A missing base fails before any gate runs. `--dry-run` prints the plan.

Selection ([`preflight/select.rs`](src/preflight/select.rs)) is pure and conservative:

- `repository` and `rust-format` always run.
- A path inside a Cargo package selects that package and, through `cargo metadata`, every package
  that depends on it (`rust-lint`/`rust-tests` with `-p`). A package's top-level `README.md` is a
  document. All of `packages/contracts` (fixtures, schemas, examples and the DTO crate) is an input
  of `cantos-contracts` and its dependents; only its non-fixture Markdown is documentation.
- An affected Web package selects `web-lint`/`web-build` for that client. Shared
  `packages/ui-system` tokens, icons, Leptos components and their generators affect `cantos-ui`,
  both Web consumers and `ui-preview`; shared semantic inputs and generators also select
  `cmp-ui-preview` and `theatre-cmp-demo`. Package-local CMP source, Gradle and resource changes
  select both local CMP gates directly because Kotlin is not a Cargo workspace member. Renderer documentation alone remains
  a document, and a Leptos-only change does not require Gradle.
  An affected `cantos-api` selects `dbsp`, `wav-reference`, `audio-inspection` and `postgres`.
- Script IR fixtures select `script-ir-reference`; Script IR sources and tests select `mutation`.
- Primitive scripts select the gate that exercises them; a script with no known impact selects every gate.
- Documents, templates, skills and issue forms need only the always-on gates. The service
  manifest, PostgreSQL/Kanidm examples and configuration docs are statically validated by
  `repository`; no Podman or runtime input is required. The Studio Mobile scope notice is documentation only.
  Theatre mock Kotlin/Gradle/native host inputs select `theatre-cmp-demo`; its docs are documents.
- Shared Cargo inputs (`Cargo.toml`, `Cargo.lock`, `.cargo/`, toolchain/format/lint files),
  verification tooling (`xtask/`, `.github/workflows/`), an
  unclassified path or an empty change set select every gate.

`--skip GATE` (repeatable) is the only way to leave out a selected gate. The gate is reported as
`NOT_RUN`, never `PASS`. Use it, for example, for `postgres` on a machine without PostgreSQL, and
state it in the pull request.

### ci

| Job | Gates |
| --- | --- |
| `repository` | repository, script-ir-reference |
| `rust` | rust-format, rust-lint (workspace), rust-tests (workspace), dbsp, wav-reference, audio-inspection |
| `web` | web-lint, web-build (Studio and Theatre) |
| `postgres` | `cargo fetch --locked`, then postgres |

`cargo xtask ci` runs all four locally; [the workflow](../.github/workflows/repository-checks.yml)
runs one per GitHub job with `RUSTUP_TOOLCHAIN=1.87.0`. CI has no `--skip`. Every full-preflight gate
runs in CI except `mutation`, `cmp-ui-preview` and `theatre-cmp-demo`. Mutation stays opt-in through
`verify mutation` and is selected by preflight when Script IR sources change. The CMP desktop preview is
a local `ui-preview cmp verify` / preflight gate until a separately provisioned Gradle CI lane is
added. Existing `verify web` and the Web CI job never require the Kotlin toolchain. Branch
protection and required checks are configured outside this repository.

### doctor

Reports the selected `rustc` against the pinned 1.87, each tool and its version, which gates can
run here, the unsupported production Theatre Mobile surface and the separate mock and Kanidm's configuration-only status. It exits with
failure only when Cargo, Git or Python 3 is missing.

### compose-preview

Capture an existing `@Preview` through the official Android CLI's
[`android studio render-compose-preview`](https://developer.android.com/tools/agents/android-cli/commands/studio_render-compose-preview).
It requires Android Studio Quail 2+ running with the selected Android Gradle project open and ready;
it does not require an emulator or phone. `ui-system-cmp` selects the isolated shared-component
library in `packages/ui-system/cmp/android-preview`; compile it with `ui-preview cmp android-build`
before rendering its thin `@Preview` wrappers around shared source. Theatre Mobile now selects
the isolated mock Gradle root and its shared Android `TheatrePreviews.kt`; compile it with
`theatre-demo android-build` and sync the root with `-Pcantos.android=true`. Missing CLI or
Quail 2+ readiness still makes capture **UNSUPPORTED**. Studio Mobile has no capture target. Tool/IDE setup is manual and never performed by
xtask. This optional operation is outside `preflight`/CI; its deterministic adapter tests run
in the existing repository gate.

`--file` resolves from the repository root and must stay inside the selected fixed project
directory: the product's mobile directory, or `packages/ui-system/cmp/android-preview` for
`ui-system-cmp`. Files elsewhere in the parent CMP package are refused too. `--project` is derived
from that directory; `--pid` selects an IDE instance.
`--android-cli` can point to the official executable when the legacy SDK tool occupies `PATH`.
Each CLI operation has a 1–300 second timeout (default 60). `--label` is provenance only:
preview annotations/state configure theme, locale, font scale and layout. `--dry-run` prints
the plan without probing the CLI/IDE or writing artifacts; its result is `NOT_RUN`.

Capture execution currently requires a POSIX host so the adapter can stop only its owned CLI
process group, including descendants, on return, failure, timeout or interruption. It never
signals the connected Android Studio process. Each command retains at most 4 MiB per output
stream; larger output fails. Preview sources are limited to 4 MiB and PNGs to 32 MiB. Special
files, symlinked project roots and linked output files are rejected. Fresh capture bundles and
adapter-written artifacts use private permissions; existing output paths are never overwritten.

The Python capture primitive saves a fresh PNG, unchanged semantics text tree (or legacy JSON),
raw logs and source/command/output provenance under ignored `artifacts/ui-evidence/compose-preview/`.
Capture metadata schema v2 names the semantics format/path and hashes. The current CLI text tree
exposes class names, bounds and text; it does not prove accessibility actions or states. Missing
prerequisites are `UNSUPPORTED`; render/invalid output/timeouts are `FAIL`. A successful capture
proves files exist, not inspected pixels, asserted semantics, interaction, a mobile build or
Android/iOS device behavior. Follow the [CMP workflow](../.agents/skills/cantos-cmp-mobile/references/compose-preview.md)
to inspect and report them; [tooling evidence](../docs/evidence/compose-preview-tooling.md)
separates the original mock adapter checks from real rendering recorded in the
[PR43 Android host acceptance](../docs/evidence/cmp-pr43-android-preview.md).

## Code layout

| Module | Responsibility |
| --- | --- |
| `main.rs`, `cli.rs` | clap definitions and dispatch; usage errors exit 2 |
| `workspace.rs`, `config.rs` | root discovery; ports, packages, directories and versions, checked against the repository by tests |
| `command.rs` | immutable `CommandSpec` values and the `Runner` abstraction |
| `tools.rs` | tool detection behind `ToolProbe` |
| `pipeline.rs`, `report.rs` | prerequisite checks, sequential execution and the summary |
| `gate.rs`, `build.rs`, `ci.rs` | gate catalog, build plans and CI jobs |
| `ui_preview.rs` | package-local Leptos/CMP preview plans and supervised execution |
| `run.rs`, `process.rs`, `probe.rs` | launch plans, supervised process groups, port ownership and readiness |
| `preflight/` | Git change collection, the Cargo package graph and pure selection |
| `doctor.rs` | read-only environment report |
| `services/` | typed process inventory/config validation, explicit local Podman lifecycle, bounded command boundary |
| `compose_preview.rs` | optional host-preview command and status mapping; the Python primitive owns capture/log/output validation |

Plans are values: tests assert exact commands with a recording runner and fake tools, while
process tests start and stop real `sh` and `python3` children. Run them with
`cargo test -p xtask --locked` (part of `verify rust`).

## Adding a capability

Add a command only for an implemented, reviewed capability. The isolated Theatre mock is
planned through `theatre-demo`; production native listening remains `UNSUPPORTED`. Studio is web-only;
do not add a mobile plan, scaffold or target without an explicit future roadmap request. A new script becomes a gate
in `gate.rs`, is classified in `preflight/select.rs` and, if CI must run it, is added to a job
in `ci.rs`. Release, deployment and provider-backed commands wait for their infrastructure.

### theatre-demo

An explicitly isolated mock command surface; starts no API, database, identity or audio adapter.

```sh
cargo xtask theatre-demo build          # :shared:compileKotlinDesktop
cargo xtask theatre-demo verify         # :shared:desktopTest; common + JVM Compose tests/captures
cargo xtask theatre-demo run            # same app, supervised diagnostic desktop window
cargo xtask theatre-demo android-build  # -Pcantos.android=true :androidApp:assembleDebug
cargo xtask theatre-demo ios-build --kotlin-native-home /absolute/installed/native-2.2.20
```

Build/verify/run require installed Gradle 8.14.3 and a full JDK 17+. Android checks an existing
absolute SDK path, platform35 and build-tools35.0.0 before Gradle; downloads are disabled.
iOS checks the installed compiler's exact2.2.20 version plus an available Xcode simulator SDK,
then links only `:shared:linkDebugFrameworkIosSimulatorArm64`; no compiler/LLVM is downloaded.
Missing prerequisites are `UNSUPPORTED`, failed executed steps are `FAIL`.

The local `theatre-cmp-demo` preflight gate runs mock common/JVM tests when its source/build
inputs or consumed CMP/shared tokens change. It does not stand for native acceptance or
component qualification. It is outside CI until a Gradle lane is separately provisioned.
`run` aligns JVM args, passes its owned launcher PID, rejects detached run JVMs, and uses
only stored owned process groups for cleanup. The supervisor keeps exited leaders unreaped
until their group has been signalled, preventing descendant leaks without stale-PGID lookup.
Details and evidence live in the [mock guide](../apps/cantos-theatre/mobile/README.md).

The retained local fixture can now select its existing test context with
`CANTOS_TEST_CONTEXT=editorial|catalog|audio` on `run cantos-studio --web --test-cluster`.
Editorial is the default. Audio selects the fresh runner's exact AudioLifecycle database and
private root; it does not provision identity/storage or use an inherited database URL.
PostgreSQL verification now discards completed suite databases only after their tests pass,
retaining all five restart fixtures for the existing crash/replay assertions. This bounds local
disk consumption without removing any test or weakening PostgreSQL durability settings.
