---
name: cantos-leptos-web
description: >-
  Web renderer execution for Cantos Studio and Cantos Theatre Web in Leptos: separate surface and
  permission contexts, component → resource/action → port → HTTP adapter layering, explicit async
  states with stale-response guards, DOM and HTMLMediaElement playback interop, the text-first
  script editor (stable dialogue keys, Vietnamese IME composition, revision conflicts),
  token-driven CSS/SCSS carrying Material 3 Expressive intent, and scoped browser evidence. Use
  when building, changing or testing Leptos components, routes, signals, resources, actions,
  web-sys code, the Studio script editor, casting or production-progress screens, the Theatre Web
  player or mini-player, Web stylesheets, startup/bundle costs, signal scope or cache behavior.
---

# Cantos Leptos Web

This skill is the **Web renderer's behavioral contract and router** for Cantos Studio and Cantos
Theatre Web. It composes two owners and redefines neither:

- [`cantos-engineering`](../cantos-engineering/SKILL.md) — the required order, the
  [evidence vocabulary](../cantos-engineering/SKILL.md#6-never-collapse-evidence-into-the-word-verified)
  and the [completion report](../cantos-engineering/SKILL.md#9-completion-report).
- [`cantos-ui-design`](../cantos-ui-design/SKILL.md) — the
  [shared UI loop](../cantos-ui-design/SKILL.md#shared-ui-loop), Material 3 Expressive intent,
  tokens, localization, accessibility, motion, component states, visual review and the
  [UI completion report](../cantos-ui-design/SKILL.md#completion-report).

Load this file, then only the references the change touches, one at a time.
For registered stateful scenarios, compose [cantos-behavior](../cantos-behavior/SKILL.md):
reuse catalog IDs and accepted `.qnt` expectations, keep platform-native tests and observations,
and never infer client/device acceptance from model simulation.

## When to use

Use for Leptos components, routes, signals, resources, actions, `view!` markup, `web-sys` or
`wasm-bindgen` code, CSS/SCSS, Web component or browser tests, and for these Web surfaces:
Studio import and adaptation review, the script editor, casting, external-audio intake/review, QC
and publication controls, and Theatre discovery, episode pages, the mini-player and full player.

| The task is instead about… | Use |
|---|---|
| visual intent, token values, type scale, motion intent, copy, accessibility targets | [`cantos-ui-design`](../cantos-ui-design/SKILL.md) (this skill applies its output) |
| Script IR shape, IDs, revisions, validator errors, import/adaptation output | [`cantos-script-ir`](../cantos-script-ir/SKILL.md) |
| external producer instructions, intake states, stale audio and QC records | [`cantos-production-pipeline`](../cantos-production-pipeline/SKILL.md) |
| rights, approvals, publish gates, manifests, signed delivery | [`cantos-publication`](../cantos-publication/SKILL.md) |
| playback state meaning, progress and bookmark sync, listener API | [`cantos-listening`](../cantos-listening/SKILL.md) |
| Android/iOS Theatre in Compose Multiplatform | [`cantos-cmp-mobile`](../cantos-cmp-mobile/SKILL.md) |
| a report-only UI/UX audit | [`cantos-ui-inspector`](../cantos-ui-inspector/SKILL.md) |
| reviewing an existing diff | [`cantos-code-review`](../cantos-code-review/SKILL.md) |

A Web screen that changes domain state composes the owner for that state: this skill decides how
the browser renders and drives it, the owner decides what is true.

## Ground rules

- **Repository reality.** Studio Web, Theatre Web's scoped live account/Library boundary and
  explicitly isolated listening mock, and the shared `cantos-ui`
  foundation are Leptos CSR packages built with Trunk. Build, serve and verify Web through
  [`cargo xtask`](../../../xtask/README.md) (`build|run|app <app> --web`, `verify web`);
  native string-render component tests and the package-local searchable development gallery exist.
  Its [scenario contract](../../../packages/ui-system/leptos/README.md#scenario-recipes-and-replay)
  owns supported recipe/mode/state inputs; build, verify and serve diagnostics with
  `cargo xtask ui-preview leptos build|verify|run`.
  Both Web clients have optional nonsecret `CANTOS_API_ORIGIN` addressing (ADR 0041); an unset
  build retains the local proxy. Direct cross-site session acceptance remains a separate gate.
  Browser observations remain separately recorded. Discover manifests, scripts and CI before citing a command. Other commands, crates, paths and
  types in this skill and its references are **proposed** or **candidates** until the repository
  implements them; a library becomes a dependency only through a recorded decision
  ([`templates/adr.md`](../../../templates/adr.md)) and a pinned, verified version.
- **One API group per client.** The Studio client calls only `/api/v1/studio/*` (base from
  `cantos_contracts::routes::STUDIO_BASE`); the Theatre client calls only `/api/v1/theatre/*`.
  Neither calls the legacy unprefixed `/api/v1/*` paths or the other group.
- **Forge has a separate client boundary.** Accepted [ADR 0022](../../../docs/decisions/0022-cantos-forge-producer-cli.md)
  selects a Studio workspace backed by a separate producer. Follow the
  [worker acceptance boundary](../../../docs/deployment/forge-worker.md); never import its
  service crate, reuse the Studio cookie cross-origin or infer cloud access from a local mock.
  This product direction does not mean the Forge tab or invocation grants are implemented.
- **Product requirements are owned by docs.** [`apps/cantos-studio/web/README.md`](../../../apps/cantos-studio/web/README.md),
  [`apps/cantos-theatre/web/README.md`](../../../apps/cantos-theatre/web/README.md),
  [repository layout](../../../docs/architecture/repository-layout.md),
  the [UI system](../../../docs/design/ui-system.md), and the work items
  [Script/editor acceptance](../../../docs/assurance/script-editor-acceptance.md),
  [020 (casting, progress)](../../../docs/work-plan/020-cast-and-generate-dialogue.md),
  [030 (QC, publication)](../../../docs/work-plan/030-mix-review-and-publish.md) and
  [040 (Theatre Web)](../../../docs/work-plan/040-theatre-web-listening.md). Link them; never
  restate a competing copy of a rule.
- **Distinguish proposed, implemented and verified** in code comments, docs and reports.
- **Performance needs scoped measurements.** For startup, bundle, cache or reactive-cost work,
  load [performance and caching](references/performance-and-caching.md): establish route/rendering
  mode, four cache layers, signal read/lifetime scope, comparable before/after conditions and
  browser/platform evidence. Build success, smaller bytes and warm HTTP reuse alone prove no speedup.

## Web non-negotiables

Before product markup, search public APIs and the preview catalog for the component ID, variant
and state; record reuse or the missing-contract rationale. Extend an existing owner where it fits.
A missing kind gets a stable declaration, anatomy, typed states/intents and preview scenarios
before product code, then [preview admission](../cantos-ui-design/references/preview-catalog.md)
before adoption. Keep Web-only compositions Web-owned and shared APIs under both-renderer gates.
Keep existing gallery fixtures and controlled state; source/string-render tests are not visual approval.

| Rule | Failure it prevents | Oracle | Depth |
|---|---|---|---|
| Studio and Theatre are separate surfaces and permission contexts with their own route roots, contexts and ports | a listener route reaches a private draft, preview or Studio port | Theatre routes mount with only `TheatreContext`; backend authorization tests | [`web-architecture.md`](references/web-architecture.md#1-two-surfaces-two-permission-contexts) |
| Components render and dispatch; ports come from context; adapters own URLs, DTOs and error classification | transport and provider details leak into markup and cannot be tested without a network | component test runs on an in-memory port | [`web-architecture.md`](references/web-architecture.md#2-layering-inside-a-surface) |
| Durable rules live in pure core crates; the browser renders server gates (approval, publish, intake scope) and never decides them | drifted copies of gates; UI-only permission checks | review; same validator crate on both sides | [`web-architecture.md`](references/web-architecture.md#3-which-decisions-the-browser-may-make) |
| No credential, private object access or signed URL in client code, build-time config, routes, storage or logs | secrets shipped in the bundle; private media made shareable | review; proposed bundle grep | [`web-architecture.md`](references/web-architecture.md#5-secrets-and-private-media-never-reach-browser-code) |
| Every remote state is explicit, failure is never empty, a stale response never wins, every mutation states its duplicate-submit behavior | "No episodes" on a 500; an old response replacing a new one; duplicate intake or save | out-of-order component test; per-state tests | [`leptos-async.md`](references/leptos-async.md) |
| A failed or conflicting request never costs the creator text; conflicts are never last write wins | lost dialogue after a timeout; silent overwrite of a competing edit | failing and conflicting fake ports | [`script-editor.md`](references/script-editor.md#4-draft-save-and-version-states) |
| Playback state comes from media events through the shared reducer; one shell-owned `<audio>` survives navigation | a "playing" button over blocked audio; playback stopping on route change | real-browser playback and navigation test | [`dom-and-media-interop.md`](references/dom-and-media-interop.md#6-theatre-playback-through-htmlmediaelement) |
| The editor keys rows by dialogue ID and never commits, reformats or rewrites text during IME composition | Telex/VNI input corrupted; edits attached to the wrong line | insert-above focus test; composition regression; recorded IME walkthrough | [`script-editor.md`](references/script-editor.md) |
| Styles consume token roles; classes are semantic; state styles hang off semantic attributes; themes, reduced motion and focus are always handled | literal colors and radii; utility walls; invisible focus | manual review now, proposed style gate later; inspected captures | [`styling.md`](references/styling.md) |
| Evidence is named exactly: `DOM-tested` ≠ `interaction-tested` ≠ `integration-tested`; `compiled` and `screenshot-captured` are not inspection | overclaiming from fakes, synthetic events or unopened images | report review against the vocabulary | [`web-testing.md`](references/web-testing.md) |
| JSON or MessagePack is chosen once per route group by a client constant; responses decode by their own `Content-Type`; a request is sent once, never retried in another codec | a JSON error read as garbage; a timed-out save applied twice | host tests on the client `wire.rs`; bundle check for the JSON build | foundation [`wire-codec.md`](../cantos-engineering/references/wire-codec.md) |

How the foundation lands in the browser:

- **Decoupling.** Web depends on pure core crates and the shared `cantos-contracts` DTOs, never on
  backend application or infrastructure crates, provider SDKs or Narrative Forge types.
- **Immutability.** Reducers take state by value and return new state; a save snapshot is an
  immutable value; accepted revisions, approvals and release manifests arrive read-only and are
  never patched client-side.
- **Functional core.** Editor, playback, retry, poll-schedule and merge decisions are pure and
  tested with plain `#[test]`; Leptos and `web-sys` are the shell.
- **Types as proofs.** `Retryability::UnkeyedWrite` cannot be retried; `SpeakerRef::Unresolved` is
  a state, not an empty string; exhaustive matches map every validator and port error. Apply
  foundation [typed errors and composition](../cantos-engineering/references/typed-errors-and-composition.md)
  through [Leptos async](references/leptos-async.md#typed-results-and-error-projection):
  `Result` carries failure, `Option` carries meaningful absence, and UI projection preserves
  correction/reconciliation facts without erasing the form.
- **Clean syntax.** Readable `view!` with semantic classes, small named helpers, no `unwrap()` on
  browser globals, formatter-clean Rust and stylesheets.
- **Web localization before markup.** Current Theatre copy lives in `messages.rs`,
  `features/auth_copy.rs` and `membership_preview/copy.rs`; Studio uses
  `messages.rs`, `messages/*`, `live/copy.rs`, `live/dashboard_copy.rs` and nested
  `live/{delegation,artwork}/copy.rs`. Before adding a label, tooltip, status,
  aria-name, dialog or preview text, follow the caller to its existing copy owner,
  then reuse a matching key or extend that owner's paired vi-VN/en API. Equal wording
  does not authorize cross-app catalog moves. Do not add
  another `resources!` macro, local translation map or hard-coded `view!` text.
  This is an interim ownership policy, not selection of `leptos_i18n` or
  `leptos_fluent`; compare those and a typed Rust + Fluent/ICU4X option in
  [#168](https://github.com/loveoverflowcom/cantos/issues/168) and
  [#167](https://github.com/loveoverflowcom/cantos/issues/167). Keep the locale
  reactive without remounting active Studio forms or the player; check
  `<html lang>`, a11y copy and story-language spans agree. Named parameter
  interpolation must never expand braces inside inserted titles. Use focused
  catalog tests while developing; defer broad verification to checkpoints.
- **Material 3 Expressive.** Applied through tokens at each surface's intensity: a quiet,
  rectangular reading area in the editor; more expressive shape and motion in Theatre discovery and
  the player ([`material3-expressive.md`](../cantos-ui-design/references/material3-expressive.md)).

## The Web development loop

Run the [shared UI loop](../cantos-ui-design/SKILL.md#shared-ui-loop); at each of its steps, apply
these Web specifics:

```text
0  discover   the product's package (apps/cantos-studio/web or apps/cantos-theatre/web), toolchain config,
              scripts, CI. If something is missing, the work item establishes it through a reviewed
              decision; never hand-roll a server or runner
1  scope      surface + permission context, route, ports, the owning doc's acceptance line,
              the claim and its failure mode
              lookup component ID/API/variant/state; reuse/extend or declare + preview missing kind
2  core       a durable decision? → its owner's pure crate with plain #[test] first
3  boundary   port provided through context; HTTP adapter over cantos-contracts DTOs, with the
              representation fixed by the client's codec constant; typed PortError
4  async      resource/action states, request key, duplicate-submit policy, retry decision
5  preview    semantic HTML, controlled values/intents, locale keys, token classes and injected preferences
6  evidence   cheapest regression first: core → component → select/reload recipe → drive browser inputs → capture/open
7  adopt      admission → product import → documented app command → SAME scenario and opened captures
8  tidy       formatter and linter of every touched toolchain, in check mode
9  report     shared UI report + the Web additions below; residual risk never empty
```

Step 8 candidates, to confirm against the real manifests: `cargo fmt --check`, Clippy for the
`wasm32-unknown-unknown` target with warnings denied, a `view!` formatter such as `leptosfmt`, and
the stylesheet formatter the package adopts. Skipping a step is a decision you state.

| The change needs… | Put it in | Never in |
|---|---|---|
| a validation, merge, reconciliation, retry or schedule decision | the owner's pure core crate | a component, resource or effect |
| a network call | a port and its HTTP adapter | a component |
| state that must survive navigation (playback session, editor draft) | the surface shell or the route root | a route leaf or a pane |
| a browser API | a `platform/` helper with a typed error | a component body |
| a color, radius, spacing or motion value | a token role ([`tokens.md`](../cantos-ui-design/references/tokens.md)) | a component literal |
| visible or accessible text | locale resources ([`localization.md`](../cantos-ui-design/references/localization.md)) | a `view!` literal |
| theme/density/locale/font scale/motion or gesture policy; immutable projection + intents | scoped composition root/context or value injection | OS reads, service/port/mock construction or effects inside a reusable component |
| an authorization or gate decision | the backend | anywhere in WASM |

Before handing off, check states with [`component-states.md`](../cantos-ui-design/references/component-states.md),
semantics and focus with [`accessibility.md`](../cantos-ui-design/references/accessibility.md),
motion with [`motion.md`](../cantos-ui-design/references/motion.md), and captures with
[`visual-review.md`](../cantos-ui-design/references/visual-review.md).

## Web report additions

Extend the [UI completion report](../cantos-ui-design/SKILL.md#completion-report), which already
extends the [foundation report](../cantos-engineering/SKILL.md#9-completion-report), with:

```text
Surface / permission context:  Studio | Theatre; routes; ports and adapters touched
Component lookup / scenarios: IDs/APIs/variants reused/extended; missing-kind rationale; scenario IDs and preview admission evidence
Rendering mode / build:        CSR | SSR per ADR; exact build and serve command; debug | release
Browsers / engines:            engine and version per result; headless | headed; flags used
Backend for evidence:          fake ports | named local services | shared environment
Async races covered:           stale response, duplicate submit, unmount, retry exhaustion
Input methods:                 keyboard, pointer, touch emulation, IME (CDP-simulated | named OS IME)
Media path:                    none | generated local file | real storage/CDN; range requests seen
Style review:                  manual token, focus and motion review scope (no gate implemented)
Performance / cache scope:     changed scenarios; four-layer classification; signal read/lifetime decisions; baseline/delta or unrun measurements; cold/resource-warm/code-cache evidence; adopt/defer/reject
```

A skill-only or docs-only edit needs no app run; say that no Web behavior was exercised.

## References — load the relevant set, one at a time

| The work is about… | Reference |
|---|---|
| surfaces, permission contexts, layering, ports in context, core crates in WASM, secrets, shell-owned mini-player, modules, SSR vs CSR | [`web-architecture.md`](references/web-architecture.md) |
| binary request/response bodies, the codec switch, decoding by `Content-Type`, codec bundle cost | foundation [`wire-codec.md`](../cantos-engineering/references/wire-codec.md) |
| startup/bundle/interaction cost, cache eligibility, cold/warm/code-cache claims, signal clone/scope cost, rendering/splitting experiments | [`performance-and-caching.md`](references/performance-and-caching.md) |
| resources, actions, explicit states, stale responses, duplicate submits, retries, optimistic UI, job progress | [`leptos-async.md`](references/leptos-async.md) |
| Result/Option, typed port failures, exhaustive UI error projection and recovery | foundation [typed errors](../cantos-engineering/references/typed-errors-and-composition.md) + [Web application](references/leptos-async.md#typed-results-and-error-projection) |
| `web-sys`, listeners and cleanup, focus and dialogs, shortcuts, storage, `HTMLMediaElement`, Media Session, autoplay, ranges, signed URLs | [`dom-and-media-interop.md`](references/dom-and-media-interop.md) |
| the Studio editor: dialogue identity, IME composition, save/version/conflict, undo, reordering, speakers, validation, large scripts | [`script-editor.md`](references/script-editor.md) |
| CSS custom properties, SCSS, themes, reduced motion, focus, container queries, Vietnamese text in layout, proposed style gate | [`styling.md`](references/styling.md) |
| evidence layers, TestWorld, determinism, stale-response test, real-browser checks, visual loop, matrix, evidence storage | [`web-testing.md`](references/web-testing.md) |

## Compose with

Foundation: [`cantos-engineering`](../cantos-engineering/SKILL.md). Shared UI policy:
[`cantos-ui-design`](../cantos-ui-design/SKILL.md). Domain owners:
[`cantos-script-ir`](../cantos-script-ir/SKILL.md) for editor content and validation,
[`cantos-production-pipeline`](../cantos-production-pipeline/SKILL.md) for casting, external producer
instructions and intake, [`cantos-publication`](../cantos-publication/SKILL.md) for QC, approval and publish
controls, and [`cantos-listening`](../cantos-listening/SKILL.md) for
[playback semantics](../cantos-listening/references/playback-semantics.md) and
[progress sync](../cantos-listening/references/progress-sync.md). Scenarios for the visual loop:
[`cantos-ui-inspector`](../cantos-ui-inspector/references/scenarios.md). Native listening shares
vocabulary and state meaning with [`cantos-cmp-mobile`](../cantos-cmp-mobile/SKILL.md), never
runtime code. Task entrypoints [`cantos-work-item`](../cantos-work-item/SKILL.md) and
[`cantos-code-review`](../cantos-code-review/SKILL.md) load this skill for Web changes.
