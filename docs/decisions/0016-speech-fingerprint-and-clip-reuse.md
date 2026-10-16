# ADR 0016: Position-free speech fingerprint, verified clip reuse and regeneration planning

> **Historical / superseded for active backend (2026-10-12).** [ADR 0018](0018-external-audio-production.md) removes server-side TTS generation/worker, provider dispatch and synthesis-job status. This document remains for audit and prior experiments only; do not implement it as current Cantos API behavior.


- Status: historical; superseded for active backend by [ADR 0018](0018-external-audio-production.md).
- Original status: proposed
- Date: 2026-10-12
- Decision owner: Cantos maintainers
- Related issue or PR: [#7](https://github.com/loveoverflowcom/cantos/issues/7) (reuse criterion),
  [#8](https://github.com/loveoverflowcom/cantos/issues/8) (partial regeneration planning); neither is closed
- Supersedes / superseded by: N/A; extends [decision 0007](0007-casting-production-inputs.md) and the
  [durable jobs foundation](../evidence/durable-tts-jobs.md)

## Context

Known: the durable foundation selects one write-once result per frozen dialogue, and records
`durable-input-v1` per step. That identity deliberately includes dialogue, character and rights
identifiers, so it links a step to its run and cannot identify "the same synthesis request" across runs.
[Business rules 14–16](../product/business-rules.md#production-caching-and-costs) require reuse only when
the fingerprint, rights/access scope and artifact integrity remain valid, and regeneration of only changed or
invalid dialogue and its dependents. Nothing reuses audio today, so every edited script regenerates every line.

Known: the fake and the eSpeak modes consume the same resolved input, so a fingerprint alone cannot tell a
synthetic fixture from real engine output. The only object store is the worker's private filesystem root;
there is no object storage, mix record or QC record.

Assumed: a bounded local slice is useful before object storage exists, provided no hit is ever recognized
without fresh evidence about the stored object.

## Decision

1. **`speech-v1`** is a position-free identity over exactly the effective synthesis request (text,
   language, provider/model, voice, adapter, output contract, rate, pitch, emotion, intensity and the
   resolved pronunciation list). Dialogue, character, rights record, scene, position and job identifiers
   are excluded by an exhaustive destructuring that fails to compile when a field is unclassified. Only a
   provider model whose identity is exact in the frozen input has one; Google and VieNeu do not. It is a
   different scheme from `durable-input-v1`, which stays as is.
2. **A clip registry on the existing PostgreSQL authority** (`speech_clips`, migration 0008). A row is
   written inside `accept_step`'s transaction and restates the audited accepted observation. PostgreSQL
   verifies that the stored canonical document hashes to the stored fingerprint and that the row matches
   its accepted attempt and that attempt's own step (fingerprint, voice-rights record, output contract); rows are
   immutable. A canonical document is bounded at 2 MiB, so a request that has a fingerprint is always registrable.
3. **Reuse is a decision over facts, not a lookup.** A pure `resolve_reuse` requires the fingerprint, the
   creator scope, the voice-rights record, the producer class, the output contract, a fresh probe of the
   stored object and an equal checksum, plus a separate current-authority input. A completed job is not
   one of these facts. Candidates are examined oldest first.
4. **Reuse is an attempt-less way for a step to succeed.** The worker looks up candidates, probes outside
   any transaction and selects under the existing fence, authority and job-state checks (`durable_steps.reused_clip_id`,
   write-once, trigger-guarded). The selection requires a `VerifiedObject` witness that only a real probe can issue,
   so "no hit without a probe" is a type barrier, not worker discipline. A hit creates no attempt, reservation or dispatch. Any conflict falls back
   to the unchanged dispatch path; an existing attempt is reconciled, never bypassed.
5. **Partial regeneration is a pure plan** over two frozen snapshots, the verified-clip facts and the
   gate. It reports per-line change reasons and cache decisions, stale scene mixes and QC, and separate
   hit/miss/blocked counts with a planning cost. It is read-only CLI JSON (`production-jobs plan`); no
   Studio route is added.

## Alternatives

| Option | Benefits | Costs and limitations | Reason selected or rejected |
| --- | --- | --- | --- |
| Add the speech fingerprint to `durable-input-v1` | One identity | Re-keys every stored step; keeps position and rights inside the key | Rejected: breaks the linkage contract and the position-free property |
| Treat a fingerprint match as a hit | Simplest | A deleted, tampered or foreign object, another creator's or a synthetic clip would be reused | Rejected: violates rule 15 and the "completed is not reusable" invariant |
| Reuse by inserting an observation/attempt row for the hit | No schema change to steps | A cache hit would look like a provider send and a settled charge | Rejected: rule 17 requires hits to be distinguishable |
| Pause the step until another run's in-flight twin finishes | No duplicate spend across concurrent runs | Needs an in-flight registry with leases and timeouts; risks starvation | Deferred; documented as a residual risk |
| Include a refresh epoch to admit unpinned models | Covers Google | Needs a refresh action, storage and drift findings with no consumer yet | Deferred; such providers are refused as `model_not_pinned` |
| Persist stale mix/QC marks now | Visible to Studio | No mix or QC tables exist | Deferred to #9/#10; the plan names the work only |
| Add a Studio route for the plan | Reachable by the UI | A new authenticated surface, DTOs and Studio evidence | Deferred to the Studio slice of #8 |

## Consequences

- Product and user impact: repeating a completed identical request, reverting an edit and editing one
  line reuse every other verified clip instead of calling a provider; the plan explains each decision.
- Compatibility and migrations: migration 0008 is additive. Steps created earlier have no speech fingerprint
  and never reuse; accepted attempts from earlier runs have no clip (no backfill). `JobSummary` gains
  `reused_steps`; `cantos-worker` gains the `before_reuse` failpoint.
- Operations, storage and cost: hits spend nothing and reserve nothing. A clip is usable only by a worker
  that can open its private output root. At most 16 clips per fingerprint are probed.
- Implementation and recovery requirements: the pure core (`speech_fingerprint`, `clip_reuse`,
  `regeneration`) has no HTTP, SQL, provider or file dependency; the shell is `postgres::durable::clips`,
  `clip_probe` and the worker. A crash between probe and selection leaves nothing durable.
- Deferred questions: cross-run in-flight de-duplication; unpinned-model refresh epochs; previous-clip identity
  for mix staleness; object-storage locators and retention; whether a per-line pronunciation projection should
  replace business rule 16's per-list invalidation; a Studio endpoint.

## Validation and revisit conditions

Evidence is in the [evidence report](../evidence/speech-clip-reuse.md): independent oracle vectors, generated
laws, single-mutant checks run by hand against the pure core and shell (a recorded procedure, not a repository
harness), and real-PostgreSQL crash, race, upgrade and forgery tests. It
includes no live provider, no listening review and no object storage. Revisit when object storage lands
(locator and verification move to it), when a mixer or QC record exists (persist staleness and previous-clip
identity), when a second pinned provider is admitted (extend the pin policy or bump the scheme), or if
cross-run duplicate synthesis proves material.
