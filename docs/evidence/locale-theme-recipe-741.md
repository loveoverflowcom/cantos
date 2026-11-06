# Locale and appearance consumers — issue 741

Source-bound review and implementation for [#741](https://github.com/loveoverflowcom/cantos/issues/741).
Base: `origin/develop@185469b1087e5aad6dab6c8433f0e6b00a3e6d12`, fetched 6 November 2026.
The PR binds the final checked content and merge SHA. This record establishes no deployment.

## Current contexts and boundary

The issue's 5 November snapshot predates [#582 / PR #749](https://github.com/loveoverflowcom/cantos/pull/749).
Studio now puts Appearance, Language and account in the authenticated rail/drawer footer.
Its unauthenticated/checking entry retains labelled preferences in the header. Neither context
uses ThemeCycleButton or short locale codes. Restoring the old icon-only Studio topbar would
contradict the newer owner. Theatre guests retain a short-code locale menu and theme cycle in
the header; signed-in preferences remain full-name selectors in Me.

The desired invariant is that shared controls retain their own anatomy, targets and names,
while app compositions select deliberate placement and presentation. Studio's explicit choice
and full labels must survive narrow layouts and enlarged text. No shared component API, token,
state store, theme cycle, locale registry, session/admission, API, cache, router or CMP changes.
No ADR is needed: existing accepted controls and preference callbacks are reused. The queue
and dependency order do not change. Original #613/#582 acceptance is not reassigned here.

## Role and state matrix at the fetched base

| Property / state | Canonical owner and renderer seed | Theatre guest header | Studio entry / authenticated footer | Decision |
| --- | --- | --- | --- | --- |
| Context and placement | App shell; shared Select/ThemeCycle API | Guest `PreferenceFields(compact)` | Entry header; workspace rail/drawer footer | Keep intentional context differences |
| Choice anatomy | `select`, Compact, existing enhanced Web menu and native fallback | Inline label, compact displayed code | Above label, full selected value | Entry reuses qualified menu; footer keeps native recipe |
| Theme interaction | `Theme`, existing shell signal | Dark → Light → System → Dark | Direct Light / Dark / System choice | Keep separate presentations and one owner per app |
| Locale data | `Locale::supported`, canonical `lang`, autonym | `vi-VN`/`en`; visible Vi/En only | Same canonical values, full autonyms | Keep registry, not a second list |
| Accessible name | Select label + full value; native fallback label | Hidden visual label and full autonym; code `aria-hidden` | Visible label and full value | Keep; verify actual AX name separately |
| Target / paint | `size.control`; Compact transparent block border | Topbar recipe: 48-unit full pill | Compact: 48-unit target, 40-unit paint at text100 | Keep intentional density difference |
| Shape / colors | Theatre expression and semantic surface/onSurface pair | Pill locale, reading-shape theme | Compact tonal Select pill | Keep expression; no palette overrides |
| Inline padding | Existing topbar recipe / compact Select | 24 CSS px each side at text100 | 16 start, 44 end at text100 | Keep renderer recipe; not comment-derived geometry |
| Value group | Theatre expression's `flex: 0 1 auto` | App repeated the identical declaration | Shared value layout | Remove confirmed redundant Theatre rule |
| Entry columns | Studio header composition | Not applicable | Two fixed columns made both native values disappear at 320px text200 | Use auto-fit columns and a wrapping enhanced value |
| Footer width | Studio composition + shared fields | Not applicable | Existing single-column footer | Keep placement and full-width affordance |
| Disabled / loading | Select `ChoiceState`, shared guards / reason / busy semantics | No disabled/loading preference state requested by caller | No disabled/loading preference state requested by caller | Reuse existing API; shared tests cover guarded controls |
| Hover / focus / expanded / dismiss | Shared Select and ThemeCycle implementations | Menu, focus-visible, Escape; inward tooltip placement | Shared menu, above labels, no theme tooltip | Keep shared owners; browser keyboard recheck |
| Persistence / System | Existing app callbacks / UiRoot media mapping | Locale key `cantos.theatre.ui-locale.v1` | Locale key `cantos.studio.ui-locale.v1` | Keep callbacks; theme reloads as System, no new storage |

## Confirmed delta

The opened baseline `studio-before-320-dark-vi-VN-2.png` showed only chevrons in both Studio
entry selects. Two fixed columns plus native single-line selected-value rendering consumed all
text room. This was reproduced on the current source build, not inferred from the old comments.
Studio entry now opts its two existing Selects into their admitted enhanced menu with app-owned
options and the same callbacks. A small app-local `wrap_values` input is enabled only by the
entry caller; footer callers retain the original native presentation. Full values wrap; the native fallback remains. The entry grid
uses auto-fit with a 12rem reading column that follows text scaling and stacks at narrow sizes.
Only `.studio-header .studio-preferences` changes its columns. No toolbar-wide rule is added.

Theatre's redundant value-flex override is removed after 36 baseline browser combinations
confirmed `0 1 auto`. The shared expression remains its owner. The existing source assertion
that could match its own test is replaced by rendering the actual compact PreferenceFields
consumer. A new regression compares compact and Me compositions across both locales and all
three modes: full names, canonical selection, a single theme action, no render-time mutation,
and no short-code/topbar classes leaking into Me. Studio's existing all-scope/locale/mode test
also requires two named enhanced comboboxes at entry, the unchanged native footer recipe,
retained native selections and no short codes.

## Component admission and source boundaries

Lookup reused `select` states `web-menu`, `web-menu-expanded`, `web-compact-inline`,
`web-native-compact`, `large-text`, `theatre-utility` and the existing ThemeCycle/icon action.
The [#737 Select qualification](ui-requalification-737-select-leptos.json) matches the current
Leptos digest. Both renderer input digests and tokens are unchanged:

- Tokens: `79294511da18b463bd20056aa70cda914d488c43d6ccaabf0b398184b4fc1218`.
- Leptos: `e92cc3969ad8f6b2dd7947917bb21ab48c01e63fab2fdb078428c0bf8053590b`.
- CMP: `f88fcfc1645899658ad5aff58259ca5a263f4f0b09a3dac4e62e03b08149fe7a`.

No generated outputs or admission baselines are edited. This is consumer reuse, not a new
shared semantic variant or a generic preferences component. Shared files have no editing owner
in this delta because none changes. Editor/auth fields and all other Select families are outside
this CSS selector and unchanged. Studio entry preferences alone gain the existing enhanced menu.

Behavior SSOT impact: none to capability semantics. `cantos.presentation.guest-navigation`
remains draft and is still selected by preflight because of existing source bindings. No state,
guard, admission, persistence contract or lifecycle changes. Model PASS is separate from DOM
and browser observations.

## Reproduction, evidence and limits

Cwd: `/Users/manhblue/Documents/Codex/2026-11-06/task-9/cantos`. Rust/Cargo 1.99.1, Trunk 0.21.14,
Chrome 156.0.8078.80 on macOS, DPR 1, task-owned headless profile. Build/check environment:
`CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0
TMPDIR=/private/tmp/cantos741-tests`. Behavior uses the already available exact Node 24.19.0 /
npm 11.9.0 through a task-owned PATH; locked Quint setup uses the existing xtask entrypoint.

Ignored evidence is in `artifacts/agent-runs/issue741/`. Browser drivers exercise compiled
production Leptos components. HTTP boundaries are synthetic: Theatre session 401 and empty
catalog; Studio session 401 or an explicitly synthetic session fact and empty private catalog.
No real Identity, cookie, database, content or CDN acceptance. Requests cannot reach production
services. No production credentials or private content are used.

Theatre baseline uses the retained #740 bundle, with identical current source at the fetched
merge and its retained build/provenance record. Final bundles are built in this checkout.
Browser WASM fixtures omit only the non-executable custom name section. Full input, fixture
and retained section hashes are recorded in `*-wasm-provenance.json`; no executable/data
section is replaced. Studio baseline is freshly built before its change. `browser-before.json`
and `studio-browser-before.json` retain computed dimensions and state measurements.

The matrix uses 1440/390/320 CSS px × Light/Dark/System × VI/EN × root font100/200.
Root font200 is text scaling, not actual browser page zoom. Normal versus enlarged sizes
are reported separately; labels/targets are allowed to grow and wrap. Gallery screenshots do
not substitute for app output. Captured-but-unopened matrix images remain screenshot-captured.
Opened images and final local gate summaries are bound in the PR/issue evidence comment.

Exact commands and full outputs are retained locally:

- `cargo test --offline --locked -p cantos-theatre --features preview guest_topbar_keeps`.
- `cargo test --locked -p cantos-ui --test fields --test actions_navigation`.
- `cargo xtask build cantos-studio --web` for Studio baseline; final handoff rebuilds both apps.
- `cargo xtask verify behavior --setup --capability cantos.presentation.guest-navigation`.
- `PHASE=before|after node artifacts/agent-runs/issue741/browser-check.mjs`.
- `node artifacts/agent-runs/issue741/studio-browser.cjs`.
- `cargo xtask preflight --changed-only --base origin/develop` on the final changed content.

Initial driver attempts had a wrong retained WASM filename, an assertion reading the short
and full text together, and an outdated Studio Menu locator. Those diagnostic failures remain
separate from corrected runs. The first Studio enhancement build had a view-macro parse error, fixed by naming the options
before the markup. The first behavior setup probe lacked the pinned npm on PATH;
its UNSUPPORTED result is retained. An intermediate attempt to enhance the footer also exposed disposed MenuState callbacks on
modal teardown/resize. That adoption is removed: the final footer keeps its original native
recipe. The errors and diagnostic stacks remain in the ignored record, not a claimed PASS or a
shared fix. Shared nested-modal cleanup remains an unaccepted follow-up at the Select owner.
No product gate or qualification is weakened to clear these.

L1: source/consumer boundary reviewed. L2: domain rules unchanged; canonical values and callbacks
retained. L3: scoped regressions and final preflight, actual statuses in PR. L4: actual production
components/HTTP adapters with the named synthetic boundaries. L5: named headless browser,
computed styles, trusted keyboard and opened images only; no real service or native device.

Remaining acceptance: AC-04 retains actual browser page zoom200, physical touch/OS preferences
and assistive-technology qualification as unrun; font scaling/media emulation are recorded on
their own terms. The native fallback has structural coverage, not a native OS picker walkthrough.
Real sign-in/session, backend/catalog/database and CMP/native acceptance belong to their owners
and are not supplied by this bounded consumer change. Theme persistence is unchanged, not newly
introduced. Motion timing/performance is not measured. No media/rights/approval/cost changes,
no live billable calls and no deployment. No Drive images were downloaded in this session.
