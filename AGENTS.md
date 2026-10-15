# Agent instructions — Cantos

> **Current product boundary (ADR 0018):** Cantos Studio is Web-only CMS for
> novel/story/podcast, chapters/episodes, versioned instructions and external audio assets.
> **AI plugins/human producers produce, mix and encode audio outside Cantos**, then use a
> secure private handoff API. No backend TTS/provider/worker. Theatre only plays releases.
> See [current decision](docs/decisions/0018-external-audio-production.md) and
> [proposed, not-yet-implemented handoff](packages/contracts/external-audio-handoff-v1.md).

## Scope and source of truth

Read [README.md](README.md), the relevant product/architecture docs and [docs/work-plan/README.md](docs/work-plan/README.md) before implementing a feature. This is a documentation and template bootstrap; inspect actual code before treating a proposed module, schema, command or API as implemented.

The current user's instructions take priority. Preserve Cantos Studio and Cantos Theatre terminology, Rust/Axum/PostgreSQL/Leptos/CMP choices and native mobile playback. Keep docs in English unless the task requests otherwise; retain correct Vietnamese names and representative content.

## Repository layout

`apps/cantos-theatre/{web,mobile}` is Cantos Theatre (listeners), `apps/cantos-studio/web` is Cantos Studio (creators, Web only), `apps/cantos-forge` is the producer-side Cantos Forge CLI (proposed in [ADR 0022](docs/decisions/0022-cantos-forge-producer-cli.md); a host-assisted local workflow of register, validate and export a local bundle, no provider call and no API handoff; runs outside the API and never makes the API generate), `services/cantos-api` is the one Rust/Axum/PostgreSQL backend, `services/kanidm` is identity-provider configuration only, `packages/` holds shared contracts and design tokens, and `xtask/` is the `cargo xtask` developer command surface. Apps depend on `packages/` and call the API over HTTP; they never import each other or service code, and `packages/` imports neither. Do not mix Theatre and Studio UI or permissions. Naming is strict: `cantos-theatre`/`theatre` is the technical name (path, package, API group, DTOs), while listeners only ever see **Cantos** (never "Cantos Theatre") and creators see **Cantos Studio**. The API has three route groups with separate routers, DTOs, authentication and data access: `/api/v1/studio/*` (implemented; the pre-split `/api/v1/*` paths are a deprecated alias), `/api/v1/theatre/*` (anonymous public discovery/playback core; delivery acceptance pending) and `/api/v1/admin/*` (opt-in dedicated capability/revocation and audit; live issuer/group acceptance pending). Never serve a Theatre or Admin feature from a Studio handler, reuse a Studio session for them, or add fake data to a reserved group ([decision 0015](docs/decisions/0015-api-surface-boundaries.md)). [Repository layout](docs/architecture/repository-layout.md) records what is implemented, a skeleton or only reserved; do not present a skeleton as a feature.

### Platform scope

Cantos Studio is web-only. Do not implement, scaffold, build or maintain a mobile client unless explicitly requested in a future roadmap.
`apps/cantos-studio/mobile` may retain a scope notice only; never add it to workspace/Gradle,
xtask, CI, build/test automation or Compose previews. Cantos Theatre still supports Web and
Android/iOS Mobile at `apps/cantos-theatre/{web,mobile}` (native playback). Shared libraries may
serve Theatre Mobile and Studio Web. Consult the [platform matrix](docs/architecture/repository-layout.md#platform-scope)
for scope versus implementation status.

## Architecture boundaries

- Begin with a modular monolith; add a service, crate or generic abstraction only for a concrete consumer and reviewed need.
- Domain rules and versioned Script IR must not depend on HTTP, UI frameworks, a specific AI provider or Narrative Forge internals.
- Use PostgreSQL for metadata, revision history, producer submission receipts and release pointers. Store verified received media in private object storage until review/publication.
- Do not invoke AI during listener playback. Preserve immutable publication manifests and cached renditions.
- Keep private drafts, voice assets and previews inaccessible to public delivery. Enforce permissions in the backend.
- Mobile uses CMP with platform-native audio adapters; no WebView audio player.

## Correctness rules

For external audio handoff, reason about **producer scope and revocation, duplicate upload,
lost finalize response, private object integrity, stale briefs, content ownership, wrong
episode binding, rights, QC and publication approval invalidation**. The AI producer owns
voice-provider timeouts, any TTS attempts and costs: there are **no Cantos generation jobs**.
Server acceptance must verify actual bytes/hash/format and persist immutable receipts.

A dialogue/text change can mark older submissions stale but never auto-regenerates audio.
Any imported or externally produced asset requires provenance and rights claims; claims
are not the same as verified rights or human-listened quality.

## Working and review

Branch from `develop` and target PRs to `develop`. Keep changes cohesive and avoid unrelated cleanup. Update the work queue after implementation; GitHub issues/PRs own execution status when linked, and the queue owns recommended order and rationale.

Use [templates](templates/README.md) for a concrete proposal or production record when relevant. Clearly distinguish proposed design, implemented behavior and verified behavior. Never commit secrets, real provider credentials, private manuscripts, downloaded media or generated audio to Git.

## Agent skills

Canonical skills live in [`.agents/skills/`](.agents/skills/README.md) and are shared by Codex, Claude Code and any other agent; `.claude/skills/<name>` entries are symlinks to them. Read the skill index, then load the skill that owns the task:

- [`cantos-engineering`](.agents/skills/cantos-engineering/SKILL.md) for any domain, backend, persistence, contract, test or verification change. It owns the required order, decoupling, immutability, functional core, the evidence vocabulary and the completion report.
- [`cantos-behavior`](.agents/skills/cantos-behavior/SKILL.md) for catalog lookup, Quint stateful expectations, AI scenario generation, non-vacuity and counterexamples across API/Leptos/CMP; models do not establish implementation conformance.
- Domain owners: [`cantos-script-ir`](.agents/skills/cantos-script-ir/SKILL.md), [`cantos-production-pipeline`](.agents/skills/cantos-production-pipeline/SKILL.md), [`cantos-publication`](.agents/skills/cantos-publication/SKILL.md), [`cantos-listening`](.agents/skills/cantos-listening/SKILL.md).
- UI: [`cantos-ui-design`](.agents/skills/cantos-ui-design/SKILL.md) (Material 3 Expressive and shared UI policy) with [`cantos-leptos-web`](.agents/skills/cantos-leptos-web/SKILL.md) or [`cantos-cmp-mobile`](.agents/skills/cantos-cmp-mobile/SKILL.md).
- Task entrypoints: [`cantos-work-item`](.agents/skills/cantos-work-item/SKILL.md) to deliver a work-plan item, [`cantos-code-review`](.agents/skills/cantos-code-review/SKILL.md) for a read-only review, [`cantos-ui-inspector`](.agents/skills/cantos-ui-inspector/SKILL.md) for a report-only UI audit, [`cantos-ui-fix`](.agents/skills/cantos-ui-fix/SKILL.md) for one bounded UI repair, [`cantos-logic-fix`](.agents/skills/cantos-logic-fix/SKILL.md) for one bounded invariant/logic repair. Both fix entrypoints compose existing owners, use minimal changes and independent layered evidence; neither adds rules or permission. Review/inspection does not authorize fixing. [`cantos-diagnostics`](.agents/skills/cantos-diagnostics/SKILL.md) investigates a failing gate, test, build or runtime log: it reads output summary-first under a context budget, separates environment/pre-existing/flaky failures from regressions, keeps full output in ignored `artifacts/agent-runs/` and reports a diagnosis without repairing.
- Story artifact workflows: [`cantos-story-ingest`](.agents/skills/cantos-story-ingest/SKILL.md), [`cantos-radio-adapt`](.agents/skills/cantos-radio-adapt/SKILL.md), [`cantos-audio-produce`](.agents/skills/cantos-audio-produce/SKILL.md) and [`cantos-story-audit`](.agents/skills/cantos-story-audit/SKILL.md) share the [Drive/local workspace contract](docs/production/story-workspace.md). They preserve raw sources, maintain `score/name_map.yml`, coordinate immutable score/audio revisions and recover blocked synchronization through local records and GitHub issues. **Only external agents run audio rendering**, never the Cantos backend. Keep all story data outside Git.

Skills own working method; `docs/` owns product rules. Keep one copy of each skill and update it rather than adding per-runtime instructions.

## Verification

Use the foundation's [verification cadence](.agents/skills/cantos-engineering/references/verification-cadence.md):
focused checks during DEV, affected module/renderer checks at CHECKPOINT, and preflight on the
final diff at HANDOFF, with CRITICAL evidence for changed high-risk invariants. Do not run a
full verification campaign after each edit or commit. Reuse attributable results while their
inputs remain valid; commit/push/PR packaging of the same content needs no duplicate run.

Follow the foundation's [layered verification order](.agents/skills/cantos-engineering/references/verification-strategy.md#layered-verification)
and [local handoff policy](.agents/skills/cantos-engineering/references/local-execution.md#local-handoff-and-pr-completion):
verify the pure core first and deliver independently verified layers through requested
commit/push/PR actions. Missing infrastructure does not hold passed core work pending; report
each layer's evidence, deferred acceptance, blocker and actual failure separately. Do not wait
for remote CI. Required checks and production acceptance retain their gates; use the existing
docs-only fallback when applicable.

Optional Studio identity operator tools are `cargo xtask kanidm reconcile --dry-run` (plan only)
and `cargo xtask kanidm verify` (explicit read-only discovery/JWKS, never login acceptance).
They never bootstrap accounts, grants, secrets, Podman or TLS; live steps require operator authority.

Tester distribution of the Theatre Android **demo** APK is the explicit, opt-in
`cargo xtask distribute firebase cantos-theatre --channel demo ...` ([contract](xtask/README.md#distribute-firebase)).
It is a remote write that may notify testers: `--dry-run` shows the plan, and an agent runs the real
command only on the user's explicit request naming the Firebase app and recipients. Build, run,
verify, preflight and ci never invoke it; an uncertain remote outcome is reconciled in the Firebase
console, never replayed blindly.

Repository-level operations go through `cargo xtask`; scripts and external tools are implementation primitives, not competing orchestration interfaces. Read the [command contract](xtask/README.md) before running or adding a check, and never add a parallel build/test/run entrypoint. Add meaningful crash/retry and publication-gate coverage when implementing those flows. UI changes require observations on relevant viewports/platforms, realistic Vietnamese text, focus/text scaling and accessibility; an image alone does not verify interaction or motion.

The table maps affected scope to checks at the applicable milestone, not a sequence to run
after every edit. Preflight already includes repository/format and selected suites; do not
run their standalone equivalents again when covered by that handoff run.

| Change | Required check at the applicable milestone |
| --- | --- |
| Repository hygiene (documents, skills, scripts) | `repository` gate, included in handoff preflight; `cargo xtask verify repository` for a focused hygiene check when needed |
| Completed handoff or a pull request | `cargo xtask preflight --changed-only` on the final diff, or cite a still-valid run covering it (broadens to every gate when impact is uncertain) |
| Rust code, Script IR or contracts | `cargo xtask verify rust` and `cargo xtask verify contracts` |
| Registered Quint models/tooling | `cargo xtask verify behavior` (model-only by default; `--conformance` adds the registered pure-core observer; explicit `--setup` for the locked development tool) |
| Script IR validator/encoder | additionally `cargo xtask verify mutation` |
| Studio or Theatre Web | `cargo xtask verify web` |
| Persistence, migrations or the HTTP host | `cargo xtask verify postgres` (PostgreSQL `initdb`, `pg_ctl` and `psql` on `PATH`) |
| Generic DBSP incremental engine (job summary binding retired) | `cargo xtask verify dbsp`; do not reintroduce a synthesis worker to satisfy it |
| Local PNG comparator/tooling | `cargo xtask visual-compare --self-test` (core plus optional pinned-Pillow tests); compare existing attributed pairs with `cargo xtask visual-compare` and open the images; metric PASS is not UX approval |

Use Rust 1.98.1 (`RUSTUP_TOOLCHAIN=1.98.1`, as in CI). PostgreSQL checks create a new disposable
cluster; never substitute a production or inherited database URL. `cargo xtask run`/`app` start
only local processes they own and never provision databases, identity or providers. Report each
gate as xtask printed it: `PASS`, `FAIL`, `NOT_RUN` (explicit `--skip` or not reached) or
`UNSUPPORTED` (missing tool, or a mobile surface without a Gradle project). Studio runtime
interaction/accessibility evidence is separate from the WASM build; no mobile runtime gate exists.
On affected macOS temporary-path setups, use `TMPDIR=/private/tmp` for Python unit/mutation runs.

For optional Web release asset/hash/compression inventory, use `cargo xtask web-performance
<cantos-studio|cantos-theatre>`; see the [measurement boundary](docs/evidence/web-performance-patterns.md).
Offline bundle sizes and reactive-work counters never establish browser startup, cache hits or
deployed headers. This command adds no required gate or production cache policy.

Static Compose UI capture goes through `cargo xtask compose-preview <cantos-theatre|ui-system-cmp>`;
read the [CMP preview workflow](.agents/skills/cantos-cmp-mobile/references/compose-preview.md).
It needs a real mobile Gradle project and Android Studio Quail 2+ with that project open, but no
emulator or phone. Theatre Mobile's isolated mock has Gradle Android preview wrappers;
capture requires the official CLI and a ready synced IDE, otherwise it is `UNSUPPORTED`.
Studio Mobile is excluded. The shared `ui-system-cmp` library has an Android preview host. Open the PNG and assert the observed semantics before claiming inspected/tested
evidence; host rendering does not replace Android/iOS runtime or screen-reader gates.

Do not report an app build, mobile test, audio quality result or end-to-end production run unless it actually ran. Record limitations and required follow-ups with evidence.

## Service runtime ownership

A service is a runnable process or independently managed infrastructure, not necessarily a Rust
crate. Follow [the service inventory](services/README.md) and
[ADR 0021](docs/decisions/0021-podman-service-runtime.md). Podman is the standard OCI runtime;
Docker Engine/Desktop/daemon and Compose are not dependencies (`docker.io` remains an allowed
registry). Only explicit `cargo xtask services up` starts local infrastructure. Build/test/run
never auto-start it. Native disposable PostgreSQL verification stays independent. Never install
Podman, initialize/start a machine or change its connection without explicit consent. Manage
only exact Cantos-owned labeled resources; stop never deletes persistent volumes. Keep secrets,
TLS and data outside Git, and distinguish configuration from observed runtime and integration.
