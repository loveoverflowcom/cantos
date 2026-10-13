# Accept externally produced masters, review and publish

> Active scope from [ADR 0018](../decisions/0018-external-audio-production.md).
> Cantos stores and verifies completed audio. Mix, resampling, mastering and encoding
> happen **outside Cantos** in AI plugins, producer tools or human production software.

## Current narrow implementation

[ADR 0020](../decisions/0020-studio-private-audio-intake.md) implements owner-session,
script-revision-bound private intake with four explicit Opus profiles and immutable receipts.
[Audio Lifecycle](../../packages/contracts/audio-lifecycle-v1.md) adds Content/Brief-bound
submissions, scoped upload permissions, admitted renditions, named review and private candidates.
[Private storage](../architecture/object-storage.md) adds filesystem/S3 adapters, durable original
targets and owner-authorized exact preview. The [read contract](../../packages/contracts/private-content-reads-v1.md)
and [storage evidence](../evidence/provider-independent-storage.md) identify actual behavior and
verification separately. Real-provider policies/performance, stronger rights/listening review,
approved publication and reference-aware retention remain their own acceptance boundaries.

Media owns private storage and preview; Publication must own approved immutable manifests and
atomic active-release changes. Prioritize actual private-provider acceptance and exact review
bindings before deployment, then #11's manifest/isolation/retention decisions. A private object,
candidate or preview grant is never a publication witness. One wholly private bucket is enough
for intake; future delivery needs reviewed policy/physical isolation and must not widen access
to private drafts.

## Why

Theatre needs an immutable playable release. Studio must let a creator inspect a submitted
recording, its metadata, format/profile, rights and QC results before publishing it, while
broken uploads and rejected replacements leave the previous release playable.

## Scope

1. Define a versioned audio profile (accepted codecs, container, channels, max duration/size,
   streaming/seek behavior, allowed low-bandwidth variants) and a shared external-producer QC
   checklist. See [#51](https://github.com/loveoverflowcom/cantos/issues/51) and
   [#64](https://github.com/loveoverflowcom/cantos/issues/64).
2. Accept private rendered files from external tools and verify object existence, actual
   checksum, size and media metadata. Do not implement an Axum TTS/mix/encoder worker.
3. Record independently identified evidence: producer-declared QC, server-inspected technical
   facts and named human listening review. Failed/stale findings block approval.
4. Authorize and atomically publish an immutable audio-release manifest referencing the
   exact accepted objects and rights records; record prior release pointers for rollback.
5. Make release artifacts streamable with HTTP byte-range or equivalent CDN support, with
   bounded signed-link lifetime and no access to private staging objects.

## Acceptance criteria

- External producer can upload an approved-format, legitimate audio file and reopen the
  same immutable receipt after restart.
- Partial, corrupt, mismatched, unauthorized, stale, or private-only uploads cannot publish.
- Subjective audio quality is not inferred from a valid WAV structure or declared metrics.
- Publishing a replacement succeeds only when its entire manifest and all grants are valid;
  a failure preserves the old playable release.
- Theatre can stream, seek and download only eligible published objects; repeated play never
  invokes synthesis or audio processing.
- Producer-origin cost/voice/model details are provenance, not Cantos charges/measurements.

## Non-goals

Building a digital audio workstation, direct Google/eSpeak/VieNeu adapters, waveform mixing
or automatic AI generation on the backend. AI producer skills may describe those tasks, but
they execute with the independent agent.

## GitHub execution

[#9](https://github.com/loveoverflowcom/cantos/issues/9) — externally supplied mix/QC review;
[#10](https://github.com/loveoverflowcom/cantos/issues/10) — verified storage and delivery;
[#11](https://github.com/loveoverflowcom/cantos/issues/11) — atomic publication.
Existing [WAV inspector evidence](../evidence/wav-pcm-inspection.md) remains a limited pure
inspection tool, not an entire audio-format/QC solution.
