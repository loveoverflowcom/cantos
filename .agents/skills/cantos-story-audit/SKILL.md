---
name: cantos-story-audit
description: >-
  Perform a bounded artifact audit across a Cantos story's raw, score and audio folders, checking
  source coverage, dialogue adaptation, cue separation, audio evidence and Drive handoff integrity.
  Write checksum-bound findings and indexes, with offline issue recovery. Use for light story or
  radio-drama QA, not repository code review or UI inspection; audit does not rewrite or publish.
---

# Cantos story audit

An artifact workflow entrypoint, separate from [code review](../cantos-code-review/SKILL.md) and
[UI inspection](../cantos-ui-inspector/SKILL.md). Compose
[engineering](../cantos-engineering/SKILL.md) for evidence,
[Script IR](../cantos-script-ir/SKILL.md) for script validation,
[production pipeline](../cantos-production-pipeline/SKILL.md) for audio/QC and
[publication](../cantos-publication/SKILL.md) for rights and approval limits.
Read [story workspace](../../../docs/production/story-workspace.md) and
[radio score](../../../docs/production/radio-score.md) before starting.

## Bounded audit

1. Resolve work identity and pin raw/score/audio revisions and checksums from their indexes.
   State the scope and budget before inspection. Default to one selected episode; for a larger
   batch check every manifest/range, then sample opening/middle/ending scenes per episode and
   expand only where a finding justifies it. A sample pass is not a full-book or full-listening pass.
2. Source: verify requested chapter coverage/order, attribution, source mapping, missing/duplicate
   passages, HTML/OCR uncertainty and actual rights evidence. Recognize censored source text;
   do not repair it by guessing. Do not accept a generic website license footer as work clearance.
3. Score: compare plot facts, motivation and source coverage; check light adaptation has increased
   performable dialogue without inventing events or flattening character voices. Check narrator
   use, stable speakers/IDs, Vietnamese names, supported emotion values, cue kinds/anchors, and
   that directions never leaked into spoken text. Use the actual IR validation path when available;
   report schema checks separately from semantic/backend acceptance.
   Audit `score/name_map.yml` and its pinned revision: inventory all in-scope entity kinds,
   source aliases and ambiguous mentions; check consistent new names throughout titles, character
   descriptions, narration, dialogue and cue descriptions. Flag surviving original proper names,
   unmapped entities, distinct entities sharing a target name, drift from the pinned map, and
   accidental changes to common nouns or opaque IDs. Preserve source attribution/provenance.
   Sampling cannot establish exhaustive proper-name replacement; record unchecked scope.
4. Audio, if present: inspect run/QC records and whether their checksums match the actual files.
   Check casting and cue coverage against the pinned score. Measure with available tools and
   listen to the declared scope when playback/listening is available. Text inspection, waveform
   metadata and a generated QC report cannot substitute for listening evidence.
5. Sync: verify artifact IDs/parents/checksums, index pointers, stale approvals and unfinished
   operation records. A later file with the same name is not automatically the reviewed artifact.
6. Write an immutable `audit_<id>.rNNNN.md` beside the stage being audited (`score/` for pre-audio,
   `audio/` for a render audit). Each finding names severity, exact artifact/hash and location,
   observed evidence, impact, confidence and smallest next action. Distinguish blocking findings,
   suggestions and checks not performed. Update affected indexes and synchronize using the common
   contract; offline failures become a deduplicated issue or local issue draft.

The audit may write its report/index and authorized recovery issue; it does not mutate source,
rewrite the score, regenerate paid audio, approve publication or change Drive permissions.
A requested follow-up fix routes to the appropriate artifact skill with the same pinned findings.

## Report

Return audited scope, pass/fail of named checks, findings, report path/checksum, next responsible
skill and Drive sync/recovery status. Use `documented`, `example-tested`, `audio-measured` and
`audio-listened` only for evidence actually obtained. `audit_pass` means the named audit checks
passed for the exact revisions; list sampling and unavailable checks as residual risk.
