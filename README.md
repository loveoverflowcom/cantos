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
and independent expiring/revocable producer read grants; see [local evidence](docs/evidence/production-brief.md). [Audio Lifecycle v1](packages/contracts/audio-lifecycle-v1.md) adds Content/Brief-bound private submissions, real Opus-byte inspection, immutable renditions, named review and private candidates. Public publication/delivery remains unimplemented.
Studio Web is implemented against the earlier editorial flow; its catalog/audio handoff UX is
still planned. Theatre Web is a session-only
[UI mock](docs/evidence/theatre-web-mock.md) without API or audio, and Theatre Mobile has an isolated
[CMP mock](apps/cantos-theatre/mobile/README.md) without native audio or API.

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

Production Brief export/read grants and private Audio Lifecycle are implemented for the development host. Public releases remain proposals.
No live routes should be inferred from [the proposed contract](packages/contracts/external-audio-handoff-v1.md).

## Repository

- `apps/cantos-studio/web`: creator Web UI; Studio Mobile is out of scope.
- `apps/cantos-theatre/web`: session-only listener Web mock; `apps/cantos-theatre/mobile`: isolated CMP mock; native listening planned.
- `services/cantos-api`: metadata/rights/revision/production-planning APIs, Studio catalog API and pure media inspection.
- `services/kanidm`: proposed identity configuration, not yet integrated.
- `packages/contracts`: versioned Script IR and existing wire contracts; future external-audio handoff.
- `.agents/skills`: portable external-producer and editorial instruction skills.
- `docs/product`, `docs/architecture`, `docs/decisions`: current product rules, system shape and ADRs.
- `xtask`: canonical repository verification and local development commands.

## Development

The default branch is `develop`; create feature branches and PRs against it.
Use Rust 1.87, a local PostgreSQL 17 cluster for database tests, and Trunk for Leptos Web.

```sh
cargo xtask doctor
cargo xtask verify repository
cargo xtask verify rust
cargo xtask verify contracts
cargo xtask verify postgres
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
