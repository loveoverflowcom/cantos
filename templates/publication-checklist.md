# Publication review: [episode name] — [render revision]

- Status: [pending / blocked / approved / published / withdrawn]
- Episode ID: [ID]
- Script revision and Script IR schema version: [versions if used, otherwise N/A with reason]
- Render revision: [version]
- Exact rendition / SHA-256 / content and brief digests: [immutable identities]
- Release candidate ID / digest: [exact reviewed candidate, or not yet created]
- Production run: [link]
- Release owner: [person or team]
- Intended publication time: [timestamp and timezone]
- Required approvers: [roles or people, according to the episode's approval policy]

Follow the [publication contract](../packages/contracts/publication-v1.md) and
[production pipeline](../docs/product/production-pipeline.md). Keep private evidence outside
Git; pin instruction links when copying. An approval, API admission and observed public release
are separate facts. Leave boxes unchecked until their named evidence exists; record unrun
checks explicitly and use N/A only with an applicable reason.

## Permissions and metadata

- [ ] Source and adaptation permission records cover this publication.
- [ ] Voice, music, ambience, and sound-effect permissions cover the published assets.
- [ ] Credits and required attribution are complete.
- [ ] Title, description, episode order, artwork, and applicable content notices are reviewed.
- [ ] Unresolved rights or permission questions are cleared.

Evidence: [links to permission and metadata review records]

## Content and audio QC

- [ ] Published audio corresponds to the approved script and casting revisions.
- [ ] Dialogue completeness, speaker consistency, pronunciation, and cue timing pass review.
- [ ] The final output meets the approved loudness, peak, silence, and encoding profile.
- [ ] A permitted final preview has been reviewed by the assigned reviewer.
- [ ] All blocking production/QC findings are resolved and verified against this release.

Evidence: [QC results, measurements, and preview approval]

## Pre-release delivery readiness

- [ ] Final assets are uploaded and their integrity is verified.
- [ ] Delivery URLs resolve through the intended storage/CDN configuration.
- [ ] Duration, seeking, playback speed, and listening-progress behavior are verified on supported implemented clients.
- [ ] Applicable native background playback, lock-screen controls, and download behavior are verified.
- [ ] Publication exposes only the intended revision and does not reveal private source or intermediate assets.

Evidence and supported clients checked: [results; mark unimplemented clients explicitly]

## Publication approval

- [ ] Required current rights/QC/editorial and publication approval records bind the exact candidate and revisions/checksums.
- [ ] Approved cost and actual production usage have been reconciled or an outstanding item has an owner.
- [ ] The release owner has recorded a correction or withdrawal path.
- [ ] The intended publication time and visibility are confirmed.

Changed content, brief, rendition, review, rights or metadata must be checked for approval
invalidation under the contract; never carry approval to different bytes by episode name alone.

## Explicit release action

- [ ] The authorized publication operation records the exact candidate, expected active release/pointer and durable receipt.
- New release ID / manifest digest / superseded release: [actual identities, or NOT_RUN]
- Operation ID / outcome / receipt: [actual result; reconcile an uncertain outcome before retry]

## Post-release verification

- [ ] The intended release is confirmed through listener catalog/manifest readback.
- [ ] The exact final asset plays through the intended delivery path without exposing private assets.
- Actual observation time / supported clients / evidence / remaining gaps: [results, or NOT_RUN]

| Approval or action | Responsible person | Decision or result | Evidence and timestamp |
| --- | --- | --- | --- |
| Rights review | [person] | [pending / approved / blocked] | [reference] |
| QC review | [person] | [pending / approved / blocked] | [reference] |
| Publication approval | [person] | [pending / approved / blocked] | [reference] |
| Publication action | [person] | [actual receipt/outcome or NOT_RUN] | [reference] |
| Publication verification | [person] | [pending / passed / failed] | [reference] |

## Corrections and recovery

- Previous published revision/release and immutable manifest, if any: [reference or N/A]
- Correction, replacement, or withdrawal steps: [documented procedure]
- CDN/cache and offline-download handling for a replacement: [procedure or N/A]
- Remaining limitations and follow-up issues: [links or none]

Do not mark the review approved while required permissions, QC, or approvals remain unresolved. Complete publication verification after release; approval and public availability are separate states.
