---
name: cantos-audio-produce
description: >-
  External producer workflow: read a pinned Cantos content brief and agent instructions,
  independently create/record/mix/encode audio, check the resulting file and submit it
  back via the authenticated external-audio handoff when implemented. Never run TTS
  in Cantos backend or treat an upload as publication.
---

# Cantos external audio producer

**Run this skill in the AI host/plugin or producer's workstation, never inside
`services/cantos-server`.** Cantos is a CMS, instruction source and publication authority.
[ADR 0018](../../../docs/decisions/0018-external-audio-production.md) and the
[handoff contract](../../../services/cantos-server/contracts/producer/handoff/v1/README.md) are authoritative.
The API contract is **proposed**, not yet implemented: do not fabricate endpoints, pretend a
file has been uploaded or silently bypass missing credentials.

## Pin the brief before producing

1. Identify the Cantos work (`novel`, `story` or `podcast`) and target chapter/episode.
   Confirm authorized access, content revision, rights, language and accepted output profile.
2. Fetch the **immutable version/digest** of the production brief through a supported Cantos API,
   once implemented. Until then read an explicitly exported creator-approved brief. Never read
   another creator's private text or transmit a paid provider request without permission.
3. Match the content form: plain narration for a chapter, recorded human speech for a podcast,
   or Script IR with multiple speakers, cues, effects and music for audio drama.
   Do not require Script IR for a non-dramatized recording.
4. Preserve optional voice maps, pronunciation lexicon, pauses, scene/cue timing and rights
   restrictions. If the selected provider cannot fulfill a required control, report a blocker
   or a human-approved fallback instead of silently substituting.

## Produce outside Cantos

5. Select local/commercial speech, recording, editing and mastering tools **in the host**.
   Record actual selected tool/model/voice versions and permissions where known; verify
   current licenses, supported Vietnamese voices and costs. Obtain explicit approval for
   spending or use of protected voice/audio assets.
6. Create or reuse private intermediate audio with a host-owned input fingerprint/cache.
   Cantos has no provider-attempt ledger or speech-clip worker. A timeout/ambiguous invoice
   is the producer's responsibility to reconcile before reissuing a paid call.
7. Mix/condition/encode in producer tools using the approved profile. Proposed Opus mono
   20/24/32/40 kbps or AAC fallback are **candidates**, not mandatory until a versioned
   profile is approved ([#64](https://github.com/loveoverflowcom/cantos/issues/64)).
8. Measure technical metadata with suitable local tools (codec, channel count, sample rate,
   duration, loudness, peaks, clipping) and record tool/version + actual outcomes.
   Keep `NOT_RUN` for unavailable checks. Use
   [audio mix & QC](../cantos-production-pipeline/references/audio-mix-and-qc.md);
   named listening review is separate from automated measurement.

## Submit and hand off

9. When the external upload API exists, acquire a **scoped producer grant** rather than
   sending Studio session cookies; prepare an upload for one exact episode/brief revision.
10. Transfer bytes privately to the authorized object-store grant; finalize with the
    expected SHA-256, size, media profile, immutable operation ID, producer provenance and
    any QC artifacts. Verify the returned server-validated receipt and immutable rendition ID.
    Never treat a success from the storage PUT alone as an accepted publication.
11. If the API is not available, deliver a local handoff bundle with audio file, hash,
    brief digest, actual media metadata, provenance and QC results; mark `UPLOAD_NOT_RUN`.
    Do not invent a successful server receipt or upload state.
12. The Cantos owner/editor separately reviews rights, QC and the rendition, then publishes
    an immutable release. An external producer **cannot self-certify** publication.

## Report

Give pinned work/content/brief IDs and hashes, rights scope, external production tool and
cost facts, finished audio paths/hash/format/duration, objective measurements, listened
evidence, actual or unrun upload/finalize result and all remaining blockers. Keep
`producer-declared`, `server-inspected` and `editor-approved` evidence distinct.
