# Theatre expressive foundation — #452

Source-bound observation, 25 October 2026. Part of [#452](https://github.com/loveoverflowcom/cantos/issues/452),
[Draft PR #457](https://github.com/loveoverflowcom/cantos/pull/457). This record does not grant
component admission or accept the downstream Discovery/Library (#453) and Auth (#454) compositions.

## Source and design provenance

- Implementation: `38c47bb6`, corrected by `29f571c6d6a1d209aa4e5ab2b68b5c15ed65012b`.
- Fetched integration base: `409ace61360d303160e55828a9bf5ecdd0a9e97b` (`develop`).
- [Drive v4 folder](https://drive.google.com/drive/folders/1CT8-icdYTGtCJQCNzb3jP3AwCxMnLFZW),
  revision `owner-feedback-2-bold-symmetry-density`.
- Read the [handoff](https://drive.google.com/file/d/1rvDU68QnWcorgRMlTsQbWalVzIV7kSTf/view)
  and publication receipt through the connected Drive service. Downloaded and opened
  [board 5](https://drive.google.com/file/d/1qQoGVhDzWSBv3ko-TeuI0L7O5Uskg47q/view);
  its SHA-256 is `ab9bdd5d4b1949b907e21df445e812e6f5595b93022963e2541bcffba62e7b6c`,
  identical to the local board.
- The large ZIP fetch failed in this session. The local 43,099,022-byte ZIP matches the
  remotely read publication receipt: SHA-256
  `2d9778d86abeb0be05f5d3718c8087c6bae3dc3874da96e30ae8dfc95764228d`.
  All 162 local files match its manifest. Opened local boards 1 and 4 for header/search/auth
  context; this is not a new full remote ZIP readback.
- Compared `source/expressive-proposal.json` with the canonical token source: all 34 color
  values (17 roles × two themes) agree. Existing Standard theme roles have no value changes.
  Static design boards specify intent; runtime evidence below is separate.

The design package, local browser probe, full logs and captures remain outside tracked source.
Local run directory: `artifacts/agent-runs/issue-452-finish/`. CMP captures are emitted to
`packages/ui-system/cmp/build/reports/cantos-preview/`. These paths are reproducible outputs,
not required repository fixtures or externally published URLs.

## Design-to-implementation comparison

| Design intent | Implemented boundary | Observation |
| --- | --- | --- |
| Theatre palette, Serif 700 editorial headings and Sans 700 controls | Canonical v0.7.0 tokens, generated CSS/Rust/Kotlin; explicit Web `UiRoot expression=Theatre` / CMP `CantosExpression.Theatre` | 34/34 proposal colors agree; scoped remap contrast is checked by the generator; Standard remains the default |
| Solid hero pair | Dedicated Theatre hero/on-hero roles | Light `#1D0B28` on `#C6A0F5`: 8.62:1; dark pair: 13.76:1 |
| 48 theme action/avatar, official 44 symbol with 12 gap | Tonal IconButton, Avatar Control/accent and existing Logo | Web geometry and CMP semantics; narrow CMP header wraps its brand/actions groups instead of clipping the avatar at 200% text |
| 68 minimum field / 72 search, one inset state perimeter | Existing Web Floating extended with trailing slot and `:has()` focus ownership; CMP Floating/FloatingSearch on both String and TextFieldValue overloads | Browser geometry; CMP pixel/state tests; idle/error/focus combinations use one owning shell; support copy stays outside it |
| Independent password eyes | Reactive Web kind/glyph and optional trailing controls; native CMP visual transformation | Same Web input/focus/selection after real pointer click; CMP selection/composition and blocked-state regressions pass |
| One icon-only docked search action | Native Web submit IconButton; CMP callback/IME slots | 52 visible action, 48 minimum target; Web Enter and click each submit once; no app search policy change |
| Check + tone + shape for selected chip | Theatre-scoped component behavior | Static captures and state tests; browser reduced-motion policy gives a zero-duration transition while selection updates |
| Tonal Select and centered content/chevron | Existing Select with Theatre styling; CMP internal container color override | Board 5 calls for `surfaceContainer`; corrected CMP fill and asserted it in the pixel test. Web keyboard selection/Escape/return focus observed |
| Existing consumers remain independent | Standard default, unchanged app compositions | Rust/Web/CMP verification is reported below; consumer admission is still pending |

Other measured solid pairs, light/dark respectively: onSurface/surfaceContainer 13.12/12.64,
onPrimary/primary 11.36/8.28, focus/surfaceContainer 6.55/7.89. The generator verifies all
remap-touched contrast pairs against their declared text (4.5) or indicator (3) floor; these
numbers do not establish contrast over artwork or animated/translucent compositions.

## Corrections made during verification

1. Removed delayed animation-frame caret restoration. It could overwrite a *newer* selection
   made after toggling the eye. Synchronous restoration retains the existing input and leaves
   subsequent edits/selections authoritative.
2. Changed the CMP header specimen to wrap the brand and action groups when space is exhausted;
   added the 320-wide dark/2× capture and 48×48 avatar/bounded-action assertions.
3. Corrected the CMP Theatre Select's idle fill to board 5's `surfaceContainer`; Standard keeps
   its existing internal button fill.
4. Updated the closed icon inventory for the two registered eye glyphs. The tonal color pixel
   assertion now samples unobstructed fill, rather than the center moon glyph's ink.
5. Moved preview tool descriptions to the existing English copy owners and fixed Vietnamese
   board fixtures to explicit fixture constants there. No production catalog or locale policy
   is introduced; Web tool notes carry `lang=en` inside the Vietnamese specimen section.
6. Made token Rust generation match pinned rustfmt for the longer Theatre role name. The design
   inventory removal regression now removes frozen state `light`, not the newly appended
   `theatre-expression` state. It still rejects loss of the original baseline contract.

## Executed checks

Host: Linux x86_64; Rust 1.98.1; Gradle 8.14.3/JDK 17.0.20.1; Chrome 156.0.8078.47,
headless real browser engine, deviceScaleFactor 1. No live API or account was used.

| Check | Result and boundary |
| --- | --- |
| `cargo xtask ui-preview cmp verify` | **PASS** — 248 tests, zero failures/errors/skips; JVM Compose test root, not Android/iOS or an OS preview window |
| `cargo xtask ui-preview leptos verify` | **PASS** — package tests, wasm clippy `-D warnings`, Trunk preview build |
| `python3 -m unittest discover -s scripts -p 'test_ui_*.py'` | **PASS** — 105 tests, including token generation/contrast, icons and catalog/inventory regressions |
| `python3 -m unittest discover -s scripts -p 'test_localization.py'` | **PASS** — 41 tests; changed-source localization scan also passes |
| Repository Python substep: `python3 -m unittest discover -s scripts -p 'test_*.py'` | **PASS** — 1,154 tests, 15 skipped; run separately because the static repository step stops before it. Mocked Kani/tool outputs in these unit tests are not proof-tool evidence |
| Browser probe against `cargo xtask ui-preview leptos run` | **PASS** for the bounded interactions and measurements below; local port 8083, owned server stopped after capture |
| `cargo xtask preflight --changed-only --base 409ace61360d303160e55828a9bf5ecdd0a9e97b` | **FAIL** — repository gate's existing qualification diagnostics; later gates are **NOT_RUN** in this invocation |
| Same preflight with `--skip repository` | **PASS with 1 explicitly skipped (NOT_RUN) step** as printed: all nine other gates pass (Rust format/lint/tests, Web lint, both Web builds, UI preview, CMP UI preview, Theatre CMP preview). Repository remains **NOT_RUN** in this invocation and **FAIL** overall |

The unrelated pre-existing untracked `services/cantos-server/schema/table-ownership.json`
was neither edited nor staged. For these preflight invocations only, `core.excludesFile`
was set through command-local `GIT_CONFIG_COUNT/KEY_0/VALUE_0` to a run-owned file containing
that one path. This kept server work out of this UI diff; no persistent Git configuration
or ignore rule was changed.

The final repository failure contains **32 diagnostics (33 lines including its heading)**.
An exported clean base and the current tree produce byte-identical catalog-check logs:
SHA-256 `0d73ea37c651c413f8c32a71fb10eace6e448656289e594da8f02c60024baee5`.
The diagnostics comprise stale source/token qualification and consumers requiring both renderer
qualifications. Qualification records were left unchanged. A fresh hash alone cannot establish
runtime qualification. Earlier localization and two Python test failures were fixed and are
not part of this remaining failure.

## Browser replay and inspected captures

With the generated Leptos gallery served by `cargo xtask ui-preview leptos run`:

1. Focus `#theatre-password`, set selection `(2,5,"backward")`, click its eye using pointer input.
   The input identity, focus and selection remain; its type becomes text while the confirmation
   field stays masked.
2. For the race regression, click the eye, then set selection `(0,1)` on the first animation
   frame and read it after the next frame. It remains `(0,1)`. This scheduling probe is synthetic;
   it does not claim real mobile IME coverage. The old two-frame restoration reverted it to `(2,5)`.
3. Measure the short field, search field, theme action and search action: heights `68,72,48,52`.
4. Enter a Vietnamese query, press Enter, then click search: the local form counter reaches 2.
5. Open the language Select, use End/Enter to select English, then reopen and press Escape:
   it closes and restores focus to its trigger.
6. Emulate `prefers-reduced-motion: reduce`, select a chip: selection changes and computed
   transition duration is `0s`. This is a CSS/state observation, not a motion recording.
7. At 320 CSS px with gallery text scale 200% and dark theme, the Theatre section's
   `clientWidth` and `scrollWidth` are both 288. This is text scaling, not browser zoom.

The local executable probe and JSON result are `web-probe.cjs` / `web-probe.json` in the run
directory. It is a diagnostic replay, not a new committed browser test runner.

CMP generated all 11 `theatre-*` registered specimens at light/dark 390, light 390/2×,
dark 320/2× and light 1100. Opened the final dark 320/2× header, Select and field matrix:
the avatar is visible, actions remain within the frame, long labels and supporting text wrap
and field height grows. Single-line editable values retain their native horizontal viewport.
Other inspected CMP states include eye/search/chips/button groups in dark 320/2× and
fields in light 390/2×. These are captures from the JVM test root.

Web captures cover light 1280/100%, light and dark 390/100%, and dark 320/200% for header,
eyes, search, selection, Select, button group and hero. Opened the final wide light header
and dark 320/200% eyes/search, plus the earlier unchanged Select/button specimens. All use
synthetic Vietnamese fixture content and the bundled fonts. The English language endonym is
observed; a complete long-English Theatre matrix is **NOT_RUN**.

## Pending acceptance

The PR remains draft. None of these passes replaces component admission.

- **FAIL:** repository qualification/admission. Resume by completing the renderer-specific
  qualification workflow, then rerun the full changed-only preflight. Next owner: unassigned —
  assignment required before acceptance. Do not mechanically refresh qualification digests.
- **NOT_RUN:** long-English Theatre light/dark/200% matrix, real browser autofill/IME and
  non-Chromium interaction; actual pressed/motion interruption recording. Resume with the
  corresponding locale and input/OS setup in the registered previews. No clipping, lost
  composition/selection, duplicated callback or ambiguous state may remain before admission.
- **NOT_RUN:** Android/iOS rendering, keyboard/IME and runtime interactions; TalkBack/VoiceOver.
  Requires the relevant host/device/assistive technology. JVM pixel/semantics results establish
  none of these. Next owner: unassigned — assignment required before acceptance.
- **NOT_RUN:** app adoption, authenticated journeys and deployed acceptance. These belong to
  #453/#454 and the existing integration owners after foundation admission. No application,
  backend, session policy, release or deployment is changed here.
