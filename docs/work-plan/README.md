# MVP Work Queue — External Audio Handoff

> Scope reset 2026-10-12: [ADR 0018](../decisions/0018-external-audio-production.md).
> Studio is Web-only and manages novel/story/podcast content and their audio; Theatre is
> the listener Web/CMP client. Backend TTS, provider workers and in-server mixing are removed.
> Older TTS implementation evidence and GitHub issue history remain archival.

## Ordered implementation

1. **Catalog / editorial identity:** work types `novel | story | podcast`, stable
   chapter/episode records, optional manuscript text or Script IR, ownership and rights.
   Do not force a script/cast workflow on plain narration or uploaded recordings.
2. **Instructions API:** versioned immutable production brief/skills bundle read by an
   external AI producer, with caller scope, text/rules/target profile and provenance digests.
3. **External audio intake:** scoped producer credentials; private prepare/PUT/finalize;
   idempotency, SHA-256/size/format verification, immutable audio rendition and review record.
4. **Review & release:** creator QC/rights decisions, atomic immutable release manifest, CDN
   and auditable replacements (old release remains playable on failure).
5. **Theatre Web/CMP:** discovery, streaming, background controls, offline downloads and
   synchronized playback progress based only on released media.

## Existing foundations to retain

Script IR validation/revision persistence, manuscript import, host-generated adaptation
proposal review, production metadata/rights, pure WAV inspector, shared UI system,
MessagePack-capable editorial routes, tests and `cargo xtask`. These are *not* work catalog
or audio ingestion APIs. Prior worker/Google/eSpeak/VieNeu samples and DBSP job views are
historical experiments, not prerequisites for the new roadmap.

## Canonical work plans

- [010 — source/script import and editor](010-import-and-edit-script.md) — completed portions retained.
- [020 — external production instructions and audio handoff](020-cast-and-generate-dialogue.md).
- [030 — external master intake, QC and release](030-mix-review-and-publish.md).
- [040 — Theatre Web](040-theatre-web-listening.md).
- [050 — native Theatre Mobile](050-cmp-native-listening.md).
- [GitHub execution map](project-planning.md) and [audio drama craft](audio-drama-quality.md)
  support these, without authorizing TTS inside Cantos.

## Shared completion evidence

Every PR records the exact baseline, files and contracts changed, commands actually run,
test outputs with `PASS`, `FAIL`, `NOT_RUN` or `UNSUPPORTED`, mock/live distinctions,
unresolved blockers and compatibility impact. Do **not** require GitHub CI to finish before
local verification-based handoff; do not claim local tests ran if this environment cannot run
them. For audio, `audio-measured`, `audio-listened` and `uploaded-and-object-verified`
are separate evidence levels.

A missing upload/storage/identity backend is **not** satisfied by a schema-only proposal.
Acceptance requires real, authorized upload, retry, wrong-scope, tampering and publication
failure tests.
