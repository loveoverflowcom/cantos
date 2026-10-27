# Shared component requalification — 27 October 2026

Bounded, source-bound requalification of the 12 stale shared-component receipts plus first
admission of IconButton, TextArea, LoadingState and NavItem, on Leptos (in-app Chromium) and
CMP (JVM desktop test root). It follows the [admission method](../../.agents/skills/cantos-ui-design/references/preview-catalog.md)
and the [catalog contract](../../packages/ui-system/shared/README.md#preview-registration-and-adoption).
It is **not** product-journey, Android/iOS device, assistive-technology, real-IME, OS-window,
motion-smoothness or design-pixel acceptance. Reviewer is a single automated agent session
(Claude Sonnet 5.5); there was no independent second reviewer and the v3/v4 design boards were
not re-opened. Prior receipts [ui-qualification-459-final.md](ui-qualification-459-final.md) stay
as history; the earlier `ui459-*` JSON records are superseded, not edited.

## Why the receipts were stale

`develop@259480dd` fails `python3 scripts/ui_preview_catalog.py --check`: all 12 `ui459-*` receipts
carried token digest `f0ab05cb…` and renderer digests from 25–26 October. About 25 UI-system
changes landed afterwards without a requalification (Search v4.3 shell, theme-cycle button and
tooltip, shared LoadingIndicator, guest CTA roles, Auth v4.3 tonal form, focus perimeter, new
icons and tokens). Because stale components are not "fresh", the checker also reported 40 dependent
"new use requires both preview qualifications" lines. Four of those components were never admitted
(IconButton, TextArea, LoadingState, NavItem) and stay failing until admitted on both renderers.
No digest, baseline or validator was relaxed; no digest was hand-edited.

## Inputs (all from `python3 scripts/ui_preview_catalog.py --digests`)

- Source base: `origin/develop@259480dd0b388c3db88ef9b97c75cf37af33f10f`.
- Token digest: `c422efece17864e4c301cd8c67a3f583ca873aea0a0faa5b0da61e92963ce527`.
- Leptos input digest: `4caa86f3d6bfd45fcae29dd9516a7e9bcd4f6e6ce951c5d471e44a3dc5e1827a`.
- CMP input digest: `672a4cd2f67a26a4f0201859e4b1b98bbe10fccde0a7e42e000dcbbcfcd22d02`. It
  includes the one source change below, `NavItemTest.kt`.
- Before the catalog edit, the generator reproduced all three generated files byte-identically.
  Repointing `qualifications` then changed only the one-line `catalog sha256` header comment of
  `preview_catalog.rs` and `PreviewCatalog.kt` (both excluded from the digests); the verified
  builds above ran on the previous header comment and were not repeated for a comment-only change.
- Tools: Rust 1.98.1, Trunk, installed Gradle 8.14.3, JDK 17, macOS. Logs are ignored local
  artifacts in `artifacts/agent-runs/rebind-preview-catalog/`.

## Executed layers

| Layer | Result |
| --- | --- |
| Leptos structural | `cargo xtask ui-preview leptos verify` PASS: package tests, WASM clippy, Trunk build. |
| Leptos runtime/accessibility/visual | `cargo xtask ui-preview leptos run` (127.0.0.1:8083); the built-in browser (Chromium 152, DPR 1) loaded the gallery. Real browser keyboard/pointer and DOM checks below. Screenshots were opened in the transcript; no PNG export or file hash is claimed. |
| CMP structural/runtime | `cargo xtask ui-preview cmp verify` PASS twice. Before the NavItem test: 286 tests / 47 suites. Final: **289 tests / 48 suites**, zero failures, errors or skips; these are the tested bytes of the bound digest. |
| CMP accessibility/visual | Compose semantics assertions inside those tests; 299 captures in `packages/ui-system/cmp/build/reports/cantos-preview/` (ignored build output) were produced by the run and a representative image per component was opened (list below). |

## Leptos observations (this session)

- Light, 1000-wide viewport: opened Navigation (NavTabs/NavItem links, contextual Back,
  `aria-current=page` on the active item), Actions (Auth/account actions, Button, chips),
  Feedback (Badge, five Status tones, Empty/Loading/Error states, Surface compositions).
- Dark, comfortable density, 200% injected text, frame 320, motion=reduced, long Vietnamese
  fixture: opened Fields (TextField, TextArea, NumberField, Select, Checkbox, Switch, Radio).
  No horizontal page overflow (`scrollWidth` ≤ viewport).
- DOM audit: 7 IconButtons all have a localized `aria-label` with `aria-hidden` SVG; 242/242
  icons are decorative; NavItems are real anchors with `aria-current`; every TextArea has a
  label, `aria-describedby` and `aria-invalid`; 13 Logo images complete, decorative alt empty,
  standalone symbol alt "Biểu tượng Cantos"; Checkbox inputs are named by their label; the
  announcing Status/LoadingState samples carry `role=status`; ErrorState exposes two recovery controls; dialogs carry
  `aria-labelledby`.
- LoadingState: the "polite announcement" sample renders `role=status` with the indicator
  animating (`cui-loading-turn`); with the workbench motion set to reduced the animation is
  `none`.
- Real input: clicking into TextArea and typing inserted text at the caret and kept the controlled
  value (166 then 169 characters after two typed inputs). Dialog: open is modal with focus inside;
  Escape closes it and returns focus to the opener.

## CMP images opened

Text area (dark, focused, Vietnamese with Ỗ/Ữ), LoadingState announcement (200% text, 372 wide),
NavTabs overflow (the existing NavItem-like selection style at 200% text), Theatre IconButton
tonal (dark), Button (idle) and Theatre fields (dark, 200%, 320 wide: empty/filled/error/read-only/
loading/long label), Auth/account Surface compositions (dark, 200%, errors), Badge tones (dark,
200%), Select selected-error (dark, 200%, 390), Checkbox with focus ring (dark), Logo header with
theme and avatar (dark, 200%, 320), BackButton focused (dark), Icon decorative registry (dark,
includes the five new glyphs), Status neutral (dark, 200%), ErrorState (dark, 200%) and Dialog
(dark, 200%, focused editor). No clipped glyph, hidden label or missing focus ring was seen in the
opened set. One note, not treated as a defect: in the 372-wide LoadingState capture at 200% text the
spinner sits at the first text line's top edge instead of centred on it; this is a visual nit left
for the owner.

## Source change in this requalification

`packages/ui-system/cmp/src/desktopTest/kotlin/com/cantos/ui/NavItemTest.kt` (test only). NavItem
on CMP had no dedicated JVM test or capture: selection is a callback request owned by the caller,
Loading/Disabled refuse activation with a visible reason, a long Vietnamese label stays enabled and
selected. No production code, token, API or catalog declaration changed.

## Per-component scope

| Component | Leptos | CMP |
| --- | --- | --- |
| Button | Auth/account and ordinary sample shapes, light; named labels. | Verified idle capture, focus-perimeter and manual-clock motion tests. |
| IconButton | DOM audit above; theme cycle and show-password examples. First admission. | Theatre tonal capture opened (dark); docked captures and focus-perimeter tests ran. First admission. |
| TextField | Dark 200% frame 320 fields; labels, help/error references. | Theatre fields capture and error/focus pixel tests. |
| TextArea | Typed into the controlled sample; labelled, described, invalid flag. First admission. | Dark focused capture with selection; ReadOnly/state tests. First admission. |
| Select | Dark enhanced select inspected in Fields; no keyboard walk this session. | Selected-error dark 200% capture; stable-value interaction tests. |
| Checkbox | Named inputs; checked/unchecked, dark Fields. | Focused dark capture; label/key selection tests. |
| Surface | Feedback/Layout compositions, light. | Auth/account compositions, dark 200%. |
| Badge | Five tones, light feedback section. | Tones, dark 200%. |
| Status | Five tones with `role=status` where announced. | Neutral and four other tones in captures. |
| LoadingState | Polite announcement, spinner and reduced-motion. First admission. | Announcement capture at 200% text plus LoadingIndicatorTest. First admission. |
| ErrorState | Title/body and two recovery controls. | Dark 200% capture, recovery dispatch tests. |
| Icon | 242 decorative SVGs. | Decorative registry capture including new glyphs. |
| Logo | 13 images complete, alt contract. | Theatre header capture dark 200% 320. |
| BackButton | Contextual Back sample and IconTonal baseline. | Focused capture; named destination tests. |
| Dialog | Real modal, Escape/restore. | Dark 200% focused-editor capture. |
| NavItem | Anchors with `aria-current`, NavTabs sample. First admission. | New NavItemTest; NavTabs overflow capture. First admission. |

## Not run / not established

Android/iOS devices, screen readers, real IME and autofill, forced colors, other browser engines,
OS-window input for the CMP desktop app, motion smoothness, and matched-pixel comparison against the
design boards. Leptos was inspected as sections of one gallery session, not as a full per-state
matrix for every component; the receipts above state only what was observed.

## Re-run required

Any later edit to `tokens.json`, the generated CSS/Rust/Kotlin outputs, the Leptos or CMP package,
the shared catalog declarations or build inputs invalidates these receipts again. Open PR
[loveoverflowcom/cantos#548](https://github.com/loveoverflowcom/cantos/pull/548) adds a token and
edits generated outputs; its branch must rerun this procedure after it lands.
