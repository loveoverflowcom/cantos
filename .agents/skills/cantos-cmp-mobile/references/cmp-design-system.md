# CMP design system

> **Scope.** How the shared Cantos design intent reaches Compose Multiplatform: the generated
> Kotlin theme, Material 3 Expressive APIs in CMP, color and dynamic color, typography with
> Vietnamese diacritics and font scaling, touch targets, semantics, insets, back navigation,
> reduced motion, localization and component states. Use for any visible change to the Theatre
> mobile app.

Design intent is owned by [`cantos-ui-design`](../../cantos-ui-design/SKILL.md) —
[material3-expressive.md](../../cantos-ui-design/references/material3-expressive.md),
[tokens.md](../../cantos-ui-design/references/tokens.md),
[typography.md](../../cantos-ui-design/references/typography.md),
[motion.md](../../cantos-ui-design/references/motion.md),
[accessibility.md](../../cantos-ui-design/references/accessibility.md),
[localization.md](../../cantos-ui-design/references/localization.md) and
[component-states.md](../../cantos-ui-design/references/component-states.md) — and product
requirements by the [UI system](../../../../docs/design/ui-system.md). This reference maps them
onto CMP. Every platform API named is a candidate to verify for the pinned CMP version on Android
**and** iOS and has not been rendered there. The token pipeline and the *implemented* rows of the
injected-preference table describe what `packages/ui-system/cmp` implements and renders on the JVM
desktop host only; rows marked *rule* state required behavior that no code observes yet.

## Token pipeline

```text
packages/ui-system/shared/tokens.json   the one versioned semantic source
  → scripts/ui_tokens.py                writes generated/CantosTokens.kt; the header names version + digest
  → generated Kotlin lookup             logical numbers by role, theme and density — never hand-edited
  → CantosTheme                         maps roles into the Material theme: colorScheme, typography,
                                        shapes; dp/sp conversion happens here, never in the source
  → Cantos component wrappers           packages/ui-system/cmp — the only place experimental opt-ins appear
  → feature screens                     consume roles, never literal colors or ad-hoc dp scales
```

| Rule | Why | Oracle |
|---|---|---|
| generated token files are never edited by hand; change the source and regenerate | a hand edit diverges silently from Web | generator `--check` and `cargo xtask verify repository` fail on drift |
| share semantic names and relationships with Leptos, not CSS pixels | Web and CMP evolve independently ([decoupling.md](../../cantos-engineering/references/decoupling.md)) | token map review: `color.primary` ↔ `colorScheme.primary` |
| no palette copied into a feature, no `Color(0xFF…)` outside generated code | identity drifts per screen | review; a lint rule is a candidate |
| a shared token change carries current Web evidence | both renderers consume it | Web capture through [`cantos-leptos-web`](../../cantos-leptos-web/SKILL.md) |

Trace token source → generated role → `CantosTheme` → mounted component before changing a
style. An outer `MaterialTheme` does not restyle a custom component that reads its own values.
CSS px never become dp/sp by copy: roles stay logical, and the Web and CMP shape scales are
independent ([token reconciliation](../../../../docs/design/ui-token-v3-reconciliation.md)).

## Injected preferences and scope

`CantosTheme(preferences, uiFontFamily, editorialFontFamily)` receives values; a component never
reads the operating system, a file or the network, and never builds a service, port or mock owner.

| Fact | How it arrives | Status | Scope and rule |
|---|---|---|---|
| theme | `UiPreferences.theme`, the *resolved* `Theme` | implemented; `ThemeModeTest`, gallery and Android preview roots | `ThemeMode { Light, Dark, System }.resolve(systemIsDark)` is pure; only a composition root resolves `System`, from a host fact (`isSystemInDarkTheme()` in the gallery and preview roots, the preferences owner in an app). `UiPreferences` has no mode or host-fact field, so a nested theme cannot re-resolve `System` (type-enforced) |
| density, text scale | `UiPreferences.density`, `textScale` | implemented; `UiPreferencesTest`, `ThemeFoundationTest`, gallery probe | scale is an absolute multiplier on the host font scale (`LocalPlatformFontScale × textScale`); a nested theme replaces it, never compounds it |
| reduced motion | `UiPreferences.reducedMotion` | partial: the field and motion-role resolution exist (`AuthActionShape`, token catalog); the gallery sets it with a manual chip and no code reads the OS setting | rule: the host resolves the OS setting and injects it; durations resolve to 0 and state feedback stays immediate |
| fonts | `uiFontFamily`, `editorialFontFamily` | implemented as parameters; a nested theme that names none inherits the host's (`ThemeFoundationTest`); the desktop gallery host injects the shared Noto bundle (`PreviewFonts`, `PreviewFontsTest`) | licensed families are injected by the composition root; a host that injects none keeps the platform sans/serif and says so in evidence. Static Noto masters exist for 400 and 700 only |
| locale | caller-supplied strings and accessible names | rule | components format nothing from the OS locale; fixtures are vi-VN |
| gestures | typed intents shared with a button/keyboard alternative | rule; the package has no gesture code | cancellation, reversal and focus are explicit; no animation-end callback gates an intent |

Hit target and visible glyph are separate roles: `size.control` (48 at both densities) sizes the
target and `size.icon` (24) the glyph (`HitTargetTest`). Preview-local geometry is a named
constant with its unit (dp), scope and reason (`PreviewGeometry`); it never becomes a token
unless a shared component needs it. Evidence viewports are dp *constraints* (`ViewportPreset`:
320, 390, 600, 840, 1140), not device models; record the real constraint, density and font scale
(the gallery prints them and the JVM capture sidecar stores them) instead of deriving dp from a PNG.

## Material 3 Expressive in CMP

Apply the intensity model from material3-expressive.md: the full player carries the highest
expression on Theatre (emphasized play control, expressive shape and motion on transport), the
library and episode lists medium, settings and account screens low. Expression serves attention;
rounded corners or a Material dependency alone do not establish it.

API reality to check before using any of it:

- Jetpack Material 3 introduced expressive APIs — an expressive theme entry point, an
  expressive motion scheme and new components such as button groups, split buttons, loading
  indicators and toolbars — largely behind an experimental opt-in
  (`@ExperimentalMaterial3ExpressiveApi`). The CMP Material 3 artifact may lag Jetpack, differ
  per target, or change signatures between releases.
- Verify each API in the pinned CMP Material 3 version on Android and iOS before use, record the
  opt-in and its upgrade risk in the ADR, and confine opt-ins to `design/` wrappers so features
  never carry `@OptIn`.
- Where an API is missing, use stable Material 3 components with Cantos shape and motion tokens.
  Judge the result on rendered output through
  [visual review](../../cantos-ui-design/references/visual-review.md), not on API names.

A wavy or waveform progress indicator may decorate the player but is never the only way to seek
or read position ([UI system § Theatre](../../../../docs/design/ui-system.md#theatre-compact-listening)).

## Color and dynamic color

- Light and dark color schemes both come from the generated roles; contrast targets and their
  recorded measurements belong to [accessibility.md](../../cantos-ui-design/references/accessibility.md).
- Dynamic system color must not silently replace the Lavender Stage identity
  ([UI system § Semantic tokens](../../../../docs/design/ui-system.md#semantic-tokens)). It is off
  by default; offering it is a product decision. iOS has no equivalent, so a design that depends
  on it breaks one platform.
- Status (failed download, expired access) pairs color with text or an icon.

## Typography

- The font must render complete Vietnamese, including stacked marks: test `Người dẫn chuyện`,
  `Ánh đèn cuối sân khấu`, `Ngày mai, mình có diễn tiếp không?` and uppercase `Ỗ`, `Ữ` at the
  largest supported scale. Clipped top marks usually come from fixed heights or tight line
  height; check the platform text style and line-height trimming options in the pinned version.
- Text sizes in `sp`; no fixed heights on text containers; truncate a title only where the full
  title is reachable (detail screen, accessibility name).
- Font scaling: Android scales nonlinearly at large settings on newer releases; verify how the
  pinned CMP maps iOS Dynamic Type into font scale. Test the default and the largest supported
  settings on each platform.
- Elapsed and total time use tabular figures (`fontFeatureSettings = "tnum"`) so the seek row
  does not jitter. Details: [typography.md](../../cantos-ui-design/references/typography.md).

## Touch targets

48 × 48 dp on Android and 44 × 44 pt on iOS
([UI system § Accessibility](../../../../docs/design/ui-system.md#accessibility-and-motion-acceptance)).
Compact visuals keep full hit areas; Material's minimum interactive size modifier is the
candidate mechanism. Oracle: `semantics-tested` bounds of every transport control at default and
largest font scale, then a device check that adjacent targets do not overlap.

## Semantics

Names state the action and the item; state goes in the state description; built-in roles first.

| Control | Accessible name | State description | Notes |
|---|---|---|---|
| play / pause | "Phát tập Một lời hẹn" / "Tạm dừng tập Một lời hẹn" | — | the name changes with the action; never only an icon |
| seek | "Vị trí phát" | "12 phút 5 giây trên 45 phút 30 giây" | use the built-in slider semantics with range and set-progress; elapsed/total also visible as text |
| download | "Tải xuống tập Một lời hẹn" | "Đang tải 45%", "Đã tải xuống", "Tải xuống thất bại" | disabled with a discoverable reason when the policy forbids offline |
| speed | "Tốc độ phát" | "1,25×" | locale-aware number format (Vietnamese decimal comma) |
| sleep timer | "Hẹn giờ tắt" | "Còn 15 phút" / "Hết tập này" | the OS controls do not show it; the app must |
| mini-player | one focus stop: "Đang phát Một lời hẹn" | playing / paused / interrupted | merge descendants but keep play/pause and open player as reachable actions (custom accessibility actions) |

- Async outcomes that matter (download complete or failed, access expired) use a polite live
  region; position ticks never do.
- Section titles are headings. Decorative artwork is excluded from focus; meaningful artwork has
  a description.
- Every gesture has an accessible alternative: swipe-to-dismiss the mini-player is also a custom
  action.
- A `testTag` is not a name. CMP maps test tags to iOS accessibility identifiers in some
  versions; verify, and never put titles, emails or IDs of private content in tags.

Oracle: `semantics-tested` name, role, state and actions on the production composable, then
`screen-reader-walked` with TalkBack and VoiceOver ([cmp-testing.md](cmp-testing.md)).

## Insets, safe areas and adaptive layout

Apply window insets once, at the level that owns the edge: system bars, display cutouts, the
iOS home indicator and the keyboard. The mini-player sits above the navigation bar and gesture
area without covering list content; the list adds bottom padding for it. Newer Android target
SDKs enforce edge-to-edge drawing; verify for the chosen target. Check compact and expanded
widths, landscape and split-screen; Theatre lists stay single-column and readable at narrow
widths ([UI system § Validation matrix](../../../../docs/design/ui-system.md#future-validation-matrix)).

## Back navigation

On Android the system back gesture (predictive back on newer releases) collapses the full player
to the mini-player and never stops playback; verify which back-handler API the pinned CMP
provides in common code. On iOS, edge-swipe back comes from the navigation implementation chosen
in the ADR. Back never traps the listener and never discards an in-progress download.

## Motion and reduced motion

Motion intent and durations come from [motion.md](../../cantos-ui-design/references/motion.md).
Read the platform setting (Android animation scale / remove animations, iOS Reduce Motion)
through a `platform/` signal and pass it into the theme or a composition local. Reduced motion
removes travel, overshoot and continuous animation (marquee titles, wavy indicators) while
keeping immediate state feedback. No animation delays an action; the mini-player expansion is
interruptible and never moves a control away from the finger mid-gesture. A still image proves
none of this; motion needs a runtime recording on a device.

## Localization

Policy is owned by [localization.md](../../cantos-ui-design/references/localization.md). On CMP,
use one typed resource mechanism (Compose Multiplatform resources are a candidate for the ADR):
semantic keys, format arguments and plurals, never concatenated fragments. Accessibility names
and state descriptions are resources too. Format numbers and durations with the locale
("1,25×", "45 phút"). The locale set (vi-VN and en, pending confirmation) and the framework
decision belong to localization.md; never hard-code a locale or ship a key in only one of them.

## Component states

Episode rows and the player distinguish played, in progress, downloaded, downloading,
unavailable and failed with text or icons as well as color
([component-states.md](../../cantos-ui-design/references/component-states.md),
[UI system § Theatre](../../../../docs/design/ui-system.md#theatre-compact-listening)). Map each
from the sealed state types with an exhaustive `when`
([KT-CLOSED](kotlin-conventions.md#kt-closed--sealed-types-and-exhaustive-when)), so a new state
cannot render as a blank row.
