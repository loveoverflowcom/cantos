---
name: cantos-production-pipeline
description: >-
  Own Cantos external audio production instructions, secure producer handoff, received
  artifact validation, idempotent upload receipts, rights and QC metadata, versioned
  low-bandwidth audio profiles and review-ready immutable renditions. Do not add local
  TTS/cloud synthesis engines, provider workers or audio mixing to the Cantos backend.
---

# Cantos external audio production pipeline

This skill is the **server-side contract and review guide**, not a TTS executor. The
[product pipeline](../../../docs/product/production-pipeline.md),
[business rules](../../../docs/product/business-rules.md) and
[ADR 0018](../../../docs/decisions/0018-external-audio-production.md) are the authority.
Use [engineering](../cantos-engineering/SKILL.md) for functional core, effect boundaries,
typed state machines, verification and honest evidence labels.

For the separate Forge producer service, follow accepted [ADR 0022](../../../docs/decisions/0022-cantos-forge-producer-cli.md)
and the [worker boundary](../../../docs/deployment/forge-worker.md). Studio may control that
external execution; the Cantos API still never synthesizes. Local worker/process tests and
filesystem attempts do not establish owner invocation authority, distributed fencing,
app-data persistence, Gemini/encoder or deployed acceptance.

## Capability ownership

| Work | Owner |
| --- | --- |
| Work/chapter/podcast identity, versioned briefs, rights, access | Cantos Studio/API |
| Selection/operation of any AI model, TTS, recording, mixing and encoding | External producer |
| Vendor credentials, inference usage, billing, retries, clip cache | External producer |
| Private upload intent, verified object, exact digest and immutable receipt | Cantos API |
| Producer-declared media/quality/cost report | External producer, stored as claims |
| Server-observed bytes/profile, QC finding records and creator approval | Cantos API/Studio |
| Immutable public release and Theatre streaming | Cantos API/Theatre |

## Working order

1. Read the [proposed handoff contract](../../../packages/contracts/external-audio-handoff-v1.md)
   and the relevant issue. Distinguish **proposed** DTOs/routes from implemented contracts.
2. Model immutable `Work`, `ContentUnit`, `ProductionBrief`, `AudioSubmission`,
   `AudioRendition`, `AudioReview` and `Release` identities and legal transitions.
   Script IR is optional for simple audiobook/podcast content.
3. Design pure decisions for ownership, accepted profile, stale brief, idempotency/replay,
   duplicate object digest, QC gate and publication eligibility before HTTP or storage I/O.
4. Implement thin adapters: authenticated scoped producer tokens, time-limited upload grants,
   bounded object verification, PostgreSQL transactional metadata and release pointers.
   Never assume an object-store PUT is atomic with Postgres.
5. Test wrong-owner/wrong-episode/wrong-digest, replay (same/different payload),
   corrupt/truncated media, expiring grants, lost finalize response, concurrent reviewers,
   permissions revocation and publication replacement failure.
6. Keep production-only guidance in producer-facing
   [`cantos-audio-produce`](../cantos-audio-produce/SKILL.md). No server dependency on
   eSpeak, VieNeu, Google TTS, local inference or mixing tools is allowed.
7. Report measured, producer-declared, listening-reviewed and object-verified evidence
   independently; never certify audio quality from a file extension or mock result.

## Reuse and regeneration

A producer may reuse intermediate speech if its own effective inputs and license are
unchanged. The server only stores immutable audio objects and their content checksum:
Cantos does not calculate TTS cache keys or automatically regenerate dialogue. Replacing
a chapter/episode creates a **new rendition** after a new producer submission and review.

## Audio profile and QC

Audio is generated and encoded outside the server, but Cantos defines its acceptable media
profile, checks actual uploaded objects under bounded resources and enforces review gates.
See [mix/QC](references/audio-mix-and-qc.md) and
[profile issue #64](https://github.com/loveoverflowcom/cantos/issues/64). Opus mono
20–40 kbps and AAC mono are proposals pending versioned adoption; no fixed target may be
presented as an established runtime check before schema, tests and a decision.

## Anti-patterns

- `CantosWorker::synthesize()`, `reqwest.post("...tts...")`, local model spawn,
  `production-jobs enqueue` or a hidden server cron job.
- Reusing the Studio user's cookie in a third-party plugin.
- Returning an upload success after only accepting a user-supplied hash.
- Publishing while file bytes, rights or QC are unverified.
- Treating producer-generated usage/cost/quality claims as independently attested facts.
- Mutating a released audio object or silently repointing a release at a new upload.


## Supporting reference documents

- [External provider adapters](references/provider-adapters.md) — producer-side only.
- [Submission durability](references/durable-jobs.md) — private upload replay.
- [Producer cache and fingerprints](references/fingerprints-and-invalidation.md) — historical TTS methods.
- [External model cost guidance](references/cost-and-budget.md) — no Cantos billing.
- [Mixing and QC](references/audio-mix-and-qc.md) — mixing belongs to producer.
- [Fault injection](references/fault-injection-testing.md) — upload/release focus.

## Verification and handoff

Use repository `cargo xtask` gates and local/mocked integration tests. If unavailable, report
`NOT_RUN`; never describe prior synthetic TTS evidence as validation of the new upload API.
A production issue closes only on tested authorized, durable file intake and review or on
a scoped change explicitly documented in its updated acceptance criteria.
