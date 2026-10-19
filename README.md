# Cantos — Vọng Đài

**Cantos is a content catalog, external-audio handoff and listening platform.** Studio manages
novels, stories, podcasts, their chapters/episodes, audio files, rights, review and publication.
Cantos does **not** run TTS, LLM inference, audio generation, mixing or encoding in its backend.
External AI hosts/plugins read versioned production instructions, create audio with their own
tools, and send verified assets back through Cantos APIs. Theatre streams published stored media;
playback never starts production.

The current direction is governed by [ADR 0018](docs/decisions/0018-external-audio-production.md)
and the [external audio handoff proposal](packages/contracts/external-audio-handoff-v1.md).
Older TTS experiments and job migrations are historical evidence, **not current runtime scope**.
The [current schema and cutover policy](docs/deployment/postgres-schema-cutover.md) makes fresh
initialization explicit and ordinary deployment refuses legacy databases. An explicit narrow
`--cutover-epoch2` lane checks the known 28-migration empty-business source, rehearses a sibling
restore/import and verifies off-host recovery before switching. A general populated-data
transformer remains unimplemented; PostgreSQL stays at 17.11 and source cleanup never resets data.

## Product surfaces

| Component | Responsibility | Platform |
| --- | --- | --- |
| Cantos Studio | Manage work/series, chapter/episode, script (optional), audio, briefs, submissions, approvals and publication | Leptos Web only |
| Cantos API | Authorized catalog, immutable content/brief revisions, signed external-audio intake, metadata, checks and release authority | Rust + Axum + PostgreSQL |
| Cantos Forge | Local artifact/register/validate/export, attempt/recovery core and local durable attempt adapter, finite Rust worker and Node process boundary; planned Studio-controlled execution with private Drive app-data drafts | Rust producer outside the API runtime; HTTP fails closed, Functions/Drive/provider execution not implemented |
| External AI agents / plugins | Fetch instructions, perform TTS/recording/editing/mixing/encoding, run production-side QC, upload deliverables | Outside Cantos API; independently operated |
| Cantos Theatre | Discover and listen to immutable published releases; resume, native background listening and offline use | Leptos Web and Compose Multiplatform mobile |

**Status, as of this pivot:** The existing code implements Script IR, revision persistence,
manuscript intake, host-created adaptation proposal submission/review, reference casting/rights
planning and a read-only WAV inspector. Studio Catalog implements work/content-unit domain rules,
authorized transactional application use cases, PostgreSQL revision persistence and JSON HTTP
routes with shared contracts; see the [Catalog API](packages/contracts/catalog-v1.md).
[Content Identity](packages/contracts/content-v1.md) adds immutable text/source/script/recording
revisions with separate metadata/content sequences and full revision pinning.
[Production Brief v1](packages/contracts/production-brief-v1.md) adds immutable instructions
and independent expiring/revocable producer read grants; see [local evidence](docs/evidence/production-brief.md). [Audio Lifecycle v1](packages/contracts/audio-lifecycle-v1.md) adds Content/Brief-bound private submissions, real Opus-byte inspection, immutable renditions, named review and private candidates. Publication, Discovery and Library have backend cores; opt-in origin delivery promotes approved bytes to separate publication storage and serves release-bound grants. Unconfigured delivery stays unavailable.
Studio Web retains the editorial flow and adds a real Catalog/text-or-recording Content/Brief/
private-audio review workspace. The [client boundary](docs/work-plan/live-client-api-matrix.md)
distinguishes connected forms from deferred controls and user acceptance. Theatre Web opens
public discovery through the implemented anonymous API; an API outage remains unavailable.
The isolated [UI mock](docs/evidence/theatre-web-mock.md) requires the opt-in `preview` Cargo
feature and is selected explicitly by `?mode=mock`; production builds contain no preview runtime.
Theatre Mobile retains its isolated
[CMP mock](apps/cantos-theatre/mobile/README.md), with production-safe UI/domain/application
code in `:shared` and explicit demo adapters/composition in the separate `:preview-sandbox`
KMP module, plus a tested common/JVM listener transport seam;
production catalog hosts use the real API on Android and iOS; native audio remains deferred. Public media delivery is opt-in and fail-closed when unconfigured. The bounded [native launch slice](docs/evidence/theatre-cmp-launch.md) adds Android/iOS launch resources and an optional app-owned CMP presentation; current synchronous owners skip the optional frame. Native and identity acceptance remain separately scoped.
The [native identity configuration slice](docs/evidence/theatre-auth-activation.md) supplies
nonsecret Android metadata/iOS plist build inputs and typed missing/invalid configuration
diagnostics. Source regression evidence does not establish a configured artifact or live login.
Production Android and iOS catalog hosts compose `:shared` only, without a playback seed. Android
release accepts an explicit HTTPS API origin; listening, account and Library remain unavailable.
`cargo xtask theatre-host android-release --api-origin https://api.cantoss.duckdns.org` inspects
its release classpaths and unsigned APK/AAB bytes for sandbox, mock and test content; the demo
APK establishes no production isolation.
See the [sandbox isolation record](docs/evidence/issue122-cmp-sandbox-isolation.md) for current checks.

[Private object storage](docs/architecture/object-storage.md) implements filesystem and generic
S3-compatible adapters, durable original targets and [exact owner-authorized audio reads](packages/contracts/private-content-reads-v1.md).
Changing the write target preserves historical location resolution and moves no bytes.
Canonical text, imports and Script IR stay in their existing PostgreSQL stores. Private previews
and scoped grants supply no listener/publication access; real-provider and deployment acceptance
remain separate in the [evidence record](docs/evidence/provider-independent-storage.md).

Studio's [private Work-cover control](apps/cantos-studio/web/README.md#private-work-artwork)
uses that owner-only artwork boundary for original PNG/JPEG upload, admission, selection and
explicit unlink. Its [UI evidence](docs/evidence/studio-private-artwork-ui.md) keeps browser
fixtures separate from outstanding API/storage acceptance and public delivery.

A voice model or provider named by a Studio planning fixture is only descriptive metadata.
No source inside `services/cantos-api` may invoke speech engines or cloud TTS providers.

## Content flow

```text
Studio: work (novel/story/podcast) → chapter/episode → pinned instructions/rights
                 ↓ read-only production brief
External AI host/plugin: synthesize or record → mix/encode → report QC/provenance
                 ↓ authenticated, scoped upload + finalize
Studio: verify object and metadata → human/editorial approval → immutable release
                 ↓
Theatre: discover → stream/download → resume
```

Production Brief export/read grants, private Audio Lifecycle and the Publication core are
implemented for the development host. Unconfigured public delivery cannot activate a release.
No live routes should be inferred from [the proposed contract](packages/contracts/external-audio-handoff-v1.md).

## Repository

The [internal actor registry](docs/architecture/identity-registry.md) adds exact external identity
bindings and explicit operator operations, retaining legacy actors/ownership. The opt-in
[Studio OIDC BFF](docs/architecture/studio-oidc.md) now verifies credentials and issues Studio-only
sessions; live-provider/production and separate Theatre/Admin identity remain gated.
See [registry evidence](docs/evidence/identity-registry.md) and [OIDC evidence](docs/evidence/studio-oidc.md).

- `apps/cantos-studio/web`: creator Web UI; Studio Mobile is out of scope.
- `services/cantos-forge-functions`: producer-side library/CLI ([ADR 0022](docs/decisions/0022-cantos-forge-producer-cli.md)); today a host-assisted local artifact/check/export workflow, attempt/recovery core, local attempt persistence and finite Rust/Node worker boundary. The owner-selected Studio-controlled local/Functions and private Drive app-data execution remains #203 implementation work; this build makes no provider call or API submission.
- `apps/cantos-theatre/web`: anonymous live discovery, separate listener account boundary and explicit session-only previews; `apps/cantos-theatre/mobile`: production-safe `shared` plus isolated `preview-sandbox` CMP demo; native listening planned.
- `services/cantos-api`: authoritative modular metadata/rights/revision/production-planning backend, Studio catalog API and pure media inspection; this does not forbid separately owned processes.
- `services/manifest.toml`: typed process inventory; [services](services/README.md) defines Podman-first ownership.
- `services/postgresql`: explicit local rootless Podman configuration; native database tests remain separate.
- `services/kanidm`: operator-owned identity configuration; Studio OIDC is implemented in the API, live acceptance pending.
- `infra/firebase/hosting`: tracked Firebase Hosting policy for the explicit, manual `cargo xtask deploy firebase` of one static Web app ([runbook](docs/deployment/firebase-hosting.md)); static delivery and API-origin/session acceptance remain separate; [ADR 0041](docs/decisions/0041-direct-hosting-api-boundary.md) selects direct Hosting. The [current production runbook](docs/deployment/vps-production.md) owns operations; Hosting serves no API.
- `infra/edge`: operator-owned public edge and DNS updater assets, separate from application code and the `xtask` command surface.
- `infra/`: [deployment/runtime assets](infra/README.md), not application domain logic; the Forge implementation lives separately at `services/cantos-forge-functions`. Moving source does not establish Functions hosting or live-provider acceptance.
- `packages/contracts`: versioned Script IR, Catalog/Content/Brief, private Audio Lifecycle and Publication/Discovery/Library contracts; public delivery acceptance remains deferred.
- `.agents/skills`: portable external-producer and editorial instruction skills.
- `docs/`: [documentation map](docs/README.md), current owners and explicit ADR lifecycle; [active queue](docs/work-plan/README.md), source-bound evidence and historical archive have separate roles.
- `xtask`: canonical repository verification and local development commands.

The [Quint behavior foundation](specs/behavior/README.md) adds an opt-in executable private
admission/replay model with named scenarios, witnesses and negative controls. Model execution
is separate from Rust/SQL/API/client conformance and leaves Protobuf-first transport intact.

## Development

The default branch is `develop`; create feature branches and PRs against it.

Open the repository root in VS Code to apply [workspace settings](.vscode/settings.json).
Generated Rust/Gradle/Web outputs, dependency caches, Python bytecode, local Xcode
DerivedData, Android Studio/Xcode user metadata, temporary files and agent-run logs are hidden in Explorer and excluded from
search and recursive file watching. Search also respects `.gitignore`. Source files,
contracts, skills, lockfiles, configuration and other evidence remain visible. To inspect
an excluded path, open it directly or temporarily set its exclude entry to `false` in
workspace settings; in Search, turn off **Use Exclude Settings and Ignore Files**.
These editor settings neither delete files nor change Git tracking or build behavior.

The same CMP-specific excludes apply when opening the [Theatre Mobile Gradle root](apps/cantos-theatre/mobile/.vscode/settings.json),
[UI-system CMP Gradle root](packages/ui-system/cmp/.vscode/settings.json) or its
[isolated Android preview root](packages/ui-system/cmp/android-preview/.vscode/settings.json)
directly in VS Code. Root settings do not automatically apply to separately opened subfolders.
Gradle wrapper files, Kotlin/Swift sources, Compose resources, Xcode projects/shared schemes
and editable `local.properties` remain visible. Generated vectors, Compose captures, APKs
and Kotlin/Native frameworks already live under excluded module `build` directories.

Use the exact Rust 1.98.1 pin in [`rust-toolchain.toml`](rust-toolchain.toml), a local PostgreSQL 17 or 18
cluster for database tests, and Trunk for Leptos Web. `rustup show active-toolchain` installs the
minimal compiler, matching rustfmt/Clippy and `wasm32-unknown-unknown` target when needed.
All seven package MSRVs are 1.98.1; edition 2021 and locked dependency versions stay unchanged.
Kani keeps its separately bundled nightly compiler. See the [upgrade review boundary](docs/architecture/rust-toolchain.md).

```sh
cargo xtask doctor
cargo xtask verify repository
cargo xtask verify rust
cargo xtask verify contracts
cargo xtask verify postgres
cargo xtask verify postgres --suite schema # current baseline/refusal and SQL acceptance
cargo xtask verify theatre-auth --suite firebase-emulator --dry-run
cargo xtask build cantos-forge
cargo xtask web-performance cantos-theatre
cargo xtask preflight --changed-only
```

These commands require a local checkout with the documented toolchain. CI status and test
evidence must be reported accurately; removing an obsolete TTS worker does not prove new
upload/playback features exist.

The opt-in [local enrollment test lane](docs/deployment/firebase-enrollment-proof.md#isolated-local-enrollment-testing)
requires separately prepared tools and sends no real email. Its emulator/PostgreSQL boundary
uses a mock Kanidm issuer; the full local Web/Kanidm/OIDC journey remains a separate gate.

For local Theatre Web with an operator-supplied, migrated app-role `DATABASE_URL` already
exported, run `cargo xtask run cantos-theatre --web`. It starts the API on 8080 with the
Theatre origin 8082, then Trunk on 8082, and checks anonymous discovery directly and through
the proxy. `--test-cluster` instead uses only the disposable fixture retained by
`cargo xtask verify postgres --keep`; launch never provisions infrastructure. For synthetic
published Works in that cluster, run `cargo xtask seed theatre` first
([runbook](docs/deployment/theatre-seeded-catalog.md)). To reuse an
API you already own, use `cargo xtask app cantos-theatre --web`. That command reports
frontend-only readiness and warns if live discovery is unavailable. For offline previews, run
`TRUNK_BUILD_FEATURES=preview cargo xtask app cantos-theatre --web`, then open
`?mode=mock`, `?mode=membership-preview` or `?mode=auth-preview`.
See the [local runbook](apps/cantos-theatre/web/README.md#build-run-and-checks) and
[compatibility proposal and evidence](docs/evidence/theatre-local-run.md). This changes the
previous frontend-only Theatre `run` behavior; option A remains **proposed pending review**,
and startup success does not accept live playback or provider integration.

The optional [Web performance inventory](docs/evidence/web-performance-patterns.md) rebuilds a
release bundle and records asset hashes and offline compression sizes. It establishes no browser
startup, cache-hit or production delivery result; those require separately attributed observations.

Start with the [product brief](docs/product/brief.md), [business rules](docs/product/business-rules.md),
[external handoff pipeline](docs/product/production-pipeline.md), [API contexts](docs/architecture/api-contexts.md),
[work queue](docs/work-plan/README.md) and [contributor guide](CONTRIBUTING.md).

For local design/render PNG review, use the optional
[`cargo xtask visual-compare`](tools/visual-compare/README.md). It exports metrics, numbered
regions and review images offline; baseline approval, opened-image inspection and runtime
acceptance remain separate. No application/release dependency or automatic UX/CI gate is added.

## Rights and delivery

Source, translation, voice, music and effects permissions are scoped claims that must be
reviewed. External uploads remain private and are **never** implicitly published. Store immutable
release manifests and deliver approved audio over object storage/CDN; explicit Opus mono20/24/32/40-v1 Brief profiles and further playback/quality
acceptance are tracked in [#64](https://github.com/loveoverflowcom/cantos/issues/64).

Cantos has no selected repository software license. Repository visibility does not grant a right
to redistribute narrative or voice material.

## Public listener backend core

[Theatre Discovery/Playback v1](packages/contracts/theatre-discovery-v1.md) adds anonymous
current-public works/search/detail, ordered units and immutable manifest projections. Media
authorization remains fail-closed unless explicit origin delivery is configured. The
[step-1 handoff](docs/evidence/theatre-live-catalog-step1.md) distinguishes local catalog/storage
evidence from deployed and native playback acceptance. Theatre Web consumes the anonymous Work
list, Work detail, units, manifest and playback authorization, and plays a chosen public unit
through one audio element ([#269 evidence](docs/evidence/theatre-web-guest-playback.md)); its
explicit mock stays a separate simulation and deployed/CDN/native playback remain unaccepted.
[Evidence and boundaries](docs/evidence/theatre-discovery.md).

The opt-in [Admin authorization and audit](docs/architecture/admin-authorization.md) backend
adds independent capability inspection and local Admin-access revocation with atomic append-only
audit. The exact issuer/group/introspection contract and live Kanidm acceptance remain gated;
this grants no Studio/Theatre resource right, Admin UI or production activation.

Firebase enrollment for Theatre is implemented as an explicitly selected, default-disabled
mode in the existing API: mailbox proof precedes Kanidm account setup; OIDC still owns login.
See [ADR0036](docs/decisions/0036-firebase-enrollment-proof.md), the
[operator acceptance runbook](docs/deployment/firebase-enrollment-proof.md) and the
[runtime evidence](docs/evidence/firebase-enrollment-runtime.md). Scoped local SQL checks use
`cargo xtask verify postgres --suite theatre-firebase`; they do not replace the full gate or
accept real email, credentials, origins or component qualification.

The [native step-2 ledger](docs/evidence/theatre-native-step2.md) owns current player/account/Library/SQLDelight implementation and exact per-platform acceptance gaps. It depends on unmerged #247; public catalog/media evidence from step1 never substitutes for installed native audio, identity or cross-client sync acceptance.
