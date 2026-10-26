# Cantos xtask

> **Native checkpoint command override.** This branch has a partial tooling migration; this
> paragraph overrides the legacy PostgreSQL/Kanidm commands and gate descriptions below.
> `build cantos-server` builds the Server binaries; `build cantos-identity` enables `service`.
> `run cantos-server` plans explicit file migrations then Identity + Server under private
> `target/native-dev/` directories with a short private Unix socket. `--dry-run` makes no
> directories or processes. Serving does not migrate files. Plain `seed theatre` refuses before
> effects until its native `catalog_http` integration target exists; the explicit prepared-sample
> flags select the isolated [sample lane](#seed). `verify rust` covers
> ordinary file-backed Server tests; `verify turso` covers shared-store/Identity feature lanes.
> PostgreSQL, DBSP and provider-enrollment gates are removed from ordinary selection/CI.
> Their explicit commands, Kanidm/service lifecycle, legacy VPS deploy and legacy API release
> refuse with exit 2 before probes/effects. `services list|validate` remain read-only.
> No native host activation is implemented. These command changes do not establish business
> context or two-process runtime acceptance; see the [checkpoint boundary](../docs/architecture/server-turso-porting.md#checkpoint-boundary).


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
| Cargo, rustup, Trunk, Python 3, Node.js, Git | Primitive tool execution |
| `scripts/*.py` | Repository checks, independent oracles, the mutation runner, historical migration oracles. Their logic is reused, not rewritten |
| `cargo xtask` | Targets, profiles, surfaces, gate catalog, prerequisite checks, changed-path selection, process supervision and the CI job contract |
| GitHub Actions | Runner lifecycle and environment: the pinned toolchain and Trunk. Each job then calls `cargo xtask ci --job <job>` |

Tool detection never installs anything. Explicit `verify behavior --setup` alone installs its
locked npm development dependency; ordinary verification never fetches it. xtask does not
install other toolchains, initialize/start Podman Machine, change connections, call
providers, create cloud resources, publish or deploy, with **explicit, opt-in exceptions**:
`distribute firebase` ([below](#distribute-firebase)) builds the selected Theatre Android APK and
hands it to Firebase App Distribution, and `deploy firebase` ([below](#deploy-firebase)) builds one
Leptos Web app and publishes its static bundle to Firebase Hosting, and `deploy cloudflare`
([below](#deploy-cloudflare)) uploads an already built, verified Web artifact to Cloudflare Pages
(it never builds), and `secrets import` ([below](#secrets)) creates a listed Google Secret
Manager secret if absent and adds one version from an approved local input. Legacy `services up` refuses. Native build/test/run/preflight/ci never
start container infrastructure or implicitly distribute/deploy.

## Commands

```sh
cargo xtask build cantos-server [--release]          # cargo build --locked -p cantos-server --bins
cargo xtask build cantos-identity [--release]        # native Identity service binary
cargo xtask build cantos-forge [--release]        # cargo build --locked -p cantos-forge --bins
cargo xtask build cantos-studio --web [--release]  # trunk build --locked in apps/cantos-studio/web
cargo xtask build cantos-theatre --web [--release]         # trunk build --locked in apps/cantos-theatre/web
cargo xtask web-performance cantos-studio|cantos-theatre  # release build + offline asset inventory
cargo xtask hosting-report --record PATH [--dry-run]    # offline operator-declared rollout report
cargo xtask hosting-report --self-test                  # synthetic report regressions only
cargo xtask hosting-smoke <cantos-theatre|cantos-studio> --origin ORIGIN --api-origin ORIGIN --deployment ID --release-manifest PATH --release-digest SHA256 --release-sha SHA (--local-artifact | --release-request PATH) [--dry-run]
cargo xtask theatre-size --apk PATH [--baseline report.json]  # offline APK size groups/deltas (no upload or install)
cargo xtask build all [--release]                  # the API, Forge and both Web clients

cargo xtask run cantos-server [--web-origin URL]
cargo xtask run cantos-studio --web [--web-origin URL] [--open]
cargo xtask run cantos-theatre --web [--web-origin URL] [--open] [--local-mail disabled|firebase|resend-local]
cargo xtask run resend-local
cargo xtask app cantos-studio --web [--open]
cargo xtask app cantos-theatre --web [--open]
cargo xtask seed theatre [--reset]                 # synthetic Theatre catalog + media in the native development files (currently refuses without catalog_http)
cargo xtask seed theatre --sample-input JSON --sample-media-source DIR --sample-root DIR [--sample-serve]
cargo xtask seed theatre --sample-root DIR --reset  # archive a proven owned sample root; never deletes it

cargo xtask ui-preview leptos build|run|verify       # package-local Leptos gallery
cargo xtask ui-preview leptos run --open             # loopback browser preview on 8083
cargo xtask ui-preview cmp build|run|verify          # package-local CMP desktop preview
cargo xtask ui-preview cmp android-build             # compile the isolated Android preview library
cargo xtask theatre-host android-release [--api-origin HTTPS_ORIGIN] # production catalog APK/AAB + boundary/origin inspection
cargo xtask theatre-host ios-build --kotlin-native-home PATH # shared-only simulator frameworks; macOS + Xcode
cargo xtask theatre-host android-debug --api-origin URL  # production host debug APK for the live catalog; installs nothing

cargo xtask verify repository|rust|forge|turso|web|contracts|mutation
cargo xtask verify behavior [--capability ID] [--setup] [--conformance] # opt-in Quint + pure-core replay
cargo xtask verify kani [--harness catalog-revision|image-bounds|wav-payload|submission-window|publication-commit|listener-access|listener-reconcile|byte-range|playback-grant|public-cover|forge-attempt-ledger|identity-security-version|identity-completion|identity-mailbox-proof]...
cargo xtask verify native-isolation --test-binary /absolute/prebuilt-rust-test --test fully::qualified::name [--timeout-seconds 60] [--identity-binary /absolute/prebuilt/cantos-identity] [--dry-run]
cargo xtask verify native-auth-qualification [--kani] [--report PATH] [--dry-run] # manual #336 Linux campaign

cargo xtask preflight [--changed-only] [--base REF] [--kani] [--skip GATE]... [--dry-run]
cargo xtask preflight --profile local --changed-only [--base REF] [--skip GATE]... [--dry-run] # DEV/CHECKPOINT subset
cargo xtask ci [--job repository|rust|web] [--dry-run]
cargo xtask doctor
cargo xtask services list|validate
cargo xtask distribute firebase cantos-theatre --channel development [--artifact apk] \
  (--groups ALIASES | --testers EMAILS)... [--app-id ID] [--release-notes TEXT | --release-notes-file PATH] [--dry-run]
cargo xtask deploy firebase cantos-theatre|cantos-studio --web --project PROJECT_ID \
  --channel live|PREVIEW_CHANNEL [--expires 7d] [--dry-run]   # explicit static Web deploy to Firebase Hosting
cargo xtask deploy cloudflare cantos-theatre|cantos-studio --web --account ACCOUNT_ID --project NAME \
  --branch BRANCH --environment production|preview --artifact DIR [--dry-run]  # opt-in Pages Direct Upload of a release-build artifact
cargo xtask secrets inventory                                  # offline: validate the tracked Secret Manager names
cargo xtask secrets status --project PROJECT_ID --account EMAIL [--environment dev|prod|ops] [--dry-run]  # version metadata only
cargo xtask secrets import --project PROJECT_ID --account EMAIL --secret ID (--file PATH | --stdin) \
  --expect-latest none|N [--dry-run]                           # operator-only remote write: create-if-absent + one version
  [--smoke-origin https://API_HOST] [--dry-run]               # explicit API deploy to the production VPS
cargo xtask compose-preview cantos-theatre|ui-system-cmp --file PATH --composable NAME \
  [--label SCENARIO] [--pid PID] [--android-cli PATH] [--timeout-seconds 60] [--dry-run]
cargo xtask visual-compare --reference PNG --actual PNG --scenario ID \
  [--metadata JSON] [--threshold 0] [--tile-size 16] [--output PATH] [--python PATH]
cargo xtask visual-compare --self-test [--python PATH]
cargo xtask visual-compare --spike-pairs JSON [--python PATH]
```

`cargo xtask <command> --help` documents every flag.

The `cantos-identity` selector builds the opt-in service binary. It is registered beside the
Server in the native process inventory. Its pure default core, persistence and service evidence
remain distinct; optional Kani kernels require actual D1 Ubuntu x86_64 execution.

Legacy `kanidm`, container lifecycle, `verify postgres`, `verify dbsp`, `verify theatre-auth`,
`--dbsp-experimental`, `--test-cluster`, named-environment seeding and `deploy vps` are hidden
retired refusals before effects. The old `api` release component is retired too; Web/Android
release paths and static Firebase Hosting/Functions/App Distribution retain separate ownership.

### Targets, surfaces and profiles

`cantos-server` is the native Axum/Turso backend (business port incomplete). `cantos-forge` is the producer-side CLI
([ADR 0022](../docs/decisions/0022-cantos-forge-producer-cli.md)); it takes no surface flag and
xtask builds its binaries and `verify forge` checks its Rust package and offline Functions
wrapper. Ordinary gates use fixtures and injected doubles; they never call a live provider or
deploy a Function. No `run` command starts Forge.
`cantos-studio` (creators) is web-only: pass
`--web`; `--mobile` is rejected as a usage error (exit 2), not a reserved implementation.
`cantos-theatre` (listeners, alias `cantos`) supports `--web` and `--mobile`; one must be named.
Theatre Mobile has a JVM-only Desktop Preview Gradle module. Production `--mobile` remains
`UNSUPPORTED` pending native runtime acceptance; use `theatre-preview` for Desktop Preview evidence
and `theatre-host android-release` for the production host's release isolation.
Studio Mobile is excluded from all build/run/test/preview plans, even if files appear in its
out-of-scope directory. `build all` covers the Server, Identity, Forge and both Web clients; it
does not build Theatre Mobile. See the
[platform matrix](../docs/architecture/repository-layout.md#platform-scope).

`dev` is the default unoptimized build. `--release` passes `--release` to Cargo or Trunk and
produces an optimized local artifact. It is not a production image, a signed bundle or a
deployment; none of those exist yet. (`distribute firebase` hands the explicitly selected QA debug APK to testers;
it is not a production release either. `deploy firebase` publishes the release Web bundle to
Firebase Hosting on explicit request; that is static delivery, not a claim about the API or
production readiness.)

Trunk runs with `NO_COLOR=true`, matching the documented commands (Trunk 0.21 rejects an
inherited `NO_COLOR=1`).

### Release artifacts and promotion

`cargo xtask release-build <api|theatre-web|studio-web|android> [--api-origin ORIGIN] [--dry-run]`
packages clean HEAD into version-2 manifests beneath
`artifacts/release-build/COMPONENT/SHA/CONFIG_DIGEST/ATTEMPT`. Web requires an explicit HTTPS
origin via flag or `CANTOS_API_ORIGIN`. API includes the matching host/runtime bundle; Android
retains its fixed origin and unsigned optimized production APK/AAB plus separate unshrunk audit and bound mapping/report hashes. No build command publishes.
`cargo xtask release-build --self-test` runs packaging, Android inspector and focused host/distribution Rust regressions;
`cargo xtask release self-test` also covers artifact validation, workflow DAG and mocked promotion.
Python regressions run in the repository gate; Rust CLI/deploy tests run in the Rust gate.
The self-test also runs focused xtask lint/tests. These commands add no ordinary gate or CI job.

The explicit same-workflow CircleCI surface is:

```sh
cargo xtask release prepare --component theatre-web|studio-web|web \
  --target REF --api-origin https://api.example.org --transfer artifacts/release-transfer
# Build only: replace --target with --build-only (also accepts android).
cargo xtask release checkout --transfer artifacts/release-transfer
cargo xtask release intent --transfer artifacts/release-transfer --component theatre-web|studio-web
# CircleCI must persist the component intent to its workspace before the next command.
cargo xtask release promote --transfer artifacts/release-transfer --component theatre-web|studio-web
cargo xtask release report --transfer artifacts/release-transfer
```

`prepare` requires trusted CircleCI identities and resolves once; `promote` is an explicit remote
write behind approval, never called by ordinary build/run/gates. The
[release contract](../docs/deployment/circleci-builds.md) owns manifest compatibility, bounded
archive/provenance validation, config/checkout/release revisions, expiry, strict develop-head
freshness, context prerequisites, additive-workspace rerun refusal and component receipts.
Cross-pipeline promotion is unsupported. No cache supplies a release or a deploy binary.

`deploy firebase APP --web --project ID --channel CHANNEL`
accept `--release-manifest PATH --release-digest SHA256 --release-sha FULL_SHA`, together with
`--release-request PATH` in CircleCI or `--local-artifact` outside CI. The latter is the operator's
explicit acknowledgement of independently verified local provenance, not proof supplied by a
hash. V1, arbitrary dist/no-build inputs, missing/wrong digests/source/origin, foreign/stale runs,
malformed archives and unexpected files fail before activation/publish. Artifact mode extracts
fresh validated bytes instead of building the product. VPS artifact deploy requires
`--smoke-origin ORIGIN` and cannot combine with bootstrap/status/rollback/reload/cutover.
Local build-and-deploy commands remain available when no artifact input is supplied.

### Existing native host upgrade

`cargo xtask release native-upgrade --request PATH --request-digest SHA256 [--execute]
[--rollback-attempt attempt-HEX]` is the explicit host-local, rootful Linux same-schema
NATIVE_READY upgrade/recovery lane. Default is offline admitted-input validation; `--execute`
changes the authorized host. It never connects over SSH, bootstraps, imports seeds, migrates a
schema or restores DBs. The [native executor contract](../docs/deployment/native-oci-release.md#existing-native-host-executor-review-boundary)
owns exact request/current-state pins, qualification requirements, cold backup, effect order,
crash refusal and explicit image-only rollback. Model/mock/file-only tests cannot admit images.
Build, verify, preflight and CI never execute host activation. `deploy vps` remains retired.

### Offline Hosting rollout report

`cargo xtask hosting-report --record PATH [--dry-run]` reads a strict local version-1
[operator record](../templates/hosting-rollout-record.json); the
[rollout runbook](../docs/deployment/cloudflare-rollout.md#offline-report-contract) owns its
fields and acceptance boundary. It reports 27 independent declared outcomes across the two Web
surfaces and global prerequisites/recovery/retirement. Missing/unknown evidence and partial
outcomes remain distinct. Mock/compiled evidence cannot satisfy declared live/browser rows.

This is offline metadata validation: no account access, HTTP smoke, browser, credential read,
build, publish, DNS change, retry or receipt write. Evidence hashes are references, not verified
files or signatures; PASS remains an operator declaration. Exit 0 / `hosting-report-schema PASS`
means only a valid local record, including one with FAIL/BLOCKED/UNKNOWN/NOT_RUN rows. Even an
all-PASS record grants no execution or retirement authority. Raw input/parser messages are not
printed; input is bounded to one regular 256 KiB file with no final symlink. Keep raw private
logs and secrets out of it. `--dry-run` prints the plan before any tool probe/read/write.

`--self-test` runs deterministic Python regressions (also in the existing repository gate).
The Rust command plan is covered by existing Rust checks. This adds no runtime/CI gate;
verification-tooling changes retain conservative full preflight selection. #277/#278 own
promotion/publishing, and #279 owns future cloud submit/status; this command implements none
of those effects. User/browser/live recovery acceptance stays with #280 after operator setup.

### Read-only Hosting HTTP observation

`cargo xtask hosting-smoke` checks one explicit static HTTPS origin against a verified Web
manifest v2, independently of `deploy`. It never builds, publishes, logs in, changes DNS,
queries a provider account or sends requests to the declared API origin. Pass the actual
returned deployment origin or approved final domain and its nonsecret deployment ID. The ID
is an operator declaration; byte equality does not prove provider ownership or deployment identity.

It reuses `release`'s artifact digest/source/configuration/provenance validation and safe extraction,
then the existing static bundle validator and HTTP judge. Each run makes one observation round:
entry HTML plus its JS/CSS/WASM, missing WASM/JS/CSS/PNG and a synthetic `/api/v1/*` path on the
**static** origin. Curl ignores `.curlrc`, follows no redirects, negotiates/decodes compression,
uses HTTPS/TLS with verification and a 60-second/128-MiB response bound. At most 128 paths are
allowed. Entry privacy additionally requires no-store, no-referrer, nosniff and DENY.

The fresh private `artifacts/hosting-smoke/<RUN>/receipt.json` records the declared deployment,
manifest/source/bundle identities, observation time, independent artifact/HTTP statuses and
per-path status/hash/decoded size/byte-match/static/privacy results. Partial observations persist
before the next request. Raw headers/stderr/cookies and response bodies are never exported;
bodies are removed after hashing. A process crash may leave a private partial body; sanitize
the receipt before sharing and keep the whole run directory private. Missing curl is UNSUPPORTED;
artifact refusal leaves HTTP NOT_RUN. A failed observation never publishes, retries or rolls back.

`--dry-run` validates only argument syntax and prints the plan before any file read, tool probe,
subprocess, network request or output write. Normal runs retain the release contract's 24-hour
freshness/current-policy requirements; this command cannot qualify older retained rollback
artifacts. Do not rebuild an old source and claim identical rollback bytes. Browser/API/auth,
fonts/images, CSP/Permissions-Policy, ETag round trips, DNS and recovery remain separate checks in
the [rollout runbook](../docs/deployment/cloudflare-rollout.md#standalone-read-only-http-check).
This adds no gate, CI job, provider dependency or automatic live request; the existing Rust gate
exercises deterministic fake extraction/HTTP boundaries.

### Native OCI release verification

`cargo xtask release oci-verify --manifest PATH --manifest-digest SHA256 --source-sha FULL_SHA
--configuration-digest SHA256 --platform linux/amd64|linux/arm64 --admission PATH
--admission-digest SHA256 [--host-inventory PATH --host-inventory-digest SHA256]
[--registry-readback [--authfile PATH]] [--dry-run]` validates an explicitly admitted existing image pair,
producer evidence and actual hash-bound qualification record files, plus the checksummed/source-bound
host bundle offline. Optional host inventory adds only an operator-declared plan with separate
platform/schema/ownership/freshness/operation blockers; even consistent input grants no activation.
By default it prints `PLAN_ONLY`; registry access, native qualification, activation and cleanup
remain `NOT_RUN`. Explicit `--registry-readback` observes exact GHCR manifest/config bytes only,
with a sanitized `OBSERVED` result and `release_eligible=false`; it never pulls layers or accepts
a release. `--dry-run` stays offline, even with that option, including no authfile/network/SSH/build
or persistent output. The [v1 contract](../docs/deployment/native-oci-release.md) owns the exact
manifest/admission/evidence schemas, limits, trust boundary and deferred producers.
The Python regressions are included by `release self-test` and the repository gate; CLI
regressions are included by the Rust gate. No gate publishes or activates an image.

`cargo xtask release oci-build --platform P --producer local --build-id ID | --producer github-actions
[--publish --authfile PATH]` builds the Server/Identity pair from one clean commit on a natively matching
host and, only with `--publish`, pushes it to GHCR and writes an **unadmitted** candidate (all gates
`NOT_RUN`; `oci-verify` refuses it). No gate or CI job calls it; the opt-in
[`native-oci-publish`](../.github/workflows/native-oci-publish.yml) workflow is manual only. See the
[producer contract](../docs/deployment/native-oci-release.md#producers-release-oci-build).

`cargo xtask release oci-qualify --manifest PATH --manifest-digest SHA256 --source-sha FULL_SHA
--configuration-digest SHA256 --platform P --output FRESH_DIRECTORY [--authfile PATH]` runs the
production pair in a private rootful Podman store on disposable matching native Linux. It checks
layers/UIDs/mounts/UDS/network controls, real HTTP/auth/persistence and cold same-schema restore.
Input and fixture digests bind separate immutable PASS/FAIL/NOT_RUN records; it never reuses input
PASS or writes an admission. The manual OCI workflow calls it after publishing. Core/file
regressions belong to release self-test/repository; CLI regressions belong to Rust. Preflight and
ordinary CI never execute containers. See [run/receipt/limit details](../docs/assurance/native-oci-qualification.md).

These are preparatory PR A slices of #348; a real registry publish and
exact-image fixture acceptance remain required before PR B's native VPS activation. The existing `deploy vps`
command and legacy `api`/`all` release selectors are retired refusals; they do not consume this contract.

The opt-in Pages CI lane reuses the existing `deploy cloudflare` publisher through:

```sh
cargo xtask release pages-intent --transfer artifacts/cloudflare-transfer/theatre-web --component theatre-web
# CI must persist intents/theatre-web before the next command.
cargo xtask release pages-promote --transfer artifacts/cloudflare-transfer/theatre-web --component theatre-web
```

`pages-intent` consumes only a trusted same-workflow v2 request/inventory. Nonsecret operator inputs
are `CLOUDFLARE_ACCOUNT_ID`, both `CLOUDFLARE_THEATRE_PROJECT`/`CLOUDFLARE_STUDIO_PROJECT`,
`CLOUDFLARE_PRODUCTION_BRANCH`, `WEB_ENVIRONMENT` and `WEB_BRANCH` (required for preview; production
may use the declared production branch). The deploy owner validates the exact mapping/intent.
`pages-promote` pins destination/source/artifact to that job, revalidates freshness, refuses reruns
and invokes the existing publisher once. Outcomes retain unknown or confirmed remote state
independently from command/smoke status. Neither command runs in ordinary gates/CI. Real pipeline,
provider, browser and cutover acceptance remain separate; see the [Pages runbook](../docs/deployment/cloudflare-pages.md#circleci-opt-in).

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

### Optional Theatre APK size report

```sh
cargo xtask theatre-size --apk apps/cantos-theatre/mobile/androidApp/build/outputs/apk/debug/androidApp-debug.apk \
  --output artifacts/apk-size/demo-debug-baseline
cargo xtask theatre-size --apk path/to/candidate.apk --baseline artifacts/apk-size/demo-debug-baseline/report.json
cargo xtask theatre-size --apk path/to/any.apk --dry-run
```

Measures one explicit APK (never a glob, never a build) and writes `report.json` plus
`summary.txt` to a fresh directory beneath ignored `artifacts/apk-size`; existing evidence is
never overwritten. The report records the exact SHA-256, byte count with decimal MB and MiB,
package/version/debuggable from `apkanalyzer`, ZIP compressed and uncompressed bytes per group
(DEX, native libs and per ABI, `resources.arsc`, `res`, fonts, assets, manifest, signature/
`META-INF`, other), top entries and the largest DEX packages with notable prefixes
(`androidx.compose.material.icons`, tooling, Compose, Kotlin). Container overhead is file bytes
minus entry compressed bytes and is never added to a group. DEX package bytes are an
uncompressed estimate; they are attribution, not download savings. APK file bytes, the
Analyzer's estimated download size and installed footprint (`NOT_MEASURED`, needs a named
device) stay separate metrics.

Variant comes only from the Gradle `output-metadata.json` beside the APK when it names exactly
that file; otherwise it is `unknown`. `--source-revision` is recorded only when supplied. The
signer digest comes from `apksigner` when installed, else `NOT_CHECKED`. A missing or failing
`apkanalyzer` yields a `PARTIAL` report and a non-zero exit, never a pass. `--baseline` adds
absolute and percent deltas per metric and group plus every input difference (package, version,
debuggable, variant, signer, source revision, tool version); differing inputs mark the
comparison not comparable. The command resolves no Gradle dependency graph, builds nothing and
adds no required gate: the primitive tests run in `repository`, the command-plan tests in
`rust-tests`.

### run and app

Local `run` defaults to disabled mail. `run resend-local` explicitly starts the prepared third-party
HTTP fixture, posts and reads back one fixed nonfunctional message, then stops its own process.
`run cantos-theatre --web --local-mail resend-local` keeps the same synthetic fixture alongside
the application; Identity signup mail stays disabled. Neither result accepts verification,
authentication or real delivery. Preparation, provenance, the patched dependency lock and
isolation limits are in the [Resend local runbook](../infra/local-mail/resend-local/README.md).

Theatre `run --web --local-mail firebase` starts a supervised, run-owned Firebase Auth Emulator from an already
installed `firebase-tools` **14.2.2** Auth module and Node.js. Missing/wrong tooling refuses before
build; no install, Firebase login or real Firebase fallback occurs. Set
`CANTOS_FIREBASE_AUTH_MODULE` to an absolute installed `lib/emulator/auth/server.js` when it cannot
be derived from the `firebase` executable. This pin is separate from Hosting/App Distribution.
The emulator has an empty environment, loopback ephemeral port, unique demo project, disposable
in-memory state and blocked outbound TCP/TLS. Its provider logs are suppressed. The console
prints only its local developer action-inventory endpoint; never log the returned codes/links.

The explicit Firebase lane selects the operation-bound Firebase emulator adapter. Native local runs reject inherited
Google/Firebase/SMTP configuration and use Theatre's actual loopback origin. Native Identity
still owns proof consumption/passwords and returns no session on signup. The
[protocol and acceptance limits](../docs/architecture/firebase-verification.md) distinguish the
real Identity/RPC journey from full browser and production acceptance, and document the unresolved
expired enrollment recovery boundary. Other local run targets keep mail disabled.
The explicit boundary test uses the installed emulator (no native signup claim):

```sh
cargo test --locked -p xtask auth_emulator::tests::installed_emulator -- --ignored --nocapture
```

| Command | Starts / boundary |
| --- | --- |
| `run cantos-server` | One Server + Identity, explicit migrations, private development files and authenticated UDS RPC |
| `run cantos-studio --web` | Same backend composition plus Trunk on 8081 |
| `run cantos-theatre --web` | Same backend composition plus Trunk on 8082, mail disabled; optional explicit local-mail lane as above |
| `run resend-local` | One bounded synthetic HTTP capture/readback; no Server, Identity, Web, native DB or migration |
| `app cantos-studio --web` | Trunk only; requires an existing API |
| `app cantos-theatre --web` | Trunk only; warns when the existing public catalog is unavailable |

The runner owns only the foreground groups/directories it creates, refuses occupied ports and
never takes over another process. Studio/Theatre have distinct loopback origins; `--web-origin`
for a Web profile must match its Trunk origin. Server uses `CANTOS_SERVER_DATABASE`, never
`DATABASE_URL`. Identity owns its own file and socket. Socket/TCP readiness is not authentication,
business HTTP parity, publication, browser/device or deployed acceptance.

Theatre offline previews require explicit `TRUNK_BUILD_FEATURES=preview` plus `?mode=mock`,
`?mode=auth-preview` or `?mode=membership-preview`. Production release tooling removes/refuses
preview overrides; a preview supplies no real account/playback acceptance.

### seed

`cargo xtask seed theatre [--reset]` is opt-in development data, never a gate/preflight/CI step.
The unqualified native planner requires the Server-owned `catalog_http` test fixture and
refuses before setup while that fixture is absent. `--environment` remains a retired refusal.

The explicit `--sample-input JSON --sample-media-source DIR --sample-root DIR` mode instead
uses the prepared #427 input and six original PNG covers, all outside Git. Absolute paths and
separate source/output roots are required. It selects the Server's `sample_catalog_turso`
target, exact ignored `sample_catalog` test: current registry, real file-backed Turso, Studio
catalog/artwork handlers, stable IDs and receipts, close/reopen and idempotent reseed. The
result is six private Works, twelve draft chapters, a Server-compatible `server.db`, private
filesystem media and a manifest preserving the input's explicitly mock MP3 metadata. It
creates no audio acceptance or publication. No flags select a production/user DB.

`--sample-serve` first performs that import/verification and then serves the same real Server
router and Store on `127.0.0.1:8080` until Ctrl-C. It refuses an occupied port. This test-hosted
development composition starts no Identity and is distinct from the standalone Server binary,
browser login, public catalog or Web/CMP/device acceptance. Keep `studio-token` private.

`--sample-root DIR --reset` checks the output ownership marker and both exclusive DB owner locks,
then archives the whole sample root to a sibling `.retired-<timestamp>-<pid>` directory. It
never deletes DB/WAL, media or evidence, and refuses an active owner or unowned output. The
original source flags are unnecessary for reset. See the [sample runbook](../docs/deployment/theatre-seeded-catalog.md)
for preparation, import/reseed, output mapping, recovery and the pending public-cover boundary.

The command adds no gate. Pure command/parser regressions belong to the existing Rust gate;
the prepared-media import/serve campaign remains explicit. Do not restore PostgreSQL setup or
use a persistent/user database to bypass a prerequisite. Identity/user data is separate.

### Explicit local service infrastructure

`cargo xtask services list|validate` reads the native inventory and starts nothing. The old
container `up|down|logs|status` commands refuse before probes/effects. Native dev/E2E/Turso/Kani
requires no PostgreSQL, Kanidm, Auth Emulator or Podman. Production OCI/Quadlet belongs to the
[release contract](../docs/deployment/native-oci-release.md), with activation separately pending.

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
| `protobuf-reference` | independent Python goldens + pinned isolated prost/protoc exact generated-schema drift oracle (both Studio pilot bindings) + its clippy + `tools/proto-codegen --check` over every consumer-owned Protobuf binding (Studio pilot/artwork, Identity RPC server/client and test peers) | cargo, python3, clippy |
| `script-ir-reference` | `scripts/reference_script_ir.py` (independent Python c1 oracle) | python3 |
| `rust-lint` | `cargo clippy <workspace or -p …> [--features cantos-theatre/preview] --all-targets --locked -- -D warnings` | clippy |
| `rust-tests` | when `cantos-server` is selected, first `cargo build --locked -p cantos-identity --features service --bin cantos-identity --target-dir target`; then `cargo test <workspace or -p …> [--features cantos-theatre/preview] --locked` with that executable's absolute path as `CANTOS_TEST_IDENTITY_BINARY` (the candidate journey never builds it) | cargo |
| `forge-functions` | `cargo build --locked -p cantos-forge --bin cantos-forge-worker --target-dir target`, then `node --test test/boundary.test.mjs test/native-worker.test.mjs` in `services/cantos-forge-functions/firebase` | cargo, installed stable Node.js 22+ |
| `contract-tests` | `cantos-server` `contract_conformance` (its own representation against the schemas/fixtures it owns), Script IR, Studio/import/production/audio-intake wire, JSON/MessagePack wire, diff, CLI admission, WAV and ordinary audio-intake test targets; then each client's own representation (`cantos-studio`, `cantos-theatre`, `cantos-web-auth` lib tests filtered to `contract::`/`wire::`) | cargo |
| `wav-reference` | `cargo build -p cantos-server --bin inspect-wav`; `scripts/reference_wav_inspection.py` | cargo, python3 |
| `audio-inspection` | `cargo test -p cantos-server --test audio_intake --locked -- --ignored` | cargo, trusted installed ffmpeg and ffprobe |
| `web-lint` | `cargo clippy -p cantos-ui -p cantos-studio -p cantos-theatre --all-targets --target wasm32-unknown-unknown --features cantos-ui/preview,cantos-theatre/preview --locked -- -D warnings`, then the production lane `cargo clippy -p cantos-theatre --target wasm32-unknown-unknown --locked -- -D warnings` | clippy, wasm32 target |
| `web-build` | `trunk build --locked` per Web client (Theatre: the production lane, no previews) | trunk, wasm32 target |
| `ui-preview` | `trunk build preview.html --locked --dist ../../../target/ui-foundation-preview` in the shared Leptos package | trunk, wasm32 target |
| `cmp-ui-preview` | installed Gradle `desktopTest` in the shared CMP package, with no persistent daemon or JDK provisioning | compatible gradle, java, real CMP Gradle project |
| `theatre-cmp-preview` (formerly `theatre-cmp-demo`, still accepted by `--skip`) | installed Gradle `:shared:desktopTest :preview-sandbox:desktopTest` for the Theatre Desktop Preview | compatible gradle, java, both KMP modules |
| `mutation` | `scripts/mutate_script_ir.py` (nine bounded mutants in a disposable offline copy) | cargo, python3 |

| `verify` suite | Gates |
| --- | --- |
| `repository` | repository |
| `behavior` | behavior (locked model tool, named runs/properties/witnesses/controls; optional pure-core conformance) |
| `rust` | rust-format, rust-lint (workspace), rust-tests (workspace) |
| `forge` | rust-lint (cantos-forge), rust-tests (cantos-forge), forge-functions |
| `turso` | turso (Identity persistence clippy/tests, shared-store tests and capability report) |
| `web` | web-lint, web-build for Studio and Theatre, ui-preview |
| `contracts` | script-ir-reference, protobuf-reference, contract-tests, wav-reference, audio-inspection |
| `mutation` | mutation |
| `kani` | explicitly selected registered `kani` kernels only; opt-in |
| `native-isolation` | explicit prebuilt exact-test isolation; separate executable/credential/network controls, never an implicit build or infrastructure start |
| `native-auth-qualification` | manual #336 campaign: builds first, then guarded exact native auth tests, model/pure-core conformance and optional Kani; never selected by gates, preflight or ordinary CI |

`verify native-isolation` accepts one absolute prebuilt Rust test executable and exact test
name. It sanitizes the child environment, creates private run directories and exercises real
executable/credential injection controls. Linux additionally requires fresh network/PID
namespaces and kernel egress refusal; macOS records best-effort test results but reports overall
`UNSUPPORTED`. `--dry-run` is a pure `NOT_RUN` plan. Its regression tests are registered with
the existing repository/Rust CI jobs; a failure before discovery does not exercise them.
The explicit fixture lane stays outside ordinary gates and CI.
`--identity-binary` names a second, already built `cantos-identity` executable that the test
starts as a child: the guard passes exactly that absolute path as the one test-only key
`CANTOS_TEST_IDENTITY_BINARY` and binds its digest before and after the run (receipt schema 2).
Read the [guard contract and evidence limits](../docs/assurance/native-isolation.md).

`verify native-auth-qualification` is the finite #336 Linux campaign. It records the source
revision, lockfile digest and tool identities; reads `cargo metadata` for the agreed Server
session target (`identity_auth_candidate`, test `real_identity_actor_session_journey`) and its
actual required-feature metadata; builds the Identity executable
and each test executable with `--locked` and resolves them from that invocation's Cargo JSON
(never a glob of old artifacts), retaining the Identity executable at a campaign-owned path;
lists each allowlisted exact test once; then runs each one under `verify native-isolation`,
checking that every receipt binds the built digests and source revision. It then runs the
behavior model and pure-core conformance for the two native auth capabilities and, with `--kani`,
the three registered Identity kernels. It reports `MODEL`, `PURE_CONFORMANCE`,
`IDENTITY_PROCESS_RPC`, `ACTOR_TURSO`, `SERVER_SESSION_CANDIDATE`, `ISOLATION`, `KANI`,
`FULL_CANDIDATE`, `FULL_SERVER_HTTP`, `BROWSER` and `DEVICE` separately and writes one sanitized
JSON report (default `target/verification/native-auth-qualification/report.json`).
`FULL_CANDIDATE` passes only when every required layer passed on one unchanged revision.
The D1 run is the manual `native-auth-qualification` job of `repository-checks.yml`
(`workflow_dispatch` with `native_auth_qualification=true`), which only provisions pinned
tools. See the [campaign boundary](../docs/assurance/native-isolation.md#native-auth-linux-qualification-campaign).

The mutation runner uses `cargo --offline`; run `cargo fetch --locked` once on a
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

`verify turso` first lints/tests the `cantos-identity` service's opt-in `persistence` feature
against run-owned real files, then checks the embedded foundation in `packages/store-turso` and
prints its engine capability report. It also lints/tests Identity's `service` feature (including
its own RPC server binding and wire-golden tests), and lints ordinary `cantos-server` targets. The former `turso-candidate`
feature no longer exists; the real Identity journey still requires a prebuilt executable.
The foundation commands enable its `testing` feature
(run-owned temporary directories and a deterministic clock), then run the
`cantos-turso-capability` binary, which exits non-zero when any of its probes differs from the
expectation recorded beside it. Files live in a private temporary directory removed afterwards. The
gate starts no server, container, database daemon or cloud service; the process probes spawn only
the report binary and `sh` (a file-size limit simulates a full disk). It is selected by changes to
that package and its dependents, belongs to the `rust` CI job, and is part of the full preflight.
Its result is for the pinned engine, this host and this filesystem
([report](../docs/evidence/turso-capability-report.md)). Linux x86_64 and macOS arm64 have recorded
host evidence; a real `ENOSPC` is not covered. Use `report DIR` of the binary to probe another disk.

The `audio-inspection` gate explicitly runs the ignored real-media tests that ordinary
`cargo test` skips. Tests encode owned synthetic signals into temporary files at runtime,
then exercise the actual bounded read-only inspection adapter at all four explicit Opus
profiles and its invalid-media cases. No audio is committed to Git. This test-only fixture
encoding does not add an encoder, TTS provider or production worker to the backend.
The adapter probes and decodes to a discarded output; it never converts or publishes media.
`ffmpeg -version` and `ffprobe -version` must succeed before any selected gate runs.
Missing tools produce `UNSUPPORTED`; skipped or unreachable inspection remains `NOT_RUN`.
xtask never installs these tools. The Rust CI job provisions the distribution's trusted
`ffmpeg` package, which supplies both executables. Historical PostgreSQL media oracles also
used them, but there is no active PostgreSQL CI job or verification lane.

Historical PostgreSQL/Firebase/DBSP runners and suites are retired source inputs; their
commands are hidden refusals. They are never prerequisites for current preflight or native acceptance.
Keep actual engine/file/process coverage separate from historical SQL oracle results.
Direct invocation of `scripts/test_postgres.py` also refuses with exit 2 before argument
parsing, tool probes or cluster setup. Its historical implementation and archived Rust
assertions remain [porting inputs](../docs/architecture/server-turso/legacy-test-oracles.md),
not runnable Cargo targets. Native checks do not establish unported business parity.

### Isolated Theatre enrollment testing

Retired with the provider handlers. `verify theatre-auth` is a hidden refusal; the Firebase
emulator and Kanidm lanes are not native acceptance. Use `verify native-auth-qualification`
for the explicitly scoped Linux campaign, keeping model/core/process evidence distinct from
production HTTP/browser/device acceptance. No infrastructure installation follows from a check.

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

The repository gate's changed-UI localization check uses the same `--base` and merge-base
comparison, including when preflight runs without `--changed-only`. Standalone `verify repository`
and CI retain their automatic develop/CI-parent comparison. No comparison silently skips an
unavailable base.

Selection ([`preflight/select.rs`](src/preflight/select.rs)) is pure and conservative:

- `repository` and `rust-format` always run.
- A path inside a Cargo package selects that package and, through `cargo metadata`, every package
  that depends on it (`rust-lint`/`rust-tests` with `-p`). A package's top-level `README.md` is a
  document. Canonical contracts are build inputs of their consumers: a non-document path under
  `services/cantos-server/contracts/` selects `cantos-server` plus that surface's clients
  (`studio/` → Studio Web; `theatre/`, `auth/` → Theatre Web and `theatre-cmp-preview`; `auth/` also
  `cantos-web-auth`), `.proto` and Protobuf fixtures add `protobuf-reference`, and Script IR fixtures
  `script-ir-reference`. `services/cantos-identity/contracts/` selects Identity and the Server;
  `apps/cantos-theatre/conformance/` selects Theatre Web and `theatre-cmp-preview`;
  `tools/proto-codegen` and every generated binding it writes select `protobuf-reference`. Only
  non-fixture Markdown is documentation.
- An affected Web package selects `web-lint`/`web-build` for that client. Shared
  `packages/ui-system` tokens, icons, Leptos components and their generators affect `cantos-ui`,
  both Web consumers and `ui-preview`; shared semantic inputs and generators also select
  `cmp-ui-preview` and `theatre-cmp-preview`. Package-local CMP source, Gradle and resource changes
  select both local CMP gates directly because Kotlin is not a Cargo workspace member. Renderer documentation alone remains
  a document, and a Leptos-only change does not require Gradle.
  An affected `cantos-server` selects `turso`, `wav-reference` and `audio-inspection`.
- Non-document inputs beneath `services/cantos-forge-functions` select its Rust lint/tests and
  `forge-functions`; they do not select the API, database or Web clients.
- Protobuf schema, fixtures, Rust adapters and oracle inputs select `protobuf-reference`; schema/fixture/adapter changes also select `theatre-cmp-preview` so both languages consume the independent corpus. The oracle has its own pinned lock and vendored protoc; runtime/WASM builds never run protoc.
- Script IR fixtures select `script-ir-reference`; Script IR sources and tests select `mutation`.
- Non-document behavior inputs select the model-only behavior gate; their Markdown needs only
  the always-on gates. Verifier code (`scripts/verify_behavior.py`, `scripts/behavior_registry.py`,
  `tools/behavior`, the root `catalog.json`) selects every model. Any other registered or
  unregistered `.qnt`, manifest or `specs/behavior` file the registry does not resolve also selects
  every model, so unknown impact is never skipped. A module keeps its models in the `behavior/`
  directory of its boundary (`services/<name>`, `apps/<name>`, `xtask`); that location alone marks
  a file as behavior, so without a registry index it selects the unscoped gate rather than the
  owning package, and `xtask/behavior` is not broadened as xtask code. A capability document there
  is Markdown and needs only the always-on gates.
- Changed-only preflight asks the registry for its reverse index (`scripts/verify_behavior.py
  --impact-index`, read-only, no Quint/Node) and plans one `behavior (CAPABILITY)` gate per
  affected capability: its model, scenarios, import closure, manifest, exact non-document
  `sourceRefs`/`owner` and bounded `ownedSources`/`implementationTests`. A shared stateless helper
  selects every importer. The index is derived from the registry, never a second handwritten map;
  when it is unavailable or the registry is invalid, selection keeps the previous unscoped gate and
  the Behavior gate reports the registry error. Registered production cores and observer tests of
  an observed capability select `behavior (CAPABILITY, conformance)`; the shared codec scripts
  replay every observer. Registered behavior files are not Cargo inputs, so a module-owned model
  does not broaden the Rust gates; production sources keep their normal package gates beside it.
- Primitive scripts select the gate that exercises them; a script with no known impact selects every gate.
- Documents, templates, skills and issue forms need only the always-on gates. The native service
  manifest and configuration docs are statically validated by
  `repository`; no Podman or runtime input is required. The Studio Mobile scope notice is documentation only.
  Theatre mock Kotlin/Gradle/native host inputs select `theatre-cmp-preview`; its docs are documents.
- Shared Cargo inputs (`Cargo.toml`, `Cargo.lock`, `.cargo/`, toolchain/format/lint files),
  verification tooling (`xtask/`, `.github/workflows/`), an
  unclassified path or an empty change set select every gate.

`--skip GATE` (repeatable) is the only way to leave out a selected gate. The gate is reported as
`NOT_RUN`, never `PASS`. State the exact skip and prerequisite in the pull request; retired commands are not selectable gates.

#### Local profile

```sh
cargo xtask preflight --profile local --changed-only --base "$BASE_REF" [--dry-run]
```

The default profile is `handoff`: everything above, unchanged. `--profile local` is a lighter
DEV/CHECKPOINT loop over the same change collection, base and reverse Cargo graph (normal, dev and
build dependencies). It requires `--changed-only`, rejects `--kani`, and plans only:

- `repository-static`: service manifest validation, `check_repository.py --base` and both
  whitespace checks, without the Python unit-test discovery. Code under `xtask/` (fixtures
  included), `scripts/` or `.github/workflows/` selects the full `repository` gate instead, and
  `xtask`'s own lint/tests for xtask or workflow inputs. Markdown under `xtask/` outside
  `xtask/fixtures` is a document here.
- `rust-format`, then `rust-lint`/`rust-tests` for the affected packages with the existing
  commands, and `web-lint` for an affected Web package. Packages keep their default features
  (plus `cantos-theatre/preview` when selected); no other feature lane is claimed.
- Shared Cargo inputs (`Cargo.toml`, `Cargo.lock`, `.cargo/`, toolchain/format/lint files) and
  unclassified paths broaden to every *local* check (full `repository`, workspace lint/tests and
  both Web clients' WASM lint) with the reason printed. A collected, truly empty diff runs only
  `repository-static` and `rust-format`. A missing base is still an error.

It never dispatches Trunk or preview builds, CMP/Gradle, `turso`,
`wav-reference`, `audio-inspection`, `script-ir-reference`, `protobuf-reference`,
`forge-functions`, `mutation`, behavior or Kani, so only the selected steps' tools are required.
The plan lists the gates the handoff selection would add for the same diff as *outside the local
profile*: they are neither `PASS` nor `NOT_RUN` rows, and notes name the uncovered feature lanes
and Kotlin/CMP sources. The summary is titled `local checkpoint`, and every result states that full
handoff, database integration, platform runtime and formal verification are not established by
this profile. Use the default profile on the final diff at handoff.

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

Capabilities are inline records or root pointers to module-owned manifests
([federation](../specs/behavior/README.md#federated-registry-and-module-owned-manifests));
`--impact-index` prints the resolved reverse index without running anything.

This gate is outside full preflight/CI until its toolchain is separately provisioned.
Non-document model/catalog/tool inputs and the behavior primitive select it in changed-only
preflight. Shared verification changes retain conservative full selection, including behavior when
changed model/tool or registered observation inputs select it. The optional conformance
observer runs production pure functions in a unit-test binary, never a database/provider
process. See the [observation protocol](../specs/behavior/observation-protocol.md).

### Optional targeted Kani proofs

`cargo xtask verify kani` selects exactly fourteen registered propositions. Ten are API-library
kernels: `catalog-revision`, `image-bounds`, `wav-payload`, `submission-window`,
`publication-commit`, `listener-access`, `listener-reconcile`, `byte-range`, `playback-grant`
and `public-cover`. One is in the `cantos-forge` package: `forge-attempt-ledger`
(`services/cantos-forge-functions/src/domain/attempt.rs`). Three are in `cantos-identity`:
`identity-security-version`, `identity-completion` and `identity-mailbox-proof`. Every kernel names exactly one workspace package
(the closed allowlist is `cantos-server`, `cantos-forge` and `cantos-identity`) and passes it to the primitive as
`--package`; the primitive refuses any other name, and an absent value means `cantos-server`.
Repeat `--harness` to select a subset;
unknown selectors and arbitrary verifier arguments are rejected. Registration owns the actual
fully qualified harness names, unwind 2, expected covers (6/13/7/8/6/5/7/6/6/6 for the API
kernels in the order above, 9 for `forge-attempt-ledger`, 6/10/16 for the Identity kernels) and
reviewed unreachable checks (none for Forge or Identity).
The last three API kernels were registered for #170/#171 without an actual Kani run (pinned x86_64 Linux
model unavailable on the authoring host); see the [proof ledger](../docs/assurance/kani.md#public-media-kernels-registered-not-yet-run).
The earlier [execution record](../docs/assurance/kani.md) describes their production reach and
limits. The listener kernel checks actual keyset order/expiry; historical Web mock harnesses remain outside this registry.

The registered Forge kernel's proof is `NOT_RUN`: the registration/selection tests and the
primitive's synthetic tests are not a Kani execution of its harness. It becomes evidence only when
`cargo xtask verify kani --harness forge-attempt-ledger` runs on a host with the pinned Kani
0.68.0 bundle and its retained output is inspected. It adds no new tool, dependency or CI job:
the same pinned bundle, primitive and opt-in gate serve all three packages.

The three Identity kernels have [their own ledger](../docs/assurance/kani.md#native-identity-kernels-registered-not-yet-run): all three passed on the D1 run recorded there.
Under #331 D1 their evidence requires an actual pinned Ubuntu x86_64 `workflow_dispatch`
run URL/SHA and complete assertion/unwinding/cover results. Normal macOS unit tests and
synthetic verifier-adapter output do not establish `bounded-model-checked`.

This is separate from normal builds, `verify rust`, full preflight and every CI job. Explicit
`preflight --kani` adds the registered gates; `--changed-only --kani` narrows to affected kernels.
Kernel source/owner inputs select their propositions. Selection is package-local: API
manifest/build/contract inputs select only the ten API kernels, and a changed
`services/cantos-forge-functions/src/domain/**`, `services/cantos-forge-functions/src/lib.rs` or `services/cantos-forge-functions/Cargo.toml`
selects only `forge-attempt-ledger`. Identity manifest/lib/module/ID/version inputs select
all three Identity kernels because completion/proof reach the ID/version constructors. Admission
source selects `identity-completion` and `identity-mailbox-proof` (which uses `Purpose`);
`domain/mailbox_proof.rs` and its child sources select `identity-mailbox-proof`. The primitive and its tests, shared Cargo/toolchain/
verification and uncertain changes select all fourteen. Unrelated documents, Web/CMP, other Forge
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
manifest and the owning package's manifest only (`services/cantos-server/Cargo.toml` or
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
before/after lock and model digests. Model binding for an API kernel covers all API source
(including its own `src/contract` representation), the Server-owned canonical contracts, its
manifest/build script, root Cargo/.cargo settings and
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
| `rust` | rust-format, protobuf-reference, rust-lint (workspace), rust-tests (workspace), forge-functions, turso, wav-reference, audio-inspection |
| `web` | web-lint, web-build (Studio and Theatre) |

`cargo xtask ci` runs all four locally; [the workflow](../.github/workflows/repository-checks.yml)
runs one per GitHub job after `rustup show active-toolchain` selects the exact
[`rust-toolchain.toml`](../rust-toolchain.toml) pin. CI has no `--skip`. Every full-preflight gate
runs in CI except `mutation`, `cmp-ui-preview` and `theatre-cmp-preview`. Mutation stays opt-in through
`verify mutation` and is selected by preflight when Script IR sources change. The CMP desktop preview is
a local `ui-preview cmp verify` / preflight gate until a separately provisioned Gradle CI lane is
added. Existing `verify web` and the Web CI job never require the Kotlin toolchain. Branch
protection and required checks are configured outside this repository.
The repository job checks out full history so manual feature-branch dispatch can compare
against `origin/develop`; checkout credentials remain disabled and a missing base still fails closed.
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
the Gradle root, which has no Android preview module today (#287); a production-safe stateless
preview would need its own consumer first. Missing CLI or
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
| `theatre_size.rs` | optional single-step plan around `scripts/apk_size.py`; reads one explicit APK and writes an ignored local report, never distributes |
| `web_performance.rs` | optional fresh release-build/inventory plans; the Python primitive records byte identities/estimates, never browser or deployment acceptance |
| `hosting_report.rs` | explicit offline operator-declared rollout reporting and self-test; no provider/status API, publisher or acceptance authority |
| `behavior.rs` | narrow model/optional conformance command and read-only probe plans; Quint owns expectations |
| `main.rs`, `cli.rs` | clap definitions and dispatch; usage errors exit 2 |
| `workspace.rs`, `config.rs` | root discovery; ports, packages, directories and versions, checked against the repository by tests |
| `command.rs` | immutable `CommandSpec` values and the `Runner` abstraction |
| `tools.rs` | tool detection behind `ToolProbe` |
| `pipeline.rs`, `report.rs` | prerequisite checks, sequential execution and the summary |
| `gate.rs`, `build.rs`, `ci.rs` | gate catalog, build plans and CI jobs |
| `ui_preview.rs` | package-local Leptos/CMP preview plans and supervised execution |
| `run.rs`, `process.rs`, `probe.rs` | launch plans, supervised process groups, port ownership and readiness |
| `seed.rs`, `seed/sample.rs` | explicit `seed theatre` plans; isolated prepared-sample dispatch and marker/owner-locked archival, separate from the retained synthetic publication lane |
| `preflight/` | Git change collection, the Cargo package graph and pure handoff/local selection |
| `theatre_auth.rs` | pure plans and redacted outcomes for explicit isolated enrollment lanes; invokes the owned Python fixture primitive, outside ordinary gates |
| `doctor.rs` | read-only environment report |
| `services/` | typed process inventory/config validation, explicit local Podman lifecycle, bounded command boundary |
| `distribute/` | explicit real-only development/release APK tester distribution: `request` (pure plan), `apk` (output metadata and tool-report validation), `firebase` (CLI argv, destination check, remote-outcome interpretation, redaction), `receipt`, `host` (files, Git, clock) and `flow` (stage ordering and short-circuiting) |
| `deploy/` | explicit static Web deployment to Firebase Hosting: `request` (pure plan), `hosting_config` (tracked policy, `.firebaserc` mapping, effective configuration), `bundle` (exact-bytes validation and manifest), `firebase` (CLI argv, site lookup, remote-outcome interpretation, redaction), `smoke` (read-only HTTPS judgement), `receipt`, `host` (files, bundle scan, Git, clock) and `flow` (stage ordering and short-circuiting) |
| `run_files.rs`, `firebase_cli.rs` | what `distribute` and `deploy` share: private per-run directories, SHA-256, Git source identity and atomic records; the Firebase CLI version gate and output redaction |
| `secrets/` | explicit operator-only Secret Manager commands: `inventory` (tracked mapping, pure), `request` (flags, payload metadata/shape, pure), `gcloud` (argv, failure classification, output parsing, pure), `host` (one approved input, the gcloud child, the ignored ledger) and `flow` (account → secret → version fence → add, short-circuiting) |
| `capture.rs` | bounded captured execution with a typed timeout/interruption/output-limit vocabulary, used where output must be interpreted |
| `native_isolation.rs`, `native_qualification.rs` | the isolation primitive's plan and typed receipt validation; the #336 campaign's fixed allowlist, Cargo JSON artifact resolution, layer statuses and sanitized report |
| `compose_preview.rs` | optional host-preview command and status mapping; the Python primitive owns capture/log/output validation |

Plans are values: tests assert exact commands with a recording runner and fake tools, while
process tests start and stop real `sh` and `python3` children. Run them with
`cargo test -p xtask --locked` (part of `verify rust`).

## Adding a capability

Add a command only for an implemented, reviewed capability. The isolated Theatre mock is
planned through `theatre-preview`; the generic production mobile gate remains `UNSUPPORTED`. Studio is web-only;
do not add a mobile plan, scaffold or target without an explicit future roadmap request. A new script becomes a gate
in `gate.rs`, is classified in `preflight/select.rs` and, if CI must run it, is added to a job
in `ci.rs`. Release, deployment and provider-backed commands wait for their infrastructure; the
exceptions are the APK `distribute firebase` command, the static Web `deploy firebase` and
`deploy cloudflare` commands and the operator-only `secrets` command, which stay outside every
gate, preflight selection and CI job.

### distribute firebase

Explicit operator-only APK distribution; may notify selected testers. `development` maps to
`:androidApp:assembleDebug` / `com.cantos.theatre.dev`; `production` maps to optimized
`:androidApp:assembleRelease` / `com.cantos.theatre`. Both are real-only. `demo`/`demo-qa` are
retired and refused before side effects. Firebase registration and signer pins are exact per
package; no `.demo` App ID/key fallback is supported. AAB distribution remains `UNSUPPORTED`.

```sh
cargo xtask distribute firebase cantos-theatre --channel development \
  --api-origin https://api.example.org --groups cantos-internal --dry-run
cargo xtask distribute firebase cantos-theatre --channel production \
  --api-origin https://api.example.org --groups cantos-internal --dry-run
```

Dry-run is pure planning: no subprocess/network/write. Real execution needs an explicit App ID,
recipients and pinned certificate. Production uses only operator-supplied `CANTOS_RELEASE_*`
signing inputs and the same release/audit primitives as `theatre-host`; debug cannot substitute
for release. Exact AGP output metadata, staged manifest/version/config/signature (no browser callback or URL scheme),
audit-marker refusal and APK/mapping binding precede the one remote write. Intent receipts
record staged hashes; unknown remote outcomes require console reconciliation before replay.

The [release/distribution runbook](../apps/cantos-theatre/mobile/docs/firebase-distribution.md)
owns inputs, audit pairing, key/registration/version migration, independent device/live gates
and recovery. [Infrastructure inputs](../infra/firebase/app-distribution/README.md) point there.
Build/run/tests/preflight/CI never distribute, install, uninstall or reset app data.

### secrets

Operator-only Google Cloud Secret Manager custody for the project named in the tracked
[inventory](../infra/google-secret-manager/inventory.toml). The
[runbook](../docs/deployment/google-secret-manager.md) owns IAM, consumers, version pins,
rotation and runtime binding; this section is the command contract.

```sh
cargo xtask secrets inventory
cargo xtask secrets status --project cantos-a348b --account operator@example.com --environment prod
cargo xtask secrets import --project cantos-a348b --account operator@example.com \
  --secret cantos-prod-server-playback-key --file /home/operator/private/playback.key \
  --expect-latest none --dry-run
```

- `inventory` validates names, environments (`dev`/`prod`/`ops`), consumers, file/env delivery,
  formats and numeric pins offline. Ops secrets cannot name Server or Identity; runtime secrets
  cannot name a deploy job or the operator; a pin is a positive number, never `latest`.
- `status` lists version metadata (`MISSING`, `IMPORTED vN`, pin `ENABLED` or not). It never
  accesses a payload, so it cannot establish `VERIFIED_ACCESS` or `RUNTIME_BOUND`.
- `import` refuses an unlisted secret, another project, an implicit account and a missing fence
  before starting gcloud. Real runs read one owner-only (`0600`/`0400`), non-symlink regular file
  outside the checkout, or piped stdin (never a terminal), bounded to 64 KiB and checked against
  the consumer's format (text without control characters, exactly 32 bytes, a service-account
  JSON shape or opaque bytes). Then: the account must already hold a gcloud credential; `get` or
  `create` (metadata only, automatic replication); the version fence (`--expect-latest` must
  equal the highest version ever issued); one `versions add --data-file=-` with the bytes on
  stdin. Every gcloud call names `--project` and `--account`.
- gcloud's stderr is classified (`NOT_FOUND`, `PERMISSION_DENIED`, API/billing not enabled,
  credential required, transport, unknown) and never printed. A transport, unknown or killed add
  is `UNCERTAIN`: it is never retried; reconcile with `status` and the fence.
- `--dry-run` starts no gcloud, makes no request, writes no ledger and does not open the input
  (a file is `lstat`ed only). Real attempts append a sanitized line (project, secret, account,
  action, result, version, time) to the ignored owner-only `artifacts/secrets/import-ledger.ndjson`.
- Missing gcloud is `UNSUPPORTED`. xtask never installs gcloud, logs in, enables APIs or
  billing, grants IAM, generates a value, reads a payload back, binds a runtime or restarts a host.

Regressions run against a scripted gcloud with synthetic payloads in `cargo test -p xtask`
(the `rust` gate); live `cantos-a348b` writes are `ACCEPTANCE(NOT_RUN)` until an operator runs them.

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
`--channel` are required and never defaulted; `cantos-server`, `cantos-forge`, `all` and mobile are
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
non-static rewrites; no rewrite that is not a literal route outside `/api`; a
`Cache-Control: no-cache` rule for every path and no long-lived caching) and renders a
run-specific effective `firebase.json` and `.firebaserc` in the private run directory holding that
one target, that one site and a run-relative `public`. Every Firebase subprocess except the initial `firebase --version` probe (run from the checkout root, with no project) gets an explicit
`--project`, `--config` and the run directory as its working directory, so no active or default
project, other product or `firebase-debug.log` in the checkout can be involved.

**Stages** (each failure stops the rest; unreached stages are `NOT_RUN`):

1. *Local prerequisites*: Python/Git for artifact mode; otherwise Cargo, the wasm32 target, Trunk and Git installed, and no inherited
   `TRUNK_*` variable that could redirect the build (reported by name, never value).
2. *Hosting configuration*: the policy and the mapping above.
3. *Firebase CLI*: `firebase --version` must be 15.x and at least 15.34.0, the shared pin with
   `distribute firebase`. xtask installs nothing and never logs in.
4. *Run record*: a new private directory `artifacts/hosting/<run-id>/` (ignored by Git; concurrent
   runs never share one), the effective configuration and the first receipt.
5. *Hosting destination*: read-only `firebase hosting:sites:list --json` in the explicit project; the
   mapped site must be in it. Missing login, site or permission stops here, before the build. This is
   not proof of deploy permission.
6. *Release preparation*: either verified v2 artifact extraction (above), or the existing locked release Trunk build (`build::release_bundle`, shared with
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
    served bytes. Curl negotiates transfer compression with `--compressed` and hashes the decoded
    response against the built bytes; wire compression does not change the bundle identity.
    A missing `.wasm` and an `/api/` path must answer `404` rather than the
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

Deliberately absent: `--no-build`, an arbitrary dist bypass, deploy-all,
cross-pipeline promotion, rollback or channel/site deletion, a provider framework, long-lived caching of
hashed assets, any Firebase SDK, Functions, Cloud Run or App Hosting adapter. An explicit Web
API-origin setting now exists under ADR 0041; this command still deploys static bytes only.
Read current release state from Firebase, and keep user-journey acceptance separate from smoke.
Historical tool evidence is [source-bound](../docs/evidence/issue199-firebase-hosting.md).

### deploy cloudflare

```sh
# Local plan only: no tool probe, extraction, login, upload, receipt or artifact write.
cargo xtask deploy cloudflare cantos-studio --web --account ACCOUNT_ID --project cantos-studio \
  --branch review-pages --environment preview \
  --artifact artifacts/release-build/studio-web/SHA --dry-run

# Publish the verified artifact as a PUBLIC preview. A REMOTE WRITE; CLOUDFLARE_API_TOKEN in the env.
cargo xtask deploy cloudflare cantos-studio --web --account ACCOUNT_ID --project cantos-studio \
  --branch review-pages --environment preview --artifact artifacts/release-build/studio-web/SHA
```

The third opt-in exception to "xtask publishes nothing" ([ADR 0012](../docs/decisions/0012-xtask-command-surface.md);
decision [ADR 0043](../docs/decisions/0043-cloudflare-pages-direct-upload.md)). Setup, the stage list,
delivery policy, unknown-outcome handling and the CircleCI lane are in the
[Cloudflare Pages runbook](../docs/deployment/cloudflare-pages.md); this section is the contract.
`build`, `run`, `verify`, `preflight`, `ci` and `build all` never invoke it and gain no Cloudflare tool
or credential requirement.

**It never builds.** It consumes the output of `release-build <app>-web` (`manifest.json` plus
`<app>-web.tar.gz`, nothing else in the directory), verifies the manifest strictly and the archive's size
and SHA-256 against it, and extracts it in-process into a new private run directory
(`artifacts/cloudflare/<run-id>/public`) refusing every non-regular, absolute, `..`, hidden, duplicate
or over-limit entry. It then adds the tracked `infra/cloudflare/<app>/` policy files (`_headers`,
`404.html`, optional `_redirects`; for Theatre only, the optional content-validated
`.well-known/assetlinks.json` and `.well-known/apple-app-site-association` of ADR 0048), validates the
exact bytes with the shared bundle validator in its Pages mode (only those root names and the two
association paths are allowed beyond the usual static types; Theatre's `verify-email.html` must equal
its `index.html`; `_worker.js`,
`_routes.json`, `functions/`, source maps, `.env`, `package.json` and foreign-app files are refused; the
Free 20,000-file and 25 MiB per-asset limits are enforced on decoded sizes), and uploads that directory.

**Request.** App, `--web`, `--account` (32 hex), `--project`, `--branch`, `--environment`
(`production`|`preview`) and `--artifact` are required and never defaulted. The private mapping
`infra/cloudflare/projects.json` (ignored by Git; `projects.example.json`) must name the same
account → app → project and a production branch; production requires that branch and preview refuses
it. Missing/disagreeing mapping also returns failure in dry-run without probing or writing. The wrong account, project, app or branch stops before extraction or upload.

**Wrangler.** The pinned release (4.151.2, Node 22+) is run from `infra/cloudflare/wrangler/node_modules/.bin`;
no `npx`, global install, login or implicit download. Every invocation gets the run directory as its
working directory, `CLOUDFLARE_ACCOUNT_ID` set to `--account`, telemetry off, and the redirecting
Cloudflare/Wrangler variables unset; a Wrangler config, `.env`, `functions/` directory or `_worker.js` in
the run or an ancestor stops the run. The one write is `pages deploy <dir> --project-name … --branch …
--commit-hash <manifest SHA> --commit-dirty=false --skip-caching`, never retried. The deployment ID, URL,
environment and production branch come from `WRANGLER_OUTPUT_FILE_PATH` (`pages-deploy-detailed`) and are
validated against the request; exit status 0 alone is never success.

**Outcomes** are *confirmed*, *mismatch* (a deployment exists but not as requested), *rejected* (a
recognised refusal, no deployment), *unknown* (timeout, interruption, exit 0 without a single valid
identity, or an unclassified failure) and *not started*. After an unknown result the command lists the
project's deployments of the same branch and commit as evidence only; the operator reads the target
back before any retry. A confirmed deployment is read back and smoked separately: a failed smoke never
changes what is known about the deployment. The receipt (`receipt.json`, schema 1, private) records the
source SHA, manifest and archive digests, policy and bundle digests, account, project, branch,
environment, tool version, stage outcomes, the remote deployment and the smoke observations;
output is redacted (Pages and dashboard links stay).

Deliberately absent: building, an arbitrary directory or ZIP, creating/changing projects, zones, DNS,
bindings, the production branch or billing, deploy-all, automatic rollback, retry, promotion or deletion,
a second provider adapter, long-lived caching of hashed assets. Real-account acceptance is `NOT_RUN`.

### deploy vps

Retired: the PostgreSQL/Kanidm/two-API archive executor refuses before effects. Use
`release oci-build` for candidate image production and `release oci-verify` for offline admission.
No xtask command activates a native host yet. [#348](../docs/deployment/native-oci-release.md)
owns activation/rollback; legacy assets and records remain recovery inputs, not native deployment
instructions. No VPS operation follows from a docs change.

### theatre-preview

The Desktop Preview command surface (#287); starts no API, database, identity provider or audio
adapter. Its default recipes are offline. The explicit [live enrollment mode](../apps/cantos-theatre/mobile/README.md#live-enrollment-integration-296)
uses real Identity v1 HTTP with explicit `CANTOS_THEATRE_PREVIEW_API_ORIGIN`; it requires
HTTPS for auth, provisions no dependency and has no mock fallback.
`theatre-demo` is its historical name and runs the same commands.

```sh
cargo xtask theatre-preview build          # :preview-sandbox:compileKotlinDesktop (depends on :shared)
cargo xtask theatre-preview verify         # :shared:desktopTest + :preview-sandbox:desktopTest
cargo xtask theatre-preview run            # supervised desktop window; offline recipes or explicitly configured live enrollment
cargo xtask theatre-preview android-build  # retired (#286): refused as UNSUPPORTED; use theatre-host android-debug
cargo xtask theatre-preview ios-build      # retired (#287): refused as UNSUPPORTED; use theatre-host ios-build
```

Build/verify/run require installed Gradle 8.13, 8.14.1, 8.14.2 or 8.14.3 and a full JDK 17+. They
consult no Android SDK, Xcode or Kotlin/Native and pass no `cantos.android|ios|native` property: the
sandbox is a JVM-desktop-only module. Missing prerequisites are `UNSUPPORTED`, failed executed steps
are `FAIL`; the retired verbs are refused before any probe.

Only the Desktop Preview consumes `:preview-sandbox`; the production-safe `:shared` module never
depends on it. Verification retains both the independent shared-core suite and the sandbox
owner/workbench/UI tests. These commands produce Desktop Preview evidence only; they cannot accept
production `releaseCompileClasspath` / `releaseRuntimeClasspath` or APK/AAB exclusion. Use
[`theatre-host`](#theatre-host) for those claims.

The local `theatre-cmp-preview` preflight gate runs mock common/JVM tests when its source/build
inputs or consumed CMP/shared tokens change. It does not stand for native acceptance or
component qualification. It is outside CI until a Gradle lane is separately provisioned.
`run` aligns JVM args, passes its owned launcher PID, rejects detached run JVMs, and uses
only stored owned process groups for cleanup. The supervisor keeps exited leaders unreaped
until their group has been signalled, preventing descendant leaks without stale-PGID lookup.
Details and evidence live in the [mock guide](../apps/cantos-theatre/mobile/README.md).

### theatre-host

The one Theatre Android application (`:androidApp`; `release` is `com.cantos.theatre`, `debug` is
`com.cantos.theatre.dev`, no flavors) composes `:shared` only, with no mock, sample or diagnostic fallback. Debug and release browse the real
public catalog when built with an API origin; release accepts HTTPS only. Both declare INTERNET
and origin metadata; release forbids cleartext. An omitted release origin is passed as an explicit
absence marker `__CANTOS_DEFAULT_API_ORIGIN__`, overriding ambient settings: the app then uses the production default
`https://api.cantos.cc` (#298); it never renders a mock and an invalid origin is refused at runtime
with a typed configuration diagnostic. Native playback,
account and Library owners are composed; platform runtime/auth acceptance remains separately
tracked under #14/#35/#191.

```sh
cargo xtask theatre-host android-release --api-origin https://api.example.org
cargo xtask theatre-host android-boundary-tests # explicit offline negative graph probes; no APK
```

Requires installed Gradle 8.13–8.14.3, JDK 17+, SDK platform 35/build-tools 35.0.0; missing tools
are `UNSUPPORTED`. Both variants retain compile/runtime graph guards and an AGP ALL pre-shrink
class observer with source/resource fixture checks. No Android task compiles the sandbox.
Release builds/inspects an unsigned unshrunk **release variant** in `androidApp/build-audit`, then
optimized release in `androidApp/build`. Source/class/graph inputs must match; exact APK/AAB,
inspection and mapping hashes go into `reports/release-boundary/release-evidence.json`. Audit
marker bytes are rejected by distribution. See the [owning runbook](../apps/cantos-theatre/mobile/docs/firebase-distribution.md)
for comparison metrics, operator signing and remaining acceptance. No third build type is added.

`theatre-host android-debug --api-origin ORIGIN` builds `:androidApp:assembleDebug`
then inspects the actual debug APK (expecting application ID
`com.cantos.theatre.dev`) and prints
installed-tool signature/version facts (`apksigner verify --print-certs -v`, `aapt2 dump badging`)
with `-Pcantos.apiOrigin=ORIGIN`. It accepts `https://` or `http://127.0.0.1|localhost:<port>`
(for `adb reverse`) and refuses anything else with exit 2 before Gradle. The debug manifest overlay
adds only a cleartext policy limited to those loopback hosts. The APK composes the production catalog, native player and account/Library owners.
The one origin owner (`selectCatalogOrigin`) admits that origin once; the public catalog and the
first-party Identity v1 JSON transport (email/password) both use it, so a debug host's loopback HTTP
origin also carries Identity v1. Release builds and iOS stay HTTPS-only and the release
manifest/network policy forbid cleartext. `--api-origin` is the only app configuration input: no
build declares a browser callback, custom URL scheme or provider issuer, and the inspector rejects a
built manifest (debug or release, audit or optimized) that carries one. The single admitted
`BROWSABLE` filter is [ADR 0048](../docs/decisions/0048-signup-email-link-return-to-app.md)'s
`autoVerify` App Link on the launcher for exactly `https://cantos.cc/verify-email`; any other host,
path, prefix, pattern, scheme or a second copy is still a finding. Installing it on a
device is the developer's own `adb install`.

`theatre-host ios-build --kotlin-native-home PATH` checks the installed Kotlin/Native 2.2.20
compiler and Xcode simulator SDK, then links only `:shared` debug/release iOS simulator frameworks
named `CantosTheatre`. The production `iosApp/Cantos.xcodeproj` / `Cantos` scheme consumes these
frameworks and shared resources; it imports no sandbox (the `CantosTheatreDemo` framework no longer exists). Set the scheme's
nonsecret `CANTOS_API_ORIGIN` to an HTTPS origin. No compiler, Xcode or SDK is installed/selected;
missing prerequisites are `UNSUPPORTED`. Framework compilation is separate from app/device acceptance.

Release defaults to unsigned; `--operator-signing` requires the supplied release key while its audit stays unsigned. Debug uses Android debug signing. Neither command installs
or distributes anything. Packaging/exclusion evidence is separate from production release and
native listening acceptance. Preflight does not run it (it selects only the JVM
mock gate for host changes and notes this command). See the
[isolation record](../docs/evidence/issue122-cmp-sandbox-isolation.md#production-android-host-and-release-artifact-acceptance).

The old `CANTOS_TEST_CONTEXT=editorial|catalog|audio` retained-cluster fixture is historical.
`run --test-cluster` and the direct PostgreSQL runner refuse before setup; do not use them
to qualify a native host. The archived fixture assertions remain #333 porting inputs.
### Studio Kanidm operator tools

Retired with provider authentication. `cargo xtask kanidm` refuses before metadata requests,
probes, account/bootstrap operations or writes. Current auth uses
[Identity v1](../services/cantos-server/contracts/auth/identity-auth/v1/README.md); Studio roster admission remains distinct
from password authentication, and Admin fails closed without its verifier.
