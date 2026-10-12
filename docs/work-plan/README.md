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

The next Catalog acceptance is the [Studio Web slice](011-studio-catalog-api.md) using the
implemented [Catalog JSON API](../../packages/contracts/catalog-v1.md). The internal foundation
and HTTP evidence are recorded separately in [session 1](../evidence/studio-catalog-foundation.md)
and [session 2](../evidence/studio-catalog-api.md). Keep #66 open for responsive and keyboard
Studio Web acceptance; GitHub owns execution status.

The [Content Identity slice](../decisions/0019-content-identity.md) adds immutable content beneath
Catalog metadata. Its [handoff to Production Brief](content-identity-production-brief-handoff.md)
keeps the next backend boundary narrow: exact content/guidance revision pinning and independent
producer access, before linking intake/publication. Catalog Web acceptance remains separately
scoped; this sequence does not close #66 or #64.

## Existing foundations to retain

Script IR validation/revision persistence, manuscript import, host-generated adaptation
proposal review, production metadata/rights, pure WAV inspector, shared UI system,
MessagePack-capable editorial routes, tests and `cargo xtask`. The earlier editorial endpoints
are separate from the implemented Catalog API and proposed audio ingestion APIs. Prior worker/Google/eSpeak/VieNeu samples and DBSP job views are
historical experiments, not prerequisites for the new roadmap.

## Canonical work plans

- [010 — source/script import and editor](010-import-and-edit-script.md) — completed portions retained.
- [020 — external production instructions and audio handoff](020-cast-and-generate-dialogue.md).
- [030 — external master intake, QC and release](030-mix-review-and-publish.md).
- [040 — Theatre Web](040-theatre-web-listening.md). The session-only
  [Web mock](../evidence/theatre-web-mock.md) completes neither 040 nor its issues.
- [050 — native Theatre Mobile](050-cmp-native-listening.md). The memory-only
  [CMP mock](../../apps/cantos-theatre/mobile/README.md) completes neither 050 nor its issues.
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
