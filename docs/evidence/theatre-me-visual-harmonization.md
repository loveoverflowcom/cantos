# Theatre Web Me — UI-REVIEW-716

4 November 2026, one automated reviewer/implementer. This is a bounded Web review after
[#716](https://github.com/loveoverflowcom/cantos/pull/716), retaining
[#710](https://github.com/loveoverflowcom/cantos/pull/710)'s compact preferences.
Base: fetched `origin/develop` at `20e881602116589ea21d7d0e100f3c191516e555` (merged #720).
The PR records HEAD, checked content identity, final production build and preflight results.
No merge, deployment, production-account acceptance or independent human approval is implied.

## Inputs and review boundary

Read the complete `CANTOS_PROMPT_INPUT_V1` UI-REVIEW-716 Library attachment, repository
engineering, UI design/inspection/fix, Leptos and code-review instructions, and the mounted
account, Library, navigation, platform and shared-control owners. Compared actual compiled
before/after screens, not only source or the supplied drafts. The light/dark advisory drafts
are [Light](https://drive.google.com/file/d/13WfqlROQOnI5At371vyggHWtNRBlq54J/view) and
[Dark](https://drive.google.com/file/d/17rvonamO_zN-nXY2ghXl3BuBKDjDbe9X/view);
the [v9.2 handoff](https://drive.google.com/file/d/1m_-Nzmr1BA2S8J_eQ_T4Cf519rVhdVLI/view)
retains the separate 600/1200 navigation contract and single-player owner.
[Material research](https://design.google/library/expressive-material-design-google-research)
informs functional grouping and hierarchy; it is not a replacement for Cantos tokens.

The route/component inventory was inspected before composing a repair:

| Region | Finding and disposition | Owner / risk |
| --- | --- | --- |
| Page heading and composition | **P2, confirmed, high confidence:** desktop row coupling makes Premium height create a large unrelated gap before Activity. Tighten heading rhythm; give listening and account tools independent stacks. | `account.rs`, `account_me.css`; narrow order and guest/recovery content must remain reachable. |
| Profile/avatar/alias | Keep existing semantic Profile Surface and generated identity; no invented account metadata. At 320/200%, a long valid alias exposed an ineffective inherited container query on `display: contents`. **P2, confirmed:** narrow container now stacks avatar/copy and reduces only inline insets. | App composition, not identity contract. Valid alias fixture is below the production 64-character bound. |
| Activity and shortcut cards | Retain both real destinations, useful localized helper text and editorial heading. **P3, confirmed:** icon well blended into the hovered card; invert the existing surface pair on hover/focus. Press has the higher surface tone. | App CSS only; no custom shared Button/Select variant or palette. |
| Shortcut destination | **P2, confirmed:** native `#library`/`#history` changed the page but focused main instead of revealing Saved/History. Resolve the existing typed section heading after mount; keep ordinary Library entry at its top. | Navigation and platform own focus. Guard a queued frame against a later route change; missing private target falls back to main. |
| Theme | Keep one shared compact Select, full accessible name, current value/icon and Light/Dark/System options. | Existing root theme state; no new persistence claim. |
| Locale | Keep one shared compact Select and VI/EN autonyms/options with current short code. | Existing callback/store/messages; one preview-only text-scale label is added through the canonical catalog. |
| Premium / Sign out | Keep truthful unavailable information, CTA, dialog and cancellation behavior. Never align Activity by stretching a fake card. | Existing membership/auth owners; no entitlement, billing or cleanup change. |
| Shell / Library / player | Retain navigation families and existing three real Library sections. Add `tabindex=-1` only to their headings and one token focus outline. | No route grammar, API, player, audio, storage, rights or approval change. Actual playback remains untested here. |

Auth-return snapshot restoration remains in its existing branch. `#history` also reveals its
heading when a pending session becomes signed in, while the existing guest normalization
still sends a private reload to Landing. No selected-tab store or duplicate Library is added.

## Layout decision

| Option | Assessment |
| --- | --- |
| A — preserve the shared row grid | Small CSS diff, but unrelated content continues to determine vertical gaps; rejected after the measured reproduction. |
| B — two entirely independent columns, preferences inside the right stack | Removes coupling, but moves the already compact utilities away from their heading relationship and makes narrow ordering harder to explain. |
| C — heading/preferences row, independent listening/tools stacks | **Selected.** One restrained two-column composition; profile and Activity stay together, Premium and Sign out stay together. Narrow reading order is heading, listening, preferences, tools. |

Scope is medium: app composition plus the existing navigation focus seam. Expression comes
from hierarchy, semantic surfaces, spacing and recognizable action icons. No gradients,
decorative outlines, new colors, fake counts, new shared APIs or animation are needed.
Legacy app override duplication and dead preference-icon rules were removed instead of adding
another cascade. No ADR or roadmap reprioritization is warranted for this existing contract.

At matched 1440/Light/VI/16px, the baseline profile ended at 292.77px and Activity began at
425.75px: a 132.98px gap driven by the other column. The repair gives a content-driven 24px
stack gap (profile bottom 252.77px, Activity top 276.77px). Premium retains its own height;
there is no minimum height pretending to balance the two columns.

Reviewer scores (1 poor, 5 strong; subjective aids, not user-study measurements): grouping
2→4, spacing rhythm 2→4, shortcut destination clarity 2→4, state distinction 3→4,
brand/control consistency 4→4. Balance on large mostly empty screens remains a design judgment,
not a correctness defect requiring more decoration.

## Actual browser observations

Compiled Leptos/Trunk on macOS, ChatGPT in-app browser, loopback `127.0.0.1:8082`, explicit
`?mode=auth-preview`; network/live effects disabled. Synthetic A has alias Mây Lam and empty
real Library sections. A second explicit valid long-alias fixture exercises production
presentation validation. The diagnostic `200%` button injects root font 32px while mounted;
it is **not browser zoom**. Requested viewports are CSS widths, height 1000px; an observed
15px scrollbar makes 320px requested / 305px client, for example. No physical device emulation
or browser-version qualification is claimed.

| Matrix / interaction | Observed result / limit |
| --- | --- |
| Dark/EN/100%, final source: 320, 390, 600, 768, 960, 1024, 1280, 1440 | All actual widths checked: document scroll width equals client width, account does not overflow, two choosers, 24px profile→Activity gap. |
| Boundary widths 599, 601, 959, 961, 1199, 1200, 1201 | Same assertions pass across Me and navigation transitions. These are actual viewport overrides, not scaled desktop screenshots. |
| Light/VI/100%, 390 and 1440; System/VI/100% desktop | Opened rendered captures; current theme/locale values, readable heading/profile and compact controls retained. System followed this host's light preference; OS theme switching was not exercised. |
| System/VI/200%, valid long alias, 320, 390, 600, 1440 | No account overflow; 48px scaled stack gap, exactly two choosers. Opened the 320px viewport and representative full-page captures. |
| Dark/EN/200%, valid long alias, 320 and 600 | Compiled layout assertions and zero document overflow pass. Does not imply every theme×locale×width combination was exercised. |
| Enter on Saved and History; Back/Forward | Actual H2 focused and visible, native addresses unchanged, Library navigation family remains current. History focus restored on Forward. |
| Pending session → `#history` → deliver fixture A | Existing History heading receives focus after private content mounts. Guest reload remains fail-closed to Landing. Real authenticated refresh is NOT_RUN. |
| Theme/locale chooser | Actual options selected in each menu. Resize closes the open theme menu; Escape leaves its trigger focused and collapsed. Existing selected Dark option remains represented. |
| Sign out → Back | Existing confirmation opens with Back focused; cancellation restores Sign out focus. No real logout requested. |
| Hover + keyboard focus | Opened Light/VI screenshot: hovered Saved icon remains distinct; Tab focuses History with the existing single outline. Pressed styling source-reviewed only, no held-pointer capture. |

Before captures were built at the exact base. Final overview and viewport screenshots were
opened, including `final-1440-light-vi-app-viewport.jpg`,
`final-1440-dark-en-app-viewport.jpg`, `final-390-light-vi-viewport.jpg`, and
`final-320-system-vi-200-profile-viewport.jpg`. Before/after images and their hashes are attached
to the PR and retained locally under ignored `artifacts/ui-evidence/ui-review-716/`.
The raw viewport JSON includes exploratory captures: old invalid long-alias fixtures,
mismatched stale-tab viewport attempts, and full-page clipping artifacts are excluded from
acceptance. Use `final-normal-matrix.json`, `browser-regressions.json`,
`final-dark-en-large.json`, `shortcut-regressions-final.json`, and the final image manifest.

## Verification and remaining acceptance

Host preview suite: **605 unit tests and 6 doctests PASS**, including session/recovery, locale,
Library heading association, valid long-alias presentation and route-family regressions.
Preview host Clippy and preview WASM Clippy pass with `-D warnings`; package formatting,
`git diff --check` and the browser helper's Node syntax check pass. The committed
`tests/me-harmonization.cua.mjs` checks the actual mounted layout and trusted keyboard
destinations; it requires an already mounted signed-in diagnostic fixture.

Both selected Quint behavior capabilities pass **model-only**:
`cantos.presentation.guest-navigation` and `cantos.presentation.discovery-retired-language`.
Implementation conformance and exhaustive proof are NOT_RUN. Their runner used pinned
Node 24.19.0/npm 11.9.0; an earlier npm-10 attempt was UNSUPPORTED and is retained in logs.
The added diagnostic scale label uses a preview-only canonical resource. Regeneration changes
Studio and CMP files only in their digest comment; generated parity and 89 localization tests
pass (two existing skips). The final broadened preflight is UNSUPPORTED without an accepted
Gradle on PATH and leaves `impact:unknown` unanswered; it is not green. Independent repository
inspection reproduces the same 65 qualification findings as clean base, with no added findings.
The PR reports final changed-only preflight, baseline comparison and production WASM build
separately; these local observations do not override repository qualification failures.

| Layer / acceptance | Status and evidence boundary |
| --- | --- |
| Architecture / types / pure behavior | PASS scoped source review and host tests; no new state owner or wire type. Model-only behavior is a separate claim. |
| Compiled app / fixture browser | PASS for the named matrix and interactions above. Existing auth/Library state owner is real, account identity/data are explicit synthetic doubles. |
| Real DB/API/auth/session reload | NOT_RUN: no backend/account provisioned for this review. Run the documented live Theatre host with a real accepted session, repeat Me→Saved/History and authenticated reload. |
| Mini-player / one audio / playback | NOT_RUN: diagnostic has no real playable data/audio element. Repeat with admitted media on the live host; verify one existing player and no overlap at 320/200%. |
| Browser zoom / AT / physical devices | NOT_RUN: browser zoom probe did not change effective viewport/type size; CSS text scale is reported separately. Repeat 200% zoom, screen-reader names/reading order and mobile touch on actual browsers/devices. |
| Full state/theme/locale Cartesian matrix | NOT_RUN: named cells only. Guest/pending/error component behavior has host test coverage, not full browser acceptance. |
| Production deployment / remote CI | NOT_RUN here. Open PR only, human review and ordinary protection remain required. |

Production scope is presentation and focus. Existing auth/private-data gates remain authoritative;
the added fixtures and scale control are explicit preview-only. There are no migrations,
provider costs, Script IR/API changes, new resource schemas, cache/audio revisions or rights
changes. Recovery is reverting this app-scoped PR. Images/logs stay outside source control.
