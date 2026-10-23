# 23 October controls delta — issue 402

Status: scoped implementation and local renderer evidence. Full product, browser-engine,
assistive-technology and Android/iOS acceptance remain separate. Related owners:
[#221](https://github.com/loveoverflowcom/cantos/issues/221),
[#228](https://github.com/loveoverflowcom/cantos/issues/228),
[#229](https://github.com/loveoverflowcom/cantos/issues/229),
[#224](https://github.com/loveoverflowcom/cantos/issues/224) and
[#282](https://github.com/loveoverflowcom/cantos/issues/282) remain open.

## Boundary and provenance

Base: `origin/refactor/cantos-native-turso` at
`c219894dfe873455b7379622d35ea776b595675e`; PR 400 was OPEN. Commands run in the
isolated `issue-402-ui-parity/cantos` worktree, on that base plus the PR diff.
No ADR is needed: this applies the owner's explicit 23 October design decision.
No domain, auth authority, session, persistence, API, playback or provider behavior changes.

Reviewer: Codex root, 23 October 2026, macOS arm64. Rust 1.98.1, Gradle 8.14.3,
Microsoft OpenJDK 17.0.19. Web uses the Codex in-app browser; engine version is unknown.
Observed viewport/DPR: 1280×720/1, 1280×900/1, 390×844/1 and 320×844/1. Scoped text scale is
100%/200%, not browser page zoom. Fonts reported loaded. CMP uses the JVM Compose
test renderer, not an Android/iOS device. Kotlin has no configured formatter/linter;
handwritten Kotlin received manual style review and compiler/test checks.

Downloaded references are outside Git. Direct Drive IDs retain their identity across revisions:

| Reference | SHA-256 of fetched bytes |
| --- | --- |
| [Component HTML](https://drive.google.com/file/d/1auZtCuYeIlYAL1CWiL4zYO80MWSmfRM-/view), modified 23 October 07:31 UTC, 5,777,988 bytes | `3e45dfa1f41dc4c5f8459083a8f3c1a1bcb49236c4aab896fb2c5b319d72c3fa` |
| [Complete ZIP](https://drive.google.com/file/d/1GWM-cA5AZEEEx_wv8UMPsgEQmIRwdSVU/view) | `5a14eda0991d7f2baf745cf63a92c1c76b32f70f2eca638f53074d524b869b01` |
| [Manifest](https://drive.google.com/file/d/1MO6du9vxTM0yfPxn38HVl7u-OgAXCb81/view) | `fd222dd7d74dad5ef565f676efe6a3e3e3fb45c973fa47618140e27e99b6921e` |
| [Handoff](https://drive.google.com/file/d/17_s9ix7Qtzj76-RwLbCQLA42qob8g3gq/view) | `5eb6b85835be330ce00b862c29048b80024b1e0daf11a98fd7183399ff42c51a` |
| [Updated overview PNG](https://drive.google.com/file/d/18_bDVmwEL8RpslKW1WWqsc7H1S_2BCZd/view), opened | `f8006e5b317f15019eedd915a959d4c8cd3384f1ecce56af5ff3c58ff6a53feb` |
| Theatre Web auth HTML | `8eb01706b839ced02569997d5c938064416dca39e8ea29d97c4754fd6c446b5d` |
| Theatre Mobile auth HTML | `12b0269e92976abd8b98cc46de6f57d55266a08d38cda8aba9f785be1e9df254` |
| Studio auth HTML | `5f4279f18d5880831ce7e1ba7966205b90c91a3bd4f877b70efcbcea8eb9a293` |
| Theatre Web catalog HTML | `2d8bf11da4692296c452db04c8e0f3cb9d0bded5500b2443c58f1de232e02a98` |
| Theatre Mobile catalog HTML | `2e797d65516571438e1392fd9834a638e682b5ef01994a489558d8ad9ce2e022` |
| Studio catalog HTML | `966ba5ec3bebc1dcbbca3006717c075c843d4aae6b2564b081c90568ad09c6c4` |

The browser refused the downloaded design's `file:` URL. No workaround executed that HTML.
Source was read and the exported overview plus ZIP `button-light.png` and `select-light.png`
were opened as the safer reference. These support anatomy review, not matched-pixel fidelity
or a fresh interactive design-HTML PASS. Historical token proposals and prototype DOM/icon
inference were not imported. Canonical palette/fonts/units remain authoritative.

## Inventory and regression oracles

| Design / component | Observed baseline and change | Oracle / consumer boundary |
| --- | --- | --- |
| Button / AuthPrimary | Web wrapped the icon above the label at frame390; label flex filled the remaining row. Both renderers painted a primary icon circle. Now only text wraps; icon plus intrinsic label are centered, with no bubble in enabled/blocked/loading states. | Web bounding boxes and opened before/after; CMP AuthAccountComponentTest, AuthButtonReplayTest and opened captures. |
| Button semantic intent | Registry already contained 34 glyphs; Studio adapters already forwarded optional icons. Added only official `content_copy` for the concrete private receipt Copy action, from the same pinned Google revision. | Existing closed-registry and decorative-icon tests; caller mappings use typed intent or explicit IconName, never translated labels. |
| Select leading/value/disclosure/check | Existing Popover and Material menus already owned keyboard, stable values and checks. Added optional semantic option icons, aligned reserved leading slots and independent trailing check; menu uses the surface role. CMP uses tonal trigger, native popup and paired selected surface. | New Web exact regression plus existing model tests; icon-enabled SelectAffordanceTest exercises native keyboard, focus return, zero value, loading interruption and single dispatch. |
| Native fallback | Native option children remain supported; glyphs are decorative enhancement only. | SSR form/value/required/help/error tests. Unsupported browsers retain OS select behavior; no claim of identical native popup pixels. |
| Compatibility admissions | Package digest changes invalidate all seven existing records. The native branch also had Status/Dialog/Checkbox callers without records. | Fresh bounded Button/TextField/Surface/Badge/BackButton/Logo/Select recheck plus Icon/Status/Dialog/Checkbox admission; historical PR304 files preserved. Unified Back and AuthTonalSpacious candidates remain outside this admission. |

## Executed preview checks and opened captures

- `RUSTUP_TOOLCHAIN=1.98.1 cargo xtask ui-preview leptos verify`: PASS (SSR tests,
  WASM Clippy with warnings denied, Trunk gallery build). The new icon regression initially
  failed because it assumed HTML attribute order; matching the class independently corrected
  the test oracle. Final package checks passed without weakening semantic assertions.
- `RUSTUP_TOOLCHAIN=1.98.1 cargo xtask ui-preview leptos run`: production shared components
  served on `127.0.0.1:8083`. Real pointer/Enter/Arrow/End/Escape/Tab input, not synthetic callbacks.
- `CANTOS_UI_PREVIEW_CAPTURE_DIR=/private/tmp/cantos-402-after-cmp RUSTUP_TOOLCHAIN=1.98.1 cargo xtask ui-preview cmp verify`:
  PASS, 216 tests in 34 suites, zero failures/errors/skips. Baseline was also run and captured
  before edits. Test results were freshly executed, not inferred from UP-TO-DATE output.
  Adding the 35th official glyph first failed the independent exact registry inventory;
  the reviewed `content_copy` entry was then added to that inventory. No original glyph was removed.

Web recipe: `v=1`, `component=auth-primary|select`, `state=enabled|loading`,
`fixture=standard|long-vietnamese`, `theme=light|dark`, `density=comfortable`, `locale=vi`,
`text=100|200`, `frame=390|320`, `motion=reduced`, `mode=isolated`, `playback=live`.
Selections were made through the workbench controls. The long dark Button at frame320/text200
had a 24px icon, centered group boxes (delta 0), contained label and no page overflow at viewport390.
Enter produced one intent. Loading exposed `aria-busy=true`, native disabled, one progress ring
and zero action glyphs. Long text grows the capsule intrinsically; it is not clipped to CTA height.

For Select at the same large-text recipe: open popup bounds `(8,735.39,359,100.61)` were inside
the 390×844 viewport and scrollable. Four leading/check slots remained separate. Arrow then
Escape produced zero intents; End then Enter selected `dark` exactly once and retained focus.
The native compact language baseline was also present; no enhancement is claimed for its OS popup.

Opened Web captures in `/private/tmp/cantos-402-web/`: `auth-before.jpg`, `auth-after.jpg`,
`select-before.jpg`, `auth-dark-long-200.jpg`, `select-dark-long-200.jpg`, `fields-dark.jpg`,
`feedback-dark.jpg`, `surfaces-dark.jpg`, `back-dark.jpg`, `icons-logos-light.jpg` and
`icons-logos-dark.jpg`. The compatibility check retained edited Vietnamese field text across
theme change. Baseline Back Enter emitted one intent and Tab left the control. All 34 Web SVGs
in the icon specimen had decorative/nonfocusable semantics. Badge text/status cues, paired
surface foregrounds and named/decorative logos were inspected in the gallery.

Opened JVM captures in `/private/tmp/cantos-402-after-cmp/`: auth account light/dark390,
auth primary dark390 pressed, badges light/dark390/text2, field light390/focus/error,
logo symbol light/dark and foundation icon decorative light/dark372. Also opened
`cmp-select-229-light-text1.0-320-open.png` and `cmp-select-229-dark-text2.0-390-open.png`;
icon/label/chevron/check remain separate, long Vietnamese wraps and the selected surface is paired.
Corresponding baseline AuthPrimary and Select captures were opened before repair.

The added `auth_support_controls_keep_status_selection_and_modal_text_in_both_themes`
captures the production Checkbox/Status/Dialog at 390×840 in both themes; it asserts selected
checkbox state, polite live-region semantics, displayed modal text, close and retained selection.
Opened `cmp-ui402-status-checkbox-{light,dark}.png` and `cmp-ui402-dialog-{light,dark}.png`.
The Web modal was edited, dismissed with Escape and reopened: text survived and focus returned
to `gallery-dialog-open`; the opened light modal and existing status/checkbox specimens were inspected.

The existing CMP tests independently assert localized roles/names, disabled/reason behavior,
field value preservation, surface pairs, badge wrapping, Logo transparency/geometry and
immediate/cancelled/repeated action dispatch. An opened still does not prove motion or a screen reader.
Fresh qualification JSONs bind source/token digests; gallery evidence is host presentation only.

`RUSTUP_TOOLCHAIN=1.98.1 cargo xtask theatre-preview verify`: PASS. The Theatre shared
suite reports 490 tests (three skipped), and the sandbox reports 124 (two skipped), with
zero failures/errors. Opened the dark Login and guest Account captures at390, plus the
light registration and account-settings text200 captures at320. The latter require scrolling;
they do not establish a fully visible small-screen form. Also opened the sandbox Discover
capture at390. These compositions use the existing in-memory/fake ports, not live Identity,
an Android/iOS renderer or native audio. Captures are in `cantos-402-product-cmp` and
`cantos-402-native-cmp` under `/private/tmp`.

## Bounded product rollout

| Actual caller | Mapping / adaptation | Evidence or remaining boundary |
| --- | --- | --- |
| Theatre Web LoginForm, AuthActions, SignupPanel, SessionRecovery | Login, PersonAdd, Mail and Refresh are unconditional semantic mappings. Removed the icon-preview property; navigation/cleanup and request barriers are unchanged. | Existing phase/route/submit tests run on default callers. Actual default `#login?from=account` opened at390/320: Login/Create-account/Retry icons visible, API-unavailable barrier preserved. At320, Login group center delta0, height60, icon24, no page overflow. |
| Theatre Web account and locale | Account theme now uses the existing enhanced Select with Settings/LightMode/DarkMode; removed its app-local select styling. Locale retains autonyms and no redundant per-language glyph. | Actual default account composition was opened at desktop and viewport390, API unavailable; retry glyph is visible, theme menu selects light and Escape dismisses. Popup at390 was inside the viewport: x40/y618.58/w295/h162. |
| Theatre Web Home / Work | Entry Login/PersonAdd, recovery Refresh and clear Close reuse shared Button. Existing search/play/media icons are retained. | Existing product tests and default Web build; no mock catalog fallback. |
| Studio SignInPanel / FormSubmit | Login decorative slot; submitting replaces it with progress and localized busy reason while retaining `type=submit`. | Native form/credential hints plus added busy/name tests. Actual Studio render is NOT_RUN: no API on8080 and another process owns8081; xtask refused before starting anything. |
| Studio workspace / delegation | Save, retry, logout, discard, revoke and Copy map explicitly to registry names. Theme selector consumes the same enhanced Select. | Existing product tests; private receipt copy/revoke and signed-in screens still need real browser observations. |
| Theatre CMP auth/account | Existing product root already opted into candidate icons at this newer base. Removed the obsolete preview property so direct/default screen callers use the same admitted AuthIntent mapping. LoginForm's existing Login glyph stays. | Theatre shared/sandbox JVM suites and captures; device and live auth remain separate. |
| Studio delegation lifetime | Kept native numeric select within the existing disabled fieldset and validation/focus contract. No semantic icon is needed for a number of days. | Intentional bounded native fallback, not a new Select implementation or a claim that the OS popup matches the custom menu. A private-form menu migration needs its own browser check. |

Other dynamic Library/player actions and proposed design specimens remain consumer work;
they were not mechanically migrated here. Numeric choices, avatar initials and media artwork
retain their semantic opt-out. No labels are parsed to infer an icon. No app-local padding patch,
global token, UI dependency, provider call or preview-mode production fallback was introduced.

## Remaining acceptance

Exact pixel fidelity awaits a permitted matching reference render and recipe. Browser page zoom,
forced colors, the full engine matrix, OS IME, TalkBack/VoiceOver and Android/iOS device behavior
are NOT_RUN. Run the same recipes on those targets under the UI owners before claiming their
acceptance. Production API/auth, background playback, deployed behavior and publication are
outside this presentation change. No design export or JVM result establishes them.

Raw command logs, screenshots and metadata stay ignored/local; they are not uploaded PR images.
Product Web captures include `product-account-menu-dark.jpg`, `product-account-light-390.jpg`,
`product-login-390.jpg` and `product-login-320.jpg`. No backend response was replaced or mocked.

## Handoff gate boundary

The initial preflight selected Apple Python3.9 and stopped on the repository's Python3.11+
syntax. Using the already installed Homebrew Python3.14.8 resolves that environment error.
The subsequent repository check exposed three changed-line localization findings; the fixed
Vietnamese sample names now use the existing SyntheticPreviewText owner. The existing
Select label/value accessible-name projection retains localized caller values and its colon
separator; no translated words, catalog or locale policy were added.

Repository gate remains FAIL because the unchanged native ownership checker reads
`services/cantos-server/src/platform/postgres/migrations.rs`, which is absent at the exact
base SHA. The focused checker was reproduced from base-owned script/schema files in a disposable
directory, with the same missing-file failure. This UI PR does not repair native migration
inventory; #333's owning boundary is the follow-up. Remaining gates run through the same
changed-only preflight with `--skip repository`; that skip is NOT_RUN, never a repository PASS.
The PR contains the actual final summary and retains this failure separately.

The changed-only preflight passed Rust format/lint/tests, both Web lint/builds and the
Leptos preview. Its shared CMP step failed on a missing SyntheticPreviewText import after
moving synthetic samples to their existing owner. Adding that import was the only later
renderer source change. The final focused shared CMP check passed all216 tests, and the
separate Theatre CMP check passed as recorded above; they supersede the failed/unstarted
CMP steps. The Rust/Web inputs remain unchanged, so their results are reused. The aggregate
preflight remains a failed run; no clean all-gates PASS is claimed.

The independent Python repository suite (`python3 -m unittest discover -s scripts -p
'test_*.py'`, Homebrew Python, `TMPDIR=/private/tmp`) is FAIL: 1,038 tests, seven failures,
four errors, 15 skips. All 11 affected tests and their implementation inputs are unchanged.
Running those exact cases from a disposable archive of the pinned base reproduced six
failures and four errors: missing native migration inventory, OCI Linux-host/fixture-Git
constraints, the stale native launcher callback assertion, the Identity dependency assertion,
BSD sed behavior and missing flock. The remaining Compose process-cleanup timing test passed
both on the base and on a focused current-branch rerun; its initial failure is flaky evidence,
not erased from the campaign. No unrelated source or test was changed. Logs are
`repository-python-tests.log`, `baseline-python-failures.log` and `compose-cleanup-recheck.log`.

The initial Rust test run also found an account-copy oracle matching `03` inside the new
official SVG path coordinates. Actor IDs, entitlement copy and forbidden component variants
still check the complete markup; only invented numeric counts now check rendered text nodes.
No product output was changed to satisfy that test. The integration remote later advanced
with backend-only work, with no UI/toolchain overlap; final verification pins the original
base SHA above instead of relying on a mutable remote ref.

Before removing the worktree, raw references, logs and opened captures are retained outside
Git in the primary checkout's ignored
`artifacts/agent-runs/issue-402-ui-parity-20261023/` directory. This is local evidence, not a
portable PR attachment or a published design asset.
