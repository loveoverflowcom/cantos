# CMP Auth v4.3 Minimal Expressive — source delta and evidence (#540)

Source: `develop@82f667f40ead7fd511938794d5df3ca91049aca7` plus this PR. The design is the owner's
Drive handoff linked from [#540](https://github.com/loveoverflowcom/cantos/issues/540); this record
states what the source now does and what has **not** been observed. It is not native, assistive
technology, motion or authentication acceptance.

## What changed

| Area | Before | After |
| --- | --- | --- |
| Login composition | eyebrow, long intro, `auth-brand` promotional card, then the form | one task title, status, form, primary action, route link, recovery |
| Register composition | eyebrow, intro, form, two secondary actions | one task title, email form, primary action, Login link; Discover link only when sign-up is unavailable |
| Completion | eyebrow, one shared "show password" text button, one visibility state | no eyebrow; two independent in-field eyes; rule (8–128), mismatch error and disabled reason stay visible |
| Field | `Floating`: inset 1/2 dp stroke for focus and error | `TextFieldVariant.FloatingTonal`: no stroke; focus is the higher tonal fill plus primary label and caret; error is `errorContainer`, error label and an inline glyph with the message |
| Auth actions | inset ring, `AuthOutlined` outline | under `tonalAuthActions()` no stroke, ring or halo; focus is a state-layer fill inside the control's own shape; `AuthOutlined` is the quiet tonal container |
| Copy | catalog had eyebrow/intro/brand strings | those nine `theatre.cmp_auth.*` keys are removed; headings are now the task titles; two long messages are shortened without changing their claims |

The tonal recipe is opt-in and package-owned (`packages/ui-system/cmp`): `Floating`, `FloatingSearch`,
`AuthTonal`, Membership/Account surfaces and every Web surface keep their existing perimeters. Auth
domain, transport, completion routing and secret lifetime are untouched.

## Invariants kept

- Accepted is not delivered: the waiting copy still says the response confirms neither delivery nor
  account creation, and still names the browser step and the return to this app.
- Link is not account, created is not signed in: the completion confirmation copy keeps that sentence.
- Errors, the password rule and disabled reasons render as text in place; none moved into a tooltip.
- The eye is a trailing `IconButton` (48 dp, own accessible name); a press toggles only its field, never
  submits, moves focus or changes the value. Passwords are not in saveable state, logs or snapshots.

## Evidence

| Layer | Command (cwd) | Result |
| --- | --- | --- |
| Pure/JVM Compose, shared UI system | `gradle desktopTest --no-daemon --offline` (`packages/ui-system/cmp`) | 286 tests, 0 failures; new `TonalAuthRecipeTest` (6) |
| Pure/JVM Compose, Theatre | `gradle :shared:desktopTest :preview-sandbox:desktopTest --no-daemon --offline` (`apps/cantos-theatre/mobile`) | 571 + 153 tests, 0 failures (5 skipped, as before); new login layout/eye/error tests |
| Android compile of the shared package | `ANDROID_HOME=… gradle compileDebugKotlinAndroid` (`packages/ui-system/cmp/android-preview`) | passes; compilation only |
| Preflight gates (`cargo xtask preflight --changed-only --base origin/develop`, in three runs) | `repository` FAIL (47 findings identical on clean `develop@82f667f4` except `Icon, IconButton` added to one already-failing `AuthScreens.kt` line); `rust-tests` one flaky `Open(AlreadyOwned)` restart test, passes alone; all other selected gates PASS | `mutation` skipped (not selected by this change) |
| Localization | `python3 scripts/localization_catalog.py --resources --check` | passes; generated Kotlin/Rust headers re-derived by the generator |

`TonalAuthRecipeTest` samples the production components' edge pixels in both themes: the tonal field
(empty, filled, error) and tonal actions (primary, outlined, text) paint the fill to the edge at idle
and while focused, nothing is painted outside the node, and focus still changes the fill. Negative
controls (stroked `Floating`, default `AuthOutlined`) must differ, so a pass is not a sampling
mistake. Captures were opened: Login (390×844 Light, failed state, 320×800 at 2× text), Register
(Dark/English, error-free and with a value). They are JVM Compose renders, 1 px per dp.

## Not run

| Acceptance | Status | Resume |
| --- | --- | --- |
| AC-07 Android before/after (320/390, Light/Dark/System, VI/EN, 200% font, IME, Back, TalkBack) | `NOT_RUN` | boot `Pixel_7`, `theatre-host android-debug`, drive the six states and open the captures |
| AC-08 iOS safe area/IME/font scale/VoiceOver | `NOT_RUN` | needs a Kotlin/Native toolchain matching the pinned version and a simulator |
| Preview qualification re-bind (AC-05 admission) | `NOT_RUN` | the `repository` gate was already red on this base for stale `button/leptos`-style qualifications and "new use requires both preview qualifications"; this PR adds `Icon`, `IconButton` to the `AuthScreens.kt` line of that same finding. Re-binding belongs to the qualification owner (#143) |
| Measured contrast of the new pairs | not measured here | the issue records 16 pairs ≥ 4.5:1 from the concept; the production tokens are unchanged |
| Real keyboard/IME composition through the eye | `NOT_RUN` | Compose test input is synthetic |
