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
distinguishes connected forms from deferred controls and user acceptance. Theatre Web defaults
to an unsupported live API boundary, with the isolated [UI mock](docs/evidence/theatre-web-mock.md)
selected explicitly by `?mode=mock`. Theatre Mobile retains its isolated
[CMP mock](apps/cantos-theatre/mobile/README.md), plus a tested common/JVM listener transport seam;
native audio and live client integration remain deferred; public media delivery is fail-closed.

[Private object storage](docs/architecture/object-storage.md) implements filesystem and generic
S3-compatible adapters, durable original targets and [exact owner-authorized audio reads](packages/contracts/private-content-reads-v1.md).
Changing the write target preserves historical location resolution and moves no bytes.
Canonical text, imports and Script IR stay in their existing PostgreSQL stores. Private previews
and scoped grants supply no listener/publication access; real-provider and deployment acceptance
remain separate in the [evidence record](docs/evidence/provider-independent-storage.md).

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
- `apps/cantos-theatre/web`: session-only listener Web mock; `apps/cantos-theatre/mobile`: isolated CMP mock; native listening planned.
- `services/cantos-api`: metadata/rights/revision/production-planning APIs, Studio catalog API and pure media inspection.
- `services/manifest.toml`: typed process inventory; [services](services/README.md) defines Podman-first ownership.
- `services/postgresql`: explicit local rootless Podman configuration; native database tests remain separate.
- `services/kanidm`: operator-owned identity configuration; Studio OIDC is implemented in the API, live acceptance pending.
- `packages/contracts`: versioned Script IR, Catalog/Content/Brief, private Audio Lifecycle and Publication/Discovery/Library contracts; public delivery acceptance remains deferred.
- `.agents/skills`: portable external-producer and editorial instruction skills.
- `docs/product`, `docs/architecture`, `docs/decisions`: current product rules, system shape and ADRs.
- `xtask`: canonical repository verification and local development commands.

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

Use Rust 1.87, a local PostgreSQL 17 cluster for database tests, and Trunk for Leptos Web.

```sh
cargo xtask doctor
cargo xtask verify repository
cargo xtask verify rust
cargo xtask verify contracts
cargo xtask verify postgres
cargo xtask build cantos-forge
cargo xtask preflight --changed-only
```

These commands require a local checkout with the documented toolchain. CI status and test
evidence must be reported accurately; removing an obsolete TTS worker does not prove new
upload/playback features exist.

Start with the [product brief](docs/product/brief.md), [business rules](docs/product/business-rules.md),
[external handoff pipeline](docs/product/production-pipeline.md), [API contexts](docs/architecture/api-contexts.md),
[work queue](docs/work-plan/README.md) and [contributor guide](CONTRIBUTING.md).

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
Web/CMP live adapter acceptance are still required; current mock clients are unchanged.
[Evidence and boundaries](docs/evidence/theatre-discovery.md).
