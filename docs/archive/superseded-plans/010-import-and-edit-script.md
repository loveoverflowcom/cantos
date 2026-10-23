# Import and edit a versioned script

> **Historical snapshot — archived 2026-10-16.** Original path: `docs/work-plan/010-import-and-edit-script.md`.
> Retained for traceability; dates, commands, permissions, blockers and task status below
> describe the original session and are not current execution instructions. Use the
> [current priorities](../../work-plan/current-priorities.md) and [documentation map](../../README.md) before starting work.

## Why

A creator needs a reliable manuscript-to-script flow before audio production can begin. Script identity, revisions, character references, and source permissions must survive adaptation and editing so later regeneration uses the intended text.

## Scope

Deliver a Studio flow that imports TXT, Markdown, DOCX, or a structured script; previews the source; proposes an AI-assisted adaptation; and lets the creator review, correct, and save a validated version. Support original scripts through the same editing flow.

Use the Work → Adaptation → Episode → Act → Scene → Dialogue → Audio hierarchy from the brief. Define the smallest versioned Script IR needed by this flow, including stable identity within a script, speakers, emotion hints, pronunciation references, sound cues, and provenance. Audio references can remain absent until generation. Document how future Narrative Forge interchange will use a versioned contract rather than shared internal storage or runtime code.

Persist source references, script versions, permission/publication eligibility metadata, and adaptation cost records in PostgreSQL. Establish API contracts with an immediate Studio consumer. Build a compact, text-first Leptos editor with scenes, speaker correction, validation errors, and explicit save/version behavior.

## Non-goals

TTS, audio mixing, public playback, a general document-conversion service, real-time collaborative editing, or a universal schema for every narrative project.

## Dependencies

The project brief and architecture guidance. An executable [Script IR contract](../../../services/cantos-server/contracts/README.md)
now supplies structured-draft validation, exports and speech bytes. The local
[revision slice](../../evidence/script-revision-persistence.md) supplies Axum/PostgreSQL and an
authorized Leptos save/read consumer with crash/retry/concurrency evidence. Review its actual
modules, v1 wire contract and read inventory before expanding intake. Workload measurements,
production identity adoption and the complete authoring journey still belong to #1/010.
The [compatible handoff](../../evidence/editorial-handoff.md) additionally supplies private preserved
source reads, bounded revision history, owner review records and local credential/evidence
tools. Build importer/source-preview/editor consumers on that shared backend; these extensions
do not implement all input formats or the complete review journey.

The [manuscript slice](../../evidence/manuscript-import.md) preserves bounded original bytes and
separate TXT/Markdown/DOCX/Script IR extraction outcomes on that shared source boundary.
Adaptation should consume those immutable records and uncertainty rather than re-read or
overwrite an accepted source. The complete adaptation/edit/revision journey remains the 010
capability review boundary.

The [bounded adaptation consumer](../../evidence/ai-script-adaptation.md) adds authenticated
context/read/submit/review tools, durable declared provenance/receipts and explicit Studio
compare/edit/accept flow. The user-controlled Gemini/ChatGPT/Codex host generates content;
Cantos performs no inference. The #3 gate is the actual tool/storage/validation journey,
separate from unrun model quality. The former local-model prerequisite is superseded, not passed.
The [#4 editor](../../evidence/studio-script-editor.md) supplies structured scene/character/dialogue
editing and the local import/proposal/edit/accept/save/restart/reopen journey. Production identity,
legal rights clearance and actual AI quality remain independent capability gates.

## Suggested sequence

1. Add representative manuscript and structured-script fixtures, including multiple speakers and scenes.
2. Implement import, source preview, and the minimum persistence/API boundaries for saved versions.
3. Export source-bound context and admit caller-created adaptation through real authenticated tools; review structured output before saving.
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

Review aid, no linked issue: a standalone [`diff-script` CLI](../../evidence/script-diff.md) compares two Script IR files of one episode by stable ID so a reviewer can see edited, moved, added and removed lines, cues and characters and provenance/rights changes. It reads two files, so it does not satisfy the save/reopen, revision-history or editor criteria above; the editor in #4 may adopt, wrap or replace it.

## GitHub execution

[#1](https://github.com/loveoverflowcom/cantos/issues/1), [#2](https://github.com/loveoverflowcom/cantos/issues/2), [#3](https://github.com/loveoverflowcom/cantos/issues/3), [#4](https://github.com/loveoverflowcom/cantos/issues/4). See the [issue/dependency map](../../work-plan/project-planning.md) and [Kanban](https://github.com/users/loveoverflowcom/projects/6/views/1) for the recommended sequence and live execution state.
