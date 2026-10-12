# GitHub planning and execution

[Cantos Project](https://github.com/users/loveoverflowcom/projects/6) · [Kanban](https://github.com/users/loveoverflowcom/projects/6/views/1) · [Planning table](https://github.com/users/loveoverflowcom/projects/6/views/2) · [Repository issues](https://github.com/loveoverflowcom/cantos/issues) · [Milestones](https://github.com/loveoverflowcom/cantos/milestones)

The repository currently contains specifications and templates. These issues plan implementation; creating the board does not establish application, audio or mobile runtime behavior. The Project is private and requires the owner or an authorized collaborator to sign in. Repository documents and issues remain available under their own repository access policy.

## Find the documents

- [Documentation map](../README.md): product, business rules, pipeline, architecture, Script IR, mobile and design.
- [MVP work queue](README.md): capability scope, prerequisites and phase review boundaries.
- [Reusable templates](../../templates/README.md): feature briefs, ADRs, production runs and publication checklists.
- The `Spec` field on each Project item links to its owning Markdown specification. Each issue links additional relevant documents.

Keep specifications in Git, linked to executable issues rather than copied into draft Project cards. Update the owning document when a decision or implemented behavior changes.

## Use the two views

| View | Purpose | Saved organization |
| --- | --- | --- |
| [Kanban](https://github.com/users/loveoverflowcom/projects/6/views/1) | Track execution and the next ready task | Columns by Status; Order ascending |
| [Planning](https://github.com/users/loveoverflowcom/projects/6/views/2) | Review scope, phase, dependencies and source documents | Group by Phase; Order ascending |

`Order` is the recommended reading/scheduling sequence, not a permanent identifier or a substitute for dependency acceptance. Issue numbers identify execution tasks; numeric specification prefixes can still be reorganized. Filter by Phase or Milestone to focus on one capability.

## Status policy

| Status | Meaning |
| --- | --- |
| Backlog | Planned work whose prerequisites or readiness decisions are pending |
| Ready | Immediate dependencies are accepted and the bounded task can start |
| In progress | Implementation has actually started |
| In review | Focused PR and relevant verification evidence are available |
| Done | Acceptance criteria and applicable capability review gate passed |

At planning setup, only [#1](https://github.com/loveoverflowcom/cantos/issues/1) is Ready; later work waits in Backlog. The live Project owns current Status, issues own acceptance checklists/evidence and native `blocked by` links, and Markdown owns intended scope, rationale and recommended order. This document is not a second live status board.

If an active task becomes blocked, record the reason and native blocking relationship on the issue. Move it back to Backlog when it cannot proceed, preserving its progress and explaining the next action. Move a dependent task to Ready only after its prerequisites are accepted. No deadline, estimate or assignee is implied by this ordering.

## Ordered implementation issues

| Order | Phase / specification | Issue and outcome | Immediate blockers |
| --- | --- | --- | --- |
| 01 | [010](010-import-and-edit-script.md) | [010.1 — Establish authenticated script persistence and contracts](https://github.com/loveoverflowcom/cantos/issues/1) | None; start here |
| 02 | [010](010-import-and-edit-script.md) | [010.2 — Import manuscripts with preserved source and rights](https://github.com/loveoverflowcom/cantos/issues/2) | [#1](https://github.com/loveoverflowcom/cantos/issues/1) |
| 03 | [010](010-import-and-edit-script.md) | [010.3 — Generate reviewable AI script adaptations](https://github.com/loveoverflowcom/cantos/issues/3) | [#2](https://github.com/loveoverflowcom/cantos/issues/2) |
| 04 | [010](010-import-and-edit-script.md) | [010.4 — Deliver the accessible Studio script editor](https://github.com/loveoverflowcom/cantos/issues/4) | [#3](https://github.com/loveoverflowcom/cantos/issues/3) |
| 05 | [020](020-cast-and-generate-dialogue.md) | [020.1 — Freeze casting and authorize production inputs](https://github.com/loveoverflowcom/cantos/issues/5) | [#4](https://github.com/loveoverflowcom/cantos/issues/4) |
| 06 | [020](020-cast-and-generate-dialogue.md) | [020.2 — Integrate verified local and commercial TTS](https://github.com/loveoverflowcom/cantos/issues/6) *(superseded by ADR 0018; re-scope below)* | [#5](https://github.com/loveoverflowcom/cantos/issues/5) |
| 07 | [020](020-cast-and-generate-dialogue.md) | [020.3 — Recover durable TTS jobs and reuse valid clips](https://github.com/loveoverflowcom/cantos/issues/7) *(superseded by ADR 0018; re-scope below)* | [#6](https://github.com/loveoverflowcom/cantos/issues/6) |
| 08 | [020](020-cast-and-generate-dialogue.md) | [020.4 — Expose casting, production progress and partial regeneration](https://github.com/loveoverflowcom/cantos/issues/8) *(re-scope below)* | [#7](https://github.com/loveoverflowcom/cantos/issues/7) |
| 09 | [030](030-mix-review-and-publish.md) | [030.1 — Mix episodes and review versioned QC findings](https://github.com/loveoverflowcom/cantos/issues/9) *(re-scope below)* | [#7](https://github.com/loveoverflowcom/cantos/issues/7) |
| 10 | [030](030-mix-review-and-publish.md) | [030.2 — Stage private assets and verify media delivery](https://github.com/loveoverflowcom/cantos/issues/10) | [#7](https://github.com/loveoverflowcom/cantos/issues/7) |
| 11 | [030](030-mix-review-and-publish.md) | [030.3 — Publish approved immutable releases atomically](https://github.com/loveoverflowcom/cantos/issues/11) | [#8](https://github.com/loveoverflowcom/cantos/issues/8), [#9](https://github.com/loveoverflowcom/cantos/issues/9), [#10](https://github.com/loveoverflowcom/cantos/issues/10) |
| 12 | [040](040-theatre-web-listening.md) | [040.1 — Implement listener APIs and conflict-safe progress](https://github.com/loveoverflowcom/cantos/issues/12) | [#11](https://github.com/loveoverflowcom/cantos/issues/11) |
| 13 | [040](040-theatre-web-listening.md) | [040.2 — Deliver Theatre Web discovery, streaming and resume](https://github.com/loveoverflowcom/cantos/issues/13) | [#12](https://github.com/loveoverflowcom/cantos/issues/12) |
| 14 | [050](050-cmp-native-listening.md) | [050.1 — Bootstrap CMP Theatre and native foreground streaming](https://github.com/loveoverflowcom/cantos/issues/14) | [#12](https://github.com/loveoverflowcom/cantos/issues/12) |
| 15 | [050](050-cmp-native-listening.md) | [050.2 — Add background playback, OS controls and interruption recovery](https://github.com/loveoverflowcom/cantos/issues/15) | [#14](https://github.com/loveoverflowcom/cantos/issues/14) |
| 16 | [050](050-cmp-native-listening.md) | [050.3 — Deliver verified and resumable offline downloads](https://github.com/loveoverflowcom/cantos/issues/16) | [#14](https://github.com/loveoverflowcom/cantos/issues/14) |
| 17 | [050](050-cmp-native-listening.md) | [050.4 — Synchronize mobile progress and verify the complete MVP journey](https://github.com/loveoverflowcom/cantos/issues/17) | [#13](https://github.com/loveoverflowcom/cantos/issues/13), [#15](https://github.com/loveoverflowcom/cantos/issues/15), [#16](https://github.com/loveoverflowcom/cantos/issues/16) |

## Re-scoped by ADR 0018

[ADR 0018](../decisions/0018-external-audio-production.md) (2026-10-12) removed the in-process TTS
runtime: Cantos manages works and audio, while external AI producers follow its instructions and
submit audio through the API. The GitHub issues below still carry their original text; this table
is the **proposed** rewrite, to be applied to the issues once the owner confirms. Until then the
issue body is not an accurate scope statement for these rows.

| Issue | Original scope | Proposed scope | Immediate blocker |
| --- | --- | --- | --- |
| [#6](https://github.com/loveoverflowcom/cantos/issues/6) | Integrate verified local and commercial TTS | Close as superseded: provider adapters live in external producers. The producer instructions and standards move to the new item below | None |
| [#7](https://github.com/loveoverflowcom/cantos/issues/7) | Recover durable TTS jobs and reuse valid clips | Replace with: audio submission API (scoped producer credentials, idempotent upload bound to revision/snapshot/dialogue, WAV inspection, stale marking) | [#5](https://github.com/loveoverflowcom/cantos/issues/5), [#10](https://github.com/loveoverflowcom/cantos/issues/10) storage decision |
| [#8](https://github.com/loveoverflowcom/cantos/issues/8) | Casting, production progress and partial regeneration | Studio submission view: per-dialogue status, preview, accept/replace, stale marks | The #7 replacement |
| [#9](https://github.com/loveoverflowcom/cantos/issues/9) | Mix episodes and review versioned QC findings | Accept and review submitted scene/episode audio: measured loudness/peak/duration, versioned QC findings; no mixing in Cantos | The #7 replacement |
| [#51](https://github.com/loveoverflowcom/cantos/issues/51) | Decide the versioned audio profile | Unchanged intent, now the delivery profile producers must meet | [#44](https://github.com/loveoverflowcom/cantos/issues/44), [#45](https://github.com/loveoverflowcom/cantos/issues/45), [#46](https://github.com/loveoverflowcom/cantos/issues/46) |
| [#52](https://github.com/loveoverflowcom/cantos/issues/52) | Condition TTS clips before mixing | Becomes a producer standard (padding, levels, joins) plus an optional measurement check, not a Cantos planner | [#51](https://github.com/loveoverflowcom/cantos/issues/51) |
| [#53](https://github.com/loveoverflowcom/cantos/issues/53) | Scene acoustics and stereo placement in the mix plan | Becomes a producer standard; Cantos measures, it does not render | [#9](https://github.com/loveoverflowcom/cantos/issues/9) |
| [#50](https://github.com/loveoverflowcom/cantos/issues/50) | Voice auditions and pronunciation lexicon | Unchanged; auditions are run against external producers | [#44](https://github.com/loveoverflowcom/cantos/issues/44), [#45](https://github.com/loveoverflowcom/cantos/issues/45) |
| [#64](https://github.com/loveoverflowcom/cantos/issues/64) | Low-bandwidth audio presets (Opus, AAC fallback) | Needs an owner decision on who encodes delivery renditions (the producer, or Cantos at publication). ADR 0018 rules out synthesis and mixing in Cantos but does not decide transcoding | None |

## Supplementary audio-quality issues

The [audio drama quality plan](audio-drama-quality.md) adds these bounded issues outside the
17-issue MVP order above. They are not Project items unless the owner adds them; their native
`blocked by` links record the dependencies. None closes or unblocks an MVP issue by itself.

| Step | Issue and outcome | Immediate blockers |
| --- | --- | --- |
| 1 | [Author an original Vietnamese reference scene for audio evaluation](https://github.com/loveoverflowcom/cantos/issues/44) | None |
| 2 | [Define the listening-review protocol for audio-listened evidence](https://github.com/loveoverflowcom/cantos/issues/45) | None |
| 3 | [Build a rights-recorded sound asset library](https://github.com/loveoverflowcom/cantos/issues/46) | None |
| 4 | [Add story sound bible and per-episode cue sheet records](https://github.com/loveoverflowcom/cantos/issues/47) | None |
| 5 | [Apply ear-first craft checks in radio adaptation and story audit](https://github.com/loveoverflowcom/cantos/issues/48) | None |
| 6 | [Decide Script IR controls for pauses, voice mode, overlap and cue extent](https://github.com/loveoverflowcom/cantos/issues/49) | None |
| 7 | [Run voice auditions and build a pronunciation lexicon](https://github.com/loveoverflowcom/cantos/issues/50) | [#44](https://github.com/loveoverflowcom/cantos/issues/44), [#45](https://github.com/loveoverflowcom/cantos/issues/45) |
| 8 | [Decide the versioned audio profile (groundwork for #9)](https://github.com/loveoverflowcom/cantos/issues/51) | [#44](https://github.com/loveoverflowcom/cantos/issues/44), [#45](https://github.com/loveoverflowcom/cantos/issues/45), [#46](https://github.com/loveoverflowcom/cantos/issues/46) |
| 9 | [Condition TTS clips before mixing (groundwork for #9)](https://github.com/loveoverflowcom/cantos/issues/52) | [#51](https://github.com/loveoverflowcom/cantos/issues/51) |
| 10 | [Add scene acoustics and stereo placement to the mix plan](https://github.com/loveoverflowcom/cantos/issues/53) | [#9](https://github.com/loveoverflowcom/cantos/issues/9), [#47](https://github.com/loveoverflowcom/cantos/issues/47), [#51](https://github.com/loveoverflowcom/cantos/issues/51) |

## Sequence and parallel work

Start with [#1](https://github.com/loveoverflowcom/cantos/issues/1), then complete the import/adaptation/editor journey before casting. Casting establishes the inputs an external producer needs; the submission API then brings audio back in.

After the audio submission API (the #7 re-scope above), the Studio submission view ([#8](https://github.com/loveoverflowcom/cantos/issues/8)), mixing/QC ([#9](https://github.com/loveoverflowcom/cantos/issues/9)) and private storage/delivery ([#10](https://github.com/loveoverflowcom/cantos/issues/10)) can proceed in parallel against concrete clip/artifact contracts. Publication ([#11](https://github.com/loveoverflowcom/cantos/issues/11)) waits for all three acceptance results.

Listener contracts ([#12](https://github.com/loveoverflowcom/cantos/issues/12)) require a complete publication/delivery gate. Theatre Web ([#13](https://github.com/loveoverflowcom/cantos/issues/13)) and native CMP streaming ([#14](https://github.com/loveoverflowcom/cantos/issues/14)) can then proceed in parallel. Native background controls ([#15](https://github.com/loveoverflowcom/cantos/issues/15)) and downloads ([#16](https://github.com/loveoverflowcom/cantos/issues/16)) can run in parallel after native foreground playback. The final cross-device journey ([#17](https://github.com/loveoverflowcom/cantos/issues/17)) waits for Web, background and download acceptance. Contract-fixture experiments before those gates must be labeled as prototypes.

## Phase exit and review evidence

A milestone is complete only when its issues pass acceptance and the owning phase's end-to-end review boundary is demonstrated. Completing a backend task alone does not finish its creator/listener capability.

- **010:** import → adaptation review → edit → save → restart/reopen, with preserved provenance, rights and immutable versions.
- **020:** casting → external producer reads the approved snapshot → audio submission → inspection → clip preview → single-line change and stale marking, including interrupted and repeated uploads.
- **030:** producer-submitted multi-voice scene and episode audio → measured and editorial QC → verified upload → exact-revision approval → atomic publication/replacement, preserving a playable prior release on failure.
- **040:** real discovery/CDN playback → bookmark → reload/resume → two-session synchronization, including deliberate rewind and release-version changes.
- **050:** Android and iOS native play → background/lock-screen → verified offline play → restart/resume → Web/mobile sync, with platform lifecycle and accessibility observations.

Branch from `develop`, keep PRs focused and target `develop`. Attach demonstrated journeys, relevant contract/migration changes, targeted failure checks and known limitations. Distinguish live evidence from mocks. A document check cannot establish a successful app build, real audio quality or native runtime behavior.

Keep [deferred expansion](backlog/deferred-expansion.md) outside the MVP board until evidence or an explicit product decision justifies promotion. Add a new issue with its own acceptance and dependency review instead of silently expanding an active task.
