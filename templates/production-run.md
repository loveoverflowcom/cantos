# Production run: [episode name] — [render revision]

- Status: [planned / running / blocked / ready for publication / completed]
- Run or job ID: [identifier]
- Owner: [person or team]
- Started / completed: [timestamps and timezone]
- Related issue: [link]

## Input identity

| Item | Identifier or immutable revision |
| --- | --- |
| Work | [ID and title] |
| Adaptation | [ID and revision] |
| Episode | [ID] |
| Source manuscript | [permitted reference and checksum] |
| Script IR | [schema version and script revision] |
| Casting / pronunciation profile | [revision] |
| Ambient, music, and sound-effect assets | [asset revisions and permission records] |

## Approved production configuration

- TTS providers and model versions: [configuration]
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
| Dialogue TTS | [state] | [reference] |
| Mixing and normalization | [state] | [reference] |
| Quality control | [state] | [reference] |
| Storage upload and delivery verification | [state] | [reference] |
| Publication approval | [state] | [reference] |
| Publication | [state] | [reference] |

## Generation, recovery, and costs

- Reused dialogue/scene assets: [IDs and cache evidence]
- Newly generated or regenerated segments: [IDs and reasons]
- Failed attempts and retry/resume events: [sanitized job evidence]
- Duplicate-generation/publication checks: [evidence]
- Actual provider usage and cost: [amount, currency, and measurement basis]
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
- Publication checklist: [link]
- Remaining blockers: [blockers or none]
