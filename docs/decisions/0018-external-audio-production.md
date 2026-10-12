# ADR 0018: Studio manages works and audio; external AI producers synthesize and submit it

- Status: accepted (direction and removal); the audio-submission API below is **proposed, not implemented**
- Date: 2026-10-12
- Decision owner: Cantos maintainers; direction given by the repository owner on 2026-10-12
- Related issue or PR: [PR #65](https://github.com/loveoverflowcom/cantos/pull/65)
- Supersedes / superseded by: supersedes the in-process execution parts of
  [ADR 0008](0008-local-tts-inspection.md), [ADR 0010](0010-local-neural-tts-research.md),
  [ADR 0011](0011-google-cloud-tts-adapter.md), [ADR 0016](0016-speech-fingerprint-and-clip-reuse.md)
  and [ADR 0017](0017-experimental-dbsp-job-summary.md) (the job-summary consumer is gone);
  narrows [ADR 0007](0007-casting-production-inputs.md) to production *guidance*. Extends the
  host-owned pattern of [ADR 0006](0006-ai-script-adaptation.md) from adaptation to audio.

## Context

Known: Cantos Studio grew an in-process text-to-speech runtime: a `synthesis` context with durable
PostgreSQL jobs, leases and fences, provider adapters (eSpeak NG, VieNeu Nano, Google Cloud TTS),
cost reservations, receipts, speech fingerprints, a verified clip registry, a `cantos-worker`
process and several probe CLIs. That runtime was about 25,000 lines, had only mocked or
sample-scale evidence, and made Cantos responsible for provider credentials, spend, retries and
ambiguous external outcomes.

Known: adaptation already works the other way round ([ADR 0006](0006-ai-script-adaptation.md)).
The user-controlled AI host (Gemini, ChatGPT, Codex) reads exported context through authenticated
tools and returns a proposal that Cantos validates and persists; Cantos performs no model inference.

Decision input from the owner: Cantos Studio only manages the titles (novels, stories, podcasts),
their episodes and their audio. It does not run TTS. It provides an API and the instructions,
skills and standards that another AI-driven plugin follows to produce audio, and that plugin
sends the audio back through the API.

Assumed: producers (plugins, agents, a human with a DAW) are outside the Cantos trust boundary and
bring their own provider accounts, rights and budget.

## Decision

1. **Boundary.** Cantos Studio and `cantos-api` own works, episodes, Script IR revisions, review,
   rights and approval records, production guidance (casting and frozen production inputs) and
   the audio that producers submit for an exact revision. They never synthesize, mix or call a
   TTS provider, and no listener request or Studio action starts generation.
2. **Instructions out.** An approved, frozen production snapshot, the Script IR revision it pins,
   the casting and pronunciation guidance, and the written standards (audio profile, naming,
   provenance and QC criteria in `docs/production/` and the agent skills) are what a producer
   reads. They are guidance and acceptance criteria, not an execution contract.
3. **Audio in (proposed).** A producer submits audio over the API under a scoped, revocable
   credential that is not a Studio browser session, using expiring upload grants to private
   object storage. A submission names the exact script revision, snapshot digest and dialogue IDs
   it covers and carries the producer's provenance claim (tool, model, voice, rights assertion).
   `.env.example` reserves the storage settings; no route, table or grant exists yet.
4. **Submitted audio is untrusted.** Cantos admits it only after its own measurement. The pure,
   read-only WAV PCM inspection (`contexts::media::domain::wav_inspection`, `inspect-wav`) is the
   retained admission primitive: it reports structure, checksums and sample peak. A producer's
   claims are recorded as claims; Cantos-measured facts are recorded as such. Acceptance of
   submitted audio, QC and publication stay separate approvals bound to exact checksums.
5. **Removed.** The `synthesis` context (jobs, attempts, leases, fences, reservations,
   observations, settlement, reuse, regeneration planning), the provider adapters and HTTP
   transport, the neural runtime helper and its fixtures, the `cantos-worker`, `production-jobs`,
   `probe-*` and `inspect-tts-receipt` binaries, the receipt/clip-reuse/durable-job contracts,
   their oracles, and the DBSP job-summary view that only served those jobs.
6. **Retained.** Editorial, ingestion, adaptation and production contexts; the `media` context
   with WAV inspection; the migrations. Migrations 0007–0009 still create the `durable_*` and
   `speech_clips` tables. Nothing reads or writes them, they are not a supported interface, and
   migration history is append-only. Retiring them needs a new migration after an explicit data
   retention decision (they may hold evidence records from the eSpeak sample runs); that
   decision is deferred. `scripts/production_oracle.py` still counts rows in three of them as its
   no-execution check, so a retirement migration must update it too. The `dbsp-experimental`
   feature and its engine-independent `platform/incremental` core also remain, with no consumer;
   removing them is a separate decision.

## Alternatives

| Option | Benefits | Costs and limitations | Reason selected or rejected |
| --- | --- | --- | --- |
| Keep the in-process runtime and finish it (#6–#8) | One system generates, reuses and records everything | Cantos owns provider credentials, spend, retries and ambiguous outcomes; every provider needs rights, budget and listening acceptance first; large surface with sample-scale evidence | Rejected by the owner's direction |
| Keep the runtime behind a feature flag or disabled | Easy to revive | Dead code that still must compile, be linted and be reviewed; docs and gates keep implying a capability that is off | Rejected: history preserves it |
| Remove it and offer instructions plus an audio-submission API | Producers vary freely; Cantos holds only metadata, rights and verification; matches the adaptation pattern | The submission API, credentials and storage are new work; quality depends on the producer following the standards | Selected |

## Consequences

- Product and user impact: Studio creators produce audio with an external tool they choose;
  Studio shows the guidance and, once the API exists, the submissions. Until then **no audio can
  enter Cantos**, so publication and everything after it (#9–#11 and listening) are blocked on
  the submission and storage work, not on TTS.
- Compatibility and migrations: no migration is added. HTTP routes, DTOs and Script IR are
  unchanged because the runtime exposed no Studio route; the crate-root aliases `tts`, `worker`,
  `jobs`, `speech_fingerprint`, `clip_reuse`, `regeneration`, `clip_probe` and
  `postgres::durable` no longer exist. Casting and approval snapshots are unaffected.
- Operations, storage and production cost: Cantos holds no TTS credential and incurs no TTS cost.
  Producers carry provider terms, spend and idempotency; Cantos cannot claim provider guarantees.
- Implementation and recovery requirements: the submission API must make an upload idempotent,
  reject unmeasured or mismatched audio, bind each submission to an exact revision and checksum,
  mark submissions stale when the dialogue or snapshot changes, keep drafts and previews private
  and follow the rights and publication-gate rules unchanged.
- Deferred questions: submission API and credential model; whether the provider catalog under
  `production/domain/catalog` stays as casting vocabulary or is trimmed; the Studio production
  screens that still describe generation; dropping the dormant tables; the open issues #6–#9,
  #50–#53 and #64, which were written for the in-process runtime and need rewriting.

## Validation and revisit conditions

Evidence: after the removal the workspace builds, `cargo xtask verify rust` and
`cargo xtask verify repository` pass, and the PostgreSQL tests for the retained contexts remain.
The mocked or local-sample evidence for the removed runtime is kept as history under
`docs/evidence/` with links pinned to the last commit that contained it (`2de8c69`).

Revisit when a concrete need appears for Cantos itself to generate audio (for example a managed
offering), in which case a new ADR must start from the correctness rules in `AGENTS.md`
(duplicate delivery, ambiguous attempts, leases, cost) rather than from the removed code.
