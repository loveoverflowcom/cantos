# Guest account card and merged shared UI checkpoint — PR #646

Reviewed by Codex on 30 October 2026, one automated reviewer. Base is the freshly fetched
`origin/develop@aa0bec8dd770305753aa8212b40bd87ea0afd79a`, merged at `133668bc` before this delta.
The adjacent `theatre-guest-tonal-646-*-{leptos,cmp}.json` records bind the actual shared inputs.
Earlier #646 records and compatibility baselines remain unchanged.

## Authority, scope and design judgment

The owner's latest “Borderless Tonal” prompt and three supplied Light/Dark concepts are the
expectation. The explicit instruction to remove the guest explanation overrides the footer
still pictured in those concepts. The owner permits background judgment and separately rejects
a gapped/double selected border. No sparkle, extra guest guidance or replacement skip action is added.
Current issues #527/#526/#613/#481 were consulted alongside their earlier linked-issue audit in
PR #646; their profile-data, auth, Membership and renderer boundaries are retained.

Existing Button `AuthPrimary`/`AuthSecondary`, Surface `PrimaryContainer`/`Profile`, the admitted
40-unit decorative Avatar and semantic type/space/color roles fit this card. No public component,
new palette or token is added. Web keeps its existing contrasting profile surface and v4.3 pair.
Only the CMP guest profile opts into the existing Theatre expression; signed-in identity and the
adjacent account/Membership/preferences groups retain their existing expression and owners.
CMP's profile-scoped filled secondary recipe now matches Leptos, rather than painting the same
fill as its parent. The legacy outlined recipe remains available to existing recovery consumers.

Computed WCAG relative-luminance ratios from the canonical Theatre token pairs (Light / Dark):

| Pair | Ratio |
| --- | --- |
| Profile / Theatre page | 15.51 / 7.98 |
| Profile text / profile (also secondary text / fill) | 16.15 / 7.66 |
| Login text / scoped Login fill | 8.60 / 7.19 |
| Login fill / profile | 8.60 / 3.41 |
| CMP guest-avatar glyph / tonal circle | 11.11 / 10.04 |

The actual CMP outer page keeps its original foundation background; its guest card's Theatre
surface still separates clearly in the opened captures. These are token-pair calculations,
not pixel-match, screen-reader or deployed acceptance.

## Design and functional deltas

- Web removes `GuestInfo`, its hover/focus/pin/outside-click/Escape state, panel and trigger,
  orphan account-menu focus helpers, CSS and two unused localization keys. The healthy guest
  test checks absence from the rendered tree in VI/EN, preserving both named action IDs.
  Profile action bottom margin is zero, leaving only the card inset after Register.
- CMP removes the redundant guest badge and unused resource, shortens the existing helper to
  “Đăng nhập để mở thư viện của bạn.” / “Sign in to open your library.”, uses a 40-unit tonal
  decorative avatar, the editorial title role and filled `AuthSecondary`. Both actions retain
  the shared 60-unit minimum. Signed-in avatar geometry/color remain unchanged.
- Actual rendering exposed two issues before handoff: CMP's previous `AuthSecondary` blended
  into the profile, and large-text Web Register split mid-word. The scoped shared profile pair
  resolves the first; the Web profile pair's token inset resolves the second. CMP's smaller
  title role keeps “Welcome,” whole at 320dp / 200% text.
- Session/auth policy, callbacks, login eligibility, attempt-owned failure suppression, pending,
  failed, cleanup/retry, typed route returns and public listening owners are unchanged. This is
  presentation and removal of optional help, never a guest-access gate or invented identity.

## Executed component and product checks

All commands ran from the repository root with Rust 1.98.1, existing Gradle 8.14.3/JDK 17 and
Python from Homebrew where required. Full logs are ignored under `artifacts/agent-runs/646-guest-tonal/`.

| Command / target | Actual result and boundary |
| --- | --- |
| `cargo test --locked -p cantos-theatre --features preview features::account::tests` | PASS, 16 account tests; includes actual SSR guest/checking/failed composition, not browser authentication |
| `cargo xtask ui-preview leptos verify` | PASS, actual native SSR tests, WASM lint and Trunk gallery build; final `leptos-verify-final.log` |
| `cargo xtask ui-preview cmp verify` | PASS, 319 JVM tests, zero failure/error/skip; `cmp-verify.log` |
| `cargo xtask theatre-preview verify` | PASS, shared 618 tests (3 skipped), sandbox 179 (4 skipped); zero failures/errors; final `theatre-verify-final.log` |
| `cargo xtask theatre-host android-release` | UNSUPPORTED before build: no configured installed Android SDK (`ANDROID_HOME` / `ANDROID_SDK_ROOT`) |

The seven existing ignored JVM cases are disclosed, not called passing. Account tests drive
production composables/owners: a failed attempt stays on Login, other failures remain visible,
Pending/CleanupRequired keep their real disabled/recovery controls, Back fences late results,
and guest Sign in/Register remain explicit intents. Existing signed-in/logout cases also ran.
The updated guest geometry expectation is 40, preserving the meaning of its layout test.
Final all-gate preflight on the base above is reported separately in the PR with its actual result.

## Actual Web observations

Local production Theatre Web on 8082 over the native Server/Identity development host on 8080;
actual healthy guest, empty public catalog, Chromium/DPR 1. The prior v9.2 navigation matrix and
interaction observations remain in [the navigation report](theatre-navigation-646-product-qa.md).
This delta rechecked the guest card at 390px VI, 1280px EN, 320px EN and both themes. There are
exactly two auth actions, a decorative avatar and no guest-info trigger/panel in the rendered tree.
Normal controls measured 60px high, border 0, distinct semantic fills. At 320px there was no
horizontal page overflow. Preferences and truthful Premium remain present.

Actual Me → Login → Back returned focus to `theatre-account-login`. Me → Register retained
`from=account`; browser Back returned Me and focused `theatre-account-register`. No credentials
or enrollment request were submitted. Keyboard Tab moved Login → Register → Appearance, with
one flush visible keyboard perimeter; the removed disclosure adds no focus stop.

A temporary localhost HTML style set the real root font to 32px. It was restored byte for byte
from `index-before-text200.html`; no text-scale switch/style ships. At 320 × 800, including the
15px vertical scrollbar (client width 305), final EN Sign in and Register each retained a single
label line; VI Sign in wrapped at its word boundary and Register stayed on one line. Heading and
helper wrapped without clipping; both 120px CTA targets remained operable and no horizontal
page overflow was measured. This is app text enlargement, not browser zoom or OS text scaling.

Opened captures in the ignored delta directory:
`web-guest-light-vi-390.png`, `web-guest-light-en-1280.png`, `web-guest-dark-en-320.png`,
`web-final-light-vi-1280.png`,
`web-guest-dark-en-text2-320-final.png`, `web-guest-light-vi-text2-320-final.png`.
The EN text-200 sidecar records viewport/font/client/scroll widths and actual label bounds.
The temporary full-page-plus-clip experiment did not preserve the expected capture geometry and
is not acceptance evidence. The final text-200 captures are ordinary viewport captures.

## Independent JVM pixels and shared freshness

Actual production Account over deterministic identity ports was captured on JVM Desktop Compose,
not Android/iOS. Opened current captures in `mobile/preview-sandbox/build/reports/theatre-auth/`:
`account-guest-light-390x844.png`, `account-guest-dark-390x844.png`,
`account-guest-dark-en-768x1024.png`, `account-guest-light-text2-actions-320x800.png`,
`account-guest-dark-en-text2-actions-320x800.png`. Normal and large-text action pairs are visible,
filled and separate; the neutral circle has no decorative border. Final normal card height is 321dp.
A previous original-light capture is retained under `before/`; it shows the 64-unit avatar,
guest badge, longer helper and outlined registration action. The after capture is shorter.

`GuestCtaPairPixelTest` checks actual pixel fills near the idle edge and text colors in both
themes, equal heights and nested/non-profile scope isolation, then writes
`cmp-guest-cta-pair-tonal-{light,dark}.png`; both images were opened. Disabled/pending/callback,
focus and motion tests run independently. No animation smoothness or device claim follows.

Merging current develop brought #660's FilterChip change and #659's mobile Search change.
The global renderer digest invalidated old paired receipts. This checkpoint reviewed that shared
source delta and reran both renderer suites; it did not merely rebind old hashes. Web's actual
FilterChip matrix was driven by pointer: selection changed `aria-pressed`, retained the check,
removed default border/shadow and preserved disabled/loading explanations. Light matrix and
Dark/200%/320-frame captures were opened (`shared-filter-chip-light.png`,
`shared-filter-chip-dark-text2.png`). On the current JVM renderer,
`cmp-theatre-filter-chip-theatre-selection-dark-text2.0-320.png` and the tonal action-pair images
were opened; the existing chip interaction/pixel/clock coverage ran.

Unchanged component scopes retain the source-attributable observations in the
[earlier #646 shared review](theatre-navigation-646-shared-qa.md) and its linked prior reviews.
The fresh 20 paired records explicitly combine that unaffected coverage, the current complete
renderer suites and the affected recipe/FilterChip observations above. They are bounded
Chromium/JVM qualification; catalog registration and Android/iOS/device/AT acceptance remain distinct.

## Residual acceptance

- Actual browser zoom, screen-reader walks, real signed-in profile facts/photos and live published
  audio remain NOT_RUN / unavailable prerequisites as detailed in the navigation report. No
  personal identity is fabricated. The original broad #641 acceptance remains partial.
- Whole-Me Preferences/Premium at 320px with root text 200% still have narrow word wrapping;
  this guest-card delta does not redesign those sections. Their broader large-text acceptance
  remains open under #527/#641. The guest heading/helper/action recheck above is narrower.
- JVM previews and semantics do not establish Android/iOS, OS Back, IME, touch hardware,
  production release exclusion, actual sign-in or deployment acceptance. Android release needs
  an installed configured SDK; device/AT acceptance then needs its corresponding host.
