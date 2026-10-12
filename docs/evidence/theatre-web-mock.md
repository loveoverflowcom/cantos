# Theatre Web session-only mock evidence

Date: 2026-10-12. Scope: requester-authorized Leptos mock UI only, on the assigned dot cloud
computer. Base: `develop` commit `8eb7f495fd07936295bda328b7392e83ed97ffa5`, verified from origin.
The change is implemented source; compilation/runtime verification is **incomplete**.

## Invariant and ownership ledger

| Invariant | Failure mode | Owner / cheapest oracle | Evidence in this handoff |
| --- | --- | --- | --- |
| Mock state stays in session memory with no real transport or playback | fake UI implies backend/audio access or sends private data | Theatre model and source/import inspection | `statically-checked`; no engine/adapter/client exists |
| Unavailable/unknown episodes cannot be selected or queued | unavailable content appears playable | `selection_is_repeat_safe_and_unavailable_or_unknown_items_cannot_play` | test implemented, `NOT_RUN` |
| Manual position stays within the selected sample duration | negative/out-of-range seek or implicit restart | `seeks_are_bounded_and_ended_mock_restarts_only_after_a_gesture` | test implemented, `NOT_RUN` |
| Repeated queue/bookmark actions cannot duplicate entries | repeated input creates duplicate session records | reducer queue/bookmark tests | tests implemented, `NOT_RUN` |
| Navigation/scenario changes preserve the chosen sample | route leaf resets position or error erases library | shell-owned state; scenario/reset and composition flow tests | source-reviewed, tests `NOT_RUN` |
| UI locales have complete keys and matching parameters | missing labels, mismatched interpolation | exhaustive resource catalog and parity test | supplemental 119-key parity diagnostic passed; Rust test `NOT_RUN` |
| New shared adoption is never falsely admitted | compatibility exemption bypasses two-preview policy | unchanged import site/catalog and existing admission primitive | supplemental admission diagnostic passed; qualification remains 0/29 |

There is no new domain core: the pure reducer owns temporary presentation choices only, while
real listener contracts/playback/sync remain outside the mock. No adapter abstraction was invented
for an unimplemented API. Production publication/provider/permission decisions remain with their
existing owners.

## Source and design provenance

- [Web HTML](https://drive.google.com/file/d/1UEMLvfNdorz4P9qZgfoVkp2HwQV_Ipwt/view),
  [complete package](https://drive.google.com/file/d/1XasS1_1Qi5gQrICwp7fb1kzqBzegEGZb/view)
  and [handoff](https://drive.google.com/file/d/1d2LA4MxQmcW02gUJElz9RseBWV0sAAah/view)
  were retrieved through the authenticated Drive connector, materialized and read on this executor.
- The static board and original Sơn Hà, river/lantern and butterfly paintings were opened and
  inspected. These are reference pixels; no changed-app screenshot exists. The board itself states
  that it is not an HTML browser capture. No original embedded painting/font was committed.
- Canonical source is [tokens v0.2.0](../../packages/ui-system/shared/tokens.json), unchanged.
  The app consumes existing light/dark paper/purple roles, Vietnamese-capable family fallbacks,
  control/space/shape/type/motion roles; no new native mapping or token changes.
- [Material icon manifest](../../apps/cantos-theatre/web/assets/icons/manifest.json) records
  official source paths, revision `49d4db35df873165d6bd6ba09b063c7dafbac2f4`, source hashes,
  adapted file hashes and Apache-2.0 provenance. SVG geometry is retained from the supplied
  handoff; only the existing root paint/accessibility adaptation is used. The app adds no icon CDN.

## Execution record

Working directory: repository checkout on the assigned cloud executor. Source revision:
`8eb7f495fd07936295bda328b7392e83ed97ffa5` with this mock diff. Python 3 and Git are available.
Tool discovery found **no Cargo, rustc, rustup or Trunk** on PATH or in accessible tool locations.
The previously referenced task-local toolchain path is absent. No installation, TLS workaround,
remote environment switch or CI dispatch was attempted.

| Command / check | Actual result | Boundary |
| --- | --- | --- |
| `cargo xtask verify repository` | blocked before xtask, `cargo: command not found` | no xtask summary was produced |
| `cargo xtask verify rust` | blocked before xtask, `cargo: command not found` | native tests/format/Clippy `NOT_RUN` |
| `cargo xtask verify contracts` | blocked before xtask, `cargo: command not found` | contracts gate `NOT_RUN` |
| `cargo xtask verify web` | blocked before xtask, `cargo: command not found` | WASM Clippy and Trunk builds `NOT_RUN` |
| `cargo xtask preflight --changed-only` | blocked before xtask, `cargo: command not found` | impact selection/aggregate gates `NOT_RUN` |
| `python3 scripts/ui_preview_catalog.py --check` | diagnostic printed PASS | only existing structural registration/drift/adoption check, not renderer acceptance |
| `python3 scripts/check_repository.py` | diagnostic passed local links/JSON/skill structure | supplemental only; code changes cannot use the docs-only fallback to replace xtask |
| `git diff --check` | passed | whitespace only |
| Bilingual resource/placeholder scan, XML icon parse, CSS role scan | diagnostic passed:119 resources,16 icons,48 existing roles | bounded source checks, not Rust parsing/compile or rendered accessibility |
| Production-source transport/auth/storage/media marker scan | passed after excluding test sentinel strings | no checked live-integration markers; source review also inspected imports |
| Browser/device/media checks | `NOT_RUN` | no compiled app or runtime to inspect |
| Remote CI | not checked | no polling, dispatch or rerun |

The marker scan initially matched a negative-test `session-token` sentinel. Excluding the test
module resolved this diagnostic false positive; it was not an app integration leak.
The Rust suite adds seven reducer tests, one resource test and three title/shell/composition tests.
None ran. Test-source existence does not establish `example-tested`, `DOM-tested` or
`mock-integration-tested` evidence. The native string-render tests, once run, would still not
establish browser interaction or screen-reader evidence.

## Independent read-only review and fixes

The reviewer loaded the repository assurance, UI, Web and accessibility skills and inspected
current source plus the reference boundaries. Confirmed source-level findings were corrected:

- Skip-to-content initially changed the router hash to an unknown route. It now prevents route
  navigation and focuses the persistent main landmark through the browser helper.
- Deleting a focused bookmark/queue row could remove focus. Deletion now has a main-landmark
  fallback, including last-item removal; removing a saved card in Library does likewise.
- Library content matched the complete state signal and could remount on unrelated events. An
  equality `Memo` now tracks only the selected library category.
- Visible action labels and accessible names were reconciled; the save toggle uses a stable
  label with `aria-pressed`. Unsupported controls have stable associated explanation IDs.
- Small accent text on the light primary-container had a computed 3.886:1 contrast. Continue-card
  text now uses its paired on-primary-container role. This is a token-pair correction, not a
  rendered contrast audit. Hover opacity and hero/brand type now use canonical roles.
- Hash navigation explicitly declares the already-pinned `web-sys` 0.3.77 Window/Location/
  HtmlElement/Document features. The sole shared lockfile edit adds `web-sys` under `cantos-theatre`;
  no package/version upgrade occurs.
- Mock speed uses a comma in vi-VN and a decimal point in English; spoken English duration uses
  singular/plural resources. Regression assertions cover 1 minute/1 second and 1,25×.
- Route changes update the localized document title, and the seek value describes elapsed and
  total duration. Queue action labels stay stable while associated reasons describe current/queued
  states. Browser behavior remains NOT_RUN.
- `WorkId` and `EpisodeId` implement `Hash` for keyed lists. No renderer API/admission change.

These fixes were source-rechecked with the same diagnostic conditions. Actual focus, hash history,
keyboard input, CSS reflow, font shaping and role behavior remain browser acceptance items.

## UI report and remaining acceptance

Screen/flow: discovery/search → work/episodes → mock player; library/bookmarks, queue,
settings/scenarios/reset and account/unknown-route recovery. Consumer: Theatre Web only.
Surface intensity: discovery/library Medium; mock transport Medium-High; mini-player Medium-Low.
Renderer: Leptos CSR target source; **no browser engine/build was exercised**.
Backend/ports: none; deterministic local fixtures, no API adapter or mocked HTTP endpoint.
Locale: vi-VN/en UI resources; original sample content stays tagged vi-VN.
Accessibility: source review and associated labels/reasons only; keyboard/focus, text zoom,
forced-colors, contrast over live pixels and assistive-technology checks **not observed**.
Visual: reference board/art inspected; changed-app screenshots, narrow/wide/light/dark/200% text
captures **not observed**. Typographic covers and system font fallback are explicit departures.
Motion: token feedback transition with reduced-motion CSS; actual recording/profile **not observed**.
Input methods/async races: no real input or remote async path exercised; no claim for IME/touch,
request races or playback events. Hash Back/Forward and deletion focus need real-browser checks.
Media path: none; no audio, range requests, playback events or native adapter.

Next acceptance: provision the supported toolchain on an authorized executor, run all required
xtask gates and correct any compile/format/lint failures; then run the reproducible scenarios in
[the app guide](../../apps/cantos-theatre/web/README.md#manual-browser-scenarios-still-required).
Do not call this PR ready to merge based on static diagnostics. New shared component adoption still
needs fresh runtime/accessibility/reference qualification in both previews. Real listener API,
publication release/rights, playback, reload persistence, CDN/progress sync and native device gates
remain separate work, with no listening issue closed by this mock.
