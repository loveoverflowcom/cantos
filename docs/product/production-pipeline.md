# External Audio Production Handoff

> Direction adopted 2026-10-12; the [handoff v1 design](../../packages/contracts/external-audio-handoff-v1.md)
> implements private intake/review through [Audio Lifecycle v1](../../packages/contracts/audio-lifecycle-v1.md); public release remains proposed, while [Production Brief v1](../../packages/contracts/production-brief-v1.md)
implements the bounded instructions/read-permission subset. Cantos is the content and asset authority; it is not an
> audio generator, mixer, model router, or provider worker.

## End-to-end flow

```text
Cantos Studio
  Work → Chapter/Episode → Text/Script (optional) → Versioned brief + rights
                      ↓ GET production instructions and skill references
External AI plugin / human producer
  Choose tools → TTS/recording → voice edits + music/effects → mix → encode → QC
                      ↓ POST upload intent → object-store PUT → POST finalize
Cantos API + Studio
  Verify private asset + checksum + metadata → editorial/QC review → approve
                      ↓ atomic release
Cantos Theatre
  Discover → stream/download → listen/resume
```

The producer, not Cantos, handles provider credentials, rendering hardware, timeout retries,
call ambiguity, inference budgets, local caches, mixing and audio encoding.

## Production instructions

A read-only, versioned brief should contain: work/content identity; work type; required
language and target audience; text/script revision or source pointer; scene/speaker/pronunciation
and music/ambience guidance **if relevant**; declared permissions and allowed uses;
audio profile/version; accessibility/text availability; QC checklist; and export/upload rules.
Briefs can link to repository `.agents/skills` or export self-contained instructions.
The digest/version must be pinned and included in the producer submission receipt.

A novel's chapter can request plain voice narration and no mix. A podcast episode can be a
human recording. Nothing requires a Script IR transformation where one is not needed.

## External upload

Implement only after publishing a versioned schema and authenticated routes:

1. **Prepare** an upload intent for one content unit, brief version, rendition type and operation
   ID. Authorize a producer without leaking the Studio cookie or provider credentials.
2. **Transfer** bytes to a private object-store key with an expiring, constrained upload grant.
   Do not proxy unbounded audio bytes through Axum or save to a public bucket.
3. **Finalize** by submitting the intent ID, object identity, expected SHA-256, size, actual
   format/profile/duration, externally declared tools/usage and QC evidence. The server verifies
   actual object facts and marks the immutable rendition as pending review, not published.
4. **Review** provenance, rights, playback/QC and stale-brief policy, with separate editorial
   approval bound to the exact object checksum.
5. **Publish** a complete immutable release manifest atomically after readiness checks; use CDN
   only for approved artifacts.

Duplicates and out-of-order transfers must be idempotent; an expired grant may be renewed
without assigning an asset to another work/episode. Unsuccessful or orphaned uploads need a
bounded private cleanup/reconciliation policy. Avoid a database transaction that assumes it
is atomic with an object-store upload.

## Audio quality

External tools do compression, loudness normalization, music mixing and conversion. Cantos
defines the profile and QC evidence shape, may inspect received bytes under limits, and enforces
review/publication gates. The pure WAV inspector is a useful primitive; it is **not** a mixer,
codec encoder, or a loudness/true-peak approval. Proposed low-bandwidth Opus and AAC settings
remain a separate decision tracked by [#64](https://github.com/loveoverflowcom/cantos/issues/64)
and [#51](https://github.com/loveoverflowcom/cantos/issues/51).

## Migration from the former worker design

Earlier `synthesis`, `cantos-worker`, `production-jobs`, provider probes, TTS receipts and
DBSP job-summary adapters belonged to server-side generation. They have been retired from the
active backend, not repurposed into a hidden provider executor. Historical migrations 0007–0009
are retained to preserve migration checksums and existing databases; they do not authorize
writing new generation jobs. Prior experiments/ADRs are retained as historical evidence.

## Not yet implemented

Production identity/credential delivery, object-store prepare/finalize, stronger rights/QC policy and public release publication. Bounded development upload capabilities, actual-byte private admission and named review/candidates are implemented; they use one capped PUT with finalization and private filesystem storage. Catalog/Content and
versioned Brief export with development read grants are implemented independently.
Existing Studio `production` endpoints manage older planning metadata; they **do not**
provide an external audio upload API.
