# Cantos agent skills

> [ADR 0018](../../docs/decisions/0018-external-audio-production.md) changes ownership:
> **external AI producer runs all TTS/record/mixing/encoding**, Cantos API only serves versioned
> instructions and accepts private verified files for review/publication. Historical provider
> worker contracts are NOT current backend implementation guidance.

Nineteen skills shared by every agent runtime in this repository: nine **rule owners** and ten
**task entrypoints** that compose them. They change what an agent *does* while working on Cantos;
they are not product documentation and they do not implement anything.

```text
cantos-engineering                 ← foundation: invariants, decoupling, immutability, types,
        ↑                             functional core, tests, evidence vocabulary, residual risk
        ├── cantos-behavior              ← shared Quint catalog, scenario generation, non-vacuity and model evidence
        ├── cantos-script-ir             ← Script IR, editorial revisions, import/adaptation boundary
        ├── cantos-production-pipeline   ← external audio handoff, profiles, QC intake and producer isolation
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
cantos-ui-fix                      ← bounded UI repair with same-scenario and layered evidence
cantos-logic-fix                   ← bounded invariant repair with regression and layered evidence
cantos-diagnostics                 ← progressive log reading, failure triage and a diagnosis report
cantos-story-ingest                ← browser capture/OCR → raw sources + Drive handoff
cantos-radio-adapt                 ← name map + dialogue/cues → versioned score
cantos-audio-produce               ← reviewed score → final audio + production evidence
cantos-story-audit                 ← artifact coverage, naming, audio and sync audit
```

Arrows point from a skill to the owner it composes. Owners never route back to the skill that
loaded them, and a task entrypoint cites owner references directly instead of restating them.

## Requirements, executable expectations and method

| Truth | Owner | Rule |
|---|---|---|
| Product behavior: hierarchy, business rules, pipeline gates, mobile behavior, UI requirements | [`docs/`](../../docs/README.md) | Link the owning document; never copy a competing version of a rule into a skill |
| Accepted executable stateful expectations | [`specs/behavior/`](../../specs/behavior/README.md) | Catalog owns IDs/lifecycle; accepted `.qnt` owns executable outcomes; draft model PASS never accepts requirements or proves implementation |
| Working method: order of work, decoupling, evidence selection, test technique, reporting | these skills | One canonical definition per method, in exactly one skill |

When a skill and an owning document disagree, the document wins for product behavior. Report the
conflict instead of silently choosing the reading that lets a change through, and fix the stale
side in the same change when it is in scope.

## Repository reality

Cantos implements Script IR validation, authenticated PostgreSQL revisions and the Leptos
Studio; inspect [the API service guide](../../services/cantos-api/README.md) and the current
[contracts](../../packages/contracts/README.md) and code. The layout is `apps/cantos-theatre/{web,mobile}` (Theatre) and
`apps/cantos-studio/web` (Studio), the authoritative modular backend in `services/cantos-api`,
current Forge code in `apps/cantos-forge`, Kanidm configuration in `services/kanidm` and shared
`packages/`. Deployment/runtime assets live under [`infra/`](../../infra/README.md); `xtask/`
owns command orchestration, not those assets. The #212 split adds no runtime; #203 owns any
future Forge service-location move and concrete Functions consumer. Read the
[platform matrix](../../docs/architecture/repository-layout.md#platform-scope) before choosing consumers.
Studio is web-only: do not implement, scaffold, build or maintain Studio Mobile unless explicitly
requested in a future roadmap. Theatre Web has anonymous live discovery/account reads and
explicit UI mocks. Theatre Mobile has a production-safe shared core, isolated Gradle CMP demo
and unavailable production Android host; native audio, downloads and durable sync remain absent.
Backend listener contracts exist; real client/provider acceptance stays separate.
Studio Mobile is out of scope / not implemented and is only a policy notice. Shared libraries may
serve Theatre Mobile and Studio Web.
Bounded manuscript import, caller-hosted adaptation tooling (Cantos performs no inference),
casting with frozen guidance; obsolete local/Google TTS probe CLIs are removed; optional MessagePack
representation for four Studio routes are implemented. Production native listening
remains planned. Story artifact skills
operate files through available tools; they are not backend workers. Use the provisional score
draft route when real source/rights bindings are unavailable. Report only checks actually run.
Every repository-level check runs through [`cargo xtask`](../../xtask/README.md). Skill edits
follow the foundation's [verification cadence](cantos-engineering/references/verification-cadence.md):
focused checks during editing and preflight at final handoff, with repository hygiene already
included. Use `cargo xtask verify repository` separately only when a focused hygiene check is
needed, not as an additional prerequisite to the same handoff preflight.

Its `scripts/check_repository.py` primitive also validates this directory: frontmatter, names, Codex metadata, index
completeness, orphaned references, local links, Claude symlinks and the shared-instruction import.
Passing it proves skill hygiene only.

## Which skill to load

| The task is about… | Load |
|---|---|
| registered stateful behavior, Quint models, shared API/Leptos/CMP scenarios, witnesses and counterexamples | [`cantos-behavior`](cantos-behavior/SKILL.md), composing the foundation |
| any Rust domain/backend change, persistence, HTTP contract, tests, verification, refactoring | [`cantos-engineering`](cantos-engineering/SKILL.md) |
| layered verification, independently verified core delivery, unavailable infrastructure or deferred acceptance | [`cantos-engineering`](cantos-engineering/SKILL.md), then [verification strategy](cantos-engineering/references/verification-strategy.md#layered-verification) and [local handoff](cantos-engineering/references/local-execution.md#local-handoff-and-pr-completion) |
| when to run checks, checkpoint/handoff scope, repeated preflight or evidence reuse | [`cantos-engineering`](cantos-engineering/SKILL.md), then [verification cadence](cantos-engineering/references/verification-cadence.md) |
| typed functional errors, refinement-preserving composition, cancellation, retry and boundary translation | [`cantos-engineering`](cantos-engineering/SKILL.md), then [typed errors and composition](cantos-engineering/references/typed-errors-and-composition.md); add the CMP/Web application reference only for that platform |
| Kani for a selected high-risk pure Rust kernel, assumptions, bounds, cover and actual verification evidence | foundation [mutation/formal reference](cantos-engineering/references/mutation-and-formal.md#bounded-model-checking); no backend-wide or Kotlin UI mandate |
| database-backed GET/read/fetch performance, DBSP evaluation, or writes feeding read models | [`cantos-engineering`](cantos-engineering/SKILL.md), then its [read-performance workflow](cantos-engineering/references/postgresql-read-performance.md); add the domain owner |
| JSON or MessagePack on a route, `Content-Type`/`Accept` negotiation, a client codec switch, codec benchmarks | [`cantos-engineering`](cantos-engineering/SKILL.md), then its [wire codec reference](cantos-engineering/references/wire-codec.md); add the renderer skill for client code |
| Script IR schema, stable IDs, revisions, digests, import, AI adaptation output, Narrative Forge | [`cantos-script-ir`](cantos-script-ir/SKILL.md) |
| external AI producer briefs, scoped private uploads, received audio, codec profiles, QC and safe publication gates | [`cantos-production-pipeline`](cantos-production-pipeline/SKILL.md) |
| rights, approvals, release manifests, object storage, CDN, publish/republish/retract | [`cantos-publication`](cantos-publication/SKILL.md) |
| catalog/listener APIs, playback state meaning, progress, bookmarks, download entitlement | [`cantos-listening`](cantos-listening/SKILL.md) |
| visual design, Material 3 Expressive, tokens, typography, motion, accessibility, copy | [`cantos-ui-design`](cantos-ui-design/SKILL.md) |
| Leptos Studio or Theatre Web, CSS/SCSS, browser behavior | [`cantos-leptos-web`](cantos-leptos-web/SKILL.md) |
| Web startup/bundle/interaction performance, cache classification, signal scope or SSR/Islands/splitting experiments | [`cantos-leptos-web`](cantos-leptos-web/SKILL.md), then [performance and caching](cantos-leptos-web/references/performance-and-caching.md), composing foundation evidence |
| Kotlin Compose Multiplatform, Android/iOS playback, downloads, native lifecycle | [`cantos-cmp-mobile`](cantos-cmp-mobile/SKILL.md) |
| emulator-free Compose @Preview, Android CLI/Quail 2+, PNG and exported semantics | [`cantos-cmp-mobile`](cantos-cmp-mobile/SKILL.md), then [Compose preview](cantos-cmp-mobile/references/compose-preview.md); audits compose [`cantos-ui-inspector`](cantos-ui-inspector/SKILL.md) |
| acquiring a website story through browser capture/extraction/OCR into Drive/local raw folders | [`cantos-story-ingest`](cantos-story-ingest/SKILL.md) |
| light radio-drama adaptation, proper-name mapping and performance cue authoring | [`cantos-radio-adapt`](cantos-radio-adapt/SKILL.md) |
| **externally** producing/uploading audio for a pinned work/chapter/podcast brief | [`cantos-audio-produce`](cantos-audio-produce/SKILL.md) |
| light source/score/audio artifact audit and handoff integrity | [`cantos-story-audit`](cantos-story-audit/SKILL.md) |
| implementing a work-plan item or GitHub issue from plan to pull request | [`cantos-work-item`](cantos-work-item/SKILL.md) |
| reviewing a PR, commit range, patch or local diff without editing | [`cantos-code-review`](cantos-code-review/SKILL.md) |
| auditing UI/UX and writing a findings report without fixing | [`cantos-ui-inspector`](cantos-ui-inspector/SKILL.md) |
| repairing one bounded UI defect from screenshots, findings, labels, overflow or presentation-state mapping | [`cantos-ui-fix`](cantos-ui-fix/SKILL.md), then the UI and renderer owners |
| repairing one bounded domain, type, state, error-handling, application or I/O defect | [`cantos-logic-fix`](cantos-logic-fix/SKILL.md), then the foundation and invariant owner; persistence/concurrency still needs real acceptance |
| finding why a gate, test, build, runner, server, browser or service fails; triaging a red or flaky check; reading or summarizing long logs | [`cantos-diagnostics`](cantos-diagnostics/SKILL.md); a requested fix then continues through the repair entrypoint |
| emitting or reviewing logs, diagnostic events, levels, redaction, correlation IDs, a logging dependency or telemetry services | [`cantos-engineering`](cantos-engineering/SKILL.md), then its [structured observability](cantos-engineering/references/structured-observability.md) reference |

`cantos-diagnostics` turns a failure into a classification (environment, pre-existing, flaky,
regression, test defect or unknown) and a confirmed or bounded cause. It reads signals from summary
to focused to deep depth (D0–D2, distinct from the L1–L5 layers), keeps full output in an ignored
`artifacts/agent-runs/<run-id>/` record and removes its temporary instrumentation; it grants no
repair authority and adds no status or gate.

The two focused-fix entrypoints locate the existing invariant owner and deliver minimal repairs
with independent L1–L5 results; they add no rule owner or outward-action authority. Correct
domain/API facts displayed incorrectly stay in UI presentation; wrong domain errors/transitions
go to logic. Review-only tasks keep the reviewer/inspector read-only/report-only. Missing I/O can
leave named core work independently verified, never hide an actual regression failure or establish
database/device semantics. Wider architecture, schema/API, security, durability or redesign work
escalates through `cantos-work-item` and its owners.

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
  object-storage/authorization effects live in a thin shell; speech/AI providers live in external plugins.
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

| Runtime | Discovery and invocation |
|---|---|
| Codex | discovers `.agents/skills/`; invoke `$cantos-engineering` or another exact skill name; `agents/openai.yaml` supplies Codex display metadata |
| Claude Code | discovers `.claude/skills/<name>/SKILL.md` through the per-skill symlinks; invoke `/cantos-engineering` or another exact skill name; root [`CLAUDE.md`](../../CLAUDE.md) imports `AGENTS.md` |
| Any other agent | read [`AGENTS.md`](../../AGENTS.md), then the canonical `SKILL.md` named for the task |

Both runtimes use the same frontmatter, body and references. `agents/openai.yaml` is required by
this repository's hygiene check, but is not Claude configuration. Keep Claude-only frontmatter,
Codex tool names and runtime macros out of the shared contract. Available shell, file, browser,
image and connected-service tools differ by session: follow the foundation's
[runtime portability](cantos-engineering/references/local-execution.md#runtime-portability)
before executing a tool-dependent step.

Keep exactly one copy of every skill here. Do not add per-runtime copies, router aliases or a
second index. The repository check fails when a skill lacks its Claude symlink or a symlink points
anywhere else. Symlink one entry per skill, never the whole `.claude/skills` directory, which
Claude Code does not discover reliably. Windows checkouts need `core.symlinks=true`; otherwise
point the runtime at this README.

Frontmatter stays portable across runtimes: only `name` (lowercase letters, digits and single
hyphens, at most 64 characters, equal to the directory name) and `description` (at most 1024
characters, stating what the skill does and when to use it).

### Check discovery separately from execution

Use the foundation's [handoff cadence](cantos-engineering/references/verification-cadence.md)
for shared-file and bootstrap validation; preflight already covers repository hygiene.
The checker requires a standalone, unindented, unfenced `@AGENTS.md` import in
root `CLAUDE.md`; an instruction to read the file or a code example does not load it.

For a Claude runtime check, start Claude Code in the checkout, type `/cantos-` and inspect the
completion list. Use `/memory` to inspect the loaded `CLAUDE.md` and `AGENTS.md`. To check actual
skill execution, invoke one bounded task such as `/cantos-code-review` with a local diff and
record the selected owners and read-only outcome. Discovery alone does not verify that run;
record authentication, permission or missing-tool blockers separately. Use the corresponding
`$cantos-code-review` request in Codex when comparing decisions across runtimes.

Claude's documented [skill discovery](https://code.claude.com/docs/en/skills) supports symlinked
skill folders; its [memory imports](https://code.claude.com/docs/en/memory) support `@AGENTS.md`.
If skills do not appear, inspect checkout symlinks, session skill restrictions and description
budget before changing shared skills; use the runtime's explicit invocation when automatic
selection misses a task. Do not change personal settings or install tools as a discovery fix.

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
