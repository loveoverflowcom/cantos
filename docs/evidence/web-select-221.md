# Web Select candidate — #221

This is source-bound implementation evidence, not product acceptance or a new design policy.
Execution status belongs to [#221](https://github.com/loveoverflowcom/cantos/issues/221) and its
PR. The ordered repair continues with [#222](https://github.com/loveoverflowcom/cantos/issues/222)
and [#223](https://github.com/loveoverflowcom/cantos/issues/223) on the same branch/PR.

## Scope and baseline

Base: `develop@b53c3af6abe25f3bd9aed706c754799d2c359324`, synchronized by fetch, switch and
fast-forward-only pull; local `develop`, `origin/develop` and remote HEAD matched. Existing
worktrees and local history were preserved. This repair does not change #203/PR #218.

The source confirms that existing `Select` styles only the native trigger and leaves the OS
popup in charge. Account theme, Settings theme and topbar locale have separate native markup.
The screenshots described in the issue were not available as original image bytes here;
runtime reproduction and computed-style confirmation were not performed.

Invariant: browsing options cannot change the caller's selected value; only an explicit enabled
choice dispatches it. Missing values must not silently become the first option. Popup anatomy,
spacing and state styling belong to the shared Web component, not route-local overrides.

Owner: `packages/ui-system/leptos/src/select_menu*`, the existing `fields::Select` entrypoint,
`foundation.css.in` and the existing recipe workbench. The menu is Web-specific; no CMP layout,
backend, auth, i18n business state or persisted preference policy changed.

## Design and contract audit

- Read [#141](https://github.com/loveoverflowcom/cantos/issues/141),
  [#142](https://github.com/loveoverflowcom/cantos/issues/142) and the
  [v3 implementation handoff](https://drive.google.com/file/d/17_s9ix7Qtzj76-RwLbCQLA42qob8g3gq/view).
  The handoff explicitly calls its geometry/token table a proposal. No proposed token replaces
  canonical `tokens.json` v0.6.0 (`25e99c49faa15a9112cd03e7e92f21360b808bbe68f0a2531017d9a0e21f0741`).
- Reuse `surfaceContainerHigh/onSurface`, `primaryContainer/onPrimaryContainer`, outline/focus,
  `space.sm`, `space.controlInline`, `size.control`, border, existing Web shapes and overlay
  elevation. Existing generated contrast pairs remain authoritative; no global role added.
- Anatomy: persistent FieldLabel → controlled trigger → top-layer listbox with padded options,
  selected check and independent active outline → existing help/error/reason associations.
  The intensity is Medium-Low: preference controls remain secondary tools.
- Follow the [WAI select-only combobox interaction pattern](https://www.w3.org/WAI/ARIA/apg/patterns/combobox/examples/combobox-select-only/).
  APG guidance and source tests do not establish assistive-technology compatibility.
- Native children remain the default. Typed `options` opt into the menu only in the preview;
  native fallback is explicit for unsupported engines and is not a uniform-M3 claim.

## Preview and acceptance boundary

Recipe: `?v=1&component=select&state=enabled&fixture=long-vietnamese&theme=dark&density=comfortable&locale=vi&text=200&frame=320&motion=reduced&mode=isolated&playback=live`.
Matrix supports enabled/disabled/loading/error/missing-value/empty-options. Theme and Language
specimens use real `Select`; expanded, hover and focus states require real input. Recipe text
200% is injected font scaling, not actual browser page zoom. Shared source registration adds
four Web diagnostic states while retaining all 198 baseline states and all 32 component APIs.
It grants no qualification and does not change compatibility import sites.

Mac UI access is **BLOCKED**: CUA inventory returned “The Mac is locked and automatic unlock
could not unlock it.” No browser/window surface was exposed. No screenshot was captured/opened,
no live DOM/keyboard/touch flow ran, and no screen reader was walked. This is an environment
blocker, not a visual pass. Browser acceptance is **NOT_RUN**. CMP verification separately
reports **UNSUPPORTED** because a supported installed Gradle is absent from PATH.

The [preview admission policy](../../.agents/skills/cantos-ui-design/references/preview-catalog.md)
says: “If any required check is `NOT_RUN`, `UNSUPPORTED`, failed or blocked, leave the component
unqualified and outside real app adoption.” Therefore the three existing Theatre consumers and
their app-local select CSS remain pending; bypassing that boundary would not complete #221.

## Verification layers and continuation

L1: existing component and Web owner reused; no new shared component, token source or service.
L2/L3: pure browsing/acceptance/typeahead decisions and native string-render anatomy have
deterministic tests. These establish `example-tested`, not `DOM-tested` or accessibility proof.
L4/L5: browser positioning, Popover dismissal, focus return/Tab, pointer/touch, controlled-value
acknowledgement, form validity and actual product preference persistence remain NOT_RUN.
Exact command results belong in the PR, with full local output in ignored agent-run artifacts.

The repository gate fails on the clean base as well: six Leptos qualification records are stale
and the same existing app import sites consequently lack fresh admission. The repair does not
rewrite qualification hashes, expand the compatibility baseline or relax that gate. The
independent preflight can run format, scoped Rust lint/tests, Web lint/builds and the preview
with `--skip repository --skip cmp-ui-preview --skip theatre-cmp-demo`; those skipped rows remain
NOT_RUN and do not turn the original repository FAIL into a pass. The contracts suite separately
cannot start because its audio-inspection prerequisite needs absent `ffmpeg`/`ffprobe`; no
contract/audio behavior changed. Token/catalog generator examples can run independently.

Next owner: the continuing implementation session; acceptance reviewer unassigned. Unlock the
Mac, serve `cargo xtask ui-preview leptos run`, load the recipe and wait for local Noto fonts.
Drive Tab/Enter/Space/Arrow/Home/End/Escape/typeahead, canceled browsing, disabled/loading,
empty/missing/reordered options, rapid repeated open/close and theme/locale changes. Inspect
actual 320/390/768/1280 CSS px browser viewports, 200% browser zoom, light/dark/system,
VI/EN, forced colors, reduced motion, scroll/resize/viewport edges and touch. Capture **and open**
the same before/after scenario with source/token/catalog provenance; record focus and hitboxes
independently. A baseline native OS popup is not a pixel oracle for the new menu.

Only after required admission, migrate Account theme, Settings theme and topbar locale to this
same `Select` presentation and remove duplicated select styling. Preserve their existing
reactive theme/locale signals, storage and system-default behavior. Run relevant consumer flows
and update the same Draft PR. Do not close #221 while these criteria remain incomplete.

Residual risk: no actual browser, visual, accessibility, motion or product persistence acceptance;
the custom presentation remains preview-only. #222 and #223 are pending separate sequential
sessions and must retain independent regression/evidence records in the same PR.
