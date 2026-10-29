# Opt-in CircleCI release artifacts and promotion

**Implemented source contract; account integration, real product builds and activation remain
separate acceptance.** Follow the [Cloudflare rollout/recovery runbook](cloudflare-rollout.md)
for #278/#280. This change retains the Firebase static publisher; it does not implement a
Cloudflare promotion adapter for the combined DAG, submit a pipeline, provision contexts, accept terms or purchase credits.
The separate opt-in [Pages lane](cloudflare-pages.md#circleci-opt-in) retains per-app approvals
and consumes the shared manifest-v2 producer with persisted exact-job Pages intents. It remains
independent of API activation and of the Firebase combined Web report. `web-provider` defaults to `firebase`; selecting
`cloudflare` routes Web jobs exclusively to that separate lane.

## Request and trust boundary

Run a reviewed configuration on `develop`, after the operator qualifies the account/project,
credits and permissions. Default pushes/PRs select no CircleCI jobs. GitHub checks are unchanged.

| Request | Parameters | Effect |
| --- | --- | --- |
| Build only | `build=true`, empty `target`, `build-component=theatre-web|studio-web|android` | One preparation job, no production context or publish |
| Release | `build=false`, nonempty `target`, `component=theatre-web|studio-web|web` | Prepare → one approval → selected deploy jobs → component report |
| Conflicting | `build=true` and nonempty `target` | Explicit failing job; never a successful no-op |

`api-origin` is a public exact HTTPS origin (default `https://api.cantos.cc`). Web build bytes
and configuration identity include it. Android retains its existing fixed production origin;
legacy `api` and `all` selections now fail in a credential-free refusal job. Their selectors
remain recognized for safe backward compatibility, but start no Web or VPS publication.
`release prepare --component api|all`, direct legacy API packaging/deployment primitives and
`deploy vps` also refuse before source work, network access or output writes.
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

Preparation runs `cargo xtask ci --job repository` once and Web once
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
| Native OCI pair | Separate opt-in [OCI producer](native-oci-release.md); not a CircleCI API archive or activation lane |
| Web | One component-specific `.tar.gz`; regular allow-listed static assets, including `index.html`; the existing Hosting bundle validator additionally checks app identity, references and integrity pins |
| Android | Unsigned optimized APK/AAB, isolated unsigned unshrunk release audit APK/AAB, both isolation/dependency/pre-shrink reports, bound `mapping.txt` and `release-evidence.json`, application ID `com.cantos.theatre`, production versionCode/versionName; no device/login acceptance |

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
Firebase's selected Web artifacts are built before one `approve-production` gate; Theatre then
Studio use independent receipts and a read-only API readiness probe. Pages keeps per-app approvals.
No workflow attaches the old VPS context or selects API activation. No API deployment is implied by
publishing compatible static Web bytes.
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

Native host activation is not implemented by this lane. The [historical VPS promotion
runbook](https://github.com/loveoverflowcom/cantos/blob/b2611c3db3896ce9733280a319008924ee8bf974/docs/deployment/vps-production.md#verified-artifact-promotion)
retains recovery semantics; it is not executable release guidance for this source.

Pages uses `release pages-intent` and `release pages-promote` with a per-app transfer directory.
The first validates source/freshness and the actual v2 archive, resolves nonsecret destination
inputs, refuses drift in an existing `infra/cloudflare/projects.json`, runs the deploy owner's
strict offline plan and records exact account/project/branch/environment plus request/manifest
identity and publisher job identity. CI persists this intent **before** invoking promotion.
Promotion rechecks source/freshness, consumes that pinned destination and invokes only the existing
`deploy cloudflare` owner once. Ambient destination inputs cannot silently retarget it. Rerun jobs
with a different identity or an existing attempt refuse before publishing. A missing/lost receipt
stays unknown; a known deployment plus failed smoke/command remains known. Sanitized per-app
outcomes are persisted to the workflow and existing provider receipts are stored in the publisher's
`run.teardown` after failure/cancellation when the executor supports it. Ordinary `store_artifacts`
steps do not accept `when`; see the [configuration reference](https://circleci.com/docs/reference/configuration-reference/#the-teardown-attribute).
An executor loss may prevent final artifact upload; reconcile the provider before a new pipeline.

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
- Provision project-restricted `production-web` (Firebase) and `cloudflare-web` contexts only
  through separately authorized operator work. Enforce reviewed `develop`/repository and allowed
  actor restrictions supported by the account. No current workflow receives `production-vps`;
  source retirement never deletes a real context or credential. Missing account restrictions block
  acceptance; names/filters in YAML are not evidence those controls exist.
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

New Web archive production follows the [runtime image policy](../design/runtime-images.md):
AVIF/SVG assets, with actual signature checks before packaging. The version-2 archive reader
accepts AVIF additively and retains historical legacy suffix compatibility for retained records;
that does not permit producing a new bundle with legacy raster. No publish/deploy is implied.
