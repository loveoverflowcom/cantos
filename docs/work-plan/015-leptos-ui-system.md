# Leptos UI system sequence

This is the requested UI-only sequence alongside the MVP queue, not advancement of #6/#7 or
production acceptance. GitHub owns linked execution status; this file owns next order and scope.
The [foundation](../../packages/ui-system/README.md) and [component library](../evidence/ui-system-components.md) establish the local shared boundary. Later sessions
must inspect their actual starting SHA and disk budget; they are not started by this task.

## Why and boundary

The original sequence established one shared token source, composable renderers and diagnostic
previews. Current APIs and consumers are recorded in the
[Leptos package](../../packages/ui-system/leptos/README.md) and
[shared catalog](../../packages/ui-system/shared/README.md#preview-registration-and-adoption);
inspect them before extending the foundation. Preserve backend permissions, unsaved text,
revision identity and native playback architecture. Do not copy the older client-shell branch
over the current apps.

## Sequential ownership

| Session | Outcome and files | Acceptance / command focus |
| --- | --- | --- |
| 1 — Ultra | Canonical schema, CSS/Rust generator, official icon subset, minimal UiRoot/Button/Icon and preview; `packages/ui-system`, generator/tests, migration docs and minimal Studio theme wiring | Token validation/determinism/contrast; native render smoke; focused WASM lint/Trunk; record runtime gaps |
| 2 — Ultra | Component library in `packages/ui-system/leptos/src` + component CSS template, tests and preview; contracts in package docs | Extend native buttons (secondary/danger/loading), icon buttons, labeled fields with help/error, chips/navigation, reading surface, status and dialog anatomy; props/children/repeatable slots/controlled state/events; keyboard/focus/dialog restore and state matrix; no app business rules |
| 3 — Extra High | Integrate existing `apps/cantos-studio/web` and bounded `apps/cantos-theatre/web` surfaces; app-owned patterns, messages and styles | Scope broad selectors, preserve editor/IME/unsaved/conflict flows, responsive panes/navigation at measured widths; inspect320/600/840/1440, both themes,200% text; use production status honestly and keep playback unavailable until its gates |
| 4 — Extra High | Independent read-only review through `cantos-code-review`, exact final range; evidence and risk report under `docs/evidence` | Check token drift, dependency graph, DOM/keyboard/focus, contrast over actual states, Vietnamese text, reduced motion/forced colors, reachable labels and app gate preservation; fixes are explicit separate scoped work |

Session 2 inherits API shapes, source v0.2.0 and `cui-` CSS scope from the package. Expand a role
only for an actual component; never create per-app theme files. Name application-specific patterns
in their app, including revision/production/QC/approval. Include realistic long Vietnamese names,
disabled explanations, errors and empty/loading states in component examples. Each session updates
the existing evidence record and this queue rationale without claiming unrun gates.

Session 3/4 regression consideration: [Google adapter evidence](../evidence/google-tts-adapter.md)
records the original intensity0 limitation. [PR29 casting evidence](../evidence/studio-provider-casting.md) now owns its correction, provider/model switching, reactive bounds and preserved unsupported input.
Numeric field/component APIs must preserve valid zero values. This is a contract consideration,
not a TTS feature or authorization to change the backend/provider gates in this sequence.

## Reproducible baseline

From repository root:

```sh
CARGO_BUILD_JOBS=2 cargo xtask verify repository
CARGO_BUILD_JOBS=2 cargo xtask verify rust
CARGO_BUILD_JOBS=2 cargo xtask verify web
CARGO_BUILD_JOBS=2 cargo xtask preflight --changed-only
```

Run app-native checks only when their owner changes. PostgreSQL, TTS and mobile verification
are not substitute evidence for this UI work. No Docker, SDK/model install, wholesale target
deletion, remote CI monitoring, push/PR/merge or shutdown is part of this local sequence.

## Risks / unknowns

Runtime visual/focus/zoom/reduced-motion evidence requires an unlocked Mac and available browser.
The named fonts need licensed delivery; system fallbacks are used meanwhile. The minimal preview
is a diagnostic sample, not a production site. Refer to [foundation evidence](../evidence/ui-system-foundation.md)
for actual runs; generation/compile/string-render results do not establish native accessibility.

## Component handoff and consumer review order

Session 3 should consume the [component API](../../packages/ui-system/leptos/README.md) and
[session 2 evidence](../evidence/ui-system-components.md) before touching app surfaces. Scope broad
app tag selectors first, then migrate a representative edit/casting flow and bounded Theatre
navigation. Preserve controlled raw numeric zero and Vietnamese composition hooks. Native
NavTabs uses links; an in-page ARIA tab widget is not supplied. Select option children need
caller-owned selected bindings when options are inserted asynchronously without a value change.
Run exact viewport, zoom, focus, IME and app state preservation checks against the integrated
surfaces; component gallery observations do not establish app or mobile acceptance.

Independent session 4 should inspect the [consumer integration record](../evidence/ui-system-integration.md)
and exact local commit before reviewing the whole branch. PR29 was reconciled through local merge
`83fbadb40c7655d5721c6c8b346638390ef49c74`, whose second parent is develop
`03ba1416aa0e646b1d6022534f97e44f5b2ea1a4`. Compare UI-only changes against that merged develop
context as well as the original requested base; do not attribute PR29's accepted behavior or
historical Linux browser evidence to this Mac integration. Prioritize mounted-pane preservation,
native Enter submission, stable casting options/neutral0, dialog Escape/return focus and CSS
scope at 320/360/600/768/840/1440, both themes and 200% zoom. Source and native HTML-string
checks do not settle those runtime gates. No later session starts automatically.

## Final review disposition

Session 4's [independent review](../evidence/ui-system-review.md) found and fixed duplicate mounted
validation IDs, Safari select sizing/reflow, native dialog overflow and pointer-trigger return
focus. Safari observations now cover a bounded local gallery and Studio fixture flow at measured
320/360/768/1440 CSS iframe widths, with native page zoom and direct-page dialog checks recorded
separately. VoiceOver, OS IME, reduced motion/forced colors, other browsers and mobile remain
explicit evidence gaps. No production or listening acceptance follows from these checks.

The authorized local PR30/develop reconciliation is recorded in that review, including canonical
shared-path and gallery gate compatibility. The requester's subsequent VotHub refinement uses
filled tonal controls, spacing and quiet inspector containers; its actual light/dark, 320/desktop,
keyboard and 200% Safari observations are recorded separately from earlier captures. Preserve
PR29's casting/neutral-zero contracts and the deferred CMP boundary. This queue recommends closing the requested UI sequence
with its evidence, then addressing only those residual gates a subsequent request selects.

## Subsequent publication request

The requester authorized pushing all UI commits and merging into develop after the local
sequence. [PR #33](https://github.com/loveoverflowcom/cantos/pull/33) owns execution status;
the [publication record](../evidence/ui-system-publication.md) records final reconciliation with
PR31/PR32, the mutation workspace-copy correction and actual combined local verification.
The UI decision is now ADR 0014, preserving upstream MessagePack ADR 0013. This request does
not start CMP or advance production/listening acceptance; residual UI gates remain explicit.

## Subsequent two-preview request

The later user request adds a concrete CMP desktop diagnostic app beside the existing Leptos
gallery, a shared component/state registry, named-token inspection and enforceable before-app
admission. [Preview evidence](../evidence/ui-system-previews.md) records the exact local checks.
Keep mobile products/listening and production backend scope unchanged. The next required step for
an unqualified new component is its missing target runtime/accessibility/design-reference evidence;
do not promote it to a real app from source parity alone. Platform-specific differences require
reviewed explicit reasons, not a pretend cross-renderer pass.

[PR41](https://github.com/loveoverflowcom/cantos/pull/41) now owns this preview slice's execution.
Its Mac review reconciles PR39/40, tightens admission/source freshness and fixes CMP selection,
availability and canonical field semantics. Bounded actual Safari observations are recorded in
the preview evidence; two-renderer qualifications remain 0/29. The supported Mac continuation
now records Gradle 8.14.3/JDK 21 compilation, all 23 CMP tests, corrected inspected captures,
owned-process cleanup and full preflight PASS. Gradle 9.7.0 remains available for the other
projects that require it. Recommended next order is the separately requested three CMP sessions,
closing OS-window input, accessibility, fonts and complete reference/state coverage before new
app adoption. These preview checks are not mobile or full CMP production acceptance.

The requested follow-up is tracked in [016](016-cmp-ui-system.md), with
[session 1 evidence](../evidence/cmp-ui-system-session1.md). It keeps Leptos consumers compatible
and adds companion specimens; application adoption remains gated.

## Reproducible Leptos preview scenarios

[Issue142](https://github.com/loveoverflowcom/cantos/issues/142) adds typed, versioned URL recipes
to the existing gallery, with catalog/isolated/matrix diagnostics over existing production
controls. The [package contract](../../packages/ui-system/leptos/README.md#scenario-recipes-and-replay)
owns supported components/states, environment settings, reset and fallback; the
[evidence record](../evidence/leptos-preview-scenarios.md) owns actual browser observations and
remaining acceptance. Frame-width simulation, still inspection and real browser/motion input
evidence remain separate.

Prioritize replaying exact existing component scenarios before expanding
[#141](https://github.com/loveoverflowcom/cantos/issues/141)'s proposed component batches. Keep
future sheet/player gesture consumers deferred until they have a concrete input/focus/cancel
contract. [#140](https://github.com/loveoverflowcom/cantos/issues/140) owns comparison metrics;
[#129](https://github.com/loveoverflowcom/cantos/issues/129) and
[#131](https://github.com/loveoverflowcom/cantos/issues/131) own product rollout. Scenario evidence
does not refresh shared qualifications, add product diagnostics or complete native/screen-reader
acceptance. GitHub owns execution status; this sequence records the next review boundary.
