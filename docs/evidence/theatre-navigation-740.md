# Theatre primary navigation — issue 740

Source-bound implementation and local evidence for [#740](https://github.com/loveoverflowcom/cantos/issues/740).
Base: `origin/develop@ae0457bbbf5fa30e2b93d33eddd149948b12b485`, fetched 6 November 2026.
The PR binds the final checked tree and merge SHA. No deployment.

## Boundary and decision

The listener's primary goal remains discovery and listening; navigation is secondary chrome
at Theatre's Medium intensity. `view/live_shell.rs::PrimaryNavigation` owns three destinations,
localized labels and the authorized account presentation; `NavItem` owns native anchor,
current-page semantics, label container and focus. The app's existing rail/drawer owners keep
layout, native modal lifetime, history, responsive text measurement and focus restoration.

The snapshot's raw anchors still existed at the fetched base. Catalog lookup found `nav-item`
/ `NavItem`, native `LinkHref`, and `idle/current/focus` scenarios. A new leading-slot API is
unnecessary: the current children contract already admits composed visible content. This app
puts a decorative `aria-hidden` leading span and a localized visible text span inside it.
Only `.theatre-nav-link > .cui-nav-label` uses `display: contents`, preserving the app's flex
geometry while the shared anchor retains its semantics. The actual browser accessibility tree
was checked separately; source anatomy alone is not that evidence.

The leading span contains the existing Home glyph, admitted LibrarySpines icon or existing
`ProfileAvatar`. It has no callback, link, control, ID or tabindex. Avatar identity is still
the account owner's admitted Server alias/seed, with the original person-outline fallback.
The localized destination name remains Tôi/Me; aliases never name navigation. Scope-specific
IDs distinguish the rail and drawer copies. The three exact hrefs remain `#discover`,
`#library`, `#account`. There is no click guard or preventDefault in this composition.

No shared API, catalog, token, qualification baseline, router, session, player, backend, wire
contract, persistence or CMP source changes. Studio's existing NavItem consumer is unchanged.
Queue: no change; this slice changes neither dependency order nor planning rationale.
No ADR is needed: this is reuse of an existing public contract.

## Admission and token map

Both-renderer admission inputs are unchanged, confirmed by
`python3 scripts/ui_preview_catalog.py --digests`:

| Input | SHA-256 / source-bound record |
| --- | --- |
| Tokens | `79294511da18b463bd20056aa70cda914d488c43d6ccaabf0b398184b4fc1218` |
| Leptos | `e92cc3969ad8f6b2dd7947917bb21ab48c01e63fab2fdb078428c0bf8053590b`; [#737 record](ui-requalification-737-nav-item-leptos.json) |
| CMP | `f88fcfc1645899658ad5aff58259ca5a263f4f0b09a3dac4e62e03b08149fe7a`; [#615 record](ui-requalification-615-nav-item-cmp.json) |

The new app import uses these existing records. No hash or qualification was refreshed just
to permit adoption. No new shared primitive or semantic variant requires CMP implementation.

| Anatomy / state | Existing roles / owner |
| --- | --- |
| Native destination and current name | NavItem / LinkHref; app-localized Discover, Library, Me keys |
| Decorative leading | `size.avatarSmall`, existing Avatar palette; app Home and shared LibrarySpines |
| Label typography and shape | NavItem `type.label.*`, `type.family.ui`, `web-shape-control` |
| Selected | `color.primaryContainer` / `onPrimaryContainer`, label weight and visible label underline |
| Hover | Existing `state.hoverOpacity` state layer; compact rail paints it on its existing indicator |
| Focus | Existing `size.focusWidth` / `color.focus`; app's flush perimeter |
| Placement | App's existing rail/drawer widths and padding; existing spacing roles |
| Reduced motion | Existing shared NavItem and app reduced-motion rules |

An initial inspected text200 image showed shared anchor underlining reaching Avatar initials.
The final app style places the underline on the visible destination label only. A browser
probe also showed that the existing compact rail's `background: transparent` shorthand erased
the anchor hover image. The final compact hover decorates its existing pseudo-element instead;
selected background and icon foreground keep their paired roles. Neither fix changes geometry.

## Behavior and regression evidence

Behavior SSOT lookup: `cantos.presentation.guest-navigation`, lifecycle `draft`, owned by
`app/routes.rs` and the shared Theatre manifest. Classification: no semantic change;
the approved [guest-entry rules](../design/theatre-guest-entry.md) and existing navigation
outcomes are preserved. No model, lifecycle, guard, admission or source binding changes.
The registered model remains separately selected by final preflight; model PASS is neither
implementation conformance nor browser acceptance.

The new `navigation_reuses_shared_links_with_decorative_leading_and_scoped_ids` test failed
against base markup: zero shared NavItems instead of six. It then passed with the production
composition. It checks unique rail/drawer IDs, composed label/leading anatomy, decorative
semantics, absence of nested interactive descendants, personalized/fallback output and removal
of identity after switching back to Guest. Existing route-family/href/current and VI/EN label
tests remain, with attribute-order-independent matching after the shared anchor added IDs.

Executed checkpoint checks, from the isolated checkout:

| Command | Actual result / boundary |
| --- | --- |
| `cargo test --locked -p cantos-theatre --features preview view::live_shell::tests` | 12 tests PASS; native string-render/application tests |
| `cargo test --locked -p cantos-ui --test actions_navigation` | 19 tests PASS; existing shared contract |
| `cargo test --locked -p cantos-studio ui::tests::` | 14 tests PASS; existing Studio consumer, including native pane links and unique drawer IDs |
| `cargo xtask build cantos-theatre --web` | PASS; production feature lane, dev CSR bundle |

The first test attempt had a test-only view-macro parse error, corrected before reproducing
the failing regression. An initial Studio `ui::tests::workspace` filter selected zero tests;
it is not counted as evidence. The corrected filter above executed 14. Full logs are retained.
The first handoff preflight also exposed one root-shell test matching the old exact raw class;
it now explicitly requires six shared NavItems, and unresolved admission rejects either class.
Its localization guard identified ID-fragment literals inside the view macro; IDs now derive
from the existing destination hash without any displayed literal or guard exemption. The
guest-navigation gate initially lacked its exact pinned runtime/installation; the repo's
explicit local setup uses already available Node 24.19.0/npm 11.9.0 and the locked Quint
dependencies. The original FAIL/UNSUPPORTED outputs are retained, separately from final results.
The installed tool's supervised-process probe also returned EPERM inside the default sandbox;
the same probe with host process permission reported all three exact pinned versions. Final
handoff runs with that permission, without weakening the model gate or its assurance obligation.
The first full repository Python run used macOS's symlinked `/var/folders` temp path:
13 failures and 88 errors among 1386 tests, including strict release-path refusal and
physical/logical path mismatches. A focused nine-test Android evidence probe passed with
`TMPDIR=/private/tmp/cantos740-tests`; final handoff uses that task-owned physical temp path,
as documented in CONTRIBUTING. No repository test or path-safety rule was weakened.
Final handoff uses `cargo xtask preflight --changed-only --base origin/develop`; its actual
summary, obligation answer, checked SHA and final self-review are recorded in the PR/issue.
After the temp-path diagnosis, only the failed repository gate is refreshed with
`cargo xtask verify repository` and the physical temp path. The unchanged passing app/model
gates retain their content-attributed evidence; the original aggregate FAIL is not renamed PASS.
It selects repository, the guest-navigation model, Rust format/lint/tests and Theatre Web
lint/build. No out-of-scope gate is skipped to manufacture success.

## Browser provenance and inspected output

Cwd: `/Users/manhblue/Documents/Codex/2026-11-06/task-8/cantos`.
Rust 1.99.1; `CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
CARGO_INCREMENTAL=0 TMPDIR=/private/tmp/cantos740-tests`. Browser checks use Chrome headless with a task-owned temporary profile,
DPR 1 and disclosed CDP focus emulation. Browser version and exact observations are retained
in `artifacts/agent-runs/issue740/browser-after.json`.

`PHASE=before node artifacts/agent-runs/issue740/browser-check.mjs` exercised the retained,
source-attributed #739/base production bundle. Its full WASM hash matches the prior stripping
record. The same driver uses the final compiled frontend for the after run. Only the
non-executable WASM custom name section is omitted in the browser fixture; executable/data
section hashes and exact full/fixture hashes are in `wasm-fixture-provenance.json`.

CDP Fetch supplies explicit synthetic Identity session facts, admitted account presentation,
empty Library and Catalog responses, session echo and exact-origin CORS headers. It blocks
other requests. This exercises actual production components and HTTP adapters with named
external doubles; no real cookie login, Identity service, API/database, content or CDN acceptance.
No production DOM or UI state is replaced with a mockup.

Matched captures: actual widths 1440/800/320 CSS px × Light/Dark × VI/EN × root text100/text200,
height 1100, DPR 1: 24 per source. Modes are selected by the real layout owner; text200 can
fall back to drawer rather than forcing an artificial compact rail. Text200 is root font
scaling, **not browser zoom**. Locale is initialized through the existing stored preference;
theme follows the emulated system preference. The labels are the actual fixed VI/EN catalog
strings, not injected long replacements. Account fixtures use a synthetic generated alias.

Baseline: 31 observations PASS, zero runtime exceptions. Final after: 34 observations PASS,
zero runtime exceptions in Chrome 156.0.8078.80, including native Meta-click, link
accessible-name, keyboard-focus, selected-hover and signed-in-to-Guest teardown observations.
The matched flow covers three visible destinations and one
current page, unique IDs, noninteractive descendants, 48px targets, label/avatar bounds,
rail toggle focus, Tab/Enter, history Back, drawer forward/backward Tab trap, Escape/opener
return, drawer navigation/main focus, authorized avatar fallback and guest chrome exclusion.
Synthetic modified-click default-cancelation checks are explicitly distinct from trusted
native Meta-click opening an actual new tab; no claim of a real clipboard or context-menu tour.

Opened base captures: `before-800-dark-vi-VN-1.png`, `before-320-dark-vi-VN-2.png`.
Opened final captures: `after-1440-light-en-1.png`, `after-800-dark-vi-VN-1.png`,
`after-320-dark-vi-VN-2.png`, `after-keyboard-focus.png`, `after-selected-hover.png`.
The decorative leading/labels stay aligned, the avatar has no underline or clipping, current
labels retain their marker, and keyboard focus/compact hover are visible. Other matrix images
are captured-only. The original avatar-underline finding and
failed hover probe are retained. Harness-only failures (presentation route, CORS/session echo,
brand-link position in focus order, Shift-Tab driver argument) were corrected against current
source contracts; a further immediate focus assertion was changed to await the existing
focus-after-mount outcome after the browser independently confirmed its eventual main focus.
None is reported as a product pass. No screen-reader walkthrough or
frame-pacing/performance claim; reduced-motion uses the existing preference rules.

## Independent gates and open acceptance

L1/L2: source/contract/admission review, no new dependency or domain decision. L3: native
regressions and final preflight, statuses in the PR. L4: actual browser/HTTP adapters with
the named doubles, driven trusted input and inspected images. L5 real services, publication,
media, native/mobile and deployed execution are NOT_RUN and outside this presentation change.
No migrations, API compatibility lane, paid provider, rights/approval or cache changes.

AC-01/02/03 have source/regression evidence. AC-05 is completed by this record, actual local
checks and fresh issue synchronization. AC-04 remains partial: the matrix uses root text200,
not actual 200% browser zoom, and does not exhaust long-label or assistive-technology cases.
Resume the same real frontend navigation scenarios at actual browser zoom200 with its renderer
owner, including expanded/compact/drawer, VI/EN and keyboard/focus checks. Next acceptance owner:
unassigned. Real-service follow-up needs the actual local API/Identity and a free frontend
port, then `cargo xtask app cantos-theatre --web`; it grants no deployment.

The issue remains open while that acceptance is incomplete. Cleanup removes this checkout's
Cargo output and any downloaded Drive images, retaining command logs, hashes and captures.
