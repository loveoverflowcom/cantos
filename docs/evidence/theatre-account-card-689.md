# Theatre account card presentation (#689)

Scope: source-bound UI repair for [#689](https://github.com/loveoverflowcom/cantos/issues/689),
based on `develop@994c8174541c659dc1607594af862653aa20815d`. Web and JVM Compose evidence below
are separate from authenticated browser, Android/iOS and deployed acceptance.

## Boundary and source audit

The invariant is that Me displays only the current admitted account's presentation, while
its shortcuts and sign-out remain readable and retain their navigation and confirmation intents.
The owners are Web `features/account.rs`, `features/guest_identity.rs`, `styles/account_me.css`
and native `features/auth/presentation/AccountScreens.kt`. Existing shared Profile Surface,
Lavender Pair Avatar and Text Button APIs are reused without shared-token or API changes.

Web `live/account.rs::session` requests `/api/v1/theatre/account/presentation` under the
existing session binding. An ordinary display outage retains an admitted scope without
presentation; session changes refuse the result. `AccountViewState::presentation` projects
facts only in `SignedIn`. Native `NativeAccountController` initially publishes `SignedIn`
without presentation, then attaches a reply only while its issued intent, exact grant and
expiry remain current. Missing/failed display data therefore legitimately uses a glyph;
Checking, Guest, Failed and LogoutPending never invent an alias. Existing owner regressions
cover refresh, logout, late replies, account switch and display failure.

The original owner screenshots have no deployed SHA, session phase or response evidence.
Their missing initials cannot be attributed to a deployed defect from source alone. Separately,
the Web auth diagnostic admitted A/B without presentation: this repair supplies clearly
synthetic A/B aliases in that preview-only module. The live adapter and generator remain
unchanged. [#662](https://github.com/loveoverflowcom/cantos/issues/662) and
[#670](https://github.com/loveoverflowcom/cantos/issues/670) retain real-account acceptance.

The browser baseline reproduced a separate presentation defect: hover filled a shortcut
with `surfaceContainer` while its icon and label inherited `onPrimaryContainer`; the chevron
used a third foreground. A Me-specific zero-padding override placed glyphs against the pill.
The shared Text Button's default `primary` ink also had insufficient contrast on the profile
card. Scoped paired ink, consistent insets and the existing shared Button fix these symptoms.

Web now groups avatar, localized bold serif greeting and supplied alias horizontally when
signed in. Guest and failure information remain explicit. Native keeps its existing editorial
alias without adding a greeting or routes; its existing navigation rows and logout share one
geometry and foreground, with lower font emphasis for logout. Confirmation state and callbacks
remain in their existing owners. Preferences (#690), identity generation, sessions and APIs
are outside this repair.

## Contrast from canonical Theatre roles

Ratios use WCAG relative luminance of the actual canonical sRGB values, rounded to two places.
They establish opaque role pairs; browser computed styles and native captures qualify their
use separately. No app color literal or new palette is introduced.

| Pair | Light | Dark |
| --- | ---: | ---: |
| Previous shortcut hover ink / fill | 1.23 | 1.28 |
| Previous Web logout ink / card | 1.50 | 1.08 |
| Profile ink / card (header, normal rows, logout) | 16.15 | 7.66 |
| Interactive shortcut ink / fill | 13.12 | 12.64 |
| Native row ink / container | 13.12 | 12.64 |
| Lavender avatar ink / fill | 9.42 | 7.96 |

## Assurance boundary

Behavior SSOT lookup searched `specs/behavior/catalog.json` and Theatre manifests by touched
paths and account lifecycle. No account-card capability is registered. Class: **no semantic
change**, authorized by #689's presentation amendment. Layout, typography, supplied diagnostic
facts and colors change; guards, authority, recovery and logout effects do not. No model change
is required. Handoff preflight retains its independent reverse-index/assurance selection.

L1/L2: existing inward ownership, immutable presentation and typed account phases retained;
no new domain rule, construction bypass, dependency or persistence operation. L3/L4: native
and Leptos regressions exercise the production compositions over explicit synthetic account
facts and recording intent sinks. L5: preview pixels and interactions are bounded renderer
evidence, never native device, authentication, storage or production evidence. Performance
is not an optimization claim; no database read or cache behavior changes.

No ADR is needed: the owner selected the layout and existing semantic roles apply. Queue and
planning guide remain unchanged because dependency order and rationale do not change. No
schema, API, Script IR, audio, rights, approval, provider-cost or deployment effect occurs.

## Reproduction and remaining acceptance

Use `TRUNK_BUILD_FEATURES=preview cargo xtask app cantos-theatre --web`, open
`http://127.0.0.1:8082/?mode=auth-preview#account`, expand diagnostics and select A or B. The
diagnostic is not a real session. Run native host checks with `cargo xtask theatre-preview verify`;
`CANTOS_ACCOUNT_CAPTURE_DIR` optionally retains the account test's synthetic PNGs outside Git.

Source, browser/JVM results, exact commands, opened capture inventory and final-diff preflight
are recorded in the accompanying PR. Images remain ignored under `artifacts/ui-evidence/689/`;
they are local evidence, not publicly attached assets.

Residual acceptance: repeat Me/nav using the same real account on Web and a named Android/iOS
target; record deployed/bundle/APK SHA, Light/Dark/System, VI/EN, narrow/wide layout, 200% text,
safe areas, keyboard/touch and TalkBack/VoiceOver. Recheck confirmation/cancel/Back/focus return
and exactly one logout intent. Missing device, screen-reader and authenticated deployment
evidence keeps AC-01/05/08 and the corresponding parts of #662/#670 open. The runtime owners
are unassigned; assignment and named environments are required before acceptance.
