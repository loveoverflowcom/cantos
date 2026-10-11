# Cantos agent skills

Fifteen skills shared by every agent runtime in this repository: eight **rule owners** and seven
**task entrypoints** that compose them. They change what an agent *does* while working on Cantos;
they are not product documentation and they do not implement anything.

```text
cantos-engineering                 ← foundation: invariants, decoupling, immutability, types,
        ↑                             functional core, tests, evidence vocabulary, residual risk
        ├── cantos-script-ir             ← Script IR, editorial revisions, import/adaptation boundary
        ├── cantos-production-pipeline   ← production runs, durable jobs, providers, cost, cache, mix/QC
        ├── cantos-publication           ← rights, approvals, staged publication, manifests, delivery
        └── cantos-listening             ← listener API, playback semantics, progress/bookmark sync
        ↑
cantos-ui-design                   ← shared UI policy: Material 3 Expressive, tokens, type, motion,
        ↑                             accessibility, localization, visual evidence
        ├── cantos-leptos-web            ← Studio and Theatre Web (Leptos)
        └── cantos-cmp-mobile            ← Theatre Android/iOS (CMP) with native playback adapters

task entrypoints (own a workflow and a report, never a rule):
cantos-work-item                   ← deliver one work-plan item or issue end to end
cantos-code-review                 ← read-only, assurance-first review of an existing change
cantos-ui-inspector                ← report-only UI/UX inspection with reusable scenarios
cantos-story-ingest                ← browser capture/OCR → raw sources + Drive handoff
cantos-radio-adapt                 ← name map + dialogue/cues → versioned score
cantos-audio-produce               ← reviewed score → final audio + production evidence
cantos-story-audit                 ← artifact coverage, naming, audio and sync audit
```

Arrows point from a skill to the owner it composes. Owners never route back to the skill that
loaded them, and a task entrypoint cites owner references directly instead of restating them.

## Two kinds of truth

| Truth | Owner | Rule |
|---|---|---|
| Product behavior: hierarchy, business rules, pipeline gates, mobile behavior, UI requirements | [`docs/`](../../docs/README.md) | Link the owning document; never copy a competing version of a rule into a skill |
| Working method: order of work, decoupling, evidence selection, test technique, reporting | these skills | One canonical definition per method, in exactly one skill |

When a skill and an owning document disagree, the document wins for product behavior. Report the
conflict instead of silently choosing the reading that lets a change through, and fix the stale
side in the same change when it is in scope.

## Repository reality

Cantos implements Script IR validation, authenticated PostgreSQL revisions and a minimal
Leptos Studio; inspect [the API service guide](../../services/cantos-api/README.md) and the current
[contracts](../../packages/contracts/README.md) and code. The layout is `apps/cantos` (Theatre) and
`apps/cantos-studio` (Studio), each with `web` and `mobile`, one backend in `services/cantos-api`,
Kanidm configuration in `services/kanidm` and shared `packages/`; read the
[repository layout](../../docs/architecture/repository-layout.md) for which parts are implemented,
skeletons or only reserved. Theatre Web is a skeleton and both mobile apps are guides.
Bounded local manuscript import is implemented. Mobile, AI adaptation and production execution remain planned. Story artifact skills
operate files through available tools; they are not backend workers. Use the provisional score
draft route when real source/rights bindings are unavailable. Report only checks actually run.
Every repository-level check runs through [`cargo xtask`](../../xtask/README.md); for skill edits:

```sh
cargo xtask verify repository
```

Its `scripts/check_repository.py` primitive also validates this directory: frontmatter, names, Codex metadata, index
completeness, orphaned references and local links. Passing it proves skill hygiene only.

## Which skill to load

| The task is about… | Load |
|---|---|
| any Rust domain/backend change, persistence, HTTP contract, tests, verification, refactoring | [`cantos-engineering`](cantos-engineering/SKILL.md) |
| database-backed GET/read/fetch performance, DBSP evaluation, or writes feeding read models | [`cantos-engineering`](cantos-engineering/SKILL.md), then its [read-performance workflow](cantos-engineering/references/postgresql-read-performance.md); add the domain owner |
| Script IR schema, stable IDs, revisions, digests, import, AI adaptation output, Narrative Forge | [`cantos-script-ir`](cantos-script-ir/SKILL.md) |
| production runs, jobs, leases, TTS/adaptation providers, budgets, cache keys, mixing, QC records | [`cantos-production-pipeline`](cantos-production-pipeline/SKILL.md) |
| rights, approvals, release manifests, object storage, CDN, publish/republish/retract | [`cantos-publication`](cantos-publication/SKILL.md) |
| catalog/listener APIs, playback state meaning, progress, bookmarks, download entitlement | [`cantos-listening`](cantos-listening/SKILL.md) |
| visual design, Material 3 Expressive, tokens, typography, motion, accessibility, copy | [`cantos-ui-design`](cantos-ui-design/SKILL.md) |
| Leptos Studio or Theatre Web, CSS/SCSS, browser behavior | [`cantos-leptos-web`](cantos-leptos-web/SKILL.md) |
| Kotlin Compose Multiplatform, Android/iOS playback, downloads, native lifecycle | [`cantos-cmp-mobile`](cantos-cmp-mobile/SKILL.md) |
| acquiring a website story through browser capture/extraction/OCR into Drive/local raw folders | [`cantos-story-ingest`](cantos-story-ingest/SKILL.md) |
| light radio-drama adaptation, proper-name mapping and performance cue authoring | [`cantos-radio-adapt`](cantos-radio-adapt/SKILL.md) |
| producing final audio artifacts from a reviewed score, including partial regeneration | [`cantos-audio-produce`](cantos-audio-produce/SKILL.md) |
| light source/score/audio artifact audit and handoff integrity | [`cantos-story-audit`](cantos-story-audit/SKILL.md) |
| implementing a work-plan item or GitHub issue from plan to pull request | [`cantos-work-item`](cantos-work-item/SKILL.md) |
| reviewing a PR, commit range, patch or local diff without editing | [`cantos-code-review`](cantos-code-review/SKILL.md) |
| auditing UI/UX and writing a findings report without fixing | [`cantos-ui-inspector`](cantos-ui-inspector/SKILL.md) |

The four story artifact entrypoints share [one workspace contract](../../docs/production/story-workspace.md),
including the configured Drive root, local fallback, indexes and recovery issues. `score/` uses
[Script IR plus a versioned proper-name map](../../docs/production/radio-score.md).
They preserve application ownership and never store story material in Git.

A task often spans owners: a Studio casting screen composes `cantos-leptos-web`,
`cantos-ui-design`, `cantos-production-pipeline` and the foundation. Load each owner's
`SKILL.md`, then only the references the change actually touches.

## Non-negotiables shared by every skill

These are defined once in the foundation and repeated here only as an index:

- **Decoupling.** Dependencies point inward toward pure domain values; the domain and Script IR
  never import HTTP, UI frameworks, provider SDKs or Narrative Forge internals.
- **Immutability.** Accepted revisions, frozen production inputs, artifacts, approvals and
  release manifests are never edited in place; change produces a new value or record.
- **Functional core.** Durable decisions are pure functions over typed facts; I/O, clocks, IDs and
  providers live in a thin shell.
- **Verification.** Every behavior change gets the cheapest deterministic regression evidence, and
  every report names its evidence level from one vocabulary and states residual risk.
- **Clean syntax.** Small named transformations, exhaustive matches, exact error types, explicit
  imports, formatter-clean code and no clever compression.
- **Material 3 Expressive.** An attention system with a calm lavender identity, applied at the
  intensity each surface deserves, verified on rendered output.

## Layout

```text
<skill-name>/
  SKILL.md              YAML frontmatter (name, description) + behavioral contract and router
  agents/openai.yaml    display metadata for the Codex skill surface
  references/*.md       technique depth, loaded only when relevant
  scenarios/*.json      reviewed, runnable-when-implemented scenario contracts (inspector)
```

[`routing-cases.md`](routing-cases.md) holds task prompts and a rubric for checking that the
skills still route correctly after an edit. It is an evaluation input, not an automated benchmark.

## Runtime discovery

| Runtime | Discovery |
|---|---|
| Codex | scans `.agents/skills/` from the working directory up to the repository root; `agents/openai.yaml` supplies optional display metadata |
| Claude Code | scans `.claude/skills/<name>/SKILL.md`; each `.claude/skills/<name>` entry is a relative symlink to `../../.agents/skills/<name>` |
| Any other agent | read [`AGENTS.md`](../../AGENTS.md), then the `SKILL.md` named for the task |

Keep exactly one copy of every skill here. Do not add per-runtime copies, router aliases or a
second index. The repository check fails when a skill lacks its Claude symlink or a symlink points
anywhere else. Symlink one entry per skill, never the whole `.claude/skills` directory, which
Claude Code does not discover reliably. Windows checkouts need `core.symlinks=true`; otherwise
point the runtime at this README.

Frontmatter stays portable across runtimes: only `name` (lowercase letters, digits and single
hyphens, at most 64 characters, equal to the directory name) and `description` (at most 1024
characters, stating what the skill does and when to use it).

## Maintaining skills

- `SKILL.md` stays a router under 300 lines; depth moves to `references/`.
- A new rule needs scope, rationale or failure mode, a good example and a counterexample, the
  oracle that checks it, its enforcement status and its exception.
- A new task entrypoint is justified only by a distinct workflow and report, never as a second
  path to the same rules.
- Proposed automation in a skill does not become an available command or an enforced gate by
  being documented. Label it proposed until the repository implements it.
- Walk [`routing-cases.md`](routing-cases.md) after changing routing or ownership, and run the
  repository checks above.
