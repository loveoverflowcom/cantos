# Select qualification for current Theatre consumers — PR 304

Repository verification exposed two existing imports without the required two-preview evidence:
`apps/cantos-theatre/web/src/features/public_home.rs` and `src/view/live_shell.rs` use shared
Select with Compact density and Inline label. The admission guard and compatibility baseline
are unchanged. This record supplies independent current Web and JVM evidence for those language
selectors and the shared controlled-value baseline. It adds no UI variant or application behavior.

The reviewed design reference is the existing Select anatomy in the shared UI catalog,
[UI system](../design/ui-system.md), canonical token roles and the existing
[Web density contract](web-select-density-221.md). Label, value, disclosure, selected check,
keyboard-active option, helper/error and blocked reason remain distinct. Existing shape, surface,
foreground/focus and48px target roles were inspected; no new palette or pixel reference is claimed.
Earlier preview-only/unqualified notes are historical registration metadata, not this acceptance
record. Future variants and platform capabilities still require their own scoped evidence.

## Web observations

Linux Codex in-app browser, actual Leptos CSR via `cargo xtask ui-preview leptos run`, after
`cargo xtask ui-preview leptos verify` PASS (native SSR, WASM lint and Trunk build). Recipe
`component=select`, frame320, Compact, vi/en, light/dark, text100/200, isolated mode. Real browser
viewports320×844,390×844 and1280×900 were used and restored. Frame and viewport are distinct.

- Regular/Compact custom menu and the explicit native compact baseline exposed localized names.
  Enter opened the listbox with selected option. End/Escape kept vi-VN and emitted no intent;
  Enter/End/Enter committed en once. A separate native selection committed vi-VN once.
- Regular End/Tab committed dark once and moved focus to Compact. Pointer selection of English
  committed once and restored focus to the exact Compact trigger. Disabled option was visible
  separately from selected/active state. No backend call is involved.
- Loading disabled all three controls, exposed `aria-busy=true` and associated the reason.
  Disabled native baseline was inert. Error exposed `aria-invalid=true` and helper/error IDs.
- Empty options and missing value showed the placeholder while retaining the caller's system/
  missing value and emitted no callback. Gallery option reordering retained narrator and callback0.
- Open menus stayed within the inspected viewport; long Vietnamese labels wrapped at200% text.
  Root text remained16px and no global horizontal overflow was observed. Screen-reader behavior,
  unsupported-Popover-engine emulation and browser200% zoom were not run.

Opened captures: `select-light-320-open.png`, `select-dark-390-200-open.png`,
`select-dark-1280-en-open.png` under local `artifacts/agent-runs/pr304/`. The last uses frame320
inside the1280px viewport. No screenshot alone is used as the keyboard/interaction oracle.

## CMP JVM observations

`cargo xtask ui-preview cmp verify`: fresh215 tests, zero skips/failures/errors; no new test or
production implementation was needed. `SelectAffordanceTest` asserts Escape focus return, exact
one callback including value0, loading closes popup and recovery never reopens it. The current
`InteractionComponentTest` asserts stable values across reordered/rebuilt options, missing/
unsupported options, named role/focus and disabled/error reason. These tests call production
Select, not a reimplemented specimen.

Opened actual captures at320/light/100%,390/dark/200%,840/light/100% showed selected value,
error, disclosure, selected check, disabled option/reason and wrapping. Empty/missing specimens
at372px retained placeholder and the caller's unchanged value. The rendered semantics export
for390/dark/text2 exposes the full label/value, Button role, collapsed description, error and
native actions. Images are `cmp-select-229-*-open.png`, `*-selected-error.png` plus the
`cmp-interaction-select-empty-options-*` / `*-missing-value-*` files listed in local
`select-inputs.json`. All cited PNGs were opened. An exploratory extra-frame capture produced
the same image hashes; that diagnostic test edit was removed and is not part of the patch.

Local immutable copies, opened image hashes, browser DOM and semantics exports are retained under
`artifacts/agent-runs/pr304/select-cmp-captures/`, `select-inputs.json` and `select-browser-dom.txt`.
The JSON qualification records pin exact renderer/token digests. These local artifacts are ignored,
not uploaded automatically with the PR.

## Scope and residual acceptance

Evidence: `example-tested`, `DOM-tested`, `semantics-tested`, `interaction-tested`,
`accessibility-checked` and `screenshot-inspected` on the named renderers. Qualitative comparison
uses the existing component/token contract; no exact design-pixel approval is fabricated.
No TalkBack/VoiceOver/NVDA, Android/iOS device, browser engine/forced-colors matrix, unsupported
Popover engine, live language persistence or production account/API acceptance was exercised.
Those remain application/platform acceptance obligations. JVM popup behavior is not evidence
for the Web popup or mobile operating system; each renderer's observations stand independently.
