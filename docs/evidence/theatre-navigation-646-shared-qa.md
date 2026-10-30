# Theatre navigation #646: shared component checkpoint

Reviewed by Codex on 30 October 2026, one automated reviewer. This records bounded Chromium
and JVM Compose evidence, before app adoption of Avatar and the canonical Library icon.
It does not establish Android/iOS, screen-reader, deployed, personal-profile-data or pixel-match acceptance.

## Source and reference

Base: `d9f79480ab42342fae9582807c774405dcea50f2`; parent commit
`ed172bc1cf009c703337f80583b242ecbbd40834`, with the reviewed working diff present.
The source digests in the adjacent JSON records bind the actual shared sources and build inputs.
No old qualification record or compatibility baseline was edited.

References opened: the v9.2 overview and tablet drawer from issue #641's Drive handoff;
its Library 24-unit SVG; the owner's selected-border correction; the canonical token/component
contracts and current Web/JVM specimens. No exact design-pixel equivalence is claimed.
Only the Library icon registry addition, its closed-inventory test and the Avatar admission
coverage/preview size layout changed in shared packages. Existing scopes remain bounded by
[the prior component review](ui-requalification-20261027.md),
[loading admission](theatre-navigation-runtime.md) and the checks repeated below.

## Executed checks

- `cargo xtask ui-preview leptos verify`: PASS (SSR tests, WASM clippy, Trunk build).
  Log: `artifacts/agent-runs/646-v92/leptos-verify.log`.
- `cargo xtask ui-preview cmp verify`: PASS, 320 tests, no failures or errors.
  Final log: `artifacts/agent-runs/646-v92/cmp-verify-final.log`.
  These are actual JVM composable/semantics/input/clock tests, not only pure Kotlin tests.
- `cargo xtask ui-preview leptos run`: actual gallery on loopback 8083, Chromium,
  CSS viewport 1280 × 720, DPR 1, bundled fonts loaded. Pointer input, native Escape,
  root text scaling and read-only DOM observations described below.
- `cargo xtask ui-preview cmp run`: the owned Gradle process reached `run`; the native
  window could not be selected by computer use. OS-window input is NOT_RUN. The independent
  JVM test root, its real composables and opened captures provide the scoped runtime evidence.

## Web observations

The existing catalog was exercised with Light/Dark, 100/200% injected root text, reduced motion,
Vietnamese accents, normal/failed offline image and controlled editing. This text scale is a
preview composition-root setting, not browser zoom. Browser zoom/product responsiveness is separate.

- Icon: the 24-unit Library glyph shows three outlined book spines, including the slanted third
  book. Its geometry is shared filled contours, not a CSS stroke or font glyph. All 44 registry
  icons in the section have decorative SVG semantics. Both theme captures were opened.
- Avatar: single `N`, pair `NC`, no-initial plain tone, offline image, failed image returning `Đ`,
  small/medium/large sizes, named CJK pair. Adjacent-text avatars are decorative;
  the standalone named specimen has `role=img` and its localized name. Failed image did not
  leave an empty hole. The source-bound shared vectors separately cover Vietnamese decomposition,
  stable IDs and cross-renderer initials/tone parity. No real account identity was supplied.
- TextField/TextArea/Select/Checkbox: labelled fields and choices rendered with Vietnamese text;
  a textarea edited to `Ngày mai, mình có diễn tiếp không? Ỗ, Ữ — kiểm tra giữ nội dung.` retained
  that exact value after changing theme. Its help remains referenced by `aria-describedby`.
- Button/IconButton/FilterChip: actual ordinary/auth/account action specimens and Theatre glyph
  controls rendered; long labels wrap. Existing disabled/loading and callback checks ran.
- NavItem/BackButton: actual anchor navigation, selected destination and contextual Back examples
  were inspected. Current-page semantics are separate from a keyboard focus stroke.
- Badge/Status/Surface/ErrorState: tone-labelled feedback, account/layout surfaces and error recovery
  displayed. The polite LoadingState has a named `role=status`. Reduced-motion gallery placeholders
  had `animation-duration: 0s`, iteration 1, decorative `aria-hidden`, a named busy region and
  retained geometry; the animation name alone is not evidence of active repetition.
- Dialog: pointer-opened actual native modal, title association and focus inside confirmed;
  Escape closed it and returned focus to `gallery-dialog-open`.
- EdgeProgress: workbench recipe v1 / `edge-progress`, idle → pending → cancel via the actual outcome
  control; pending edge renders above retained content. Existing pure/DOM/JVM clock and lifetime
  tests run independently. No motion-smoothness or publication/API result is inferred.
- Logo: official local symbol/wordmarks on Light/Dark and transparent backing were opened.

Opened Web PNGs are ignored local artifacts under `artifacts/agent-runs/646-v92/`:
`leptos-icon-avatar-light.png`, `leptos-icon-avatar-dark-text2.png`,
`leptos-avatar-dark-text2.png`, `leptos-fields-text2.png`, `leptos-feedback-text2.png`,
`leptos-navigation-text2.png`, `leptos-dialog-text2.png`, `leptos-actions-text2.png`,
`leptos-pulse-text2.png`, `leptos-edge-pending.png`.
These are representative inspected sections, not a complete every-state Web visual matrix.

## JVM observations and a repaired preview issue

`AvatarAdmissionTest` exercises decoded-image loss on the production Avatar, preserves its
localized image name and 48-unit geometry, and renders the existing Avatar preview states,
all four sizes, a truthful empty-name decorative control and Home/Library glyphs in both themes
at 840/100% and 320/200%. The narrow `sizes` specimen initially squeezed its adjacent helper into
broken fragments; the preview owner now places that helper below the size row. The same scenario
was rerun and its final capture opened: complete Vietnamese words wrap normally. The final size-row helper uses the existing
`SyntheticPreviewText.narratorVi` resource; its changed scenario was rerun and the final
`avatar-library-646-dark-320.png` reopened, with the same sound layout.

Opened current PNGs in `packages/ui-system/cmp/build/reports/cantos-preview/`:

- `avatar-library-646-light-840.png`, `avatar-library-646-dark-320.png`.
- `cmp-theatre-avatar-theatre-control-light-text1.0-390.png`.
- `cmp-auth-badges-dark-390-text2.png`, `cmp-qualification-status-neutral-dark-text2.0-320.png`.
- `cmp-foundation-text-area-selection-dark-compact-text1.0-1140.png`.
- `cmp-qualification-dialog-dark-text2.0-320-focused.png` (editor focused; title is scrolled above).
- `cmp-theatre-text-field-theatre-fields-dark-text2.0-320.png`.
- `cmp-interaction-checkbox-label-activation-dark-comfortable-text2.0-372.png`.
- `cmp-theatre-icon-button-theatre-tonal-dark-text2.0-320.png`, `cmp-recipe-dark-320-text2-button.png`.
- `cmp-auth-account-dark-390-text2-error.png`, `cmp-select-229-dark-text2.0-390-selected-error.png`.
- `cmp-qualification-error-state-dark-text2.0-320.png`, `cmp-theatre-logo-theatre-header-dark-text2.0-320.png`.
- `cmp-focus-478-back-standard-dark-focused.png`, `cmp-interaction-nav-tabs-overflow-light-compact-text2.0-372.png`.
- `loading-admission-dark-320-reduced.png`, `edge-progress-dark-840.png`.
- `cmp-theatre-filter-chip-theatre-selection-dark-text2.0-320.png`.

The captures accompany actual Compose runtime tests and semantics assertions. They do not
replace native assistive technology or demonstrate animation smoothness. Avatar image retrieval,
validation and decoding remain host responsibilities. Application session/profile projections
still provide no authorized display-name/photo fact; personal identity acceptance remains open.

## Freshness

New records preserve the old receipts and point only to these actually reviewed inputs. Any
subsequent shared source, build, token or declared-state change requires another attributable
checkpoint. Product layout, focus, viewport and auth/route evidence are recorded separately.
