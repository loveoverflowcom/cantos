# Theatre Web navigation v9.2 — PR #646

This source-bound local report supplements issue #641 and the
[shared component checkpoint](theatre-navigation-646-shared-qa.md). Reviewed by Codex on
30 October 2026 against base `d9f79480ab42342fae9582807c774405dcea50f2` and the PR working diff.
The v9.2 overview/tablet references were opened. The owner's later correction takes priority:
selected destinations have a filled indicator, bold label and check; no selected border or gap.
Keyboard-only focus retains one flush perimeter. No exact pixel-match claim is made.

## Implemented behavior and independent evidence

The pure Rust layout decision consumes viewport width, an in-memory wide-collapse preference and
actual localized text-fit facts. It owns no route, session, network or playback state. The browser
adapter measures real font metrics and owns only DOM presentation, modal/focus and scroll effects.
The three destinations remain Discover / Library / Me. Existing auth-return, recovery, explicit
preferences, shell logout and player owners are retained.

The supplied v9.2 design is the expectation for this change. The stable behavior catalog's
`cantos.presentation.discovery-retired-language` model was run successfully, model-only;
implementation conformance is NOT_RUN. There is no registered root-navigation responsive model.
That catalog gap is explicit; the pure Rust boundary tests and actual browser observations below
supply scoped implementation evidence, never a claim that a Quint model establishes UI conformance.

## Normal-size browser observations

Actual local Theatre Web, `cargo xtask app cantos-theatre --web` on 8082 with its native Server /
Identity development host on 8080; empty public catalog and admitted guest, no production data.
In-app Chromium, DPR 1, Vietnamese/English, bundled Noto fonts. Both locales were checked at:
320, 390, 599, 600, 700, 768, 839, 840, 1024, 1199, 1200, 1280 and 1440 CSS px.
The observations wait for the resulting mode, rather than sampling before the resize event.

- 320–599: drawer-only header; no persistent rail or Web bottom bar.
- 600–1199: 96px rail with 24px icons and complete captions; Me stays at the bottom.
- 1200–1440: 216px expanded rail. Collapse/expand is local and preserves the route.
- All measured normal-size widths had no horizontal page overflow. Full results are ignored
  `artifacts/agent-runs/646-v92/responsive-widths-{vi,en}.json`.
- Tablet modal at 768: drawer width 320; content x=112 and width=640 before/after opening;
  body overflow becomes hidden, current Library gets focus. No content-width shift.
- Open tablet drawer survives 600→768 (same mode). At 1200 it closes, unlocks scrolling and
  focuses the current Library rail link. Escape at 320 restores the visible header Menu.
- 320 drawer is exactly 288px, with equal client/scroll width. Tab from last Me wraps to Cantos;
  Shift+Tab returns to Me. Modal semantics and title association are present.
- Library after pointer activation: `border:0px`, `outline-style:none`, `box-shadow:none`,
  `:focus-visible=false`. Keyboard navigation to Library gives a solid outline with offset 0px.
- Explicit Dark was also selected from Me. Library was opened selected in Light with its
  actual shared SVG measured at 24 × 24px beside Home, with no pointer outline/border.
- The compact selected check was moved inside its filled indicator so its paired dark ink
  remains visible in Dark. Icon geometry remains shared; no stroke-width or font-glyph override.
- Me uses the same admitted shared decorative Avatar fallback as navigation, truthful guest
  actions, one theme selector and one language selector. Language was changed EN→VI and the
  explicit Light selector exercised; System followed the host's Dark fact. The header has no
  preference/avatar utility controls. Premium still says no information rather than inventing access.

Opened normal-size captures (ignored local artifacts): `library-selected-1280-dark.png`,
`tablet-drawer-768-dark.png`, `me-390-light-vi.png`, `drawer-390-light-vi.png`,
`library-selected-1280-light-final.png`, `tablet-drawer-700-final.png` under
`artifacts/agent-runs/646-v92/`. The early tablet capture predates the final shared-dialog-style
scrim adjustment; later phone/text captures show the final primary-role 32% scrim.

The final guest journey also submitted `Ánh đèn cuối sân khấu` through the actual catalog search,
then crossed compact→narrow; the exact query value remained. From the drawer, Library opened its
direct sign-in form with `Back to Library`; that contextual return restored Library. These are
actual guest/query/auth-navigation observations, not an authenticated sign-in or private read.

A final selected-Me compact review found that the 40px circle overhung the 36px indicator.
Navigation now consumes the admitted Small (32px) Avatar, while the profile keeps Medium (40px).
The compact Me caption has the same 46px offset as the other destinations. The actual 768px
recheck measured the 32px circle wholly within the indicator; `me-compact-768-final.png` was
opened against `me-compact-before-polish.png`. This late geometry-only check used the actual
Failed/recovery state with the local backend stopped; no healthy session is inferred from it.

## Actual font enlargement and browser-zoom boundary

A temporary localhost HTML style set the root font from 16px to 32px (`html {font-size:200%}`).
This changed actual rendered text and measured overflow, not a prototype query, width heuristic,
CSS class forcing a layout mode or a production detector. The exact original `index.html` bytes
were retained and restored; no test flag/style ships. This verifies app font reflow, not browser
font preferences, operating-system text scaling or browser zoom.

At 1440px the Vietnamese compact probe measured 121px into 80px, and the expanded probe could not
fit; the actual Rust decision selected drawer. At 320px both VI/EN destination words remained
complete, the modal had client=scroll width 288px and no horizontal page overflow. Drawer chrome
now stays bounded while labels grow: fixed insets, 24px icons and 20px checks leave room for words.
The narrow header keeps an accessible 48px Menu icon and a 44px brand mark.

English was observed at 320, 390, 600, 700, 768, 1024, 1200, 1440 with actual root font 32px:
all chose drawer, preserved the open modal across same-mode changes and had no page overflow.
Vietnamese was opened at 1440 and 320 in the same 32px font environment. Final captures opened:
`drawer-320-text200-vi.png`, `drawer-320-text200-en.png`; the earlier before-fix capture is
retained locally to attribute the chrome/word-space correction. Full English facts are in
`text200-widths-en.json` in the same ignored artifact directory.

Actual browser zoom is NOT_RUN. Native computer control reports missing permissions, shortcuts
through the tab adapter did not change DPR/CSS viewport, and Chrome settings is blocked by the
browser URL policy. No zoom success is inferred from those attempts. Resume with a permitted
browser at genuine 200% zoom; repeat the 599/600 and 1199/1200 CSS-width transitions and focus checks.

After that size adjustment, the same 320px/32px-font scenario was repeated in EN and VI.
Both modal client/scroll widths were 288px; all caption scroll widths equalled their available
widths. The navigation avatar was 64px under this enlarged-root fixture, with complete labels.
Opened: `drawer-320-text200-small-avatar-final.png` and
`drawer-320-text200-small-avatar-vi-final.png`. The original HTML was restored again. The native backend was restarted for the final normal-font
VI Library capture (`final-library-1280-vi.png`); actual guest actions returned and no horizontal
overflow was observed at 1280px.

## Preflight repair scope

The previous #646 failures were independently reproduced on the clean base. Repairs are bounded:

- Metadata dependency kinds now distinguish dev-only edges. The existing Server→Identity native
  test dependency is accepted only as dev-only; runtime/build cross-service imports remain denied.
  Its impact still reaches Server checks. The graph fixture includes actual localization and
  native-test edges; selection expectations now include their real downstream gates.
- Identity environment ownership includes its actual `firebase.rs` provider configuration module.
- Mutation scratch copying includes localization's catalog/build inputs, excluding build outputs.
- Additional clean-base reproduction found two stale Python fixture expectations: the source-only
  Android version reader correctly refuses the current dynamic Gradle expressions, and a journal
  tamper fixture used GNU-only `sed -i`. Tests now cover a declared literal version fixture plus
  explicit refusal of the dynamic production host, and mutate the exact journal field portably
  with Python. No production release parser/host operation was changed; actual built APK version
  acceptance remains outside this work. A missing real macOS `flock` test prerequisite was
  installed from Homebrew (`flock` 0.4.0); no lock test was replaced with a fake or skipped.
- Locked Node 24.19.0 and npm 11.9.0 were installed only in ignored task artifacts; the official
  Node archive checksum was verified. Quint setup used its existing locked owner, no manifest drift.
- Stale qualification receipts were replaced by new source-bound paired records after actual
  Web/JVM checks and opened captures. The old records and compatibility baseline remain intact.

Focused graph/environment tests passed; the final preflight selection tests passed 80/80.
Full runs exposed two existing macOS socket-teardown races: a released TCP bind and a drained
Unix listener can remain briefly observable after their last handle closes. The test observations
now poll the actual endpoint for at most one second and retain the final free/refused assertion.
The production probes are unchanged; no assertion is skipped or replaced with a fake.
The final preflight log/summary is reported in the PR, rather than asserted in advance here.

## Deferred acceptance

Authorized account display-name/photo facts are absent from the actual session/profile projection
(#481). The shared Avatar implementation/fallback is admitted, but personalized identity is not
claimed; no name, actor ID, email buffer or fixture is substituted. The profile owner must supply
an authorized projection before that portion can be accepted.

The local catalog has no playable published release. Persistent player/no-autoplay/private-fetch
claims retain the existing source/functional tests and owner boundaries; a live paused-playing-
seek journey across these new geometry transitions is NOT_RUN. Resume with an authorized local
immutable published release. Authenticated/recovery/log-out scenarios retain the earlier v9.1
and final Rust test evidence; this browser pass is an actual guest journey, not fresh deployed or
signed-in acceptance. Screen readers, Android/iOS and deployed auth/browser acceptance are NOT_RUN.
