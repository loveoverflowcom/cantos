# Import and edit a versioned script

## Why

A creator needs a reliable manuscript-to-script flow before audio production can begin. Script identity, revisions, character references, and source permissions must survive adaptation and editing so later regeneration uses the intended text.

## Scope

Deliver a Studio flow that imports TXT, Markdown, DOCX, or a structured script; previews the source; proposes an AI-assisted adaptation; and lets the creator review, correct, and save a validated version. Support original scripts through the same editing flow.

Use the Work → Adaptation → Episode → Act → Scene → Dialogue → Audio hierarchy from the brief. Define the smallest versioned Script IR needed by this flow, including stable identity within a script, speakers, emotion hints, pronunciation references, sound cues, and provenance. Audio references can remain absent until generation. Document how future Narrative Forge interchange will use a versioned contract rather than shared internal storage or runtime code.

Persist source references, script versions, permission/publication eligibility metadata, and adaptation cost records in PostgreSQL. Establish API contracts with an immediate Studio consumer. Build a compact, text-first Leptos editor with scenes, speaker correction, validation errors, and explicit save/version behavior.

## Non-goals

TTS, audio mixing, public playback, a general document-conversion service, real-time collaborative editing, or a universal schema for every narrative project.

## Implemented foundation and remaining boundary

The [server](../../apps/server/README.md) implements authenticated Script IR save/read/history,
immutable acceptance, PostgreSQL persistence and conflict/retry handling. The
[evidence ledger](../evidence/script-revisions.md) records its exact checks. Source/provenance
and pending rights assertions are retained; they do not grant publication eligibility.
The next boundary is the minimal Leptos save/reopen consumer for #1. Importers, adaptation and
the complete editor remain subsequent work; this slice does not meet the full 010 review gate.

## Dependencies

The project brief and architecture guidance. An executable [Script IR contract](../../contracts/README.md)
now supplies structured-draft validation, exports and speech bytes. Inspect that module before
selecting migrations, API routes or editor libraries. Establish Axum/PostgreSQL and an authorized
save/retrieve consumer before expanding intake. Immutable revision persistence, actor checks,
stale-write/retry behavior and restart evidence remain in #1.

## Suggested sequence

1. Add representative manuscript and structured-script fixtures, including multiple speakers and scenes.
2. Implement import, source preview, and the minimum persistence/API boundaries for saved versions.
3. Add one real adaptation integration behind a narrow boundary and review its structured output before saving.
4. Implement editing, speaker resolution, and validation in Studio; demonstrate a saved version reopening correctly.

## Acceptance criteria

- All four accepted input formats produce an inspectable import result; malformed or unsupported input fails with a useful error and does not create a misleading ready script.
- The creator can add/edit scenes and dialogue, correct a speaker, review AI suggestions, and reopen the saved result after a backend restart.
- Saving an edit creates the intended version without altering prior saved versions or silently overwriting a competing edit.
- Unresolved speakers, invalid references, and incomplete required metadata are visible; they cannot silently pass into production.
- Adaptation failures, invalid AI output, and interrupted saves preserve the source and last valid script. Cost/provider records distinguish known usage from unavailable estimates.
- A missing publication permission is recorded and will block publication; it does not disappear during import or adaptation.
- The editor demonstrates keyboard access, accessible contrast, and usable narrow-screen layout using the shared design tokens.

## Review boundary

Verify one end-to-end import → adaptation review → edit → save → reopen journey, Script IR fixtures and version compatibility behavior, and PostgreSQL migration/restart behavior. Inspect preserved text and character references in addition to schema validity. Review only the contracts consumed by this slice.

## Risks / unknowns

DOCX extraction may lose meaningful structure; compare the imported result against the source. AI output may invent speakers, omit text, or confuse dialogue with narration; creator review and source provenance are required. Confirm supported manuscript languages and character pronunciation needs with real fixtures rather than assuming provider capabilities.

## Follow-ups

Defer extra import formats, direct Narrative Forge integration, and collaborative editing. Promote any loss of source content or version identity as a correctness fix before casting work.

## GitHub execution

[#1](https://github.com/loveoverflowcom/cantos/issues/1), [#2](https://github.com/loveoverflowcom/cantos/issues/2), [#3](https://github.com/loveoverflowcom/cantos/issues/3), [#4](https://github.com/loveoverflowcom/cantos/issues/4). See the [issue/dependency map](project-planning.md) and [Kanban](https://github.com/users/loveoverflowcom/projects/6/views/1) for the recommended sequence and live execution state.
