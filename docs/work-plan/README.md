# MVP work queue

The requested UI-only [Leptos UI system sequence](015-leptos-ui-system.md) delivers a small
canonical foundation before components, responsive app integration and independent review.
The [independent session 4 review](../evidence/ui-system-review.md) records scoped Safari fixes,
actual local interaction/reflow evidence and remaining accessibility/platform gates. The four
requested sessions stop here. Any publication or additional UI/CMP work needs a new request.
This sequence does not advance production/listening acceptance or change #6/#7 ordering below.

The bounded [#5](https://github.com/loveoverflowcom/cantos/issues/5) casting and immutable
production-input implementation in [020](020-cast-and-generate-dialogue.md) has
[local verification evidence](../evidence/casting-production-inputs.md), using the source,
adaptation and structured editor authority from 010. The current work remains
[#6](https://github.com/loveoverflowcom/cantos/issues/6): its user-authorized local eSpeak NG
1.52.0 sample path is implemented, with real generation and native 22,050 Hz mono 16-bit PCM
validation for all three supported Vietnamese voices. Listening quality is **NOT_RUN**.
GitHub owns acceptance and execution status; this queue preserves recommended order and rationale.

The [local probe and receipt inspection](../evidence/tts-receipt-inspection.md) bind private
frozen-input/intent/receipt/output records to exact #5 snapshot identity and conservative
recovery facts. They grant no current production authorization, durable jobs or asset acceptance.
The [local selection](../decisions/0008-local-tts-inspection.md) is resolved for this sample scope;
commercial provider/terms/voice rights/authentication/spending cap and its real sample remain
deferred in [provider prerequisites](../evidence/tts-provider-prerequisites.md). Continue only
within #6 after those decisions. This is partial delivery, cannot close #6, and does not recommend
starting #7. The current requested execution stops at #6.

[PR #26](https://github.com/loveoverflowcom/cantos/pull/26) delivers the local research slice.
The requester directed its merge into `develop` on 2026-10-11; it does not close #6, and the
listening and production-rights gates below still block any production use.
The requested [local neural investigation](../evidence/local-neural-tts-research.md) compares
Piper Vietnamese models with VieNeu Nano. A bounded [research adapter](../evidence/local-neural-tts.md)
retains exact frozen input/receipt identity, rejects unsupported controls and refuses production
dispatch. Local installation and all six original samples have now run successfully across the
two admitted presets, Đức Trí and Mỹ Duyên, with native 24,000 Hz mono 16-bit PCM and checked
WAV hashes/receipts. The next recommended task is a named-listener assessment of pronunciation,
naturalness and the neutral delivery; listening remains **NOT_RUN**. Resolve training/voice
provenance before considering production eligibility, which remains **NO_GO**. Then continue
the separate unfinished commercial #6 criteria after the required provider/terms/rights and
budget decisions. Preserve eSpeak and keep #6 open; these research samples do not advance the
queue to #7's durable workers.

The owner-selected Google Cloud TTS path follows the same pattern: Google vi-VN voices are in the
casting catalog with an explicit neutral-delivery rule, and an opt-in probe consumes one frozen
line through the shared receipt audit, with [mocked evidence](../evidence/google-tts-adapter.md)
only ([ADR 0011](../decisions/0011-google-cloud-tts-adapter.md)). Studio can now record the
explicit neutral delivery at intensity 0 that eSpeak and Google casting require, and freeze and
approve those inputs ([Studio casting evidence](../evidence/studio-provider-casting.md)). Within
#6, next record an operator-paid, capped live sample with real envelopes. [#7](https://github.com/loveoverflowcom/cantos/issues/7)
then persists attempts and reservations before any production dispatch.

Cross-cutting infrastructure, not a work-plan item: four Studio revision routes can negotiate
MessagePack beside JSON ([ADR 0013](../decisions/0013-optional-messagepack-wire.md),
[evidence](../evidence/messagepack-wire.md)). The Studio still ships JSON; this changes no order,
prerequisite or issue status above.

The repository has documentation/templates, an executable
[Script IR contract](../../packages/contracts/README.md), and a local authenticated immutable revision
store with a minimal Studio consumer. The end-to-end capabilities below remain
planned. Numeric filename prefixes express the current recommended sequence, may be renumbered,
and are not permanent task IDs.

| Order | Outcome | Prerequisite |
| --- | --- | --- |
| [010](010-import-and-edit-script.md) | A creator imports a manuscript, reviews an adaptation, and saves an editable Script IR version. | Project brief and architecture decisions |
| [020](020-cast-and-generate-dialogue.md) | A creator assigns voices and generates dialogue through durable, resumable jobs. | Validated scripts from 010 |
| [030](030-mix-review-and-publish.md) | A creator mixes, reviews, uploads, and publishes a complete episode. | Generated dialogue from 020 |
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
next automatic implementation.
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
| [020](020-cast-and-generate-dialogue.md) | [#5](https://github.com/loveoverflowcom/cantos/issues/5), [#6](https://github.com/loveoverflowcom/cantos/issues/6), [#7](https://github.com/loveoverflowcom/cantos/issues/7), [#8](https://github.com/loveoverflowcom/cantos/issues/8) | [020 — Cast and generate dialogue durably](https://github.com/loveoverflowcom/cantos/milestone/2) |
| [030](030-mix-review-and-publish.md) | [#9](https://github.com/loveoverflowcom/cantos/issues/9), [#10](https://github.com/loveoverflowcom/cantos/issues/10), [#11](https://github.com/loveoverflowcom/cantos/issues/11) | [030 — Mix, review and publish](https://github.com/loveoverflowcom/cantos/milestone/3) |
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
not complete the planned importer, provider workers, Studio UI or publication gates. Continue
with the existing recommended implementation order above. Artifact sync failures are tracked
by deduplicated recovery issues (or local drafts while GitHub is unavailable).

For every item, provide the user journey demonstrated, relevant contract/migration changes, targeted failure-case checks, and known limitations. A successful mock response alone does not demonstrate a real provider, storage, or native playback integration. Label mocked and live evidence separately.

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
Historical reference capabilities establish no real voice availability or billable eligibility.
The catalog now also exposes the explicit eSpeak local inspection entry, with billable dispatch
unavailable. [#6](https://github.com/loveoverflowcom/cantos/issues/6) has actual local sample
evidence under the user's subsequent local authorization; its commercial path still needs an
explicit decision and remains unrun. The earlier host-owned adaptation policy and the #5
no-generation boundary remain intact. This local sample choice grants neither cloud credentials
nor paid-call permission and does not advance execution to #7.
DBSP remains deferred for bounded authorized point/history reads; revisit only if measured
query/cardinality changes meet the admission conditions in the affected read inventory.
