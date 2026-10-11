# Leptos UI system sequence

This is the requested UI-only sequence alongside the MVP queue, not advancement of #6/#7 or
production acceptance. GitHub owns linked execution status; this file owns next order and scope.
The [foundation](../../packages/ui-system/README.md) and [component library](../evidence/ui-system-components.md) establish the local shared boundary. Later sessions
must inspect their actual starting SHA and disk budget; they are not started by this task.

## Why and boundary

Studio has a real editor but only a Studio-specific token generator; Theatre remains a skeleton.
One validated shared palette and a small composable renderer foundation reduce drift before
adding components. Preserve backend permissions, unsaved text, revision identity and native
playback architecture. Do not copy the older client-shell branch over the current apps.

## Sequential ownership

| Session | Outcome and files | Acceptance / command focus |
| --- | --- | --- |
| 1 — Ultra | Canonical schema, CSS/Rust generator, official icon subset, minimal UiRoot/Button/Icon and preview; `packages/ui-system`, generator/tests, migration docs and minimal Studio theme wiring | Token validation/determinism/contrast; native render smoke; focused WASM lint/Trunk; record runtime gaps |
| 2 — Ultra | Component library in `packages/ui-system/leptos/src` + component CSS template, tests and preview; contracts in package docs | Extend native buttons (secondary/danger/loading), icon buttons, labeled fields with help/error, chips/navigation, reading surface, status and dialog anatomy; props/children/repeatable slots/controlled state/events; keyboard/focus/dialog restore and state matrix; no app business rules |
| 3 — Extra High | Integrate existing `apps/cantos-studio/web` and bounded `apps/cantos/web` surfaces; app-owned patterns, messages and styles | Scope broad selectors, preserve editor/IME/unsaved/conflict flows, responsive panes/navigation at measured widths; inspect320/600/840/1440, both themes,200% text; use production status honestly and keep playback unavailable until its gates |
| 4 — Extra High | Independent read-only review through `cantos-code-review`, exact final range; evidence and risk report under `docs/evidence` | Check token drift, dependency graph, DOM/keyboard/focus, contrast over actual states, Vietnamese text, reduced motion/forced colors, reachable labels and app gate preservation; fixes are explicit separate scoped work |

Session 2 inherits API shapes, source v0.2.0 and `cui-` CSS scope from the package. Expand a role
only for an actual component; never create per-app theme files. Name application-specific patterns
in their app, including revision/production/QC/approval. Include realistic long Vietnamese names,
disabled explanations, errors and empty/loading states in component examples. Each session updates
the existing evidence record and this queue rationale without claiming unrun gates.

Session 3/4 regression consideration: [Google adapter evidence](../evidence/google-tts-adapter.md)
records that Studio cannot yet explicitly enter intensity0 for providers without intensity support.
Numeric field/component APIs must preserve valid zero values. This is a contract consideration,
not a TTS feature or authorization to change the backend/provider gates in this sequence.

## Reproducible baseline

From repository root:

```sh
python3 scripts/ui_tokens.py --check
python3 scripts/ui_icons.py --check
python3 scripts/check_repository.py
TMPDIR=/private/tmp python3 -m unittest discover -s scripts -p 'test_*.py'
cargo fmt --all -- --check
CARGO_BUILD_JOBS=2 cargo test -p cantos-ui --locked
CARGO_BUILD_JOBS=2 cargo clippy -p cantos-ui -p cantos-studio --target wasm32-unknown-unknown --features cantos-ui/csr --locked -- -D warnings
(cd apps/cantos-studio/web && CARGO_BUILD_JOBS=2 NO_COLOR=true trunk build --locked)
(cd packages/ui-system/leptos && CARGO_BUILD_JOBS=2 NO_COLOR=true trunk build preview.html --locked --dist ../../../target/ui-foundation-preview)
git diff --check
```

Run app-native checks only when their owner changes. PostgreSQL, TTS and mobile verification
are not substitute evidence for this UI work. No Docker, SDK/model install, wholesale target
deletion, remote CI monitoring, push/PR/merge or shutdown is part of this local sequence.

## Risks / unknowns

Runtime visual/focus/zoom/reduced-motion evidence requires an unlocked Mac and available browser.
The named fonts need licensed delivery; system fallbacks are used meanwhile. The minimal preview
is a diagnostic sample, not a production site. Refer to [foundation evidence](../evidence/ui-system-foundation.md)
for actual runs; generation/compile/string-render results do not establish native accessibility.

## Component handoff and next integration order

Session 3 should consume the [component API](../../packages/ui-system/leptos/README.md) and
[session 2 evidence](../evidence/ui-system-components.md) before touching app surfaces. Scope broad
app tag selectors first, then migrate a representative edit/casting flow and bounded Theatre
navigation. Preserve controlled raw numeric zero and Vietnamese composition hooks. Native
NavTabs uses links; an in-page ARIA tab widget is not supplied. Select option children need
caller-owned selected bindings when options are inserted asynchronously without a value change.
Run exact viewport, zoom, focus, IME and app state preservation checks against the integrated
surfaces; component gallery observations do not establish app or mobile acceptance.
