# Studio Production Pipeline

> Status: target design. Bounded source import and caller-owned adaptation tools/review have
> [implementation evidence](../evidence/ai-script-adaptation.md). Generation belongs to the
> user-controlled Gemini/ChatGPT/Codex host; Cantos validates and persists submitted proposals.
> [Casting/input authorization](../evidence/casting-production-inputs.md) implements
> settings, script-scoped rights/budget declarations, immutable candidates and exact approvals.
> Audio follows the same pattern ([ADR 0018](../decisions/0018-external-audio-production.md)):
> Cantos does not synthesize speech. An external AI producer reads the approved snapshot and the
> published standards, produces audio and submits it through the API. The submission API, storage,
> mixing, QC records, delivery and publication below remain implementation requirements; the
> former in-process TTS runtime was removed and survives only as history.

## End-to-end flow

**Draft Import → AI Script Adaptation → Scene Segmentation → Character Casting → Production Snapshot → External Audio Production → Audio Submission → Quality Control → Object Storage → CDN → Publication**

Object storage may also hold source imports and intermediate artifacts earlier in the flow. The final storage/CDN stages concern verified deliverables. Upload does not grant publication approval or public access.

| Stage | Input and durable output | Gate or recovery behavior |
| --- | --- | --- |
| Import | TXT, Markdown, DOCX or structured script → preserved source, import metadata and parsed draft | Validate size/type, safely parse files, record provenance and report parsing failures |
| Adapt | Draft → proposed versioned Script IR with dialogue, speakers, emotions and cues | Preserve source references; require script review and applicable source rights |
| Segment | Script IR → episodes, acts, scenes, ordered lines and sound-cue placement | Validate stable IDs, ordering, missing speakers and timing assumptions |
| Cast | Characters → resolved voice/model/profile and pronunciation versions | Review voice samples; validate provider capability and voice permissions |
| Produce (external) | Approved snapshot + standards → audio made by a producer outside Cantos | The producer owns provider accounts, spend, retries and its own mixing; Cantos neither runs nor monitors it |
| Submit | Producer audio + provenance claim → inspected, checksummed submission bound to revision, snapshot and dialogue IDs | Reject unreadable, mismatched, stale or oversized audio; repeated uploads are idempotent; a submission is not an approval |
| QC | Submitted audio and script → technical results, editorial findings and approval records | Blocking findings prevent publication; corrections invalidate dependent output |
| Store | Approved deliverables → verified objects, checksums, durations and release manifest | Retry interrupted uploads; verify all referenced objects before marking ready |
| Deliver | Ready objects → configured CDN-accessible release assets | Keep previews/intermediates private; verify the actual delivery path |
| Publish | Ready manifest + valid rights/QC/approvals → active release | Commit publication metadata atomically; dispatch downstream updates durably |

## Freeze production inputs

Before a producer starts, freeze a production snapshot: an immutable record of

- Script IR schema and script revision.
- Dialogue order, effective text and character identities.
- Voice and model identifiers, delivery parameters and catalog version the producer is asked to use.
- Resolved pronunciation and performance settings.
- Sound-asset versions, cue placement and mix/output profile.
- Required rights evidence, approval policy and production budget.

Editing the draft must not change a frozen snapshot. A creator may freeze a replacement against the revised script. Preserve the relationship between revisions, snapshots, submissions and releases.

The bounded [#5 contract](../../packages/contracts/production-v1.md) freezes a planning candidate
before any audio exists. It pins one complete accepted export/c1/e1, resolved controls, required
claim versions and budget/rate policy under a separate production-input digest. A blocked candidate
may be preserved for inspection; a separate owner approval requires current editorial review,
rights and budget eligibility. Current head/settings/required-rights or permission changes and term
expiry make eligibility stale without mutating the document. The catalog cannot dispatch or
produce audio; no reservation or charge exists. Claims/rates remain actor assertions rather than
legal/provider verification. See [ADR 0007](../decisions/0007-casting-production-inputs.md),
narrowed to guidance by ADR 0018. An approved snapshot is what a producer reads: it grants a
producer no permission to spend and no publication authority.

## Submission identity and partial regeneration

A submission is identified by the script revision, snapshot digest, dialogue ID and the checksum
Cantos measures on the submitted bytes. It records the producer's claim (tool, model or voice,
version, rights assertion) and any unpinned-model uncertainty as stated by the producer.

A submitted result can be accepted only after Cantos re-measures it and checks its rights/access
scope. Several submissions may exist for a dialogue; exactly one is the accepted result, chosen by a
recorded decision. Accepted submissions are immutable.

Maintain dependency edges from dialogue to scene mix, scene mix to episode master, and master to release outputs. Changing one line makes that line's submission and dependent output stale. Changing a shared voice/profile makes its dependent lines stale. Changing only a scene sound cue can keep accepted dialogue while the relevant mix is redone. Never invalidate unrelated dialogue just because a new episode render is needed.

Keep producer-supplied raw output, normalized dialogue artifacts, scene mixes and final deliverables separate. Retention must honor provider restrictions and the creator's rights.

## Submission and failure recovery

Use PostgreSQL as the authoritative record of snapshots, submissions, inspection results and
publication state. Cantos keeps no generation job state: a producer's own queue, retries and
provider calls are outside it.

- Make submission idempotent with a producer-chosen operation ID: a repeat returns the prior result, and a changed payload under the same ID conflicts.
- Upload to a private staging namespace, then verify size, checksum and structure before the submission exists; a partial upload never becomes a submission.
- Reject, or mark stale, a submission for a superseded revision or snapshot instead of attaching it to the current one.
- Classify rejections (unreadable, wrong format, mismatched checksum, over limit, stale) with a clear next action for the producer.
- Use a transactional outbox when a committed database change must trigger publication indexing or other external work. Consumers must tolerate repeat delivery.

Cantos cannot know what a producer's provider did or charged, and must not claim to. It records what
the producer states and what Cantos measured, and keeps the two apart.

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

Producer-reported cost and usage, if supplied, are recorded as claims and never presented as settled charges. Cantos itself incurs no generation cost. Record submissions, rejections and their reasons, stage durations and publication state, and redact credentials and sensitive source content from operational logs.

Useful operational checks include stale or rejected submissions, outbox backlog, missing artifacts and releases whose metadata disagrees with delivery readiness. Set concrete alert thresholds after measuring the initial system.

## MVP proof points

Verify the pipeline using a small multi-character drama with narration, ambience and effects. The acceptance exercise must include a changed line, an interrupted or repeated upload, a submission for a superseded revision, an upload failure, stale QC approval and a replacement release. Confirm that Theatre can play the approved release and that repeated playback creates no production jobs.

Related: [product brief](brief.md) and [business rules](business-rules.md).
