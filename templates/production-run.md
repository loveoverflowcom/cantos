# Production run: [episode name] — [render revision]

This is an external-producer record, not a Cantos API generation job. TTS, recording, mixing
and encoding run outside the API. Follow the [production pipeline](../docs/product/production-pipeline.md)
and retain private records under the [workspace contract](../docs/production/story-workspace.md).
Pin source links when copying outside Git. Leave unheard/unperformed checks NOT_RUN; use N/A
with a reason only for stages that do not apply to this narration, recording or drama.

- Status: [planned / running / blocked / ready for publication / completed]
- External producer / tool and version: [actual identity; manual recording if applicable]
- Producer run / attempt IDs: [existing identifiers; separate attempts, or absent with reason]
- Owner: [person or team]
- Started / completed: [timestamps and timezone]
- Related issue: [link]

## Input identity

| Item | Identifier or immutable revision |
| --- | --- |
| Work | [ID and title] |
| Adaptation | [ID and revision, or N/A with reason] |
| Episode | [ID] |
| Content / Production Brief | [exact revisions and digests, or not admitted to API] |
| Source manuscript | [permitted reference and checksum] |
| Script IR or recording plan | [exact revision/checksum; Script IR schema if used, otherwise N/A with reason] |
| Casting / pronunciation profile | [revision] |
| Proper-name map / frozen producer input manifest | [immutable revisions and byte checksums when used] |
| Ambient, music, and sound-effect assets | [asset revisions and permission records] |

Retain immutable inputs and outputs; changed bytes or guidance need a new revision/attempt
record, not replacement of an earlier result. Distinguish producer byte checksums from the
owning contract's canonical content/brief digests. Do not fabricate API IDs for offline drafts.

## Approved production configuration

- Audio source: [actual TTS provider/model, human recording or imported asset; provenance/rights]
- Character voice mappings: [reference to versioned mapping]
- Provider parameters and emotional controls: [configuration]
- Mixing configuration and approved audio profile: [revision and target measurements]
- Output formats: [codec/container settings]
- Budget, estimate, and approving owner: [amount, currency, and evidence]
- Required QC checks and approvals: [checks and responsible roles]

## Pipeline evidence

| Stage | State | Evidence or output reference |
| --- | --- | --- |
| Import and adaptation | [state] | [reference] |
| Segmentation and casting | [state] | [reference] |
| External TTS / recording / import | [state] | [reference] |
| Mixing and normalization | [state] | [reference] |
| Quality control | [state] | [reference] |
| Private draft save/sync and readback | [state] | [exact-byte evidence; not API admission] |
| Explicit API admission, when requested | [state] | [submission/rendition and receipt; not publication] |
| Publication approval | [state] | [reference] |
| Publication | [state] | [reference] |

## Generation, recovery, and costs

- Reused dialogue/scene assets: [IDs and cache evidence]
- Newly generated or regenerated segments: [IDs and reasons]
- Failed attempts and retry/resume events: [attempt/input identity, sanitized outcome and evidence]
- Unknown provider/upload outcome and reconciliation: [readback/status evidence and owner decision before replay]
- Duplicate-generation/admission/publication checks: [separate evidence; no blind paid retry]
- Actual usage and cost per attempt: [amount, currency, measurement basis; producer claim versus independent observation]
- Unresolved cost or usage entries: [entries or none]

## QC and final outputs

- Casting, pronunciation, dialogue completeness, and cue timing: [results and evidence]
- Loudness, peak, clipping, silence, and format checks against the approved profile: [measurements]
- Listening protocol and review record: [use the shared
  [protocol](../docs/production/listening-review-protocol.md) and
  [template](listening-review.md); immutable private record path/checksum, named reviewer/date,
  exact played filename/SHA-256, input revision and profile revision when present]
- Listening coverage and editorial decision: [both headphones and phone speaker; transcript-free
  pass then transcript pass, heard ranges, dimension results, blocking/advisory findings,
  overall decision and actual owner threshold decision; NOT_RUN for unheard checks]
- Script-to-render correspondence: [revision verification]
- Output assets and checksums: [stable internal references; no credentials or signed URLs]
- Explicit handoff: [selected exact bytes; owning API contract/version; prepared submission,
  producer PUT outcome and immutable receipt/readback, or NOT_RUN with next action]
- Publication checklist: [link]
- Remaining blockers: [blockers or none]
