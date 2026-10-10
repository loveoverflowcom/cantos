# MVP work queue

Start with [#1](https://github.com/loveoverflowcom/cantos/issues/1) within [010 — Import and edit a versioned script](010-import-and-edit-script.md). Each item delivers a usable business capability; implementation details must be confirmed against the code created in earlier items.

The [script revision backend](../../apps/server/README.md) supplies validated Script IR persistence for #1. The next boundary within #1 is a minimal Leptos save/reopen consumer using that contract; import/adaptation/editor expansion follows the linked issues. The end-to-end capabilities below remain planned. Numeric filename prefixes express the current recommended sequence, may be renumbered, and are not permanent task IDs.

| Order | Outcome | Prerequisite |
| --- | --- | --- |
| [010](010-import-and-edit-script.md) | A creator imports a manuscript, reviews an adaptation, and saves an editable Script IR version. | Project brief and architecture decisions |
| [020](020-cast-and-generate-dialogue.md) | A creator assigns voices and generates dialogue through durable, resumable jobs. | Validated scripts from 010 |
| [030](030-mix-review-and-publish.md) | A creator mixes, reviews, uploads, and publishes a complete episode. | Generated dialogue from 020 |
| [040](040-theatre-web-listening.md) | A listener discovers and streams published episodes in Theatre Web. | Published manifests and media from 030 |
| [050](050-cmp-native-listening.md) | A listener uses native Android/iOS playback, downloads, and synchronized progress. | Publication contracts from 030 and listening contracts from 040 |

## GitHub execution

[Kanban](https://github.com/users/loveoverflowcom/projects/6/views/1) · [Planning table](https://github.com/users/loveoverflowcom/projects/6/views/2) · [Detailed issue/dependency map](project-planning.md)

The five capability specifications map to 17 bounded implementation issues. Use the Project for current execution Status and issue checklists/dependencies for evidence; this queue retains recommended sequence and scope. The first implementation task is [#1](https://github.com/loveoverflowcom/cantos/issues/1).

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

The [story artifact workspace](../production/story-workspace.md) supplies agent-operated intake,
proper-name mapping, radio-score authoring, production handoffs and audit through Drive/local
folders. Its skills and local initializer support editorial preparation for 010–030; they do
not complete the planned importer, provider workers, Studio UI or publication gates. Continue
with the existing recommended implementation order above. Artifact sync failures are tracked
by deduplicated recovery issues (or local drafts while GitHub is unavailable).

For every item, provide the user journey demonstrated, relevant contract/migration changes, targeted failure-case checks, and known limitations. A successful mock response alone does not demonstrate a real provider, storage, or native playback integration. Label mocked and live evidence separately.

Keep Rust/Axum and PostgreSQL as a modular backend with Leptos Web and CMP native mobile clients. Avoid introducing independent services without a demonstrated operational need. Apply the shared Material 3 Expressive design requirements to each UI slice.
