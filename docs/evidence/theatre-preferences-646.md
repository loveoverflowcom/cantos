# Compact Me preferences — PR #646

Reviewed by Codex on 30 October 2026, one automated reviewer. Integration base remains
`origin/develop@aa0bec8dd770305753aa8212b40bd87ea0afd79a`, merged at `133668bc`.
This amendment follows the guest-card checkpoint `e2c111c8`. The adjacent
`theatre-preferences-646-*-{leptos,cmp}.json` records bind the current shared renderer inputs;
the earlier navigation and guest records are retained as historical evidence.

The subsequent [Web-only self-audit](theatre-preferences-self-audit-646.md) removes the redundant
outer preference card and top margin, with matched before/after observations. Shared renderer
inputs and the CMP implementation below are unchanged by that later amendment.

## Scope, invariants and design judgment

The owner's latest compact-preferences prompt names CMP as its primary scope, while its draft
pictures a Web header. Both existing Theatre consumers were inspected and adjusted. Current
issues #583, #527 and #613 were read alongside the linked-issue audit already recorded in PR #646.
The owner permits judgment on backgrounds and rejects a gapped/double selected perimeter.
The earlier explicit removal of guest help still overrides the footer in older concepts.

The observable invariant is that each Me preference exposes its actual selected value and opens
the existing chooser, without introducing a second preference authority or changing rejection,
retry, System appearance, auth, Library or playback state. Root `PreferencesController` remains
the CMP authority; the production Web owner and admitted Select retain their current lifetimes.
Only local chooser visibility and focus-return state live in the new CMP composition.

Existing semantic surface, primary-container, typography, spacing and size roles provide the
contrast hierarchy. There is no new palette, gradient, shadow or decorative row border. The guest
card keeps the contrasting profile treatment from the [guest checkpoint](theatre-guest-tonal-646.md),
including the admitted neutral avatar and filled auth pair. Personal identity is not fabricated.

Design differences are intentional:

- Web normally presents icon, label, value and the Select's own downward chevron on one 48px row.
  The app-owned CSS composition reuses the existing compact inline-labelled Select, including
  its native fallback, option state, keyboard handling, popover and visible keyboard focus.
  Both visible text and decorative icon labels activate the same real trigger; there is no
  extra focus stop or independent click-state owner.
- CMP uses the existing full-row `PreferenceRow` with an optional compact presentation and
  canonical leading icon. Its value sits under the label within the row, leaving more width
  for Vietnamese and enlarged English. The right chevron still opens the same `PreferenceDialog`.
  The 32dp tonal icon well, minimum 48dp target and current theme roles retain touch capacity.
- At constrained enlarged-text Web widths, the same control reflows onto two or three lines.
  Whole labels and values take priority over preserving a horizontal draft layout. The smallest
  Membership inset also contracts through existing space tokens so its existing CTA fits.
- CMP Theme and Language move into one Me section, after identity and before Membership.
  Their duplicated Settings entries, the account theme shortcut and normal shell theme cycle
  are removed. Speed, Timer and Diagnostics retain their plain row path and existing choosers.
  Focused Auth/Membership headers remain outside this amendment. Native still has its existing
  two-root navigation and Account opener; #583's complete three-root navigation is not claimed.

## Component and resource ownership

Reused catalog IDs: `select`, `icon`, `surface`, `button`, `avatar`, `dialog`, `nav-item` and
`icon-button`; existing account/profile variants remain qualified compositions. No new public
shared control or consumer-specific global compact variant was introduced. CMP's optional
compact parameters belong to Theatre's existing app-owned row, not the shared component API.

`light_mode`, `dark_mode` and `theme_system` already existed. The missing globe is now the exact
official Material Outlined `action/language/24px.svg`, pinned to upstream commit
`49d4db35df873165d6bd6ba09b063c7dafbac2f4`, with source path/checksum in the canonical manifest
and the existing Apache-2.0 license. The registry contains 45 icons: 43 upstream, two Cantos.
Both renderer bindings were regenerated through `scripts/ui_icons.py`; there is no per-app SVG.
The short Language, Light/Dark values and Preferences heading were added to the canonical
localization catalog and all consumers regenerated through `localization_catalog.py --resources`.
Studio's generated digest changes only as an owner-generated catalog consumer.

## Executed checks and provenance

Rust 1.98.1; existing Gradle 8.14.3 / JDK 17.0.19; Homebrew Python; local macOS arm64.
Full output and screenshots remain ignored under `artifacts/agent-runs/646-preferences/`.

| Command | Result and actual boundary |
| --- | --- |
| `cargo test --locked -p cantos-theatre --features preview features::account::tests` | PASS, 16 production account/SSR tests; `account-tests-final.log` |
| `cargo xtask ui-preview leptos verify` | PASS, actual SSR tests, WASM lint and gallery build; `leptos-verify.log` |
| `cargo xtask ui-preview cmp verify` | PASS, 319 JVM tests, zero failures/errors/skips; `cmp-verify.log` |
| `cargo xtask theatre-preview verify` | PASS, shared 619 tests (3 skipped), sandbox 179 (4 skipped), zero failures/errors; final `theatre-verify-dev6.log` |
| `cargo xtask theatre-host android-release` with installed SDK | PASS at the checkpoint described below; `android-release-final.log`; exact handoff receipt/result recorded separately in the PR |

The seven existing ignored JVM cases are not called passing. Earlier development logs contain
failures corrected before `dev6`; they are not final results. Final-diff preflight uses the same
base above and the pinned Node/npm lane from the earlier repaired preflight. Its actual gate
ledger is recorded in the PR, separately from these focused checkpoints.

`MePreferencesTest` mounts the production group at 320dp with the real 16dp outer gutter,
200% text and both locales. It checks target bounds/roles, whole-word English Appearance,
actual System selection, focused selected radio, Escape/close/choice focus return, autonyms,
exact typed intents and no optimistic claim before a controlled owner acknowledges a choice.
The app tests independently check that System remains selected while host appearance changes,
and that explicit Dark ignores the host. The sandbox flow drives Me through a real controlled
store rejection, keeps Vietnamese plus the failure notice, retries successfully to English,
and preserves query, navigation, account, Library, player state and engine command history.
Top-bar tests retain full 48dp Account targets and verify there is no empty Account action group.
Existing preference persistence/controller and listening-setting tests run in the same suite.

## Actual Web observations

Production Theatre Web on loopback 8082, native Server/Identity development host on 8080,
healthy guest with an empty public catalog; Chromium, DPR 1. Existing responsive navigation,
drawer lifecycle, typed Library return and auth Back evidence remains in
[the navigation report](theatre-navigation-646-product-qa.md).

The new rows were opened in VI/EN and Light/Dark/System, at actual 390px/1280px normal text,
320px enlarged text and 600px enlarged text. Normal rows measured 48px. A visible label and
the decorative icon label both opened the real Select. Escape canceled it; real choices updated
the visible values. Keyboard order was Sign in → Register → Appearance → Language → Premium,
with no decorative-icon stop. The keyboard perimeter remains visible and flush. The guest
disclosure count is zero, and both original auth actions and truthful Premium remain present.

For enlarged-text observation only, a temporary localhost HTML style set the actual root font
to 32px. The original HTML was restored byte for byte; no text-scale switch or forced style ships.
At 320 × 800 with a vertical scrollbar, client and scroll widths were both 305px. The final
three-line layout kept Appearance, Language and English whole. At 600px the two-line layout
also retained whole words. VI Membership and its CTA wrapped at word boundaries within the
page. The normal 390 × 844 recheck measured font 16px, client/scroll 390px and both rows 48px.
This is app text enlargement, not browser zoom or OS text-size acceptance.

Opened current captures in the ignored amendment directory include:
`web-final-light-vi-390-idle.png` (full page from the measured viewport),
`web-final-light-en-1280.png`, `web-final-preferences-dark-en-text2-320.png`,
`web-preferences-dark-en-text2-600.png`, `web-final-preferences-light-vi-text2-320.png`,
and `web-membership-light-vi-text2-320.png`. Sidecars record actual dimensions and text/bounds
where collected. Earlier mislabeled or experimental full-page-plus-clip captures are not evidence.
Viewport overrides were reset and owned test tabs/processes stopped after observation.

## Independent CMP and shared visual observations

Production composables on JVM Desktop Compose over deterministic external ports, not a device.
Opened final captures under `mobile/preview-sandbox/build/reports/theatre-auth/`:
`me-preferences-light-390x844.png`, `me-preferences-dark-en-text2-320x800.png`,
`me-theme-dialog-light-390x844.png`, `me-language-dialog-dark-text2-320x800.png`.
The normal group has two distinct tonal rows and values Sáng / Tiếng Việt. At 320dp / 200%,
Appearance and Language stay whole; Dark and English remain readable. The real theme dialog
retains all three original descriptions/options; the locale dialog retains both autonyms.
The initial larger icon well/padding broke Appearance at that width; the same scenario was
rerun after reducing only compact-row spacing, and its whole-word oracle and opened image passed.
Guest/profile and prior auth/signed-in/logout captures/tests also remain in the executed suite.

The shared delta is the canonical globe, generated bindings, closed-registry assertions and
the actual Avatar admission capture row. Both complete renderer suites ran. The actual Web
gallery rendered 45 decorative SVGs with `aria-hidden=true`; its Light and Dark/200% globe
captures (`shared-globe-light.png`, `shared-globe-dark-text2.png`) were opened. Current JVM
`avatar-library-646-light-840.png` and `avatar-library-646-dark-320.png` were opened and show
the globe alongside Home/Library and existing Avatar states. No image identity is invented.

The other shared components did not change in this amendment. Their source-attributable
observations remain bounded by [the shared navigation review](theatre-navigation-646-shared-qa.md)
and [the merged guest/FilterChip checkpoint](theatre-guest-tonal-646.md), including their named
interaction, pixel, motion-clock and semantic coverage. The fresh 20 paired records combine
that unchanged coverage, current complete renderer suites and the affected globe observations;
they are not a new every-state visual matrix or a hashes-only review. Registration, qualification,
browser interaction, JVM pixels and Android/device acceptance remain distinct.

## Android packaging and remaining acceptance

The earlier unconfigured-SDK result is superseded for packaging: an actual installed SDK was
found at `/Users/manh.pd1/Library/Android/sdk` and explicitly supplied as `ANDROID_HOME`.
The first build and artifact scan passed but final source binding correctly failed because a
README was edited during the run. Nothing bypassed that guard. A frozen-tree retry passed the
unshrunk audit, optimized production APK/AAB build, source/graph parity, exact artifact/mapping
hash binding and forbidden sandbox/mock/demo/test scan. This is an unsigned default-origin
release artifact, not distribution, sign-in, deployment or a device run. The source guard hashes
the whole nonignored repository plus HEAD; the final committed handoff is therefore rebuilt
with that entire tree frozen, with its exact receipt/result reported in the PR.

Behavior SSOT impact: `cantos.presentation.locale-preference` is draft. Its manifest/model and
controller ownership were consulted; this is an outcome-preserving presentation/placement change,
so no model or lifecycle change is justified. Existing rejection/restoration/theme-isolation rules
remain implementation-test oracles. The broader responsive-root model gap from #641 remains
explicit in the navigation report. A behavior gate PASS is model-only unless separately stated.

Layer ledger: L1 ownership and L2 immutable values/typed intents are preserved; L3 production
controller/layout and L4 real renderer interaction checks pass within the scopes above. L5 local
Web host is a real healthy guest observation, not actual enrollment/sign-in/publication acceptance.
No backend, authoritative database, contract, storage, session or media engine rule changed here.

Residual acceptance: browser zoom, TalkBack/VoiceOver, actual Android/iOS touch/Back/IME,
production sign-in, real profile name/photo, live published audio and motion smoothness are
NOT_RUN. Device/runtime prerequisites and #481 profile-data ownership remain separate; #583's
complete native three-root bar is still deferred. The broader #641 acceptance must remain partial.
No deploy, merge, enrollment, payment or distribution was performed. Source/doc logs and generated
records contain no credentials or private media; screenshots, APK/AAB, downloads and tool installs
stay outside Git.
