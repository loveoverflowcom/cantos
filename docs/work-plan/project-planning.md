# GitHub planning and execution — Cantos

> Updated on **2026-10-12** for [ADR 0018](../decisions/0018-external-audio-production.md).
> Cantos Studio is a Web-only novel/story/podcast CMS and external audio review authority;
> AI providers and human producers make audio **outside** Cantos. Theatre plays releases.

[Cantos Project](https://github.com/users/loveoverflowcom/projects/6) ·
[Kanban](https://github.com/users/loveoverflowcom/projects/6/views/1) ·
[Planning table](https://github.com/users/loveoverflowcom/projects/6/views/2) ·
[GitHub Issues](https://github.com/loveoverflowcom/cantos/issues)

GitHub issues and the Project own live execution status; this Markdown owns only the
**recommended dependency order**. Do not mark a feature ready, closed or implemented
because a proposal or AI prompt exists.

## Recommended delivery chain

| Order | Issue | Deliverable | Prerequisite |
| --- | --- | --- | --- |
| 1 | [#66 — Work catalog](https://github.com/loveoverflowcom/cantos/issues/66) | `novel|story|podcast` works, chapter/episode IDs, metadata, ownership; Script IR optional | Existing editorial/source contracts (#1–#5) |
| 2 | [#6 — External producer instructions](https://github.com/loveoverflowcom/cantos/issues/6) | Immutable brief, skills, versioned rights/profile and read-only scoped producer API | #66 |
| 3 | [#7 — External audio intake](https://github.com/loveoverflowcom/cantos/issues/7) | Scoped producer credentials, private upload/finalize, verified bytes and immutable receipt | #6, #66 |
| 4 | [#8 — Studio audio UI](https://github.com/loveoverflowcom/cantos/issues/8) | Upload state, private preview, correction, stale-brief status | #6, #7 |
| 5 | [#9 — QC review](https://github.com/loveoverflowcom/cantos/issues/9) | Measured-vs-declared QC and named editorial review of externally produced audio | #7, #51 |
| 6 | [#10 — Storage and delivery](https://github.com/loveoverflowcom/cantos/issues/10) | Private target/adapters/owner preview, provider acceptance and ready-for-release delivery | #7; coordinate #9 |
| 7 | [#11 — Publication](https://github.com/loveoverflowcom/cantos/issues/11) | Atomic manifest/approval gate, immutable published rendition, safe replacement | #8, #9, #10 |
| 8 | [#12 — Listener APIs](https://github.com/loveoverflowcom/cantos/issues/12) | Discovery/progress/entitlement across published releases | #11 |
| 9 | [#13 — Theatre Web](https://github.com/loveoverflowcom/cantos/issues/13) | Streaming, seek, progress/resume | #12 |
| 10 | [#14 — Theatre Mobile](https://github.com/loveoverflowcom/cantos/issues/14) | Native foreground playback on CMP Android/iOS | #12 |
| 11 | [#15 — Background playback](https://github.com/loveoverflowcom/cantos/issues/15) | OS media controls/interrupt recovery | #14 |
| 12 | [#16 — Offline](https://github.com/loveoverflowcom/cantos/issues/16) | Integrity-checked, authorized offline downloads | #14 |
| 13 | [#17 — Cross-device](https://github.com/loveoverflowcom/cantos/issues/17) | Native/Web progress, offline/resume E2E | #13, #15, #16 |

**#67 — Follow-up refactor** removes remaining provider-shaped planning/UI/dead dependencies
**safely**, preserving existing editorial APIs and the historical SQL migration checksums.
It can progress independently but must not add runtime TTS back to Cantos.

The previous 010 editorial/import/Script IR work (#1–#5) is retained as a source/review
foundation. Planning-only casting is optional guidance, **not a prerequisite for plain narration
or human-recorded podcast media**.

The [private storage/read slice](../architecture/object-storage.md) supplies Media-owned
target resolution and authorized preview, with [separate evidence](../evidence/provider-independent-storage.md).
Next for #10, prioritize operator identity/private policy and actual provider capability/range/
expiry/capacity acceptance. Keep named review/rights under #9, approved immutable publication
and retention under #11, then separate listener authorization/projections under #12. Private
S3 admission cannot satisfy a publication or listener prerequisite. A future migration must
copy, verify and register a ready location before a controlled read switch; it is not a default
configuration change. GitHub continues to own assignments and status.

## External producer craft and audio profile

| Issue | Current ownership |
| --- | --- |
| [#44](https://github.com/loveoverflowcom/cantos/issues/44), [#45](https://github.com/loveoverflowcom/cantos/issues/45) | Reference material and named listening review |
| [#46](https://github.com/loveoverflowcom/cantos/issues/46), [#47](https://github.com/loveoverflowcom/cantos/issues/47), [#48](https://github.com/loveoverflowcom/cantos/issues/48), [#49](https://github.com/loveoverflowcom/cantos/issues/49) | External producer assets/rights, cue bible, script adaptation and optional controls |
| [#50](https://github.com/loveoverflowcom/cantos/issues/50) | External voice auditions and producer-ready pronunciation lexicon |
| [#51](https://github.com/loveoverflowcom/cantos/issues/51), [#64](https://github.com/loveoverflowcom/cantos/issues/64) | Versioned codec/QC profile, low-bandwidth Opus and AAC options |
| [#52](https://github.com/loveoverflowcom/cantos/issues/52), [#53](https://github.com/loveoverflowcom/cantos/issues/53) | **External AI producer** audio conditioning/mixing guidance |

The earlier TTS provider implementations, worker leases, budgets, speech-clip cache and
DBSP generation-job summary are **historical**, not active roadmap milestones.
[ADR 0018](../decisions/0018-external-audio-production.md) supersedes those assumptions;
the [proposed external handoff contract](../../packages/contracts/external-audio-handoff-v1.md)
states only proposed routes, not a feature implementation.

## Exit criteria and evidence

- **Catalog:** create a plain novel chapter and an unscripted podcast episode without casting.
  The [backend API](../../packages/contracts/catalog-v1.md) is implemented; #66 also requires
  responsive/keyboard Studio Web flows and mocked client integration against that contract.
- **Instructions:** independently authorized plugin can read exact pinned brief/version/digest.
- **Intake:** real external file, private upload, server-observed integrity, idempotent finalize,
  cross-owner/expired-token/wrong-digest/restart failure tests.
- **Review:** independent listening vs measured metadata, rights, provenance, stale revision gate.
- **Publication:** exact immutable release pointer; failed replacement leaves older release playable.
- **Theatre:** seek, download, offline resume, background and Web/native sync from release only.

Every focused PR branches from `develop`, targets `develop`, carries implemented contract
and migration evidence, and explicitly labels `PASS`, `FAIL`, `NOT_RUN` or `UNSUPPORTED`.
Local tests/mock integration are preferred before handoff; remote CI is not required to finish.
Never claim provider-live production or successful media playback from docs/mock tests.
