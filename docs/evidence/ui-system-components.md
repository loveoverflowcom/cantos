# Leptos component library — session 2

Date: 2026-10-11. Local-only continuation on `feat/leptos-ui-system` from foundation
`cecad0472e4f8cef18160c542acbfa7ac4844536`, whose develop base is
`dc7c2de311c400a7581347a80b33d9b8b34c25a4`. Working directory:
`/Users/manhblue/projects/loveoverflow/cantos`. No reset, rebase, cherry-pick, push, PR or merge.
The final local commit SHA is supplied in the task checkpoint; verify `git rev-parse HEAD`
before starting session 3. This document records the component boundary, not app acceptance.

## Implemented boundary and API ownership

The [Leptos library](../../packages/ui-system/leptos/README.md) adds native controls, layout,
feedback, ordinary link navigation and a controlled native dialog to the foundation. Its
[interactive gallery](../../packages/ui-system/leptos/preview/main.rs) shows long Vietnamese
content, theme/density, action/field/error states, numeric zero, callback counters, IME diagnostics
and dialog input retention. The selected [sequence](../work-plan/015-leptos-ui-system.md) leaves
app migration to session 3 and independent final review to session 4. Internal parallel source
work and precommit review in this session do not start those later sessions.

| File owner | Public API / next consumer |
| --- | --- |
| `leptos/src/button.rs` | Backward-compatible Button props plus Secondary/Danger/Loading; IconButton, AccessibleName, DisabledReason; guarded action dispatch |
| `leptos/src/navigation.rs` | LinkHref, Link, NavItem, NavTabs and controlled FilterChip; apps own destinations/current state |
| `leptos/src/fields.rs` | TextField/TextArea/NumberField, Select, Checkbox/Radio/Switch/RangeField; FieldState/ChoiceState, native label/help/error and composition hooks |
| `leptos/src/layout.rs` | Surface/Stack/Grid/Container, typed tone/spacing/direction/maximum column count/measure |
| `leptos/src/feedback.rs` | Badge/Status/LoadingState/EmptyState/ErrorState; text/icon/tone and opt-in Announcement |
| `leptos/src/dialog.rs` | Dialog, DialogCloseReason, DialogError; native browser interop in private platform module, optional initial focus and actions slot |
| `leptos/foundation.css.in` | Component and diagnostic-gallery styles; regenerate CSS with `scripts/ui_tokens.py` |
| `leptos/preview/main.rs`, `preview.html`, `leptos/tests/` | Diagnostic CSR composition and native HTML-string/type tests; no production route |
| `scripts/test_ui_component_states.py` | Independent opaque-surface navigation hover contrast oracle over actual CSS role and canonical values |

All paths in the table are relative to `packages/ui-system` except the explicit scripts path.
Canonical v0.2.0 tokens, eight official outlined icon originals, licensing and generated token/icon
APIs remain unchanged. No new dependency, crate, artwork/font binary, domain/schema/backend/HTTP
API, CMP/native implementation, model, TTS or playback change is made. Native SSR is used only
for string-render tests; this is not an SSR application deployment.

## Invariant and evidence ledger

| Claim / invariant | Failure mode | Owner / cheapest oracle | Evidence level |
| --- | --- | --- | --- |
| Disabled/loading actions retain text/reason and refuse activation; chips dispatch requested state | accidental repeat or hidden blocker | ButtonState and actual callback guard; type/HTML tests | type-enforced, example-tested |
| Native controls have stable labels and only visible help/error/reason associations | unnamed control or dangling descriptions | fields + native string-render fixtures | example-tested |
| Numeric strings preserve valid zero and empty editing input without parsing/defaults | intensity 0 becomes another value | Number/Range raw contract and exact fixtures | example-tested |
| Text/native textarea serialization cannot escape into markup | generated HTML admits a closing tag/script | original failing TextArea regression, narrow encoding shell | example-tested |
| Composition hooks expose native lifecycle while raw input stays unchanged | app cannot defer IME validation/autosave | typed callback props, compiled gallery diagnostic | compiled; OS IME NOT_RUN |
| Navigation is native links/current state, icon names nonblank, href schemes bounded | fake keyboard widget, unnamed control or active-document destination | closed/refined types and string-render refusal tests | type-enforced, example-tested |
| Status uses visible text/icons and opt-in announcements; loading announcement is not permanently busy | color-only state or deferred announcement | HTML anatomy tests and WAI-ARIA semantics review | example-tested; assistive technology NOT_RUN |
| Grid has explicit width under a horizontal Stack; styles retain focus and shared roles | collapsed intrinsic grid or clipped focus outline | source review, template generation; gallery fixture | documented/source-reviewed; pixels NOT_RUN |
| Nav hover retains normal-text contrast on all shared opaque surfaces | tonal composition drops below 4.5:1 | independent anchored sRGB/compositing regression | example-tested |
| Dialog renders native closed anatomy, safe autofocus, controlled close/error contract | faux modal or silent native error | native anatomy tests and WASM compile | example-tested, compiled; focus runtime NOT_RUN |
| Gallery uses all families with unique reserved IDs and authentic conditional-state fixtures | examples omit a family or collide labels | production Gallery inclusion in native tests | example-tested |
| Studio still consumes the unchanged shared foundation API | broken library/app boundary | focused WASM lint and Studio Trunk build | compiled, statically-checked |

No durable application decision needed extraction: presentation mappings/state guards form the
small pure core; native DOM/focus and Leptos event dispatch form the shell. Apps still own all
validation, permission and asynchronous outcomes.

## Source provenance and review corrections

The already materialized reference directory `/private/tmp/cantos-ui-reference` was inspected
read-only; no repeat download was needed. The shared component manifest, Web control styles and
Studio component spec informed the inventory, tonal containment, visible native labels, button
variants, 44px Web contract and native dialog anatomy. Recomputed hashes match foundation:

| Artifact | SHA256 |
| --- | --- |
| Shared component manifest | `0b1a3a01b67814db55c19b64eca3abb687aa18bebc3bddd8d53b282411bc1cf5` |
| Theatre Web HTML | `5fdd41bd99460fdad12d3092277568a2b624c0e2139790a96e66f90067d37fd4` |
| Studio Web HTML | `de39cc8d0a032a1a2abd9d021b4703ec7af288c0124abab585c41533bf5b1ae6` |

No private Drive IDs/URLs, raw HTML, embedded artwork/fonts or media enter Git. Selected palette,
font family/fallback, density and official icon policy follow the source. Static source inspection
is not rendered-reference fidelity.

The source review applied `cantos-code-review` against the exact foundation base, through the
engineering/UI/Leptos and personal UI design criteria. It remained read-only; evidence-backed
repairs were performed in the implementation lane:

- The original native TextArea escaping test failed with a literal closing textarea and script.
  Pinned tachys marks textarea children unescaped for HTML-string serialization. A narrow native
  initial-text encoder escapes ampersands/angle brackets and preserves initial LF; wasm retains
  the raw string/text node and controlled value property. The unchanged injection regression,
  existing entity and leading-newline tests now pass. No raw HTML injection API was added.
- LoadingState no longer marks its live-region owner permanently busy. WAI-ARIA allows content
  updates to be ignored while [aria-busy](https://www.w3.org/TR/wai-aria-1.2/#aria-busy) remains true.
  The regression asserts an opted-in status with no indefinite busy flag; no VoiceOver claim follows.
- Navigation hover on tonal surfaces failed an independent contrast regression: light Container
  4.4167:1 and High 4.1004:1. Normal nav text now uses `color.onSurface`; selected text keeps its
  matched primary-container pair. Both themes/all four opaque backgrounds pass the same oracle.
- Grid's size-containment wrapper now has explicit full-row width/flex basis inside horizontal
  stacks. Forced-color current navigation uses underline thickness without overriding focus.
  Dialog scroll padding reserves focus-width + focus-offset instead of clipping a full-width ring.
  These are source corrections; their runtime appearance remains unobserved.

The final read-only source review concluded `no-actionable-findings`,
`complete-for-declared-scope` before the local checkpoint. It inspected implementation, fixtures,
CSS and handoff; it did not independently rerun builds/tests or observe browser behavior.

## Executed checks

Python 3.13.5, Rust/Cargo 1.87.0, Trunk 0.21.14; Apple Silicon/macOS 15.6.
Build jobs limited to 2, one heavy lane at a time, existing target reuse. Initial free disk 2.5GiB;
final available disk is approximately 2.1GiB (checked after both builds). No cache/model/audio/database deletion,
Docker/SDK/toolchain install, CI polling or shutdown occurred. Command output is in this task's
attributable tool transcript. Commands default to repository root unless noted.

| Command | Result / exact boundary |
| --- | --- |
| `python3 scripts/ui_tokens.py --check` | PASS: canonical mappings/template match |
| `python3 scripts/ui_icons.py --check` | PASS: eight official SVG hashes/anatomy |
| `CARGO_BUILD_JOBS=2 cargo test -p cantos-ui --locked` | PASS: 35 native tests; zero doctests are not test evidence |
| `CARGO_BUILD_JOBS=2 cargo clippy -p cantos-ui --all-targets --locked -- -D warnings` | PASS: native library/tests/gallery source |
| `CARGO_BUILD_JOBS=2 cargo clippy -p cantos-ui --all-targets --target wasm32-unknown-unknown --features preview --locked -- -D warnings` | PASS: exact CSR gallery/shared target |
| `CARGO_BUILD_JOBS=2 cargo clippy -p cantos-ui -p cantos-studio --target wasm32-unknown-unknown --features cantos-ui/csr --locked -- -D warnings` | PASS: shared/Studio Web |
| Gallery `CARGO_BUILD_JOBS=2 NO_COLOR=true trunk build preview.html --locked --dist ../../../target/ui-foundation-preview` | PASS; cwd `packages/ui-system/leptos`, debug CSR |
| Studio `CARGO_BUILD_JOBS=2 NO_COLOR=true trunk build --locked` | PASS; cwd `apps/cantos-studio/web`, debug CSR |
| `TMPDIR=/private/tmp python3 -m unittest discover -s scripts -p 'test_*.py'` | PASS: 127 tests |
| `python3 scripts/check_repository.py` | PASS: 287 text files, 15 skills |
| `cargo fmt --all -- --check` / `git diff --check` | PASS after final code edits |

Ignored build artifacts were measured after the successful builds:

| Artifact | Bytes | SHA256 |
| --- | ---: | --- |
| Gallery `target/ui-foundation-preview/ui-foundation-preview-6608a87be5dc4863_bg.wasm` | 5,487,336 | `e01d16aa207f2477a8fac739d2fc974b7ded99c1accc308b290675cb50da68bb` |
| Studio `apps/cantos-studio/web/dist/cantos-studio-d4194b5638e47e50_bg.wasm` | 32,202,307 | `41e0b120e0fa855e0d620815640ae396c62caf48f7e5d1a0ff3aae4fdea4e80d` |

Initial failures also remain attributable: dialog used unsupported `View.on` (fixed with
renderer-owned AddAnyAttr); field fixtures used unsupported closure-to-Callback conversion
(fixed with explicit Callback); original TextArea injection assertion failed and was fixed;
the first gallery family test incorrectly required conditional loading in the default empty
fixture (replaced by authentic initial-state fixtures); WASM Clippy flagged nonminimal bool
(fixed with the same positive early-return guard); nav hover contrast failed before the fix.
One Trunk retry was accidentally invoked from root and failed root-package discovery; the
correct package-directory command passed. No failed command is reported as passing.

## Runtime gate and handoff

Computer Use could read existing Safari AX/pixels, but its first native input attempt to open
this built gallery reported: the Mac is locked and automatic unlock could not unlock it. No
bypass was attempted. The pre-existing Theatre screenshot is not evidence for this component
change. Safari 18.6/WebKit on macOS 15.6 was available for read-only discovery; gallery viewport,
DOM, focus and interaction were not observed.

A task-owned loopback-only Python server was attempted under the default sandbox (binding
blocked), then the same safe loopback serve was allowed by automatic escalation review. This
establishes a host server, not a browser run. Build outputs stay ignored under `target/` and Studio
`dist`; opening HTML/building WASM does not establish accessibility. The task-owned server was
stopped after the blocked browser attempt; no background serve is left running.

Session 3 must first scope existing broad app selectors, then consume the documented APIs. Keep
numeric values raw through parsing, preserve zero, and gate IME validation/autosave through the
composition callbacks. Bind native option `selected` for SSR or late option insertion without a
value change; Select has no hidden option MutationObserver. NavTabs is ordinary links, so an
in-page tab widget would be a separate scoped implementation. Native choice inputs do not
support readonly. Dialog close requests require the caller to set open false, and errors must
be surfaced/reconciled. Removed initiators require app-owned navigation focus.

Required remaining gates: 320/600/840/1440 CSS-px viewports, both themes, 200% text scaling,
Vietnamese wrapping/diacritics, input/selection counters and numeric 0/empty behavior in browser,
Select non-first mount and late-option scenario, native keyboard control activation, dialog
Tab/Shift+Tab containment, safe focus, Escape/explicit close and return focus, rapid reopen,
unmount/custom-focus errors, focus-ring clipping, forced colors and reduced-motion observation,
OS Vietnamese IME, screen-reader walkthrough. App unsaved/conflict/permission flow preservation
belongs to session 3; independent final review belongs to session 4.

## Completion report

- Work item / review boundary: [015](../work-plan/015-leptos-ui-system.md), session 2 component
  library and diagnostic gallery. Prerequisite: requester supplied completed local foundation.
- Invariant / owner: presentation remains composable, native and controlled; the package owns
  anatomy/event dispatch, apps own durable facts and effects.
- Evidence level: type-enforced, example-tested, statically-checked, compiled. Exact checks above;
  no DOM-tested, interaction-tested or accessibility-checked claim.
- Provenance / edge classes: foundation SHA + selected source hashes; actual native production
  components/gallery; escaped text/attributes, disabled/loading/reasons, safe href refusal,
  label/error associations, numeric zero/empty, closed modal anatomy, state and contrast fixtures.
- Screens / flow / consumers: development gallery for future Studio/Theatre Web use; current
  Studio compile smoke only. Surface intensity: low/medium planned; no changed pixels inspected.
- Renderer / target / build: Leptos 0.7.8 CSR debug WASM; native HTML strings only for tests.
  Browser: Safari 18.6 headed input blocked; no viewport/scale observed. Backend for evidence: none.
- Tokens / localization: canonical v0.2.0 unchanged; no CMP mapping. Caller-owned strings; vi-VN
  diagnostics are a developer-only exception, no production locale keys changed; no private errors.
- Accessibility / visual / motion / input: gallery keyboard/touch/OS IME/screen reader, screenshots,
  both-theme/responsive/scaling, forced colors and reduced-motion runtime are NOT_RUN.
- Async/media: no network race or media path. Native stale-close guard source compiled;
  actual rapid reopen/cleanup runtime NOT_RUN. No provider call, audio or publication execution.
- Style review / findings: token roles, scoped selectors, native focus, state attributes, no raw
  HTML or per-app palette; repaired findings recorded above; same-scenario pixels not rechecked.
- Docs / queue / outward actions: package/API docs, work-plan rationale and this handoff updated;
  one authorized local commit only. No linked GitHub execution status changed.
- Not covered / residual risk: all runtime gates above; full workspace/backend/DB/TTS/audio/mobile
  checks NOT_RUN for the explicitly bounded UI slice. Font distribution rights remain unresolved.

## Consumer successor

[Session 3 integration](ui-system-integration.md) records actual Studio and Theatre-skeleton
consumers, the PR29 reconciliation and their fresh focused checks. This component record remains
historical; it does not imply that the changed apps have passed browser, focus or IME checks.
