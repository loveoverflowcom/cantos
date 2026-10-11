# Cantos — Product Brief

> Status: target product design. The [Script IR contract](../../packages/contracts/README.md) is
> executable and locally tested. Local revision persistence/auth and a minimal Studio
> save/read consumer have [bounded evidence](../evidence/script-revision-persistence.md).
> [Casting/input authorization](../evidence/casting-production-inputs.md) adds reference-only
> settings, rights/budget declarations and immutable candidates with exact approval. The complete
> Studio journey, Theatre, audio production and native listening remain planned.

## Identity and purpose

- **Brand:** Cantos — Vọng Đài, “Theatre of Voices”.
- **Repository:** `cantos`.
- **Cantos Studio:** a workspace for creators to turn drafts into reviewed, published audio drama.
- **Cantos Theatre:** a discovery and listening experience for audiences.

Cantos produces performed drama: distinct characters, expressive dialogue, narration, ambience, music and effects. A manuscript is an input to adaptation, not an instruction to read every paragraph aloud unchanged.

## Product structure

`Work → Adaptation → Episode → Act → Scene → Dialogue → Audio`

A work can have multiple adaptations. Each adaptation defines its language, script interpretation and character cast. Episodes contain acts and scenes; dialogue includes spoken lines and narration. Sound cues are separate scene events that reference audio assets rather than pretending to be spoken dialogue.

The hierarchy describes editorial content. Script revisions, production runs and published releases separately record how a particular version became playable audio. A release pins its script revision, cast, audio manifest and approval records.

## Core workflows

1. **Prepare:** import TXT, Markdown, DOCX or a structured script; establish source rights; review AI-assisted adaptation and scene segmentation.
2. **Cast:** assign consistent voices, pronunciation and performance guidance to characters; preview representative lines.
3. **Produce:** generate dialogue audio, reuse eligible cached results, mix scenes and episodes, normalize loudness and run quality checks.
4. **Publish:** approve a specific revision and render, upload verified assets, and make a complete release available.
5. **Listen:** discover episodes, stream published files, bookmark moments and synchronize listening progress; download eligible episodes for offline use.

Changes create new revisions. Creators can correct individual lines, regenerate affected output and publish a replacement release without overwriting the previous release's audit trail.

## Technical direction

| Surface | Planned technology and responsibility |
| --- | --- |
| Backend | Rust + Axum; editorial, production and listener APIs |
| Metadata | PostgreSQL; revisions, job state, rights, costs and publication records |
| Web | Leptos; Studio and Theatre interfaces |
| Mobile | Kotlin Compose Multiplatform; Android/iOS listener UI with native playback |
| Audio storage | S3-compatible object storage and CDN delivery |
| Production | Background workers, pluggable TTS providers and audio processing |

Start with a modular monolith and separately runnable background worker where useful. Avoid introducing independent microservices until operational evidence justifies them. Narrative Forge concepts may inform a versioned Script IR; Cantos must not require its database, deployment or internal packages to operate. Shared schemas should define Rust/Kotlin contracts where practical rather than assume the runtimes can share executable business code.

## Design direction

Use Material 3 Expressive principles: a soft lavender/purple palette, accessible contrast, expressive rounded shapes, pill controls, compact adaptive layouts and purposeful motion. Maintain shared semantic tokens and component behavior across Web and CMP.

- **Studio:** text-first script editing, character casting, voice controls, scene timeline and audio preview.
- **Theatre:** podcast-style discovery, episode library, persistent mini-player and clear progress.
- **Mobile:** responsive touch targets, screen-reader support, native background playback and lock-screen controls.

An attached UI image is referenced in the originating brief but has not been inspected for this scaffold. These written requirements are the current design baseline; visual matching requires inspecting the reference first. Avoid oversized banners, excessive borders, shadows and unnecessary decoration.

## MVP priorities and acceptance

The MVP spans a complete production-to-publication path and a usable listening experience, rather than disconnected demonstrations.

- Import a supported draft, review the resulting structured script and cast its characters.
- Generate a multi-character episode, recover an interrupted production run and regenerate a changed line without regenerating unrelated dialogue.
- Review actual quality findings, source/voice/asset permissions and cost records before approving publication.
- Publish a complete release whose assets and metadata can be retrieved by Theatre Web.
- Play the release on native Android/iOS, including background playback, lock-screen controls, speed, sleep timer and offline downloads.
- Persist bookmarks and listening progress with a documented synchronization policy.
- Serve listeners from published cached assets; listening must not trigger TTS or adaptation jobs.

MVP delivery can proceed in slices, but the product is not complete until the Studio, Theatre Web and native mobile path work together. Billing, creator marketplaces, social features, real-time collaboration and recommendation personalization are later scope.

## Decisions still required

Select initial TTS providers, supported Script IR schema, identity/role model, audio quality profile, deployment/CDN arrangement and offline-download entitlement policy. Set measurable QC thresholds and production budgets before enabling automated publication. None of these choices is implied to be finalized by this brief.

Related: [business rules](business-rules.md) and [production pipeline](production-pipeline.md).
