---
name: cantos-audio-produce
description: >-
  Produce a Cantos radio-drama episode from a reviewed score folder: resolve casting and sound
  cues, synthesize supported dialogue, mix final audio, record QC and synchronize the Drive
  workspace. Use for an artifact production run or partial regeneration; use the production
  pipeline owner for implementing worker/backend code. Never implies publication.
---

# Cantos audio production

An artifact workflow entrypoint composing [engineering](../cantos-engineering/SKILL.md),
[production pipeline](../cantos-production-pipeline/SKILL.md),
[publication](../cantos-publication/SKILL.md) and [Script IR](../cantos-script-ir/SKILL.md).
Read [story workspace](../../../docs/production/story-workspace.md) for all folder, index,
sync and offline handoff rules, and [radio score](../../../docs/production/radio-score.md).

## Run from a pinned score

1. Resolve the story by its existing identity/index and read pending operations. Pin the exact
   reviewed `score/*.script-ir.json` revision, its pinned `name_map.rNNNN.yml` checksum,
   byte/content digests, casting, pronunciation,
   voice/model revisions, cue assets/rights and output/mix profile. Raw source is never fed straight
   to TTS when an adaptation is required. An unreviewed/unresolved score is a blocker.
2. Inspect actual executables/provider adapters before selecting commands. Production workers are
   still planned; a manual local run must be labeled as such and must not claim durable backend
   jobs, automatic acceptance or a production API exists. Use the
   [production-run template](../../../templates/production-run.md) for the run evidence.
3. Reuse installed TTS/audio tools or install a necessary maintained package in an isolated
   environment under the runtime's permission rules. Confirm voice/model licensing, Vietnamese
   support, available emotion/pronunciation controls and actual cost. A tool described as free
   does not prove every hosted endpoint or voice is free. For a paid call, use the user's actual
   budget and approved destination; ask only for missing consequential cost/scope.
4. Apply the owner's [provider protocol](../cantos-production-pipeline/references/provider-adapters.md):
   persist intent/request ID and frozen inputs before a call; record response or ambiguity before
   retrying; reconcile uncertain attempts. Send only spoken `text` and mapped supported controls.
   Music, effects, panting and ambience are cue/mix events, not text for a voice to read aloud.
   Unsupported required performance controls stay blocked or receive an explicitly recorded fallback.
5. Reuse eligible speech by the owner's
   [fingerprint rules](../cantos-production-pipeline/references/fingerprints-and-invalidation.md).
   A changed line regenerates its speech and dependent mixes; moving a line alone does not force
   new speech. Store intermediate speech/caches outside the final `audio/` handoff and outside Git.
6. Render the mix, then run the agreed
   [audio/QC checks](../cantos-production-pipeline/references/audio-mix-and-qc.md). Record tool
   version, profile, duration, codec/sample rate, clipping/loudness/peak where measured and cue
   timing. Mark listening review absent unless someone actually listened to the named render.
7. Place only completed master/delivery mixes and their run/QC records in `audio/` with immutable
   revision names. Update indexes and synchronize artifacts before pointers. Hand off checksums
   and remaining editorial/technical questions to `cantos-story-audit`.

A folder named `audio` or a completed upload is not a published release. Publication uses the
existing rights/approval rules and the separate application release mechanism when implemented.

## Report

Give the pinned inputs, cast/provider/tool and cost evidence, generated/reused/blocked lines,
final paths and hashes, measured versus listened evidence, sync IDs or pending recovery issue,
regeneration scope and residual risk. Use the foundation report; never claim real provider or
listening verification from mock output, silence-only fixtures or file existence.
