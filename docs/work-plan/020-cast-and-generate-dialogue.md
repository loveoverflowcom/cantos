# Cast characters and hand dialogue to external producers

> Direction change, 2026-10-12 ([ADR 0018](../decisions/0018-external-audio-production.md)): Cantos
> does not synthesize speech. This specification replaces "generate dialogue durably". The
> file name is kept so existing links work; numeric prefixes are not task IDs.

## Why

Audio drama needs consistent character voices. Cantos Studio manages the work, its episodes and
its audio, but the audio is made by an external AI producer (a plugin, agent or person) that follows
Cantos's instructions and standards and sends the result back through the API. That only works if
the instructions are exact and versioned, and if returned audio is bound to the script version it
was made for and is verified by Cantos rather than trusted.

## Scope

1. **Casting and frozen production inputs** (the implemented #5 slice). Studio casts characters
   and freezes the selected voice guidance, versioned performance/pronunciation settings, model
   and supported controls against one accepted script revision. Production authorization binds that
   exact input scope, applicable rights records and budget policy; draft edits cannot change an
   existing snapshot. The [production input contract](../../packages/contracts/production-v1.md)
   and [casting evidence](../evidence/casting-production-inputs.md) describe it, including real
   PostgreSQL/HTTP recovery and observed Studio freeze/approval/stale-input behavior. The snapshot
   is guidance for a producer: it dispatches nothing and grants no permission to spend.
2. **Producer instructions and standards.** Publish what a producer needs: how to read an approved
   snapshot and its script revision, the delivery profile, naming and provenance expectations, and
   the QC criteria submitted audio must meet. These live in `docs/production/` and the agent skills,
   following the host-owned pattern of [adaptation](../../packages/contracts/adaptation-v1.md).
3. **Audio submission API** (proposed, not implemented). Scoped, revocable producer credentials;
   expiring upload grants to private object storage; idempotent submissions bound to the exact
   revision, snapshot digest and dialogue IDs, carrying the producer's provenance claim. Cantos
   re-measures the audio (the retained WAV inspection is the first admission primitive) and records
   its measurements apart from the producer's claims.
4. **Studio submission view.** Per-dialogue submission status, preview, accept or replace, and stale
   marks when a line, voice or pronunciation changes.

## Non-goals

Hosting TTS in Cantos, provider adapters, provider credentials, cost reservation or settlement,
generation jobs and workers, voice cloning, automated expressive-quality scoring, and mixing
(mixing and the master are external as well, see [030](030-mix-review-and-publish.md)).

## Dependencies

[010 — Import and edit a versioned script](010-import-and-edit-script.md). The submission design
should be chosen after inspecting the persisted Script IR, the approved snapshot and the first real
producer's output formats. Storage and credential decisions are shared with
[030](030-mix-review-and-publish.md).

## Suggested sequence

1. Casting and generation-free production inputs for one validated episode version (done, #5).
2. Write the producer instructions and standards and the read path for an approved snapshot, then
   exercise them with a real producer on the original reference scene.
3. Implement the submission API, storage and inspection with a synthetic audio fixture first, then
   the real producer's output.
4. Add the Studio submission view and stale marking.

## Acceptance criteria

- A character retains the selected voice guidance and pronunciation across dialogue; deliberately
  changed settings produce a new snapshot.
- A producer following only the published instructions can read an approved snapshot and submit
  audio for every dialogue of an episode.
- Repeating an identical submission returns the prior result; an interrupted upload leaves no
  accepted output; concurrent submissions cannot attach audio to another dialogue or revision.
- A submission for a superseded revision or snapshot is rejected or marked stale. Changing one line
  marks only that line and its dependent scenes stale.
- Unreadable, mismatched or oversized audio is rejected with an actionable reason; the producer's
  claims and Cantos's measurements are stored separately.
- A creator can replace a rejected or stale submission and keep unaffected accepted ones.
- No Cantos component holds a TTS provider credential or makes a generation call.

## Review boundary

For #5, demonstrate accepted revision → versioned casting/settings → rights/budget findings →
immutable frozen candidate → exact owner approval → reopened snapshot and current eligibility.
Verify unsupported controls, every scoped-change invalidation, rights expiry/revocation, actor
isolation, concurrent freeze, exact retry and restart recovery on real PostgreSQL. Rights claims
are recorded assertions, not legal verification; missing/pending claims block when required.

The complete 020 boundary later demonstrates approved snapshot → producer reads instructions →
submission → inspection → acceptance → single-line edit → stale mark → resubmission, on real
PostgreSQL and object storage, including interrupted and repeated uploads. Product approval never
authorizes an agent to make a real payment or provider call.

## Risks / unknowns

Producer quality and honesty are outside Cantos: provenance and rights statements are claims, and
only measurements are facts. Voices and models differ in the controls they support, so the
instructions must say which controls a voice honors. Real-person voices and cloning need specific
recorded consent and rights. The credential model for non-browser producers is undecided.

## Follow-ups

Decide whether the provider catalog under `production/domain/catalog` remains casting vocabulary
or is trimmed, and whether the dormant `durable_*` and `speech_clips` tables are retired by a new
migration. Listening-quality assessment of a producer's output follows the shared
[listening-review protocol](../production/listening-review-protocol.md). The removed runtime's
history is preserved: [durable jobs](../evidence/durable-tts-jobs.md),
[provider verification](../evidence/tts-provider-verification.md) and
[clip reuse](../evidence/speech-clip-reuse.md) record what it did and what was never run.

## GitHub execution

[#5](https://github.com/loveoverflowcom/cantos/issues/5) is the implemented casting slice.
[#6](https://github.com/loveoverflowcom/cantos/issues/6), [#7](https://github.com/loveoverflowcom/cantos/issues/7)
and [#8](https://github.com/loveoverflowcom/cantos/issues/8) were written for the removed in-process
runtime; their proposed replacements are listed in the [issue map](project-planning.md#re-scoped-by-adr-0018).
See the [Kanban](https://github.com/users/loveoverflowcom/projects/6/views/1) for live status.

The [audio drama quality plan](audio-drama-quality.md) adds voice auditions, a distinctness
matrix and a pronunciation lexicon ([#50](https://github.com/loveoverflowcom/cantos/issues/50)) and
a shared listening-review protocol ([#45](https://github.com/loveoverflowcom/cantos/issues/45)).
They supply evidence for judging a producer's output but cannot authorize production or publication.
