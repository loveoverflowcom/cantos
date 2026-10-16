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

## Product surfaces

| Component | Responsibility | Platform |
| --- | --- | --- |
| Cantos Studio | Manage work/series, chapter/episode, script (optional), audio, briefs, submissions, approvals and publication | Leptos Web only |
| Cantos API | Authorized catalog, immutable content/brief revisions, signed external-audio intake, metadata, checks and release authority | Rust + Axum + PostgreSQL |
| External AI agents / plugins | Fetch instructions, perform TTS/recording/editing/mixing/encoding, run production-side QC, upload deliverables | Outside Cantos; independently operated |
| Cantos Theatre | Discover and listen to immutable published releases; resume, native background listening and offline use | Leptos Web and Compose Multiplatform mobile |

**Status, as of this pivot:** The existing code implements Script IR, revision persistence,
manuscript intake, host-created adaptation proposal submission/review, reference casting/rights
planning and a read-only WAV inspector. Studio Catalog implements work/content-unit domain rules,
authorized transactional application use cases, PostgreSQL revision persistence and JSON HTTP
routes with shared contracts; see the [Catalog API](packages/contracts/catalog-v1.md).
[Content Identity](packages/contracts/content-v1.md) adds immutable text/source/script/recording
revisions with separate metadata/content sequences and full revision pinning.
[Production Brief v1](packages/contracts/production-brief-v1.md) adds immutable instructions
and independent expiring/revocable producer read grants; see [local evidence](docs/evidence/production-brief.md). [Audio Lifecycle v1](packages/contracts/audio-lifecycle-v1.md) adds Content/Brief-bound private submissions, real Opus-byte inspection, immutable renditions, named review and private candidates. Publication, Discovery and Library have backend cores; public media delivery remains unavailable.
Studio Web retains the editorial flow and adds a real Catalog/text-or-recording Content/Brief/
private-audio review workspace. The [client boundary](docs/work-plan/live-client-api-matrix.md)
distinguishes connected forms from deferred controls and user acceptance. Theatre Web opens
public Home through the implemented anonymous discovery API; an API outage remains unavailable,
and the isolated [UI mock](docs/evidence/theatre-web-mock.md) is selected explicitly by
`?mode=mock`. Theatre Mobile retains its isolated
[CMP mock](apps/cantos-theatre/mobile/README.md), with production-safe UI/domain/application
code in `:shared` and explicit demo adapters/composition in the separate `:preview-sandbox`
KMP module, plus a tested common/JVM listener transport seam;
native audio and live client integration remain deferred; public media delivery is fail-closed. The bounded [native launch slice](docs/evidence/theatre-cmp-launch.md) adds Android/iOS launch resources and an optional app-owned CMP presentation; current synchronous owners skip the optional frame. Native and identity acceptance remain separately scoped.
A production Android host composes `:shared` only and, until production listening adapters
exist, states that listening is unavailable. `cargo xtask theatre-host android-release` inspects
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
- `apps/cantos-forge`: proposed producer-side CLI ([ADR 0022](docs/decisions/0022-cantos-forge-producer-cli.md)); today a host-assisted local workflow (workspace, artifact registration, deterministic checks, local handoff bundle). It runs outside the API, does not generate audio and does not submit anything.
- `apps/cantos-theatre/web`: anonymous live discovery, separate listener account boundary and explicit session-only previews; `apps/cantos-theatre/mobile`: production-safe `shared` plus isolated `preview-sandbox` CMP demo; native listening planned.
- `services/cantos-api`: metadata/rights/revision/production-planning APIs, Studio catalog API and pure media inspection.
- `services/manifest.toml`: typed process inventory; [services](services/README.md) defines Podman-first ownership.
- `services/postgresql`: explicit local rootless Podman configuration; native database tests remain separate.
- `services/kanidm`: operator-owned identity configuration; Studio OIDC is implemented in the API, live acceptance pending.
- `services/firebase-hosting`: tracked Firebase Hosting policy for the explicit, manual `cargo xtask deploy firebase` of one static Web app ([runbook](docs/deployment/firebase-hosting.md)); live deployment acceptance pending, the API is not served.
- `packages/contracts`: versioned Script IR, Catalog/Content/Brief, private Audio Lifecycle and Publication/Discovery/Library contracts; public delivery acceptance remains deferred.
- `.agents/skills`: portable external-producer and editorial instruction skills.
- `docs/product`, `docs/architecture`, `docs/decisions`: current product rules, system shape and ADRs.
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

Use the exact Rust 1.98.1 pin in [`rust-toolchain.toml`](rust-toolchain.toml), a local PostgreSQL 17
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
cargo xtask build cantos-forge
cargo xtask web-performance cantos-theatre
cargo xtask preflight --changed-only
```

These commands require a local checkout with the documented toolchain. CI status and test
evidence must be reported accurately; removing an obsolete TTS worker does not prove new
upload/playback features exist.

For local Theatre Web with an operator-supplied, migrated app-role `DATABASE_URL` already
exported, run `cargo xtask run cantos-theatre --web`. It starts the API on 8080 with the
Theatre origin 8082, then Trunk on 8082, and checks anonymous discovery directly and through
the proxy. `--test-cluster` instead uses only the disposable fixture retained by
`cargo xtask verify postgres --keep`; launch never provisions infrastructure. To reuse an
API you already own, use `cargo xtask app cantos-theatre --web`. That command reports
frontend-only readiness and warns if live discovery is unavailable; explicit
`?mode=mock`, `?mode=membership-preview` and `?mode=auth-preview` still work offline.
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
authorization remains fail-closed with unavailable public delivery. PostgreSQL/runtime and
Web/CMP live adapter acceptance are still required. Theatre Web consumes the bounded anonymous
Work list; its explicit mock and real playback boundary remain separate.
[Evidence and boundaries](docs/evidence/theatre-discovery.md).

The opt-in [Admin authorization and audit](docs/architecture/admin-authorization.md) backend
adds independent capability inspection and local Admin-access revocation with atomic append-only
audit. The exact issuer/group/introspection contract and live Kanidm acceptance remain gated;
this grants no Studio/Theatre resource right, Admin UI or production activation.
