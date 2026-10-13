# Prepare external audio instructions and submissions

> Active scope from [ADR 0018](../decisions/0018-external-audio-production.md).
> The former `020 — Cast and generate dialogue durably` design was superseded. Cantos
> does not run TTS, retry provider calls or bill synthesis.

## Current narrow implementation

[ADR 0020](../decisions/0020-studio-private-audio-intake.md) implements owner-session,
script-revision-bound private development intake with four explicit Opus profiles, verified
bytes, private filesystem objects and immutable receipts. Rust/PostgreSQL runtime verification
is **NOT_RUN**. This does not complete the general external producer/catalog-bound brief or
QC/release/storage-delivery acceptance criteria below. Production S3, scoped producer grants,
rights/listening review, publication and reference-aware orphan retention remain separate work.

## Why

Novels, stories and podcasts should be published from externally generated **or recorded**
audio. Creators need a durable source of truth for what to produce, while independent AI
agents/human producers select their own voices, models, costs and mixing tools.

## Scope

- Keep planning-only voice/cast, pronunciation, Script IR, cue, rights and review data as
  **optional** production guidance. Plain narration and human podcasts must work without them.
- Export a frozen work/content-unit brief with version, checksum, permission scope, audio
  profile requirements and links to agent skills.
- Implement a scoped read-only instructions API for external producers.
- Implement authenticated, private audio-upload prepare/finalize with idempotency, verified
  stored object hash/size/format, producer-declared provenance, and readback of immutable receipt.
- Let producers resubmit a corrected rendition without overwriting historical objects or
  changing published releases. Cantos may flag stale instructions, but never regenerates audio.
- Surface submitted, rejected, incomplete and pending-review status in Studio Web.

Full production/mixing occurs in the external producer, not `services/cantos-api`.
No backend TTS probe, local neural model, Google adapter, worker leasing, TTS receipt dispatch
or DBSP generation-job read model may be reintroduced.

## Suggested sequence

1. Catalog CRUD for works and content units; permissions and accepted content revisions.
2. Versioned producer brief, including optional script/cast guidance and target rendition.
3. Short-lived scoped producer authorization and secure private upload initiation.
4. Finalize and object verification, exact digest/idempotency/replay/conflict behavior.
5. Studio rendering for brief export, upload receipts, per-rendition state and QC handoff.

## Acceptance criteria

- Novel chapter audio and an ordinary podcast recording both work without requiring
  a dialogue Script IR or a TTS provider choice.
- A producer can fetch the exact frozen brief and cannot access another creator's content.
- Wrong work/episode/brief hash, bad actual object digest, size/format mismatch, expired token
  and duplicate conflicting idempotency keys are rejected and remain private.
- Valid repeated finalize returns the same immutable receipt; upload failure is recoverable.
- An external provider timeout does not create a Cantos provider attempt/charge or new TTS call.
- New/changed chapter text cannot mutate an approved historical rendition.
- No submitted asset is listener-visible until QC/rights/release gates approve it.

## Existing work and migration

The completed [#5](https://github.com/loveoverflowcom/cantos/issues/5) frozen metadata/rights
foundation remains usable. [#6](https://github.com/loveoverflowcom/cantos/issues/6),
[#7](https://github.com/loveoverflowcom/cantos/issues/7) and
[#8](https://github.com/loveoverflowcom/cantos/issues/8) are retargeted to instructions,
external asset receipts and Studio submission/progress, not internal TTS execution.
Retain historical provider research documents and SQL migration checksums for traceability.
