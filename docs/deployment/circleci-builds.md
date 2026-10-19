# Opt-in CircleCI release artifacts and promotion

**Implemented source contract; account integration, real product builds and activation remain
separate acceptance.** Follow the [Cloudflare rollout/recovery runbook](cloudflare-rollout.md)
for #278/#280. This change retains the Firebase static publisher; it does not implement a
Cloudflare adapter, submit a pipeline, provision contexts, accept terms or purchase credits.

## Request and trust boundary

Run a reviewed configuration on `develop`, after the operator qualifies the account/project,
credits and permissions. Default pushes/PRs select no CircleCI jobs. GitHub checks are unchanged.

| Request | Parameters | Effect |
| --- | --- | --- |
| Build only | `build=true`, empty `target`, `build-component=api|theatre-web|studio-web|android|all` | One preparation job, no production context or publish |
| Release | `build=false`, nonempty `target`, `component=api|theatre-web|studio-web|web|all` | Prepare → one approval → selected deploy jobs → component report |
| Conflicting | `build=true` and nonempty `target` | Explicit failing job; never a successful no-op |

`api-origin` is a public exact HTTPS origin (default `https://api.cantos.cc`). Web build bytes
and configuration identity include it. Android retains its existing fixed production origin;
release `all` means API + both Web apps, while build-only `all` also includes unsigned Android.
Select one target for the first bounded acceptance run. Android signing/distribution is unchanged.

The credential-free preparation job calls `cargo xtask release prepare --component COMPONENT
--api-origin ORIGIN --transfer artifacts/release-transfer [--target REF|--build-only]`.
It requires actual CircleCI identities, clean source and `HEAD == CIRCLE_SHA1`; fetches the
repository; checks provider-supplied `pipeline.config.sha`, `pipeline.config.repository.url` and
`pipeline.config.ref` for this repository/develop; resolves the target exactly once into a full commit SHA; and requires that commit
be an ancestor of `origin/develop`. The immutable request records **config revision**, **release
revision**, requested ref, origin, selected components and producer pipeline/workflow/job/number.
The request also records the initial checkout SHA independently from the config SHA.
No code rewrites `CIRCLE_SHA1`. Source checks after checkout compare against that resolved request
and still reject dirty or moving source. Older revisions lacking this contract cannot be promoted.

The request is meaningful only inside a **trusted reviewed CI workflow**. Hashes detect changed
bytes; caller-written JSON and CircleCI-shaped environment variables do not authenticate a
producer. Do not provide production secrets as project-wide variables, build parameters or
checkout keys. Review config/source and effective context restrictions before approval.
Cross-pipeline artifact promotion, artifact downloads by `latest`, and reuse of dependency caches
as release storage are unsupported.

## Build once and transfer

Preparation runs `cargo xtask ci --job repository` once, Rust if API is selected, and Web once
if either Web app is selected. A failed required gate stops packaging. It never suppresses
qualification failures. Verification compilation is independent of the release build.

Local build-only packaging remains `cargo xtask release-build COMPONENT [--api-origin ORIGIN]
[--dry-run]`. Web requires the flag or `CANTOS_API_ORIGIN`; it is never overwritten by a constant.
`cargo xtask release self-test` exercises contract, packaging, DAG, mocked promotion and existing
VPS guard tests plus focused xtask lint/tests;
`cargo xtask release-build --self-test` retains focused packaging tests. Repository verification
discovers the Python tests; the Rust gate includes the CLI/deploy tests. No test invokes a real publisher, SSH deployment or product build.

Manifest v2 is the only promotable version; v1 artifacts require a rebuild. Output identity is
`artifacts/release-build/COMPONENT/SOURCE_SHA/CONFIG_SHA256/ATTEMPT/manifest.json` locally.
CI uses the same layout beneath a new `artifacts/release-transfer/ATTEMPT/artifacts/` and records
exact manifest paths/digests in one inventory. No mtime discovery or output overwrite occurs.
Interrupted attempts have no final manifest. Fresh staging, clean-source/stability checks,
actual-byte SHA-256/size, exact file inventories and archive validation precede finalization.

The manifest binds component, repository, full source/checkout/config SHA, public build inputs,
configuration/policy digest, version, toolchain/host/platform, producer identities when available,
request digest, creation time and every output's size/hash. The configuration hashes the tracked
Rust pin, packager/validator, and relevant Containerfile or Web Trunk/Hosting policy.

| Component | Exact output |
| --- | --- |
| API | `cantos-api.tar.zst`, `image.json`, `runtime.tar`; matching committed VPS assets, edge policy, legacy schema manifest and `REVISION`, plus Linux amd64 image ID/source/schema epoch |
| Web | One component-specific `.tar.gz`; regular allow-listed static assets, including `index.html`; the existing Hosting bundle validator additionally checks app identity, references and integrity pins |
| Android | Existing unsigned APK/AAB and isolation/dependency reports, application ID `com.cantos.theatre`, production versionCode/versionName; no device/login acceptance |

Limits are 10,000 archive entries, 2 GiB aggregate expanded/payload bytes, 2 MiB JSON and 24 hours
from request/artifact creation to promotion. Absolute/traversal paths, symlinks, hardlinks,
devices, duplicate entries, file/directory collisions and unexpected outputs are refused.
Image layers remain OCI image contents, never extracted onto the host filesystem; their bytes
are checked against the image config's diff IDs before transfer.

`persist_to_workspace`/`attach_workspace` transfer this one workflow's request, inventory and
artifacts. `store_artifacts` supplies retention/inspection only. Each deploy job persists only
its own outcome directory in teardown, including failed/canceled attempts when supported by the
runner. Reruns inherit additive workspace layers: duplicate requests/inventories or an existing
component intent refuse promotion. Start a new pipeline after reconciliation, never pick a
newest directory or replay a write blindly.

## Approval, ordering and outcomes

Only one release workflow is selected for a request. Native CircleCI expression filters restrict
every production-context job to this repository and `develop` for both checkout and config
source, before checkout code can execute with secrets. This complements the separately required
account context restrictions; a check inside a compiled helper is too late for untrusted source.
All selected artifacts are built before
one `approve-production` gate. Selected API activation/health precedes Theatre Web, then Studio
Web. Web-only requests have no VPS/Android job and use a read-only API readiness probe.
Per-component serial groups retain CircleCI pipeline-order protection. Additionally, every
promotion fetches and **requires the resolved release SHA still equal current `origin/develop`**;
if the branch moved, inspect the old artifacts but prepare/approve a fresh pipeline for live
publication. Historical merged targets are buildable, not promotable through this strict live
lane. Prerequisite component receipts must confirm the same request, digest, activation and
smoke before downstream writes; a serial-group skip cannot impersonate a healthy API.
This is ordering/freshness protection, not a distributed transaction or a deployment lease.

Deploy jobs use `cargo xtask release checkout --transfer PATH` to verify actual config SHA,
same pipeline/workflow, expiry and develop ancestry before checking out the single release SHA.
Each deploy job first calls `cargo xtask release intent --transfer PATH --component COMPONENT`
and persists its component intent to the workspace in a successful step **before** promotion.
The intent binds the actual publisher job ID and unique job number. An executor loss cannot
erase this already-transferred intent; a rerun/new job refuses it, even without an outcome receipt.
They then call `cargo xtask release promote --transfer PATH --component COMPONENT`, which passes
exact manifest/digest/source/request inputs to the existing deploy owner. No Trunk/API image
rebuild occurs. No deploy binary is restored from `target/debug` or a dependency cache.

VPS promotion validates actual image config/layers, platform/schema and matching runtime;
checks the host's Linux x86_64 inventory; uploads a verified snapshot with transfer hashes; then
uses the artifact's own `cantos-vps promote REVISION TREE IMAGE_ID`. One host lock covers load,
ID/platform/revision/schema checks, matching runtime install and the existing deploy protocol:
backup → schema/migration/grants → image switch → health → switch-back when applicable.
See the [VPS runbook](vps-production.md#verified-artifact-promotion).
Image rollback never rolls back migrations. Bootstrap, identity, epoch cutover and restore
remain separate operator operations.

`cargo xtask release report --transfer PATH` consumes independent component receipts even when
dependencies fail or are not run (`terminal` DAG dependencies). It records requested SHA,
manifest digest, producer, remote identity, command result, activation and smoke independently.
Missing/lost receipts stay unknown; confirmed activation plus failed smoke stays a known remote
release. Studio failure after Theatre success is a partial release: retain successful components,
reconcile the failed site's real version, and choose recovery under the owning provider runbook.
No command retries all writes. Browser/identity/media acceptance always remains separate.

## Operator prerequisites and retention

- Qualify a GitHub App integration exposing `pipeline.config.*` metadata, project/repository
  mapping, reviewed config branch, actor approval
  authority, plan credits/concurrency, workspace/artifact access and retention before execution.
  Integrations without actual config-source metadata are unsupported by this lane; do not
  substitute caller parameters or label the checkout SHA as config evidence.
- Provision **separate**, project-restricted `production-vps` and `production-web` contexts only
  through authorized operator work. Enforce `develop` and allowed actor/security-group/expression
  restrictions actually supported by the account/integration. Missing restriction is a blocker;
  context names in YAML are not evidence those controls exist.
- VPS context: explicit `VPS_HOST`, `VPS_USER`, dedicated `VPS_SSH_PRIVATE_KEY`, and independently
  checked `VPS_KNOWN_HOSTS`. BatchMode and strict host-key verification remain mandatory.
- Web context: `FIREBASE_PROJECT`, `FIREBASE_THEATRE_SITE`, `FIREBASE_STUDIO_SITE` and a scoped
  `FIREBASE_SERVICE_ACCOUNT_JSON`. Credentials and ignored mapping are outside artifact storage;
  teardown removes keys. Never retain raw Firebase debug logs as shared artifacts.
- Rust follows 1.98.1, Trunk 0.21.14, checksum-pinned Node 22.23.3/Gradle 8.14.3, Android SDK/build
  tools 35/35.0.0 and JDK 17; missing SDK/license acceptance fails. Executor images remain rolling
  vendor images, so this is not a fully reproducible environment snapshot. No resource class was
  increased. Registry/git downloads and build-only tool caches contain no release or secrets;
  deploy jobs restore none of those caches.
- Workspace retention defaults to the account's setting (CircleCI documents 15 days absent an
  override); promotion expiry is independently 24 hours. Set and record the actual retention and
  visibility, costs, backup/recovery window and private receipt policy before the first run.
  Preserve VPS recovery journals/volumes and Hosting recovery versions under their existing rules.

`circleci config validate/process` checks source configuration, not account restrictions, credits,
executor execution, real artifacts or activation. Actual CircleCI runs, API/Web builds, public
smoke, browser acceptance and Cloudflare cutover need separately authorized live evidence.
The `.yml` file uses the JSON subset of YAML so deterministic DAG tests need only Python stdlib.

Official references: [configuration](https://circleci.com/docs/reference/configuration-reference/),
[workspaces](https://circleci.com/docs/guides/orchestrate/workspaces/),
[contexts](https://circleci.com/docs/guides/security/contexts/).
