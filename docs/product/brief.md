# Cantos — Product Brief

> Status: target product design. The [Script IR contract](../../packages/contracts/README.md) is
> executable and locally tested. Local revision persistence/auth and a minimal Studio
> save/read consumer have [bounded evidence](../evidence/script-revision-persistence.md).
> [Casting/input authorization](../evidence/casting-production-inputs.md) adds reference-only
> settings, rights/budget declarations and immutable candidates with exact approval. Cantos does
> not synthesize audio: external AI producers read its guidance and submit audio
> ([ADR 0018](../decisions/0018-external-audio-production.md)). The complete Studio journey, the
> submission API, Theatre and native listening remain planned.

## Identity and purpose

- **Brand:** Cantos — Vọng Đài, “Theatre of Voices”.
- **Repository:** `cantos`.
- **Cantos Studio:** a workspace for creators to manage works (novels, stories, podcasts), their episodes and their audio, from draft to reviewed, published audio drama.
- **Cantos Theatre:** a discovery and listening experience for audiences.

Cantos presents performed drama, made by external AI producers that follow its instructions: distinct characters, expressive dialogue, narration, ambience, music and effects. A manuscript is an input to adaptation, not an instruction to read every paragraph aloud unchanged.

## Product structure

`Work → Adaptation → Episode → Act → Scene → Dialogue → Audio`

A work can have multiple adaptations. Each adaptation defines its language, script interpretation and character cast. Episodes contain acts and scenes; dialogue includes spoken lines and narration. Sound cues are separate scene events that reference audio assets rather than pretending to be spoken dialogue.

The hierarchy describes editorial content. Script revisions, production runs and published releases separately record how a particular version became playable audio. A release pins its script revision, cast, audio manifest and approval records.

## Core workflows

1. **Prepare:** import TXT, Markdown, DOCX or a structured script; establish source rights; review AI-assisted adaptation and scene segmentation.
2. **Cast:** assign consistent voices, pronunciation and performance guidance to characters; preview representative lines.
3. **Produce:** an external AI producer reads Cantos's instructions and standards, makes the audio and submits it through the API; Cantos inspects it and runs quality checks.
4. **Publish:** approve a specific revision and render, upload verified assets, and make a complete release available.
5. **Listen:** discover episodes, stream published files, bookmark moments and synchronize listening progress; download eligible episodes for offline use.

Changes create new revisions. Creators can correct individual lines, ask a producer for corrected audio and publish a replacement release without overwriting the previous release's audit trail.

## Technical direction

| Surface | Planned technology and responsibility |
| --- | --- |
| Backend | Rust + Axum; editorial, production and listener APIs |
| Metadata | PostgreSQL; revisions, rights, submissions and publication records |
| Web | Leptos; Studio and Theatre interfaces |
| Mobile | Kotlin Compose Multiplatform; Android/iOS listener UI with native playback |
| Audio storage | S3-compatible object storage and CDN delivery |
| Production | External AI producers that read Cantos guidance and submit audio through the API; Cantos inspects and records it |

Start with a modular monolith. Avoid introducing independent microservices until operational evidence justifies them. Narrative Forge concepts may inform a versioned Script IR; Cantos must not require its database, deployment or internal packages to operate. Shared schemas should define Rust/Kotlin contracts where practical rather than assume the runtimes can share executable business code.

## Design direction

Use Material 3 Expressive principles: a soft lavender/purple palette, accessible contrast, expressive rounded shapes, pill controls, compact adaptive layouts and purposeful motion. Maintain shared semantic tokens and component behavior across Web and CMP.

- **Studio:** text-first script editing, character casting, voice controls, scene timeline and audio preview.
- **Theatre:** podcast-style discovery, episode library, persistent mini-player and clear progress.
- **Mobile:** responsive touch targets, screen-reader support, native background playback and lock-screen controls.

An attached UI image is referenced in the originating brief but has not been inspected for this scaffold. These written requirements are the current design baseline; visual matching requires inspecting the reference first. Avoid oversized banners, excessive borders, shadows and unnecessary decoration.

## MVP priorities and acceptance

The MVP spans a complete production-to-publication path and a usable listening experience, rather than disconnected demonstrations.

- Import a supported draft, review the resulting structured script and cast its characters.
- Receive a multi-character episode from an external producer, survive an interrupted or repeated upload and replace a changed line without redoing unrelated dialogue.
- Review actual quality findings and source/voice/asset permissions before approving publication.
- Publish a complete release whose assets and metadata can be retrieved by Theatre Web.
- Play the release on native Android/iOS, including background playback, lock-screen controls, speed, sleep timer and offline downloads.
- Persist bookmarks and listening progress with a documented synchronization policy.
- Serve listeners from published cached assets; listening must not trigger TTS or adaptation jobs, and Cantos itself runs no TTS.

MVP delivery can proceed in slices, but the product is not complete until the Studio, Theatre Web and native mobile path work together. Billing, creator marketplaces, social features, real-time collaboration and recommendation personalization are later scope.

## Decisions still required

Select the producer credential and submission model, supported Script IR schema, identity/role model, audio quality profile, deployment/CDN arrangement and offline-download entitlement policy. Set measurable QC thresholds before enabling automated publication. None of these choices is implied to be finalized by this brief.

Related: [business rules](business-rules.md) and [production pipeline](production-pipeline.md).
