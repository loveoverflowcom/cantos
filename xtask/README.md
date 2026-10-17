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
| Cargo, rustup, Trunk, Python 3, Node.js, Git, PostgreSQL binaries | Primitive tool execution |
| `scripts/*.py` | Repository checks, independent oracles, the mutation runner, the disposable PostgreSQL runner and its retained-fixture API host. Their logic is reused, not rewritten |
| `cargo xtask` | Targets, profiles, surfaces, gate catalog, prerequisite checks, changed-path selection, process supervision and the CI job contract |
| GitHub Actions | Runner lifecycle and environment: the pinned toolchain, Trunk, PostgreSQL 17 binaries. Each job then calls `cargo xtask ci --job <job>` |

Tool detection never installs anything. Explicit `verify behavior --setup` alone installs its
locked npm development dependency; ordinary verification never fetches it. xtask does not
install other toolchains, initialize/start Podman Machine, change connections, call
providers, create cloud resources, publish or deploy, with **two explicit, opt-in exceptions**:
`distribute firebase` ([below](#distribute-firebase)) builds the Theatre Android *demo* APK and
hands it to Firebase App Distribution, and `deploy firebase` ([below](#deploy-firebase)) builds one
Leptos Web app and publishes its static bundle to Firebase Hosting. Only explicit `services up`
starts local container infrastructure; build/test/run/preflight/ci never start it and never reach
distribution or deployment.

## Commands

```sh
cargo xtask build cantos-api [--release]          # cargo build --locked -p cantos-api --bins
cargo xtask build cantos-api --dbsp-experimental # adds --features dbsp-experimental (opt-in incremental core; no consumer)
cargo xtask build cantos-forge [--release]        # cargo build --locked -p cantos-forge --bins
cargo xtask build cantos-studio --web [--release]  # trunk build --locked in apps/cantos-studio/web
cargo xtask build cantos-theatre --web [--release]         # trunk build --locked in apps/cantos-theatre/web
cargo xtask web-performance cantos-studio|cantos-theatre  # release build + offline asset inventory
cargo xtask build all [--release]                  # the API, Forge and both Web clients

cargo xtask run cantos-api [--test-cluster] [--web-origin URL]
cargo xtask run cantos-studio --web [--test-cluster] [--web-origin URL] [--open]
cargo xtask run cantos-theatre --web [--test-cluster] [--web-origin URL] [--open]
cargo xtask app cantos-studio --web [--open]
cargo xtask app cantos-theatre --web [--open]
cargo xtask seed theatre [--reset]                 # synthetic Theatre catalog in the retained disposable cluster

cargo xtask ui-preview leptos build|run|verify       # package-local Leptos gallery
cargo xtask ui-preview leptos run --open             # loopback browser preview on 8083
cargo xtask ui-preview cmp build|run|verify          # package-local CMP desktop preview
cargo xtask ui-preview cmp android-build             # compile the isolated Android preview library
cargo xtask theatre-host android-release             # production host release APK/AAB + boundary inspection
cargo xtask theatre-host android-debug --api-origin URL  # production host debug APK for the live catalog; installs nothing

cargo xtask verify repository|rust|forge|web|contracts|mutation|dbsp
cargo xtask verify behavior [--capability ID] [--setup] [--conformance] # opt-in Quint + pure-core replay
cargo xtask verify postgres [--keep]
cargo xtask verify postgres --suite theatre-firebase # narrow Firebase + SMTP registration SQL only
cargo xtask verify postgres --suite schema           # current baseline/refusal and SQL acceptance
cargo xtask verify theatre-auth --suite firebase-emulator|firebase-web-e2e [--dry-run]
cargo xtask verify kani [--harness catalog-revision|image-bounds|wav-payload|submission-window|publication-commit|listener-access|listener-reconcile|byte-range|playback-grant|public-cover|forge-attempt-ledger]...

cargo xtask preflight [--changed-only] [--base REF] [--kani] [--skip GATE]... [--dry-run]
cargo xtask ci [--job repository|rust|web|postgres] [--dry-run]
cargo xtask doctor
cargo xtask services list|validate|status
cargo xtask services [--instance NAME] up postgresql|kanidm [--timeout-seconds 60]
cargo xtask services [--instance NAME] up --all
cargo xtask services [--instance NAME] logs postgresql|kanidm
cargo xtask services [--instance NAME] down postgresql|kanidm|--all
cargo xtask distribute firebase cantos-theatre --channel demo [--artifact apk] \
  (--groups ALIASES | --testers EMAILS)... [--app-id ID] [--release-notes TEXT | --release-notes-file PATH] [--dry-run]
cargo xtask deploy firebase cantos-theatre|cantos-studio --web --project PROJECT_ID \
  --channel live|PREVIEW_CHANNEL [--expires 7d] [--dry-run]   # explicit static Web deploy to Firebase Hosting
cargo xtask deploy vps cantos-api --host USER@HOST [--bootstrap|--rollback|--status] \
  [--smoke-origin https://API_HOST] [--dry-run]               # explicit API deploy to the production VPS
cargo xtask compose-preview cantos-theatre|ui-system-cmp --file PATH --composable NAME \
  [--label SCENARIO] [--pid PID] [--android-cli PATH] [--timeout-seconds 60] [--dry-run]
cargo xtask visual-compare --reference PNG --actual PNG --scenario ID \
  [--metadata JSON] [--threshold 0] [--tile-size 16] [--output PATH] [--python PATH]
cargo xtask visual-compare --self-test [--python PATH]
cargo xtask visual-compare --spike-pairs JSON [--python PATH]
```

`cargo xtask <command> --help` documents every flag.

### Targets, surfaces and profiles

`cantos-api` is the Axum/PostgreSQL backend. `cantos-forge` is the producer-side CLI
([ADR 0022](../docs/decisions/0022-cantos-forge-producer-cli.md)); it takes no surface flag and
xtask builds its binaries and `verify forge` checks its Rust package and offline Functions
wrapper. Ordinary gates use fixtures and injected doubles; they never call a live provider or
deploy a Function. No `run` command starts Forge.
`cantos-studio` (creators) is web-only: pass
`--web`; `--mobile` is rejected as a usage error (exit 2), not a reserved implementation.
`cantos-theatre` (listeners, alias `cantos`) supports `--web` and `--mobile`; one must be named.
Theatre Mobile has an isolated mock Gradle project. Production `--mobile` remains
`UNSUPPORTED` because native listening adapters are absent; use `theatre-demo` for mock evidence
and `theatre-host android-release` for the production host's release isolation.
Studio Mobile is excluded from all build/run/test/preview plans, even if files appear in its
out-of-scope directory. `build all` covers the API, Forge and both Web clients and notes that Theatre
Mobile and `services/kanidm` (configuration only) are not built. See the
[platform matrix](../docs/architecture/repository-layout.md#platform-scope).

`dev` is the default unoptimized build. `--release` passes `--release` to Cargo or Trunk and
produces an optimized local artifact. It is not a production image, a signed bundle or a
deployment; none of those exist yet. (`distribute firebase` hands a *demo* debug APK to testers;
it is not a production release either. `deploy firebase` publishes the release Web bundle to
Firebase Hosting on explicit request; that is static delivery, not a claim about the API or
production readiness.)

Trunk runs with `NO_COLOR=true`, matching the documented commands (Trunk 0.21 rejects an
inherited `NO_COLOR=1`).

### Optional Web release inventory

```sh
cargo xtask web-performance cantos-theatre
cargo xtask web-performance cantos-studio --output artifacts/web-performance/studio-baseline
cargo xtask web-performance cantos-studio --baseline artifacts/web-performance/studio-baseline/inventory.json
cargo xtask web-performance cantos-theatre --dry-run
```

This optional command runs the existing locked release Trunk build with explicit app config,
entrypoint and fresh run-owned `dist`, then records source/build fingerprints and per-asset SHA256,
raw/gzip/Brotli estimates, HTML/CSS references and static streaming-loader observations. `--output`
must be new and beneath `artifacts/web-performance`; existing evidence is never overwritten.
Inherited `TRUNK_*` overrides are refused so a redirected build cannot inventory an old dist.
Source changes during the build fail measurement. Optional Python Brotli or an installed Brotli
CLI supplies quality-11 estimates; absent tools are recorded `UNSUPPORTED` in the report, without
installation. Gzip uses level 9 and mtime 0. Baseline comparison exposes changed bytes under the
same URL, added/removed URLs and raw-byte delta; it establishes no deployed rollback behavior.

The command creates no service and changes no static-host/API cache policy. It adds no required
gate: primitive tests run in `repository`, command-plan tests in `rust-tests`. Static filename
shape never establishes immutable eligibility; expected MIME and loader text do not establish
HTTP headers or streaming execution. HTML-linked totals are a subset of initial network work.
Browser timings, V8 cache hits, host compression/headers and deploy/rollback acceptance remain
`NOT_RUN`. See the [pattern audit and limits](../docs/evidence/web-performance-patterns.md).

### run and app

| Command | Starts | Requires |
| --- | --- | --- |
| `run cantos-api` | `cantos-server` on 127.0.0.1:8080 after building it | `DATABASE_URL` for a migrated local app-role database, or `--test-cluster` |
| `run cantos-studio --web` | the API with `CANTOS_WEB_ORIGIN=http://127.0.0.1:8081`, then `trunk serve` on 8081 (its `Trunk.toml` proxies `/api/` to 8080) | as `run cantos-api` |
| `run cantos-theatre --web` | the API with `CANTOS_WEB_ORIGIN=http://127.0.0.1:8082`, then `trunk serve` on 8082; checks anonymous discovery directly and through the proxy | as `run cantos-api`; a readable Theatre public catalog, including a legitimate empty page |
| `app cantos-theatre --web` | only `trunk serve` on 8082; warns prominently when the existing API's public catalog cannot be reached or decoded | no API for explicit offline previews; live mode needs an API already listening on 8080 |
| `app cantos-studio --web` | only `trunk serve` on 8081 | an API already listening on 8080, for example `run cantos-api --web-origin http://127.0.0.1:8081` |

**Compatibility proposal — pending review ([#183](https://github.com/loveoverflowcom/cantos/issues/183)).**
Option A changes Theatre `run` from frontend-only to API+Web, matching Studio. Existing
frontend-only users should replace `run cantos-theatre --web` with
`app cantos-theatre --web`; keep the explicit preview query when working offline. A running
API on 8080 is reused only through `app`, never taken over by `run`. This implementation is
delivered for review in a Draft PR; it does not declare the compatibility choice accepted.
See the [decision and evidence ledger](../docs/evidence/theatre-local-run.md).

Web `run` profiles select their actual Trunk origin. An explicit `--web-origin` must exactly
match `http://127.0.0.1:8081` for Studio or `http://127.0.0.1:8082` for Theatre; a different
port or `localhost` is rejected before launch. Host identity matters for cookies and callbacks.
Standalone `run cantos-api --web-origin URL` remains configurable within its loopback HTTP
origin contract. `app` starts no API and takes neither `--web-origin` nor `--test-cluster`.
The development host has one Web origin; simultaneous Studio and Theatre authentication
against that one host is not established by these profiles.

`--test-cluster` serves the API through `scripts/serve_studio.py` against the disposable cluster
retained by `cargo xtask verify postgres --keep`; that script refuses any other database. xtask
never reads, prints or invents a database URL. It sets `CANTOS_ENV=development` and the loopback
bind address; the server itself refuses anything else. Theatre selects
`CANTOS_TEST_CONTEXT=theatre` and the retained record's separate `theatre_app_url`, backed by
the disposable audio fixture with bounded public-discovery read grants. That fixture has no
published Work. After an explicit `cargo xtask seed theatre` (below), `serve_studio.py` serves
the recorded seed database instead, but only when its record names the same retained cluster
and port; it prints which one it serves. Neither enables public media delivery. A retained
record predating `theatre_app_url` must be recreated by an explicit fresh
`verify postgres --keep`, never patched to point to another DB.

With an already-exported migrated local app-role `DATABASE_URL`, the one-command Theatre
profile is:

```sh
cargo xtask run cantos-theatre --web
# Alternative: only after a successful explicit disposable verification retained its cluster.
cargo xtask run cantos-theatre --web --test-cluster
```

The [Theatre runbook](../apps/cantos-theatre/web/README.md#build-run-and-checks) lists modes,
manual HTTP smoke checks and corrective actions. `run`/`app` never provision PostgreSQL,
roles, migrations, credentials, Kanidm, S3 or Podman. The explicit verification command owns
its disposable test-cluster setup; operator-owned databases remain operator-managed.

Process lifecycle:

- Preconditions are checked before any build: required variables and files, tools, and that
  every port is free. A taken port fails with an explanation; xtask never stops a process it did
  not start, and never looks processes up by port or name.
- Each service starts in its own process group with inherited output. Standalone API and
  Studio retain their existing TCP API readiness and Studio's HTML-title readiness. Theatre
  full-stack readiness requires HTTP 200 with a JSON catalog envelope from
  `/api/v1/theatre/works?limit=20` on 8080, then the Theatre HTML title and the same JSON catalog-envelope
  route through Trunk on 8082. A zero-item page is success;
  a 404, typed unavailable error, non-JSON body or connection failure is not ready. Probes use
  bounded waits/backoff: the API has 60 seconds and Trunk up to 15 minutes for its first WASM
  build. A process that exits before readiness fails the command.
- `/api/v1/theatre/session` is optional for anonymous discovery and is not a readiness gate.
  An unconfigured/unauthenticated session's 404/401 does not invalidate a working public catalog.
  A successful catalog probe establishes no provider login, publication delivery or playback.
- Theatre `app` probes its existing catalog dependency before launch and prints a prominent
  `WARNING` if it is missing or invalid (red on a suitable terminal). It still permits explicit
  offline previews. Its final readiness message always describes **frontend-only** readiness;
  it does not certify an integrated API, including when the advisory probe succeeded.
- Ctrl-C, SIGTERM or SIGHUP stops every started group, newest first: SIGINT (graceful for Axum and
  Trunk), then SIGKILL after 10 seconds. If any service exits by itself, the others are stopped and
  the command fails.
- If xtask itself is killed with SIGKILL its children survive; the next run finds their ports taken
  and refuses to start. `--open` uses `xdg-open` or `open` after readiness.

The Trunk development proxy is a convenience. The browser walkthroughs recorded in
`docs/evidence/` used the API serving `apps/cantos-studio/web/dist` same-origin; do not cite a
`trunk serve` session as that evidence.

### seed

`cargo xtask seed theatre` creates synthetic development data for the Theatre catalog, only in
the disposable cluster retained by `verify postgres --keep`. It is never a gate, preflight
selection or CI job; the full PostgreSQL gate already runs the same seed test as a regression
without retaining its database. The primitive `scripts/seed_theatre.py`:

- refuses without a retained record whose `cluster-<number>` directory sits directly under
  `target/revision-evidence/` with `pgdata/PG_VERSION`, and never reads `DATABASE_URL` or any
  inherited `PG*`/`CANTOS_*` variable;
- confirms with `SHOW data_directory` that the recorded loopback port is still served by that
  retained `pgdata` before any write;
- runs `cargo test -p cantos-api --test catalog_http_postgres -- --ignored --exact
  …::theatre_development_catalog_seed` with `CANTOS_TEST_CLUSTER_URL` set to the retained
  admin URL. The test publishes through the real Studio HTTP routes, app-role SQL and private
  Ogg/Opus intake; only the public-delivery observer is the test-only fake;
- writes `target/revision-evidence/theatre-seed.json` atomically and then drops the previous
  seed database recorded for the same cluster.

`--reset` drops only that recorded seed database and removes the record. Requirements: Python 3
and `psql`; seeding also needs Cargo, `ffmpeg` and `ffprobe`. Work and unit IDs are stable across
reseeds, publication IDs are new releases. The [runbook](../docs/deployment/theatre-seeded-catalog.md)
lists the dataset, Web/Android steps, a live withdrawal and troubleshooting. A seeded catalog
is discovery data only: playback authorization stays `rendition_unavailable`.

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
Gradle 8.13, 8.14.1, 8.14.2 or 8.14.3 (8.14.3 preferred; matches the wrappers) and a compatible JDK 17 or newer on `PATH`. All Gradle invocations use
`--no-daemon` and `-Dorg.gradle.java.installations.auto-download=false`: the JDK must already be
available. Gradle resolves declared project dependencies normally. xtask never invokes a
downloading wrapper, installs Gradle or provisions a JDK. A missing or incompatible prerequisite
is `UNSUPPORTED` before any build or launch; an actual Gradle compilation/test failure is `FAIL`.
The Theatre `--mobile` surface remains reserved and is separate from this desktop preview.
Shared CMP previews do not create a Studio mobile client.

`ui-preview cmp android-build` compiles `compileDebugKotlinAndroid` in the isolated
`packages/ui-system/cmp/android-preview` library with the same installed Gradle/JDK
checks. That project consumes the shared CMP source and requires a locally configured Android
SDK. The command starts no IDE, emulator, app or device process. Its build is optional and
separate from the desktop build/test gate, so desktop users do not acquire an Android SDK
requirement. `android-build` applies only to CMP; Leptos refuses it as a usage error.

### Verification gates

The optional [local PNG comparator](../tools/visual-compare/README.md) is an advisory command,
not a merge gate. It requires explicitly prepared Pillow 11.3.0, preserves exact PNGs and writes
fresh `artifacts/ui-evidence/<run-id>/comparison` bundles with metrics, boxes, overlay/diff/mask/
heatmap and detail crops. Equal and different pixels both mean the tool completed; neither
accepts UX or a baseline. Dimension/known scenario mismatches fail without resizing/scoring.
Default verification never installs its dependencies or contacts the network. Standard-library
core tests are included by the repository Python suite; optional real-PNG tests skip visibly
when the exact Pillow pin is absent. `--self-test` runs this focused suite. The opt-in
`--spike-pairs` compares the four isolated, pinned research backends after explicit setup.

Every check is a gate with declared prerequisites; `verify`, `preflight` and `ci` select from the
same catalog in [`gate.rs`](src/gate.rs).

| Gate | Commands | Needs |
| --- | --- | --- |
| `repository` | typed `services validate`; `scripts/check_repository.py`; `python3 -m unittest discover -s scripts -p 'test_*.py'`; `git diff --check`; `git diff --cached --check` | cargo, python3, git |
| `rust-format` | `cargo fmt --all -- --check` | rustfmt |
| `protobuf-reference` | independent Python goldens + pinned isolated prost/protoc exact generated-schema drift oracle + its clippy | cargo, python3, clippy |
| `script-ir-reference` | `scripts/reference_script_ir.py` (independent Python c1 oracle) | python3 |
| `rust-lint` | `cargo clippy <workspace or -p …> --all-targets --locked -- -D warnings` | clippy |
| `rust-tests` | `cargo test <workspace or -p …> --locked` | cargo |
| `forge-functions` | `cargo build --locked -p cantos-forge --bin cantos-forge-worker --target-dir target`, then `node --test test/boundary.test.mjs test/native-worker.test.mjs` in `services/cantos-forge-functions/firebase` | cargo, installed stable Node.js 22+ |
| `dbsp` | `cargo clippy -p cantos-api --features dbsp-experimental --all-targets --locked -- -D warnings`; `cargo test -p cantos-api --features dbsp-experimental --locked` | clippy |
| `contract-tests` | `cargo test -p cantos-contracts`; `cantos-api` Script IR, Studio/import/production/audio-intake wire, JSON/MessagePack wire, diff, CLI admission, WAV and ordinary audio-intake test targets | cargo |
| `wav-reference` | `cargo build -p cantos-api --bin inspect-wav`; `scripts/reference_wav_inspection.py` | cargo, python3 |
| `audio-inspection` | `cargo test -p cantos-api --test audio_intake --locked -- --ignored` | cargo, trusted installed ffmpeg and ffprobe |
| `web-lint` | `cargo clippy -p cantos-ui -p cantos-studio -p cantos-theatre --all-targets --target wasm32-unknown-unknown --features cantos-ui/preview --locked -- -D warnings` | clippy, wasm32 target |
| `web-build` | `trunk build --locked` per Web client | trunk, wasm32 target |
| `ui-preview` | `trunk build preview.html --locked --dist ../../../target/ui-foundation-preview` in the shared Leptos package | trunk, wasm32 target |
| `cmp-ui-preview` | installed Gradle `desktopTest` in the shared CMP package, with no persistent daemon or JDK provisioning | compatible gradle, java, real CMP Gradle project |
| `theatre-cmp-demo` | installed Gradle `:shared:desktopTest :preview-sandbox:desktopTest` in the isolated Theatre mock | compatible gradle, java, both KMP modules |
| `mutation` | `scripts/mutate_script_ir.py` (nine bounded mutants in a disposable offline copy) | cargo, python3 |
| `postgres` | `scripts/test_postgres.py [--keep]` (new disposable cluster; never an inherited URL) | cargo, python3, initdb, pg_ctl, psql, ffmpeg, ffprobe |
| `postgres (theatre-firebase)` | `scripts/test_postgres.py --suite theatre-firebase` (new disposable cluster; Firebase enrollment and SMTP registration SQL only) | cargo, python3, initdb, pg_ctl, psql |
| `postgres (schema)` | `scripts/test_postgres.py --suite schema` (new disposable cluster; current schema bootstrap/refusal and current SQL flows) | cargo, python3, initdb, pg_ctl, psql |

| `verify` suite | Gates |
| --- | --- |
| `repository` | repository |
| `behavior` | behavior (locked model tool, named runs/properties/witnesses/controls; optional pure-core conformance) |
| `rust` | rust-format, rust-lint (workspace), rust-tests (workspace) |
| `forge` | rust-lint (cantos-forge), rust-tests (cantos-forge), forge-functions |
| `web` | web-lint, web-build for Studio and Theatre, ui-preview |
| `contracts` | script-ir-reference, protobuf-reference, contract-tests, wav-reference, audio-inspection |
| `mutation` | mutation |
| `dbsp` | dbsp |
| `postgres` | postgres |
| `kani` | explicitly selected registered `kani` kernels only; opt-in |

The mutation and PostgreSQL runners use `cargo --offline`; run `cargo fetch --locked` once on a
new machine. On macOS setups affected by `/var` versus `/private/var`, prefix Python-heavy runs
with `TMPDIR=/private/tmp`.

`verify forge` is a focused checkpoint for the relocated producer service. The Node gate first
builds the native Rust worker into the repository's explicit `target` directory (independent of
inherited `CARGO_TARGET_DIR`), then uses the built-in runner with explicit test entrypoints for
the isolated boundary and actual Node-to-Rust child transport. Missing test files fail instead
of silently passing with no tests. The test binary path is derived from the test file's
repository location; requests and environment cannot override it. The tests require no npm
installation, Firebase CLI, credential, live provider, network or deployed service. Missing
Cargo/Node or an older/prerelease Node version is
`UNSUPPORTED` before any selected command executes. Local stable Node 22+ is sufficient for
these wrapper tests; their result does not qualify the separately selected hosted Node 22
runtime. This prerequisite is independent of the exact Node/npm pins for Quint.

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

`cargo xtask verify postgres --suite theatre-firebase` selects a separately named narrow gate.
It runs the Firebase enrollment repository tests and existing SMTP registration tests through
the same migration path, each with a nonzero passing test requirement, in a newly initialized
native PostgreSQL cluster. The runner strips inherited database/libpq/Cantos configuration and
stops only its owned cluster on success, failure or interruption. It neither starts containers
nor accepts an external database URL. Its logs and scope record are under
`target/revision-evidence/theatre-firebase/`; it does not overwrite retained full-suite fixtures.
`--keep` is refused for this scope. Media, other database contexts, HTTP hosts and the complete
restart campaign remain unrun by this command. A `PASS` for `postgres (theatre-firebase)` never
satisfies the full `postgres` gate: default verification, preflight and CI still select the full
gate and retain their ffmpeg/ffprobe prerequisites.

`cargo xtask verify postgres --suite schema` selects current schema bootstrap/replay/refusal,
least-privilege and current SQL owner acceptance without media prerequisites. It includes caller
adaptation, imports, revisions, Catalog/Content/Brief and delegation repository/HTTP, actor/Theatre
identity, native credentials,
Studio OIDC and both registration SQL owners. It uses a new owned cluster, locked/offline Cargo
and nonzero passing-test checks per target. It never takes
an inherited database URL or performs a live cutover. Logs and the exact selected scope are under
`target/revision-evidence/schema/`; `--keep` is refused. The scope record lists all integration
targets, registration selectors and skipped tests. Catalog HTTP's nested `audio_lifecycle_http::`
tests (including publication/discovery media fixtures) are excluded because they require real
`ffmpeg`/`ffprobe`. Restart-only tests and their full-suite fixture writers are skipped to preserve
retained fixture records; completed disposable databases are
released between targets. Full media, actual HTTP process/restart and PostgreSQL 18.6 acceptance
remain separate; this gate never replaces full `postgres`.
The retired `legacy-dbsp` suite is no longer accepted. Generic `verify dbsp` remains optional
incremental-core evidence, without a PostgreSQL feed or production consumer.

### Isolated Theatre enrollment testing

`cargo xtask verify theatre-auth --suite firebase-emulator` is an explicit local integration
lane outside ordinary gates, preflight and CI. It uses the existing registration HTTP/application/
PostgreSQL code and an owned Auth Emulator, with a recording mock Kanidm issuer. It is separate
from the registration-only SQL gate. The [runbook](../docs/deployment/firebase-enrollment-proof.md#isolated-local-enrollment-testing)
owns its source-to-test map, separately installed prerequisites, private recipient handling,
recovery and acceptance limits.

`--dry-run` prints a pure plan without tool subprocesses, network, fixture creation or receipt
writes during xtask execution. Missing or incompatible tools produce `UNSUPPORTED`; nothing is
installed automatically. Runtime requires Linux user/network/PID namespace isolation with
loopback-only egress, PostgreSQL 17 binaries, Node and the explicitly selected installed `firebase-tools`
14.2.2 Auth module. It creates only fresh owned fixtures and clears inherited cloud/database/
proxy configuration. Normal summaries include no proof, setup or session credentials.

`--suite firebase-web-e2e` reports `UNSUPPORTED` because the disposable real Kanidm/PKI/Web
fixture and complete browser journey are not implemented. It never converts mock onboarding
or zero selected tests into a successful login. The required setup/OIDC/session/reload/logout
and separate-browser isolation remain open under #242; live provider and deployed-origin
acceptance remain separate.

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

`cargo xtask preflight` runs every ordinary gate; the targeted Kani campaign requires explicit
`--kani`. `--changed-only` compares the working tree with the merge
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
- Non-document inputs beneath `services/cantos-forge-functions` select its Rust lint/tests and
  `forge-functions`; they do not select the API, database or Web clients.
- Protobuf schema, fixtures, Rust adapters and oracle inputs select `protobuf-reference`; schema/fixture/adapter changes also select `theatre-cmp-demo` so both languages consume the independent corpus. The oracle has its own pinned lock and vendored protoc; runtime/WASM builds never run protoc.
- Script IR fixtures select `script-ir-reference`; Script IR sources and tests select `mutation`.
- Non-document `specs/behavior`, `tools/behavior` and `scripts/verify_behavior.py` inputs select
  the default model-only behavior gate; their Markdown needs only the always-on gates.
- Registered observer/codec and private admission/projection seam changes select behavior
  with `--conformance`, preserving the ordinary affected native Cargo gates.
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

### Executable behavior models

`cargo xtask verify behavior` selects executable entries in [the stable catalog](../specs/behavior/catalog.json);
`--capability ID` narrows the selection and `--setup` explicitly installs the official integrity-locked
Quint npm development tool with lifecycle scripts disabled. Node/npm are provided separately.
The shared tool probe validates the exact installed versions/lock before pipeline execution.
Missing/incompatible tools are `UNSUPPORTED`; partial/error/zero-evidence model runs are `FAIL`.
The Python primitive uses existing bounded POSIX process cleanup and retains fresh commands,
seeds, type/effect output, raw ITF, input/tool hashes and counterexamples under ignored
`target/verification/behavior/`. By default it checks sampled sequential models only; implementation
conformance and exhaustive proof are NOT_RUN. `--conformance` adds a separately reported
test-only observer of actual private-admission pure-core decisions, with synthetic shell
effects and full immutable receipts; it never proves IO/database/runtime acceptance. See [scope and reproduction](../specs/behavior/README.md).

This gate is outside full preflight/CI until its toolchain is separately provisioned.
Non-document model/catalog/tool inputs and the behavior primitive select it in changed-only
preflight. Shared verification changes retain conservative full selection, including behavior when
changed model/tool or registered observation inputs select it. The optional conformance
observer runs production pure functions in a unit-test binary, never a database/provider
process. See the [observation protocol](../specs/behavior/observation-protocol.md).

### Optional targeted Kani proofs

`cargo xtask verify kani` selects exactly eleven registered propositions. Ten are API-library
kernels: `catalog-revision`, `image-bounds`, `wav-payload`, `submission-window`,
`publication-commit`, `listener-access`, `listener-reconcile`, `byte-range`, `playback-grant`
and `public-cover`. One is in the `cantos-forge` package: `forge-attempt-ledger`
(`services/cantos-forge-functions/src/domain/attempt.rs`). Every kernel names exactly one workspace package
(the closed allowlist is `cantos-api` and `cantos-forge`) and passes it to the primitive as
`--package`; the primitive refuses any other name, and an absent value means `cantos-api`.
Repeat `--harness` to select a subset;
unknown selectors and arbitrary verifier arguments are rejected. Registration owns the actual
fully qualified harness names, unwind 2, expected covers (6/13/7/8/6/5/7/6/6/6 for the API
kernels in the order above, 9 for `forge-attempt-ledger`) and reviewed unreachable checks (none
for the Forge kernel).
The last three API kernels were registered for #170/#171 without an actual Kani run (pinned x86_64 Linux
model unavailable on the authoring host); see the [proof ledger](../docs/assurance/kani.md#public-media-kernels-registered-not-yet-run).
The earlier [execution record](../docs/assurance/kani.md) describes their production reach and
limits. The listener kernel checks actual keyset order/expiry; historical Web mock harnesses remain outside this registry.

The registered Forge kernel's proof is `NOT_RUN`: the registration/selection tests and the
primitive's synthetic tests are not a Kani execution of its harness. It becomes evidence only when
`cargo xtask verify kani --harness forge-attempt-ledger` runs on a host with the pinned Kani
0.68.0 bundle and its retained output is inspected. It adds no new tool, dependency or CI job:
the same pinned bundle, primitive and opt-in gate serve both packages.

This is separate from normal builds, `verify rust`, full preflight and every CI job. Explicit
`preflight --kani` adds the registered gates; `--changed-only --kani` narrows to affected kernels.
Kernel source/owner inputs select their propositions. Selection is package-local: API
manifest/build/contract inputs select only the ten API kernels, and a changed
`services/cantos-forge-functions/src/domain/**`, `services/cantos-forge-functions/src/lib.rs` or `services/cantos-forge-functions/Cargo.toml`
selects only `forge-attempt-ledger`. The primitive and its tests, shared Cargo/toolchain/
verification and uncertain changes select all eleven. Unrelated documents, Web/CMP, other Forge
modules and service configuration select none and print a no-affected-kernel note.
`--skip kani` reports selected proofs as `NOT_RUN`;
`--dry-run` prints plans without probing or executing the verifier. Service infrastructure is
never started.

Prepare official Kani **0.68.0** separately with its existing matching
`nightly-2026-08-21-x86_64-unknown-linux-gnu` toolchain and CBMC **6.11.0**, then set `KANI_HOME`
to its bundle parent (default `$HOME/.kani`). The current compatible model is
x86_64-unknown-linux-gnu/64-bit `usize`, bundled
`rustc 1.100.0-nightly (8925ea358 2026-08-20)`, with CaDiCaL. Production still uses **Rust 1.98.1**.
The shared read-only probe checks the existing bundle, compiler/host and both version lines.
It invokes only the absolute installed driver in cargo-plugin mode, never an installer proxy,
setup, download or global toolchain change. Missing/incompatible tools are `UNSUPPORTED` before
proof commands. Doctor's `PASS`/runnable rows establish capability only, never proof execution.

The [single-harness primitive](../scripts/verify_kani.py) uses a fixed package-library/
no-default-feature (`-p` the kernel's registered package), `--exact`, unwind 2, regular output and
default safety/unwinding checks. Child-only bundle/PATH/
toolchain selection is isolated from normal builds. Ambient compiler/flag/wrapper overrides and
implicit Kani manifest/Cargo configuration are rejected rather than weakening the model; no
additional Python 3.11/TOML parser is required. Rust uses the already locked TOML parser.
It hashes the exact bytes it approves and binds existing and absent root/ancestor/Cargo-home
configuration candidates to each proof command. The required manifests are the workspace
manifest and the owning package's manifest only (`services/cantos-api/Cargo.toml` or
`services/cantos-forge-functions/Cargo.toml`); Rust computes one snapshot per package and the primitive
re-derives the identical set from the same package name, so an approval for one package is
never accepted for the other. The primitive refuses stale approval before
proof execution and rechecks the complete candidate set on every outcome.
An empty `CARGO_HOME` uses `HOME/.cargo`, matching Cargo. Both layers require a nonempty
`CARGO_HOME` or `HOME`; unresolved platform-home fallback is refused before probing so its
global configuration cannot escape approval or freshness checks.
No `--locked` switch exists in this Kani release: every outcome instead records and compares
the unchanged workspace lock. Generated verifier files stay in `target/assurance-kani`, preserving
the earlier campaign cache.

Each selected gate is a separate existing pipeline row. A successful process alone is insufficient:
the adapter requires one matching executed harness, exactly one successful completion, complete
positive check evidence with zero failures, all registered cover identities satisfied and only
the reviewed unreachable assertions. Zero/wrong/extra harnesses, incomplete/undetermined output,
unsatisfied covers, timeout (900 seconds), output above 4 MiB, changed inputs or lock mutation are
`FAIL`; later gates are `NOT_RUN`. Bounded subprocess cleanup signals only its owned process group
before reaping the leader.

Fresh raw output and metadata are retained under ignored `artifacts/agent-runs/kani-*/` with Git
revision and sanitized dirty/diff hashes, source/configuration identities, the package and
before/after lock and model digests. Model binding for an API kernel covers all API source,
contracts API source/schema inputs, their manifests/build scripts, root Cargo/.cargo settings and
the primitive/registry, including created/deleted files. For the Forge kernel it covers all
`services/cantos-forge-functions/src` files, the Forge manifest, root Cargo/.cargo settings and the
primitive/registry, and no API or contracts source (the Forge manifest depends only on external
crates), also including created/deleted files. Relevant ancestor/Cargo-home configuration is
also bound. These endpoint
checks require frozen inputs: concurrent mutate-and-restore writes are not filesystem isolation.
An observed endpoint change cannot retain `PASS`. This is bounded local arithmetic/admission
evidence, not SQL, authorization, publication, actual audio, native listening or complete-feature
acceptance. See the exact retained
output before making a `bounded-model-checked` claim; ordinary adapter tests do not establish one.

### ci

| Job | Gates |
| --- | --- |
| `repository` | repository, script-ir-reference |
| `rust` | rust-format, protobuf-reference, rust-lint (workspace), rust-tests (workspace), forge-functions, dbsp, wav-reference, audio-inspection |
| `web` | web-lint, web-build (Studio and Theatre) |
| `postgres` | `cargo fetch --locked`, then postgres |

`cargo xtask ci` runs all four locally; [the workflow](../.github/workflows/repository-checks.yml)
runs one per GitHub job after `rustup show active-toolchain` selects the exact
[`rust-toolchain.toml`](../rust-toolchain.toml) pin. CI has no `--skip`. Every full-preflight gate
runs in CI except `mutation`, `cmp-ui-preview` and `theatre-cmp-demo`. Mutation stays opt-in through
`verify mutation` and is selected by preflight when Script IR sources change. The CMP desktop preview is
a local `ui-preview cmp verify` / preflight gate until a separately provisioned Gradle CI lane is
added. Existing `verify web` and the Web CI job never require the Kotlin toolchain. Branch
protection and required checks are configured outside this repository.
The Rust CI job also provisions Node 22 through a SHA-pinned `actions/setup-node` with package
manager caching disabled, then runs the same offline wrapper gate. xtask itself installs nothing.

### doctor

Reports the selected `rustc` against the pinned 1.98.1, each tool and its version, which gates can
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
| `web_performance.rs` | optional fresh release-build/inventory plans; the Python primitive records byte identities/estimates, never browser or deployment acceptance |
| `behavior.rs` | narrow model/optional conformance command and read-only probe plans; Quint owns expectations |
| `main.rs`, `cli.rs` | clap definitions and dispatch; usage errors exit 2 |
| `workspace.rs`, `config.rs` | root discovery; ports, packages, directories and versions, checked against the repository by tests |
| `command.rs` | immutable `CommandSpec` values and the `Runner` abstraction |
| `tools.rs` | tool detection behind `ToolProbe` |
| `pipeline.rs`, `report.rs` | prerequisite checks, sequential execution and the summary |
| `gate.rs`, `build.rs`, `ci.rs` | gate catalog, build plans and CI jobs |
| `ui_preview.rs` | package-local Leptos/CMP preview plans and supervised execution |
| `run.rs`, `process.rs`, `probe.rs` | launch plans, supervised process groups, port ownership and readiness |
| `seed.rs` | explicit `seed theatre [--reset]` plan; `scripts/seed_theatre.py` owns cluster ownership checks, the seed test run and the record |
| `preflight/` | Git change collection, the Cargo package graph and pure selection |
| `theatre_auth.rs` | pure plans and redacted outcomes for explicit isolated enrollment lanes; invokes the owned Python fixture primitive, outside ordinary gates |
| `doctor.rs` | read-only environment report |
| `services/` | typed process inventory/config validation, explicit local Podman lifecycle, bounded command boundary |
| `distribute/` | explicit demo-APK tester distribution: `request` (pure plan), `apk` (output metadata and tool-report validation), `firebase` (CLI argv, destination check, remote-outcome interpretation, redaction), `receipt`, `host` (files, Git, clock) and `flow` (stage ordering and short-circuiting) |
| `deploy/` | explicit static Web deployment to Firebase Hosting: `request` (pure plan), `hosting_config` (tracked policy, `.firebaserc` mapping, effective configuration), `bundle` (exact-bytes validation and manifest), `firebase` (CLI argv, site lookup, remote-outcome interpretation, redaction), `smoke` (read-only HTTPS judgement), `receipt`, `host` (files, bundle scan, Git, clock) and `flow` (stage ordering and short-circuiting) |
| `run_files.rs`, `firebase_cli.rs` | what `distribute` and `deploy` share: private per-run directories, SHA-256, Git source identity and atomic records; the Firebase CLI version gate and output redaction |
| `capture.rs` | bounded captured execution with a typed timeout/interruption/output-limit vocabulary, used where output must be interpreted |
| `compose_preview.rs` | optional host-preview command and status mapping; the Python primitive owns capture/log/output validation |

Plans are values: tests assert exact commands with a recording runner and fake tools, while
process tests start and stop real `sh` and `python3` children. Run them with
`cargo test -p xtask --locked` (part of `verify rust`).

## Adding a capability

Add a command only for an implemented, reviewed capability. The isolated Theatre mock is
planned through `theatre-demo`; production native listening remains `UNSUPPORTED`. Studio is web-only;
do not add a mobile plan, scaffold or target without an explicit future roadmap request. A new script becomes a gate
in `gate.rs`, is classified in `preflight/select.rs` and, if CI must run it, is added to a job
in `ci.rs`. Release, deployment and provider-backed commands wait for their infrastructure; the
two exceptions are the demo-APK `distribute firebase` command and the static Web `deploy firebase`
command, which stay outside every gate, preflight selection and CI job.

### distribute firebase

Nonsecret CLI-only operator inputs are indexed under
[`infra/firebase/app-distribution`](../infra/firebase/app-distribution/README.md); this product
does not consume Hosting configuration, target/channel semantics or Functions codebases.

```sh
# Local plan only: no tool probe, Gradle, Firebase, login, upload, receipt or artifact write.
cargo xtask distribute firebase cantos-theatre --channel demo --groups cantos-internal --dry-run

# Build, inspect and intentionally distribute. A REMOTE WRITE that may notify testers.
cargo xtask distribute firebase cantos-theatre --channel demo --artifact apk \
  --app-id 1:1234567890:android:0a1b2c3d4e5f67890 --groups cantos-internal \
  --release-notes-file release-notes.txt
```

This is the single opt-in exception to "xtask publishes nothing" ([ADR 0012](../docs/decisions/0012-xtask-command-surface.md)
defers release commands). `build`, `run`, `verify`, `preflight`, `ci` and `build all` never invoke
it and gain no Firebase tool or credential requirement; a test asserts that no ordinary gate, CI
or build plan contains a Firebase/distribution command. Studio is Web-only and is not a target.
The operator quickstart, signing notes and acceptance checklist are in
[Firebase distribution](../apps/cantos-theatre/mobile/docs/firebase-distribution.md).

**Channel, variant and artifact are different things.** `--channel` is a distribution policy
and is required. Phase A maps `demo` onto the existing `debug` Gradle variant
(`:androidApp:assembleDebug`); there is no Gradle flavor named demo and no `assembleDemo`.
`--artifact` defaults to `apk`. The recognized-but-unavailable combinations `demo+aab` and
`production+{apk,aab}` report `UNSUPPORTED` before any tool starts (no fallback to demo); invalid
targets, values and flag combinations are usage errors (exit 2).

**Recipients and notes.** At least one `--groups` or `--testers` entry is required (comma
separated, trimmed, deduplicated, empty entries rejected, union of both); there is no implicit
"all testers". Tester addresses reach the Firebase CLI only through a private run-owned file and
never appear in output or the receipt (only a count). `--release-notes` and
`--release-notes-file` are mutually exclusive; with neither, a short demo-labeled summary of the
version and source identity is generated. Notes are passed as a file, so multiline and Unicode text
is preserved exactly. Nothing tracked is modified.

**Configuration** is nonsecret: `--app-id` takes precedence over `CANTOS_FIREBASE_DEMO_APP_ID`,
and the effective ID and its source are shown. There is no default project or app and no
production fallback; an App ID is not a package name or a Firebase project ID. The optional
`CANTOS_FIREBASE_DEMO_CERT_SHA256` rejects an APK signed with a different certificate. The CLI
authenticates with its own `firebase login` or Application Default Credentials; xtask reads,
prints and stores no credential and never logs in.

**Stages** (each failure stops the rest; unreached stages are `NOT_RUN`):

1. *Local prerequisites*: installed Gradle 8.13–8.14.3, JDK 17+, the Theatre Gradle project and
   Android SDK platform 35 / build-tools 35.0.0 including `aapt2` and `apksigner`.
2. *Firebase CLI*: `firebase --version` must be 15.x and at least 15.34.0, the release whose
   output contract the adapter was written against. xtask installs nothing.
3. *Run record*: a new private directory `artifacts/distributions/<run-id>/` (ignored by Git;
   concurrent runs never share one) and the first receipt.
4. *Firebase destination*: read-only `firebase apps:sdkconfig ANDROID <app-id>`. The response must
   name the requested app and its registered package must equal `com.cantos.theatre.demo`. Only
   those two facts are parsed in memory; the configuration (which includes an API key) is never
   printed or stored. A nonexistent app makes this CLI exit 0 with an empty client list, so the
   response itself is checked, not the exit status. Failure here stops before the build.
5. *Android build*: the existing `theatre-demo android-build` Gradle invocation, reused as is
   (`--no-daemon`, bounded workers, no JDK/SDK auto-download).
6. *APK inspection and staging*: the variant's `output-metadata.json` must declare exactly one
   unsplit APK for the debug variant and `com.cantos.theatre.demo`; zero, several or split outputs
   are refused, never chosen by glob order. The declared file is copied to a run-owned read-only
   path while hashing, then `aapt2` and `apksigner` verify package, version (matching Gradle),
   debuggable, demo version label, exactly one valid signer and any configured certificate.
   Gradle `UP-TO-DATE` output is valid when these checks pass; mtime is not provenance. Just
   before the upload the staged bytes are hashed again and must still match.
7. *Intent receipt*: written (atomically) before the remote write. If it cannot be written, nothing
   is uploaded.
8. *Firebase distribution*: one `firebase appdistribution:distribute` call, never retried.
9. *Final receipt*: the observed outcome and durable console/tester links.

**Remote outcomes.** The Firebase CLI uploads, updates the release notes, then distributes, and
prints the release links after the upload, before the later stages. A link or an "uploaded" line
is therefore not distribution success. The adapter reports exactly one of: confirmed (exit 0 and a
recognized release; service-side acceptance only, not installation, startup or listening),
rejected (a recognized refusal before any upload), partial (the release exists but notes or
distribution failed; the release identity is kept), unknown (timeout after 20 minutes,
interruption, output lost or over 4 MiB, changed or unrecognized output, an unclassified failure)
and not-started. Anything not recognized after the write may have begun is *unknown*, never a
success. A failed or uncertain run exits nonzero with a typed reason and keeps the existing
vocabulary (`FAIL` with the detail; no global `UNKNOWN`). On partial or unknown, **reconcile in
the Firebase console before replaying**: stopping a local child does not undo a remote upload, a
repeated binary may update an existing release, and a replay can notify testers again.

**Receipt and redaction.** `receipt.json` (schema version 1) records the run ID, source commit with
a dirty flag and content fingerprint, tool versions, channel and the actual Gradle variant/task,
package/version, staged SHA-256 and size, signing fingerprint, effective App ID, group aliases,
tester count, notes digest, stage results with timestamps and the release identity with its
durable console and tester links. If persisting the final receipt fails after a successful
remote write, the report prints a recovery pointer and exits nonzero. Output, saved diagnostics
and argv are redacted before display: no tester address, token, credential, SDK configuration or
signed `binary_download_uri` (it expires after about an hour and is not even representable in the
release record). The CLI may leave `firebase-debug.log` in the private run directory; keep it out of
shared CI artifacts.

**Signing.** The demo channel accepts the signed debug APK and records its certificate
fingerprint. Debug keystores differ between machines (and CI), so an APK signed elsewhere cannot
update an installed one in place. Supply a stable key through the existing Android signing
mechanisms and set `CANTOS_FIREBASE_DEMO_CERT_SHA256` to enforce it; xtask never creates,
commits or replaces keys. Production signing is a separate, unimplemented decision.

Deliberately absent from Phase A: `--no-build`, an arbitrary artifact path, an interactive wizard,
a provider plugin framework, automatic recovery or retry, and any Firebase SDK, Crashlytics,
Google Services plugin or `google-services.json` in the app. The production APK/AAB and an
optional manual CI workflow are follow-ups with their own acceptance.

### deploy firebase

```sh
# Local plan only: no tool probe, Trunk, Firebase, login, upload, receipt or artifact write.
cargo xtask deploy firebase cantos-theatre --web \
  --project PROJECT_ID --channel review-hosting --dry-run

# Build Theatre Web once, validate the bundle, publish a PUBLIC preview. A REMOTE WRITE.
CANTOS_API_ORIGIN=https://api.example.org \
cargo xtask deploy firebase cantos-theatre --web \
  --project PROJECT_ID --channel review-hosting --expires 7d

# A deliberate LIVE release of one app; Studio is independent and never builds Theatre.
CANTOS_API_ORIGIN=https://api.example.org \
cargo xtask deploy firebase cantos-theatre --web --project PROJECT_ID --channel live
CANTOS_API_ORIGIN=https://api.example.org \
cargo xtask deploy firebase cantos-studio --web --project PROJECT_ID --channel live
```

The operator setup (installing and logging in the Firebase CLI, creating sites, mapping them),
verification and recovery are in the [Firebase Hosting runbook](../docs/deployment/firebase-hosting.md);
this section is the contract. It is the second opt-in exception to "xtask publishes nothing"
([ADR 0012](../docs/decisions/0012-xtask-command-surface.md)). `build`, `run`, `verify`, `preflight`,
`ci` and `build all` never invoke it and gain no Firebase tool or credential requirement; a test
asserts that no ordinary gate, CI or build plan contains a Firebase, Hosting or deploy command. It is
local and manual: no GitHub Actions workflow, secret, `firebase init hosting:github` or automatic
trigger exists or is needed. Firebase Hosting serves only the static bundle; the Rust API,
PostgreSQL, Kanidm and object storage stay outside it. [ADR 0041](../docs/decisions/0041-direct-hosting-api-boundary.md) selects direct Hosting with explicit
`CANTOS_API_ORIGIN` at Web build time and separately configured API CORS/routing. The receipt records the
nonsecret build input in `build.api_origin` and the build command. A real Hosting deploy requires
a valid HTTPS API origin before creating a run, reconciling a receipt or building; missing,
empty, HTTP or malformed values refuse the command. The local unset default for ordinary
build/run commands keeps relative Trunk-proxy paths;
cross-site web.app authentication is not accepted. See the [current topology/runbook](../docs/deployment/vps-production.md).

**Request.** The app (`cantos-theatre` or `cantos-studio`, nothing else), `--web`, `--project` and
`--channel` are required and never defaulted; `cantos-api`, `cantos-forge`, `all` and mobile are
rejected before any tool runs (usage error, exit 2; Theatre mobile is `UNSUPPORTED` like `build`).
Four identifiers stay distinct: the **project ID** (`--project`), the **Hosting site ID** (read from
the operator's `.firebaserc`), the **deploy target** (`cantos-theatre` → `theatre`, `cantos-studio` →
`studio`) and an Android Firebase App ID (not used). `--channel live` is the only spelling of a live
release; any other value is a public preview channel. `Live`, ` live` and every spelling containing
`/ : _ #` or uppercase are refused because the CLI would normalize them to another destination.
`--expires` (`30m`, `12h`, `7d`; positive, at most 30 days) is for previews only.

**Configuration.** `infra/firebase/hosting/firebase.json` is the tracked policy;
`infra/firebase/hosting/.firebaserc` (ignored by Git, from `firebaserc.example` or
`firebase target:apply`) maps each target to **exactly one** site in the requested project, and the
two targets to different sites. Site IDs must be hostname labels of at most 30 characters, never
domains or URLs; every configured target is validated so an ambiguous other-app mapping cannot
conceal a shared site. Shorter desired names remain conditional on verified allocation; see the
[transition/rollback runbook](../docs/deployment/firebase-hosting.md#shorter-site-names-controlled-transition-237).
Nothing is edited. xtask validates the policy (only `hosting`; no `site`, hooks, redirects or
rewrites; a `Cache-Control: no-cache` rule for every path and no long-lived caching) and renders a
run-specific effective `firebase.json` and `.firebaserc` in the private run directory holding that
one target, that one site and a run-relative `public`. Every Firebase subprocess except the initial `firebase --version` probe (run from the checkout root, with no project) gets an explicit
`--project`, `--config` and the run directory as its working directory, so no active or default
project, other product or `firebase-debug.log` in the checkout can be involved.

**Stages** (each failure stops the rest; unreached stages are `NOT_RUN`):

1. *Local prerequisites*: Cargo, the wasm32 target, Trunk and Git installed, and no inherited
   `TRUNK_*` variable that could redirect the build (reported by name, never value).
2. *Hosting configuration*: the policy and the mapping above.
3. *Firebase CLI*: `firebase --version` must be 15.x and at least 15.34.0, the shared pin with
   `distribute firebase`. xtask installs nothing and never logs in.
4. *Run record*: a new private directory `artifacts/hosting/<run-id>/` (ignored by Git; concurrent
   runs never share one), the effective configuration and the first receipt.
5. *Hosting destination*: read-only `firebase hosting:sites:list --json` in the explicit project; the
   mapped site must be in it. Missing login, site or permission stops here, before the build. This is
   not proof of deploy permission.
6. *Release build*: the existing locked release Trunk build (`build::release_bundle`, shared with
   `web-performance`) of the selected app only, into the run's own fresh `dist`, with the app's own
   `Trunk.toml`, explicit entrypoint and root public URL. The source identity (commit, dirty flag and
   content fingerprint) is read before and after; a change refuses the bundle.
7. *Bundle validation*: the exact public bytes (see the runbook): regular allow-listed static files
   only, the selected app's `index.html` and `*.js`/`*_bg.wasm`, every reference present and inside
   the bundle, `integrity` pins that match, no HTML masquerading as an asset, a path-ordered manifest
   and one digest. Nothing from outside the bundle (credentials, `.env`, logs, receipts, audio) can be
   public: the allow-list refuses it.
8. *Intent receipt*: the bundle is scanned again and must have the same digest; intent is recorded
   atomically. If it cannot be, nothing is uploaded.
9. *Hosting release*: exactly one call, never retried. Preview: `firebase hosting:channel:deploy
   <channel> --only <target> --project <id> --config <run>/firebase.json [--expires …]
   --no-authorized-domains --non-interactive --json`. Live: `firebase deploy --only
   hosting:<target> --project <id> --config <run>/firebase.json --message … --non-interactive
   --json`. The preview command refuses `live`, so the two are different commands.
10. *HTTP smoke*: read-only HTTPS GETs (no redirects, `https` only) to the URL the CLI reported,
    judged on status, media type (notably `application/wasm`), `Cache-Control` and the SHA-256 of the
    served bytes; plus a missing `.wasm` and an `/api/` path that must answer `404` rather than the
    page. Up to 3 attempts, 5 s apart, while a release propagates; the write is never repeated. Needs
    `curl`; without it the row is `NOT_RUN`. Static delivery only.
11. *Final receipt*: the observed outcome.

**Remote outcomes.** Exactly one of: *confirmed* (exit 0 and a result naming exactly the selected
target, site and channel, with an `https` Hosting URL for a preview or a version of exactly this site
for a live release), *rejected* (a recognised refusal; no release was reported), *unknown* (timeout
after 20 minutes, interruption, output lost or over 4 MiB, changed or unrecognised output, an
unclassified failure, or a nonzero exit) and *not started*. A failure after the CLI began is never
reported as "nothing was written": a preview channel may exist and a version may be uploaded
unreleased. Identifiers (version, URL, expiry) come from the CLI's result only; a live URL comes
from the site lookup; none is invented, and "latest release" is never used to identify a run. On
*rejected* or *unknown*, **reconcile in the Firebase console before replaying**. A successful
release followed by a failed smoke or a failed receipt write stays a known remote release with a
separate local failure and a recovery pointer.

**Receipt and redaction.** `receipt.json` (schema version 1) records the run ID, source identity, tool
versions, project, site, target and channel, effective-configuration digests, the build command, the
bundle manifest and digest, the release identity, the smoke observations and each stage with a
timestamp. Output and saved diagnostics are redacted (no addresses, tokens, signed or foreign links;
Hosting and console links stay). The CLI may leave `firebase-debug.log` in the private run directory;
keep it out of shared CI artifacts.

Deliberately absent from this first delivery: `--no-build`, an arbitrary artifact path, deploy-all,
automatic promotion, rollback or channel/site deletion, a provider framework, long-lived caching of
hashed assets, any Firebase SDK, Functions, Cloud Run or App Hosting adapter. An explicit Web
API-origin setting now exists under ADR 0041; this command still deploys static bytes only.
Read current release state from Firebase, and keep user-journey acceptance separate from smoke.
Historical tool evidence is [source-bound](../docs/evidence/issue199-firebase-hosting.md).

### deploy vps

```sh
# Plan only: resolves HEAD and prints every step; builds, uploads and runs nothing.
cargo xtask deploy vps cantos-api --host root@HOST --dry-run
# One-time and idempotent host setup from the committed infra/vps bundle (no image).
cargo xtask deploy vps cantos-api --host root@HOST --bootstrap
# Build HEAD's image here, upload, then backup -> migrate -> grants -> switch -> health gate.
cargo xtask deploy vps cantos-api --host root@HOST --smoke-origin https://api.example.org
cargo xtask deploy vps cantos-api --host root@HOST --rollback   # previous image
cargo xtask deploy vps cantos-api --host root@HOST --status     # read-only
```

The third opt-in remote write ([ADR 0039](../docs/decisions/0039-vps-production-runtime.md),
[runbook](../docs/deployment/vps-production.md)). Never invoked by `build`, `run`, `verify`,
`preflight` or `ci`. The plan (`xtask/src/deploy/vps.rs`) is pure and unit-tested: every local
step (git archive of the **committed** `HEAD`, digest-pinned Podman build, export, compression,
bundle) precedes the first remote write, and the remote side only calls the uploaded
`infra/vps/bin/cantos-vps`, which owns backup, migration, grants, the image switch, health gating
and automatic switch-back. SSH runs with `BatchMode=yes` and the operator's own agent/config;
xtask never reads, stores or prints a key, password or token, and the host generates its own
secrets. `--host` accepts only `user@host` (no options or ports); `--smoke-origin` only an exact
`https` origin. A failed remote step reports that the remote state may have changed: run
`--status` before retrying. Run records stay in ignored `artifacts/vps/<stamp>-<sha>/`.

### theatre-demo

An explicitly isolated mock command surface; starts no API, database, identity or audio adapter.

```sh
cargo xtask theatre-demo build          # :preview-sandbox:compileKotlinDesktop (depends on :shared)
cargo xtask theatre-demo verify         # :shared:desktopTest + :preview-sandbox:desktopTest
cargo xtask theatre-demo run            # same app, supervised diagnostic desktop window
cargo xtask theatre-demo android-build  # -Pcantos.android=true :androidApp:assembleDebug
cargo xtask theatre-demo ios-build --kotlin-native-home /absolute/installed/native-2.2.20
```

Build/verify/run require installed Gradle 8.13, 8.14.1, 8.14.2 or 8.14.3 and a full JDK 17+. Android checks an existing
absolute SDK path, platform35 and build-tools35.0.0 before Gradle; downloads are disabled.
iOS checks the installed compiler's exact2.2.20 version plus an available Xcode simulator SDK,
then links only `:preview-sandbox:linkDebugFrameworkIosSimulatorArm64`, which exports the shared
core through the existing `CantosTheatreDemo` framework; no compiler/LLVM is downloaded.
Missing prerequisites are `UNSUPPORTED`, failed executed steps are `FAIL`.

The demo desktop, Android and iOS hosts explicitly consume `:preview-sandbox`; the production-safe
`:shared` module never depends on it. Verification retains both the independent shared-core suite
and sandbox port/workbench/UI tests. These commands produce demo evidence only; a demo debug APK
cannot accept production `releaseCompileClasspath` / `releaseRuntimeClasspath` or APK/AAB
exclusion. Use [`theatre-host`](#theatre-host) for those claims.

The local `theatre-cmp-demo` preflight gate runs mock common/JVM tests when its source/build
inputs or consumed CMP/shared tokens change. It does not stand for native acceptance or
component qualification. It is outside CI until a Gradle lane is separately provisioned.
`run` aligns JVM args, passes its owned launcher PID, rejects detached run JVMs, and uses
only stored owned process groups for cleanup. The supervisor keeps exited leaders unreaped
until their group has been signalled, preventing descendant leaks without stale-PGID lookup.
Details and evidence live in the [mock guide](../apps/cantos-theatre/mobile/README.md).

### theatre-host

The production Theatre Android host (`androidProductionApp`, `com.cantos.theatre`) composes
`:shared` only, with no mock, sample or diagnostic fallback. Its debug build browses the live
public catalog when built with an API origin (below); release declares no network permission or
origin and renders an explicit unavailable state until listening adapters and a production
origin exist (#14).

```sh
cargo xtask theatre-host android-release
# 1. gradle -Pcantos.android=true :androidProductionApp:assertReleaseDependencyBoundary
#      :androidProductionApp:assembleRelease :androidProductionApp:bundleRelease
#      :preview-sandbox:compileReleaseKotlinAndroid
# 2. python3 scripts/inspect_android_release.py --apk ... --aab ... --aapt2 <SDK>/build-tools/35.0.0/aapt2
```

The SDK and tool prerequisites match `theatre-demo android-build`; the host, shared Android
sources and sandbox resource inventory must exist. Missing prerequisites are `UNSUPPORTED`.
The Gradle task resolves both release classpaths and allows no local project except `:shared`
and no test, mock or Compose tooling module. The sandbox is compiled only as the forbidden
class inventory. The inspector reads the actual unsigned, unshrunk APK and AAB: DEX
`class_defs`, sandbox class names and resource bytes, Compose resource packages, entry paths and
the `aapt2`-decoded manifest. It also checks package, launcher, debuggable/test-only flags and
allowlisted permissions. Its APK/AAB class sets must match and named production classes must
exist; an empty inventory or unreadable artifact exits 2, never `PASS`. A failed build
stops before inspection, so stale outputs are never inspected. Reports are written under
`apps/cantos-theatre/mobile/androidProductionApp/build/reports/release-boundary/`.

`theatre-host android-debug --api-origin ORIGIN` builds only `:androidProductionApp:assembleDebug`
with `-Pcantos.apiOrigin=ORIGIN`. It accepts `https://` or `http://127.0.0.1|localhost:<port>`
(for `adb reverse`) and refuses anything else with exit 2 before Gradle. The debug manifest overlay
adds INTERNET, the origin meta-data and a cleartext policy limited to those loopback hosts; release
declares none of them. The APK composes the live public catalog only (no playback, progress or
identity); installing it on a device is the developer's own `adb install`.

Nothing is signed, installed, shrunk or distributed; this is release-isolation evidence, not a
production release or listening acceptance. Preflight does not run it (it selects only the JVM
mock gate for host changes and notes this command). See the
[isolation record](../docs/evidence/issue122-cmp-sandbox-isolation.md#production-android-host-and-release-artifact-acceptance).

The retained local fixture can now select its existing test context with
`CANTOS_TEST_CONTEXT=editorial|catalog|audio` on `run cantos-studio --web --test-cluster`.
Editorial is the default. Audio selects the fresh runner's exact AudioLifecycle database and
private root; it does not provision identity/storage or use an inherited database URL.
PostgreSQL verification now discards completed suite databases only after their tests pass,
retaining all five restart fixtures for the existing crash/replay assertions. This bounds local
disk consumption without removing any test or weakening PostgreSQL durability settings.

### Studio Kanidm operator tools

`cargo xtask kanidm reconcile --issuer ISSUER --web-origin ORIGIN --group GROUP --dry-run`
prints a typed Studio-only client/callback/openid plan. It has no mutation execution path and
reports provider reconciliation NOT_RUN. No container, user, grants, credential or TLS bootstrap
is started. `cargo xtask kanidm verify --issuer ISSUER --web-origin ORIGIN [--dry-run]` invokes the
API's bounded read-only discovery/JWKS primitive only when explicitly run; dry-run starts nothing.
Metadata PASS establishes no user login, disable/logout or production acceptance. These optional
operator operations are outside preflight/CI; Rust tests exercise their plans and the real adapter
against ephemeral signed HTTP mocks. See the [operator policy](../services/kanidm/README.md).
