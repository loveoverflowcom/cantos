# Studio Production Pipeline

> Status: target design. Bounded source import and caller-owned adaptation tools/review have
> [implementation evidence](../evidence/ai-script-adaptation.md). Generation belongs to the
> user-controlled Gemini/ChatGPT/Codex host; Cantos validates and persists submitted proposals.
> [Casting/input authorization](../evidence/casting-production-inputs.md) implements reference-only
> settings, script-scoped rights/budget declarations, immutable candidates and exact approvals.
> Production workers, audio, storage/delivery and publication below remain
> implementation requirements, not capabilities supplied by this repository.

## End-to-end flow

**Draft Import → AI Script Adaptation → Scene Segmentation → Character Casting → AI TTS → Audio Mixing → Quality Control → Object Storage → CDN → Publication**

Object storage may also hold source imports and intermediate artifacts earlier in the flow. The final storage/CDN stages concern verified deliverables. Upload does not grant publication approval or public access.

| Stage | Input and durable output | Gate or recovery behavior |
| --- | --- | --- |
| Import | TXT, Markdown, DOCX or structured script → preserved source, import metadata and parsed draft | Validate size/type, safely parse files, record provenance and report parsing failures |
| Adapt | Draft → proposed versioned Script IR with dialogue, speakers, emotions and cues | Preserve source references; require script review and applicable source rights |
| Segment | Script IR → episodes, acts, scenes, ordered lines and sound-cue placement | Validate stable IDs, ordering, missing speakers and timing assumptions |
| Cast | Characters → resolved voice/model/profile and pronunciation versions | Review voice samples; validate provider capability and voice permissions |
| Synthesize | Approved revision + casting → per-dialogue immutable audio and usage records | Check budget; reuse valid cache; reconcile uncertain calls; retry eligible failures |
| Mix | Dialogue, cues and licensed sound assets → scene mixes and episode master | Align cues; apply configured levels, fades, loudness and format profile |
| QC | Master and script → technical results, editorial findings and approval records | Blocking findings prevent publication; corrections invalidate dependent output |
| Store | Approved deliverables → verified objects, checksums, durations and release manifest | Retry interrupted uploads; verify all referenced objects before marking ready |
| Deliver | Ready objects → configured CDN-accessible release assets | Keep previews/intermediates private; verify the actual delivery path |
| Publish | Ready manifest + valid rights/QC/approvals → active release | Commit publication metadata atomically; dispatch downstream updates durably |

## Freeze production inputs

Before synthesis, create a production run that references an immutable snapshot of:

- Script IR schema and script revision.
- Dialogue order, effective text and character identities.
- Provider, model/voice identifiers, synthesis parameters and adapter version.
- Resolved pronunciation and performance settings.
- Sound-asset versions, cue placement and mix/output profile.
- Required rights evidence, approval policy and production budget.

Editing the draft while a run is active must not change that run's inputs. A creator may start a replacement run against the revised script. Preserve the relationship between revisions, runs, attempts, artifacts and releases.

The bounded [#5 contract](../../contracts/production-v1.md) freezes a planning candidate before
any run exists. It pins one complete accepted export/c1/e1, resolved reference-only controls,
required claim versions and budget/rate policy under a separate production-input digest. A
blocked candidate may be preserved for inspection; a separate owner approval requires current
editorial review, rights and budget eligibility. Current head/settings/required-rights or
permission changes and term expiry make eligibility stale without mutating the document.
The catalog cannot dispatch or produce audio; no reservation or charge exists in this slice.
Claims/rates remain actor assertions rather than legal/provider verification. See
[ADR 0007](../decisions/0007-casting-production-inputs.md). The host-owned policy supersedes the
former local/open-source inference prerequisite; later adapters need explicit provider/rights/
spend authorization and cannot treat a planning approval as agent permission to pay.

The first provider adapter, Google Cloud TTS for vi-VN ([ADR 0009](../decisions/0009-google-cloud-tts-adapter.md),
proposed), checks declared capabilities before any send, classifies each observation with its
charge exposure and recovery action, and validates returned audio. Because Google documents no
request ID, idempotency key, lookup or billing rule for errors, every possibly billed failure
requires explicit resolution before a repeat. It is reachable only through an opt-in operator
command; no frozen production line can dispatch until casting gains Google capabilities with an
explicit emotion mapping and durable attempts/reservations exist.

## Cache and partial regeneration

Build a canonical input fingerprint for each dialogue synthesis request. It must include every effective setting that could alter the output, including provider/model changes and pronunciation rules. For providers whose model version cannot be pinned, record that limitation and provide an explicit refresh/invalidation mechanism.

An accepted artifact can be reused after validating its fingerprint, rights/access scope, checksum and availability. Provider attempts and accepted artifacts are different records: several attempts may exist, but a step selects only one accepted result.

Maintain dependency edges from dialogue to scene mix, scene mix to episode master, and master to release outputs. Changing one line invalidates that line and dependent output. Changing a shared voice/profile invalidates its dependent lines. Changing only a scene sound cue can reuse synthesized dialogue while rerunning the relevant mix. Never invalidate unrelated dialogue just because a new episode render is needed.

Store raw provider output where allowed, normalized dialogue artifacts, scene mixes and final deliverables separately. Cache policy and retention must honor provider restrictions and the creator's rights.

## Jobs and failure recovery

Use PostgreSQL as the authoritative record of production runs, steps, attempts, usage and publication state. An initial design can use database-backed workers; a separate queue is optional.

Suggested run states are `queued`, `running`, `waiting_review`, `paused`, `failed`, `cancelled` and `completed`. Suggested step states are `pending`, `leased`, `succeeded`, `failed` and `cancelled`. Keep editorial/publication status separate from worker execution status.

- Claim eligible work transactionally with an expiring lease and owner/generation token. Use heartbeats and fencing checks so an expired worker cannot overwrite a newer worker's accepted result.
- Persist each stage's inputs and outputs. On restart, continue unfinished work and revalidate accepted artifacts rather than rerun the whole pipeline.
- Make dispatch and accepted-result writes idempotent. Use stable operation IDs and provider idempotency keys when supported.
- Classify failures. Use bounded exponential backoff with jitter for transient failures; stop on invalid input, missing rights, unsupported voices or exhausted budgets. Record a clear recovery action.
- A timeout may mean the provider already generated and charged for audio. Persist the provider job/request ID and attempt reconciliation before repeating paid work. Without provider idempotency or lookup, expose the uncertain outcome for explicit resolution.
- Cancellation prevents new work and final publication; already issued provider calls may still complete or incur cost. Record those outcomes and discard/select output according to policy.
- Use a transactional outbox when a committed database change must trigger queue dispatch, publication indexing or other external work. Consumers must tolerate repeat delivery.

Lease expiry, network retries and outbox delivery provide at-least-once execution. Correctness comes from idempotent acceptance and reconciliation; do not claim exactly-once third-party synthesis or billing.

## Audio processing and QC

Use a versioned audio profile covering sample rate, channels, master/delivery formats, loudness target, true-peak limit and timing policy. Select the actual numerical targets after testing representative drama content and intended platforms.

Mix in a format suitable for processing, then encode delivery formats. Keep speech intelligible over music and effects; apply explicit gain/ducking/fades and preserve deliberate pauses. Do not silently change dialogue text to fix audio processing problems.

Automated QC should detect missing/empty assets, corrupt output, unexpected durations, clipping/peak violations, profile mismatches and incomplete dialogue coverage. Editorial review should check speaker identity, pronunciation, performance, dialogue order, sound cues and the final listening experience. Any automated speech-alignment check must surface uncertainty rather than certify text fidelity without evidence.

QC results identify the exact artifact checksum and input revision they evaluated. A creator can preview lines, scenes and the final episode, correct inputs, and regenerate the affected graph. Approvals become stale when their evaluated inputs or outputs change.

## Upload and publication boundary

1. Upload immutable objects to a private staging namespace with checksums and required metadata.
2. Verify every final object and its availability through the intended delivery path. Assemble a release manifest containing asset references, checksums, codec/duration information and production provenance.
3. Recheck source/voice/sound rights, blocking QC findings and approvals against this exact release. Pending or stale approval blocks publication.
4. Commit the ready release and active-release pointer in a database transaction; include outbox events for discovery/index/cache updates.
5. Grant listener access through the configured publication mechanism. Keep delivery activation idempotent and recoverable if external updates fail. A release becomes listener-visible only when its complete assets are ready.

Database and object storage/CDN changes do not share a transaction. The implementation must choose and test a staged publication protocol with retries and reconciliation; toggling a database flag alone is insufficient if assets are not yet deliverable. CDN behavior, signed access and cache invalidation rules remain deployment decisions.

Republishing creates a replacement release and switches the active pointer after readiness checks. Retraction updates access and discovery, with a documented policy for CDN caches, existing links and offline downloads.

## Cost and operational visibility

Record estimates, reserved budget, provider-reported usage, actual/reconciled costs, cache hits, attempts, stage durations and failure reasons. Use these records to explain why a run paused or cost more than estimated. Redact credentials and sensitive source content from operational logs.

Useful operational checks include stalled leases, unresolved paid calls, outbox backlog, missing artifacts, budget overruns and releases whose metadata disagrees with delivery readiness. Set concrete alert thresholds after measuring the initial system.

## MVP proof points

Verify the pipeline using a small multi-character drama with narration, ambience and effects. The acceptance exercise must include a changed line, an interrupted worker, a provider failure/unknown outcome, an upload failure, stale QC approval and a replacement release. Confirm that Theatre can play the approved release and that repeated playback creates no production jobs.

Related: [product brief](brief.md) and [business rules](business-rules.md).
