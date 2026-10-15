# Leptos scenario workbench evidence

[Issue #142](https://github.com/loveoverflowcom/cantos/issues/142) is the owning request.
This change delivers a preview-only recipe/replay boundary over existing controls. It does
not complete every component scenario, shared admission, or product/device acceptance.
The [package contract](../../packages/ui-system/leptos/README.md#scenario-recipes-and-replay)
owns the schema and supported state matrix.

## Boundary and attribution

- Invariant: a copied recipe restores the selected component, initial state, fixture and
  environment; changing theme/mode or hiding a specimen preserves its controlled values.
  Reset restores the selected recipe. Real callbacks never wait for the observation sampler.
- Owner: package-local Leptos preview; no Studio/Theatre route, permission context, port,
  HTTP adapter, API, database, identity or audio effect. Dependencies remain inward toward
  existing `cantos-ui` production primitives. Recipes contain closed enum values only.
- Base: `develop@68b2fc6ca22f8d67b515b1b703ddd3ccf95623eb`, dirty implementation during capture.
  Browser export declares `source_sha=68b2fc6`, `source_dirty=true`; these are explicit
  build inputs, not authenticated source claims. Local artifacts retain source hashes.
- Toolchain: Rust 1.98.1; Leptos 0.7.8 CSR, Trunk 0.21.14 debug build. Browser: Codex in-app
  Chromium, user agent Chrome 155.0.0.0 on macOS; background tab, DPR 1, no added browser flags.
- Reuse: catalog IDs `button` (`Primary` / `AuthPrimary`), `filter-chip`, `switch`, `dialog`;
  existing `UiRoot`, `Select`, `TextField` and layout APIs compose the host. No shared API,
  canonical token, CMP mapping or app import was added. Six existing Leptos qualifications
  were refreshed after actual browser rechecks; their unchanged CMP records are retained. Other APIs remain
  searchable in the retained gallery; they do not yet have exact recipe specimens.
- Surface intensity: quiet diagnostic chrome with one specimen region; no product design
  direction or reference-fidelity claim. Existing component roles provide the colors,
  spacing, focus, shapes and motion. Workbench-owned geometry names its local scope.
- Localization: fixtures and their editable/blocked copy select vi-VN/en; diagnostic chrome
  is English and the existing catalog remains Vietnamese. Dialog heading/close copy is
  bilingual through its existing immutable String contract. No shared locale resource changed.

## Commands and local gates

All commands run at the repository root with `RUSTUP_TOOLCHAIN=1.98.1`.
Final broad gates use `PATH=/opt/homebrew/bin:$PATH` and `TMPDIR=/private/tmp`;
full Rust and preflight also use `RUST_TEST_THREADS=1`. Preflight prepends the existing
`/Users/manh.pd1/.gradle/wrapper/dists/gradle-8.14.3-bin/cv11ve7ro1n3o1j4so8xd9n66/gradle-8.14.3/bin`
to PATH (Gradle8.14.3/Microsoft JDK17.0.19). No wrapper/toolchain installation was run. Full output is retained
under ignored `artifacts/agent-runs/issue-142/`.

| Command | Result / evidence |
| --- | --- |
| `cargo test --locked -p cantos-ui --test gallery recipe::tests` | PASS: eight exact recipe tests; `example-tested` finite preference/state roundtrips, bounded malformed/duplicate/version refusal |
| `cargo xtask ui-preview leptos verify` | PASS: native package tests (including two new workbench render assertions), WASM Clippy with warnings denied, preview Trunk build; `example-tested`, `statically-checked`, `compiled` |
| `cargo xtask verify repository` | PASS with installed Python3.14 and canonical `TMPDIR=/private/tmp`; `repository-final.log` |
| `cargo xtask verify rust` | PASS with `RUST_TEST_THREADS=1`; format, workspace Clippy and all workspace tests; `rust-serial.log` |
| `cargo xtask verify contracts` | PASS: script-ir-reference, protobuf-reference, contract-tests, wav-reference, audio-inspection; `contracts.log` |
| `cargo xtask verify web` | PASS: Studio/Theatre WASM lint/build and preview build; `web.log` |
| `cargo xtask preflight --changed-only` | PASS: all ten selected repository/Rust/Web/preview/CMP gates; `preflight.log` |

Serve command used for the browser observations:

```sh
RUSTUP_TOOLCHAIN=1.98.1 CANTOS_PREVIEW_SOURCE_SHA=68b2fc6 \
  CANTOS_PREVIEW_SOURCE_DIRTY=true cargo xtask ui-preview leptos run
```

Only the owned Trunk process was started and stopped. No infrastructure was provisioned.
The browser tab was closed and its temporary viewport override reset. Remote CI is not checked. Both selected CMP desktop gates passed; Gradle reused some
up-to-date task outputs. This is no fresh device or native-runtime acceptance.
L1 is the documented ownership review, L2/L3 are the recipe tests and static/render checks,
and L4/L5 cover this local browser shell only. DB/API/provider layers are inapplicable.

The original environment used Xcode Python3.9 (missing tomllib/waitid) and a noncanonical
macOS temp path; the repository rerun used installed Python3.14 and `/private/tmp`. The initial
sandboxed full Rust run could not bind unchanged mock HTTP servers. With local socket access,
one unchanged xtask port-ownership assertion failed during parallel tests. Its exact isolated
run passed; probable ephemeral-port reuse is the bounded diagnosis. The full owning Rust gate
then passed with `RUST_TEST_THREADS=1`, without skipping tests or weakening assertions. Earlier
outputs remain in `repository.log`, `rust.log`, `rust-final.log` and `port-probe-diagnosis.log`.

## Browser observations

Trusted pointer/keyboard actions were driven through the browser tool. DOM reads, sampled
computed styles, opened images and real input are recorded separately. These are bounded
`DOM-tested`, `interaction-tested`, `accessibility-checked` and `screenshot-inspected` claims;
no screen reader, production service or native device was exercised.

| Scenario / oracle | Observed result |
| --- | --- |
| `auth-primary/enabled/long-vietnamese`, dark, text200, frame320, system motion, isolated | Copied canonical URL survived reload; browser1280×720 remained separate from the measured320px frame. Label28px, document root16px, button286×405.98px; Vietnamese label wraps without fixed-height clipping |
| Same recipe, quick pointer activation then Space held160ms | Exactly one callback per activation; callback visible while release profile still had no end sample. Real border radius changed from143px through intermediate values, reversed at key release, and returned to143px with stable bounds |
| Same control, `motion=reduced` | Computed transition0s; pressed radius16px and released143px immediately. State/callbacks preserved. Trace explicitly reports an unavailable midpoint for instant changes; OS preference itself remained false |
| `filter-chip/unselected`, change theme and isolated→matrix | Selected value survived; matrix showed four separate production state specimens. Switching away and back also preserved it. Reset restored unselected and cleared callback/frame history |
| Recipe/environment changes during/after frame observation | Old profile cancelled and cleared; metadata cannot attribute previous samples to the new recipe |
| `dialog/closed`, long Vietnamese, light→dark | Real opener focused; Escape closed and restored focus to opener. Text entered before close survived theme change and reopen. Tab from close reached the input; exhaustive focus containment was not assessed |
| Actual390×844, frame390 then320, light, text100 | Measured frame390 intentionally exceeded the padded host and scrolled locally; frame320 fit. Document did not acquire a global horizontal overflow. Keyboard-focusable scroll region remains the width-simulation boundary |
| Actual1440×900, frame840, en, text100 | English fixture rendered through the same production button; measured840px frame. Separate dialog run measured1440px frame |
| Existing four-face font diagnostic | Noto Sans400/700 and Noto Serif400/700 all reported loaded through the actual WASM FontFaceSet diagnostic before final captures; locally bundled, versioned bytes remain unchanged |
| `v=999` | Visible UnsupportedVersion fallback, usable default recipe, no crash or echoed unknown value; no services constructed |
| Export metadata | Real browser download read successfully; recipe, source/token/catalog identifiers, viewport/DPR, browser/version, visibility, font readiness, capture label, callback and sampled frame history present |

The motion probe only observes trusted input and computed styles. It never dispatches input,
forces a pressed class, delays intent, creates a media state or drives animation. Samples are
bounded by600ms/24 lines; a new input, hidden/offscreen target, recipe/reset change or unmount
cancels the owned request and listeners. The profile is not a video or performance benchmark.
No custom gesture consumer exists here: native activation/keyboard/cancellation policy remains
browser-owned. No invented slop, axis, velocity or snap constants were added.

## Opened artifacts and same-scenario recheck

The agent opened these JPEG captures; files remain local and ignored, not PR attachments:

- `artifacts/agent-runs/issue-142/dark-320-text200.jpg`:1280×720 browser,320 frame, vi-VN,
  dark,200% specimen text; an intermediate/reversing auth shape with visible focus.
- `artifacts/agent-runs/issue-142/dark-320-settled.jpg`: same recipe and viewport, final
  capsule shape, complete wrapped label and stable focus outline.
- `artifacts/agent-runs/issue-142/light-390.jpg`:390×844 browser,390 frame, vi-VN,100%; local
  horizontal scrolling is intentional because frame width is distinct from padded host space.
- `artifacts/agent-runs/issue-142/light-390-frame320.jpg`: same browser with320 frame, vi-VN,
  light,100%; complete label fits and wrap remains legible.
- `artifacts/agent-runs/issue-142/light-1440-frame840-en.jpg`:1440×900 browser,840 frame, en,
  light,100%; wide production button and intent log inspected.

Additional local artifacts: `normal-motion.txt`, `reduced-motion.txt`, `exported-metadata.txt`,
`dialog-result.json`, `source-manifest.json` and the named gate logs. No design PNG was used
as a browser pixel oracle; #140's comparator was outside this change.

Findings rechecked on the same scenarios: global legacy200% scaling no longer leaks from the
hidden gallery; incompatible states are absent from each component selector; scroll-frame
focus consumes the existing focus-width role; dialog environment changes preserve drafts;
old profiles clear at recipe/reset boundaries. No new token or color value was introduced.

## Existing admission compatibility recheck

The renderer digest includes preview sources, so the new workbench invalidated the six
previously admitted Leptos records. New `issue142-*-leptos.json` records preserve the existing
admitted set; original issue141 records remain historical. CMP source and its source digest
are unchanged. Generated CMP catalog output changes its provenance comment only.

The reviewer opened the previous issue141 Button reference and actual Button/TextField/Surface/
Badge/BackButton captures, then drove the same existing fixtures in this preview. Button Enter
sent one intent; loading/disabled stayed blocked; Tab exposed focus. TextField retained entered
text through readonly, error, dark theme and200% scoped text, with error association intact;
loading disabled it. Back Enter sent one callback and blocked states remained inert. Surface
reading/auth/profile/member groups retained paired ink and quiet hierarchy. Five Badge tones
were inspected at390×844 in light/dark; actual color pairs retain contrast12.65/10.69/5.92/5.64/
5.54 light and10.79/7.69/7.20/7.59/7.57 dark. Passive components have no interaction to drive.

All nine actual Logo variant/tone specimens loaded with the original vector aspect ratios;
decorative images remained hidden and exactly one named symbol was exposed. The historical
`logos-wide-dark.jpg` reference did not actually show Logos, so it was not reused as Logo pixel
evidence. Current captures expose the real artwork and its original SVG/anatomy contract.
The recheck found Light artwork in the dark host's named/decorative/aspect-fit specimens; these
preview-only fixtures now have an explicit Light surface and were reopened after correction.

Opened current captures, all under `artifacts/agent-runs/issue-142/`:
`qualification-buttons.jpg` (1280×900 light), `qualification-field.jpg` (1280×900 dark200%),
`qualification-field-narrow.jpg` (390×844 dark200%), `qualification-back.jpg` (390×844 light),
`qualification-badges-light.jpg` / `qualification-badges-dark.jpg` (390×844),
`qualification-surfaces.jpg` and `qualification-logos-top.jpg` / `qualification-logos-bottom.jpg`
(1280×900 dark host, explicitly paired nested Logo themes). `final-qualification-checks.json`
retains the final-build callback/input/named-symbol recheck. The scoped gallery scaling keeps
font roles doubled while the document root stays16px, including after hiding the gallery.

These are bounded compatibility/accessibility/visual reviews, not exhaustive screen-reader,
OS IME, all-state animation or pixel-fidelity certification. No new application import was
admitted and none of the future proposed components became qualified.

## Deferred acceptance

- Exact recipe specimens beyond the five implemented component/variant kinds; the rest of
  the catalog remains available through its existing family/state samples.
- Pointer drag-out/release, real pointercancel, offscreen/background cancellation and long
  repeated mount/cleanup stress. Cleanup has source review and compiled evidence, not a
  complete browser leak measurement. No new scripted animator was introduced.
- Browser OS-reduced preference emulation and explicit force-full override under that setting;
  the available browser control exposes viewport sizing but no media emulation. The selected
  reduced recipe was observed; CSS system preference remains intact.
- Controlled-clock immediate-dispatch assertion, video recording, exhaustive dialog focus tour,
  named OS IME and screen-reader walkthroughs, Firefox/WebKit, CMP and Android/iOS devices.
- Future sheet and mini↔full player gesture consumers need their own production contract and
  native evidence. Preview fixtures do not implement product playback or imply playing state.

Next owner is unassigned; acceptance requires replaying the selected recipe with the required
browser/input/device tools via `cargo xtask ui-preview leptos run` and recording the missing
oracle. The admitted set remains unchanged; six Leptos records have fresh browser evidence. These gaps are not waived by a passed build
or by opening a PR.
