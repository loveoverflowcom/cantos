# MVP work queue

Platform scope follows the [repository matrix](../architecture/repository-layout.md#platform-scope):
Studio deploys only to Web; Theatre supports Web and native Android/iOS. Studio Mobile is out of
scope / not implemented and must not be scaffolded as a dependency of any work item. Shared
libraries may serve Theatre Mobile and Studio Web; reopen Studio Mobile only on an explicit
future roadmap request. This scope clarification changes no capability acceptance or issue status.

The requested UI-only [Leptos UI system sequence](015-leptos-ui-system.md) delivers a small
canonical foundation before components, responsive app integration and independent review.
The [independent session 4 review](../evidence/ui-system-review.md) records scoped Safari fixes,
actual local interaction/reflow evidence and remaining accessibility/platform gates. The four
implementation sessions are complete. The subsequent requester-authorized publication is
[PR #33](https://github.com/loveoverflowcom/cantos/pull/33), with
[combined verification evidence](../evidence/ui-system-publication.md). Additional UI/CMP work
needs a new request; GitHub owns publication execution status.
This sequence does not advance production/listening acceptance or change the issue ordering below.

The subsequent three-session [CMP UI-system sequence](016-cmp-ui-system.md) has its bounded
[foundation](../evidence/cmp-ui-system-session1.md),
[interaction](../evidence/cmp-ui-system-session2.md) and
[final review/readiness](../evidence/cmp-ui-system-session3.md) local handoffs. The final inventory
records 29 APIs and 152 scenarios in both previews. All candidates remain unqualified for new app
use; the remaining native/reference/accessibility acceptance is explicit. The separately authorized
[PR43 integration acceptance](../evidence/cmp-pr43-acceptance.md) preserves that boundary after
reconciling develop and fixing Select menu state. GitHub owns merge status; the next component
adoption task must first close its two-renderer runtime/accessibility/reference qualification.
The requested [choice focus-spacing follow-up](../evidence/cmp-choice-focus-spacing.md) keeps
supporting text beyond the painted ring, with failing-first JVM and fresh Android host evidence.
It preserves that adoption prerequisite; GitHub owns its publication status.

The bounded [#5](https://github.com/loveoverflowcom/cantos/issues/5) casting and immutable
production-input implementation in [020](020-cast-and-generate-dialogue.md) has
[local verification evidence](../evidence/casting-production-inputs.md), using the source,
adaptation and structured editor authority from 010. GitHub owns acceptance and execution status;
this queue preserves recommended order and rationale.

**Direction change, 2026-10-12 ([ADR 0018](../decisions/0018-external-audio-production.md)).**
Cantos Studio manages works, episodes and audio; it does not synthesize or mix. External AI
producers read its instructions and standards and send audio back through the API.
[PR #65](https://github.com/loveoverflowcom/cantos/pull/65) removed the in-process text-to-speech
runtime (durable jobs, the `cantos-worker` process, the eSpeak, VieNeu and Google adapters,
receipts, clip reuse and the probe CLIs) and the DBSP job-summary view; the pure WAV inspection
remains as the admission primitive for submitted audio. The earlier evidence for that runtime under
`docs/evidence/` is history, with links to removed files pinned to commit `2de8c69`. No audio can
enter Cantos until the submission API exists. Issues #6, #7 and #8 describe the removed runtime
and #9 assumed in-process mixing; their proposed replacements are in the
[issue map](project-planning.md#re-scoped-by-adr-0018) until the issues themselves are rewritten.

## Recommended bounded follow-ups

1. Write the producer instructions and standards (documents and agent skills) and the read path an
   external producer uses for an approved snapshot, then run them with a real producer on the
   original [reference scene](../production/examples/vietnamese-reference/episode_0001.r0001.review.md).
2. Decide the producer credential and upload-grant model and the object-storage layout (an ADR),
   then implement the submission API: idempotent, bound to revision/snapshot/dialogue, inspected by
   Cantos (reusing the WAV inspection), with stale marking. Private storage and delivery is #10.
3. Add the Studio submission view and stale marks, then continue 030's QC records, publication and
   release gates against submitted audio.
4. Cleanup decisions, each its own reviewed change: whether the casting provider catalog stays as
   vocabulary, retiring the dormant `durable_*`/`speech_clips` tables, removing the now
   consumer-less `dbsp-experimental` feature, and the Studio screens that still describe generation.

These are recommendations, not newly authorized execution. GitHub issues/PRs own live status.

Cross-cutting infrastructure, not a work-plan item: four Studio revision routes can negotiate
MessagePack beside JSON ([ADR 0013](../decisions/0013-optional-messagepack-wire.md),
[evidence](../evidence/messagepack-wire.md)). The Studio still ships JSON; this changes no order,
prerequisite or issue status above.

The requested single-PR [API context refactor](../architecture/api-contexts.md) finishes the
in-progress internal module/application split and adds source dependency guards while
preserving existing wire, stored identities and transaction authority. Its
[handoff evidence](../evidence/api-bounded-contexts.md) records local results and remaining
limits. This maintenance changes no capability acceptance, execution order or issue status;
resume the bounded follow-ups above after its handoff. GitHub owns the PR's live status.

The [audio drama quality plan](audio-drama-quality.md) reconciles external research on adapting
novels for audio drama with these contracts and orders ten issues, [#44](https://github.com/loveoverflowcom/cantos/issues/44)–[#53](https://github.com/loveoverflowcom/cantos/issues/53),
that improve the adaptation process and the audio output. Its editorial phase can proceed now
through the story workspace; the audio profile and clip conditioning are groundwork for #9, and
scene acoustics follow #9. It changes no order, prerequisite, authorization or gate above.

The project-original [Vietnamese reference episode](../production/examples/vietnamese-reference/episode_0001.r0001.review.md)
provides #44's shared two-scene material with per-ID craft/cue coverage and unresolved controls;
its [local evidence](../production/examples/vietnamese-reference/verification.md) pins the exact
JSON bytes. The shared [listening-review protocol](../production/listening-review-protocol.md) and
[template](../../templates/listening-review.md) supply #45's manual evidence format for lines,
scenes and episodes. Use them to judge an external producer's output; no listening result exists.

The next editorial preparation is #46 rights-recorded asset intake, then #47 cue records and
#48 adaptation checks. #50's auditions consume the same pinned reference and protocol, actual
render/listening evidence and recorded owner decisions; #51 additionally needs #46's asset intake.
Text validation and the protocol establish no listening result, approved threshold, owner rights
decision or production acceptance, and advance no backend, provider or publication gate.
GitHub retains execution status, including #44/#45.

The requested [Compose preview tooling](../evidence/compose-preview-tooling.md) adds an optional
Android Studio host-capture command and updates CMP/inspection skills. Its CLI boundary is
tested with a process double; the Theatre Mobile app and real Android/iOS acceptance remain
unimplemented/unrun. Studio Mobile is excluded from the command surface. This enabling change does not advance 050, change the issue ordering or
introduce a competing mobile UI implementation.
The [PR #39](https://github.com/loveoverflowcom/cantos/pull/39) review hardens owned CLI cleanup,
bounded output and private capture paths. The next mobile evidence still comes from an actual
configured project and renderer; GitHub owns this tooling PR's execution status. The
[PR43 Android host follow-up](../evidence/cmp-pr43-android-preview.md) now records that configured
shared-component library and four actual Android Studio captures. Product app, device, IME and
screen-reader acceptance remains the next prerequisite before mobile adoption.

The repository has documentation/templates, an executable
[Script IR contract](../../packages/contracts/README.md), and a local authenticated immutable revision
store with a minimal Studio consumer. The end-to-end capabilities below remain
planned. Numeric filename prefixes express the current recommended sequence, may be renumbered,
and are not permanent task IDs.

| Order | Outcome | Prerequisite |
| --- | --- | --- |
| [010](010-import-and-edit-script.md) | A creator imports a manuscript, reviews an adaptation, and saves an editable Script IR version. | Project brief and architecture decisions |
| [020](020-cast-and-generate-dialogue.md) | A creator assigns voices, freezes production inputs and receives dialogue audio from an external producer through the API. | Validated scripts from 010 |
| [030](030-mix-review-and-publish.md) | A creator reviews submitted scene and episode audio, uploads, and publishes a complete episode. | Submitted dialogue audio from 020 |
| [040](040-theatre-web-listening.md) | A listener discovers and streams published episodes in Theatre Web. | Published manifests and media from 030 |
| [050](050-cmp-native-listening.md) | A listener uses native Android/iOS playback, downloads, and synchronized progress. | Publication contracts from 030 and listening contracts from 040 |

## GitHub execution

[Kanban](https://github.com/users/loveoverflowcom/projects/6/views/1) · [Planning table](https://github.com/users/loveoverflowcom/projects/6/views/2) · [Detailed issue/dependency map](project-planning.md)

The five capability specifications map to 17 bounded implementation issues. Use the Project for
current execution Status and issue checklists/dependencies for evidence; this queue retains
recommended sequence and scope. Review the
[persistence evidence](../evidence/script-revision-persistence.md),
[affected read inventory](../evidence/script-revision-reads.md) and
[bounded read benchmark](../evidence/script-revision-performance.md) before continuing
[#1](https://github.com/loveoverflowcom/cantos/issues/1). Prompt 3 measures the implemented
point reads and a disposable covering-index comparison; it adds no engine or production
schema/query change. DBSP remains deferred: there is no measured hot repeated aggregate or
large join consumer to maintain. The completed
[conditional prompt-4 decision](../evidence/script-revision-dbsp-decision.md) records DEFER,
the missing admission evidence and measurable reopening criteria. Reopen an engine experiment
only for a concrete hot consumer after its simpler PostgreSQL comparison, with a reviewed
adapter, committed-position/freshness contract and resource/recovery budgets. This is not the
next automatic implementation. Revision reads keep that DEFER; the separate durable job summary
had an experimental DBSP slice, described below, that has since been removed.
Continue the bounded importer/editor work in 010 while preserving complete exports, actor
filters, immutable revision identity and retry semantics. Revisit read optimization when that
work changes cardinality, evidence fan-out or introduces an actual list/join/aggregate. The
minimal JSON editor and development sessions do not satisfy the full #1/010 capability or
authorize closing its issue. GitHub execution status is unchanged by this local evidence.

The independent [final sequence review](../evidence/script-revision-final-review.md) fixes
exact numeric admission, normalization/export bounds, pronunciation validation memory,
disposable-runner isolation and cleanup, and Studio save-target status. Its final local checks
include real PostgreSQL recovery and a bounded Safari walkthrough. Continue 010 with these
guards; the review does not complete its importer/editor acceptance or reopen DBSP integration.

The compatible [editorial handoff](../evidence/editorial-handoff.md) retains that backend and
Studio while adding preserved source text, owner review records, bounded history and local
operator provisioning. Continue 010's importer/source-preview/editor consumer on these existing
boundaries; expose the handoff operations through a reviewed UI when that journey needs them.
Production identity, rights eligibility and full authoring acceptance remain separate gates.

The [bounded manuscript import](../evidence/manuscript-import.md) extends that same source
boundary with preserved original bytes, extraction outcomes and a minimal Studio comparison.
The [AI adaptation #3](https://github.com/loveoverflowcom/cantos/issues/3) consumer preserves
immutable source IDs/checksums, ordered blocks, uncertainty and recorded rights claims.
The structured #4 editor now consumes these records. Keep #1's production identity and actual
host-generated quality gates visible; local import/adaptation/editor evidence does not accept
the entire production 010 capability.

The [bounded adaptation implementation](../evidence/ai-script-adaptation.md) uses that source
and revision authority for immutable context, caller-submitted proposals, declared provenance,
receipts and explicit acceptance. The user-controlled Gemini/ChatGPT/Codex host owns generation;
Cantos owns the authenticated tool pipeline and validation. #3 acceptance requires real tool
transport and PostgreSQL journey evidence; actual model output quality is separately NOT_RUN.
The former local-model gate is superseded, not passed. Its evidence now records the actual
tool/storage journey. #4 consumes the same proposal/revision authority for structured
scene/speaker authoring, preserving provenance and concurrency without another draft store. GitHub issues/PRs retain delivery and execution status.

| Capability | Execution issues | Milestone |
| --- | --- | --- |
| [010](010-import-and-edit-script.md) | [#1](https://github.com/loveoverflowcom/cantos/issues/1), [#2](https://github.com/loveoverflowcom/cantos/issues/2), [#3](https://github.com/loveoverflowcom/cantos/issues/3), [#4](https://github.com/loveoverflowcom/cantos/issues/4) | [010 — Import and edit a versioned script](https://github.com/loveoverflowcom/cantos/milestone/1) |
| [020](020-cast-and-generate-dialogue.md) | [#5](https://github.com/loveoverflowcom/cantos/issues/5), [#6](https://github.com/loveoverflowcom/cantos/issues/6), [#7](https://github.com/loveoverflowcom/cantos/issues/7), [#8](https://github.com/loveoverflowcom/cantos/issues/8) | [020 — Cast and hand dialogue to external producers](https://github.com/loveoverflowcom/cantos/milestone/2) |
| [030](030-mix-review-and-publish.md) | [#9](https://github.com/loveoverflowcom/cantos/issues/9), [#10](https://github.com/loveoverflowcom/cantos/issues/10), [#11](https://github.com/loveoverflowcom/cantos/issues/11) | [030 — Review and publish](https://github.com/loveoverflowcom/cantos/milestone/3) |
| [040](040-theatre-web-listening.md) | [#12](https://github.com/loveoverflowcom/cantos/issues/12), [#13](https://github.com/loveoverflowcom/cantos/issues/13) | [040 — Theatre Web listening](https://github.com/loveoverflowcom/cantos/milestone/4) |
| [050](050-cmp-native-listening.md) | [#14](https://github.com/loveoverflowcom/cantos/issues/14), [#15](https://github.com/loveoverflowcom/cantos/issues/15), [#16](https://github.com/loveoverflowcom/cantos/issues/16), [#17](https://github.com/loveoverflowcom/cantos/issues/17) | [050 — CMP native listening](https://github.com/loveoverflowcom/cantos/milestone/5) |

## How to use this queue

- Read the project brief and architecture documents before starting an item; the brief is authoritative.
- Use the linked implementation issues when execution begins; create additional issues only for newly bounded work. Linked GitHub issues are the source of truth for assignment, current status, discussion, and acceptance; these files preserve scope, reasoning, and sequence. Do not maintain a competing status board here.
- Deliver one reviewable outcome at a time. Internal steps may span several PRs, but each PR must identify its review boundary and avoid implying that the whole item is complete.
- Confirm domain and platform assumptions against the actual code, migrations, provider behavior, and runtime before choosing concrete modules, tables, or routes.
- After a completed item, update this queue to make the next task obvious. Record necessary correctness work as a dependency; put optional expansion in [the backlog](backlog/deferred-expansion.md).

## Shared completion evidence

Database-backed read work in every slice uses the foundation's
[PostgreSQL/DBSP evaluation workflow](../../.agents/skills/cantos-engineering/references/postgresql-read-performance.md).
Prioritize measured GET/read/fetch bottlenecks as their implementation appears; keep the order
above. [Skill-update evidence](../evidence/postgresql-dbsp-skills.md) records instruction checks,
not an engine integration or benchmark. A candidate with measured need must earn a scoped
proposal/ADR within its owning slice; this update schedules no engine deployment or migration.

The [story artifact workspace](../production/story-workspace.md) supplies agent-operated intake,
proper-name mapping, radio-score authoring, production handoffs and audit through Drive/local
folders. Its skills and local initializer support editorial preparation for 010–030; they do
not complete the planned importer, submission API, Studio UI or publication gates. Continue
with the existing recommended implementation order above. Artifact sync failures are tracked
by deduplicated recovery issues (or local drafts while GitHub is unavailable).

For every item, provide the user journey demonstrated, relevant contract/migration changes, targeted failure-case checks, and known limitations. A successful mock response alone does not demonstrate a real producer, storage, or native playback integration. Label mocked and live evidence separately.

Keep Rust/Axum and PostgreSQL as a modular backend with Leptos Web and CMP native mobile clients. Avoid introducing independent services without a demonstrated operational need. Apply the shared Material 3 Expressive design requirements to each UI slice.

The [structured Studio editor](../evidence/studio-script-editor.md) consumes that authority for
text-first scene/dialogue/character editing, validation previews, source comparison, explicit
review and immutable acceptance/save/reopen. Keep production identity/legal rights and actual
host-generated quality separate from this local authoring evidence.

The [casting input evidence](../evidence/casting-production-inputs.md) and
[production v1 contract](../../packages/contracts/production-v1.md) bound #5 to versioned casting,
rights/budget policy, owner approval and immutable snapshots on that same PostgreSQL authority.
The bounded implementation is locally verified through domain/wire/schema checks, real
PostgreSQL/HTTP restart journeys and the observed Studio freeze/approval/stale-input flow.
Reference capabilities establish no real voice availability. The earlier host-owned adaptation
policy and the #5 no-generation boundary remain intact; the removed local-sample work grants no
provider or paid-call permission.
DBSP remains deferred for bounded authorized point/history reads; revisit only if measured
query/cardinality changes meet the admission conditions in the affected read inventory. The
job-summary read model that
[#58](https://github.com/loveoverflowcom/cantos/issues/58) added as an experimental DBSP slice
([ADR 0017](../decisions/0017-experimental-dbsp-job-summary.md),
[evidence](../evidence/dbsp-job-summary.md)) was removed with the runtime it served
([ADR 0018](../decisions/0018-external-audio-production.md)). The feature-gated engine core stays
until its removal is decided; the default path is PostgreSQL and the recommended order is unchanged.
