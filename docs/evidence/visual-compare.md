# Local visual comparison — issue 140

Review boundary: an offline PNG comparison tool, an optional four-backend spike and integration
with the existing UI skills. No production UI, public API, runtime dependency, design approval,
capture harness or automatic merge gate changes. Source base:
`7965039e1f61cefe42f84b9408363a396290b57d` (`develop`); local tool sources are dirty during this
record and each comparison retains their SHA-256. The
[tool contract](../../tools/visual-compare/README.md) owns report fields; the
[visual review owner](../../.agents/skills/cantos-ui-design/references/visual-review.md#10-reference-pairing-and-local-comparison)
owns method. [ADR 0029](../decisions/0029-local-visual-comparison.md) is **proposed**, with a local
MVP implemented for review. No product prerequisite is selected or accepted by this spike.

## Calibration provenance

All screenshots remain in ignored `artifacts/ui-evidence/visual-spike/`; no screenshots or private
data are committed/uploaded. The original [HTML specimen](../../tools/visual-compare/spike/calibration.html)
uses original Vietnamese text, Arial, light, `vi-VN`, 390×844 CSS px at DPR 1 and 100% text scale.
It was captured in headless Chrome `155.0.8059.41` using a throwaway local Playwright Core
1.64.1 driver; this is **HTML evidence, not a Leptos render or approved product design**.

The additional CMP input is the actual `DemoCaptureTest.light_discovery_is_captured_at_390_by_844`
PNG, produced by `cargo xtask theatre-demo verify` with existing cached Gradle 8.14.3,
Microsoft OpenJDK 17.0.19 and the mock's bundled Noto fonts on macOS 26.5.2 arm64. Its metadata
and semantics remain beside `apps/cantos-theatre/mobile/shared/build/reports/theatre-demo/`.
The native Android/iOS boundary was not exercised. Capture metadata lacks an explicit timestamp
and font hashes; the test log/source and local file times identify this run, not authenticated
native provenance. Calibration pairing supplements do not repair those historical omissions.

Five controlled HTML pairs: identical bytes; button background `#7150a5` → `#a53155`; card padding
24 → 32 px and radius 24 → 8 px; rasterizer-noise simulation; mini-player bottom inset 28 → 52 px.
The noise fixture perturbs 1144 mid-tone heading-edge pixels by +2 channels after capture; it is
**synthetic AA-like noise**, not an actual comparison between font engines. The sixth pair moves
a crop of the real CMP mini-player up 16 px and fills the original strip. That is an injected
bitmap mutation, not a render of modified Kotlin or an application defect.

Calibration baselines are versioned `v1` and attributed to the original-fixture author, Codex,
with `approval.status=calibration`; none is owner-approved product design. Exact PNG identities:

| Input | SHA-256 |
| --- | --- |
| HTML reference / identical | `8437aee13fb601dcc52dbb5191e30b7a96ae9c6955aa8745ecd9d7bc0e3443f5` |
| Button color | `9c2129235b54761cdcfcd657c021b7b005dff389d40a0774a7b240b9eb358de2` |
| Padding/radius | `62ba86c58622c694c5eea5ddb4ffdda8eb71090a101b745a0b2e8aa21f2154d4` |
| Synthetic raster noise | `77138196b0fa1a24c7d3ea48973cb5f04355989b3f6684b49761c81f4e6f1c05` |
| HTML player moved | `c548922f07c9c9e97e00270669ee55e0255f377cb0954ff4870b97fa77a755c6` |
| CMP reference | `f5278204464d102ec1390406cce66318a949fb7a385d135510236b62f17129dd` |
| CMP injected displacement | `bf5ebe04847a9c604c998855ce663499ab65c830aceb8ba08409d3a28a5abe49` |

## Four-backend experiment

`cargo xtask visual-compare --spike-pairs artifacts/ui-evidence/visual-spike/pairs.json` ran
offline after explicit isolated setup. Python 3.9.6 / Pillow 11.3.0; Node 26.5.0; looks-same
10.0.1, pixelmatch 8.0.0 / pngjs 7.0.0 and odiff-bin 4.5.0. The committed isolated npm lock pins
research dependencies; no application or default verification invokes npm/setup.

Method: one warm-up, five samples, median PNG decode + compare + diff/mask encode milliseconds.
This excludes process startup, fixture capture and MVP grouping/crops/reporting. Looks-same's
identical-byte shortcut emits no diff, so that row has unequal output work. Different comparator
models/counts are not interchangeable. Short single-host measurements are exploratory.

| Pair | A looks-same ms / pixels | B pixelmatch ms / pixels | C odiff ms / pixels | D Pillow ms / pixels |
| --- | --- | --- | --- | --- |
| Identical | 0.19 / 0 | 22.32 / 0 | 6.28 / 0 | 4.33 / 0 |
| Button color | 11.46 / 5900 | 24.12 / 5831 | 5.18 / 5840 | 4.24 / 6277 |
| Padding/radius | 15.37 / 14466 | 23.89 / 8361 | 6.75 / 8137 | 4.70 / 20595 |
| Synthetic raster noise | 8.15 / 0 | 24.06 / 0 | 4.24 / 0 | 4.20 / 0 |
| HTML player moved | 12.88 / 17355 | 24.91 / 17306 | 10.18 / 1079 | 4.43 / 18860 |
| CMP player moved | 20.70 / 14741 | 30.09 / 13737 | 11.18 / 13608 | 6.32 / 17601 |

A uses tolerance 2.3, AA filtering and `ignoreCaret:false`; B threshold 0.1 / `includeAA:false`;
C threshold 0.1 / AA filtering; D max premultiplied RGBA channel delta >8, without an AA
heuristic. All detected the four deliberate visible changes; zero reported pixels for identical
and the synthetic noise at these policies (0/2 false-positive pairs in this tiny sample).
Exact default comparison counts the 1144 noise pixels, as intended. This does not establish a
safe threshold for real fonts or rule out false negatives. In particular, C suppresses most of
the low-contrast player fill movement; a low ratio cannot be read as low impact.

Pillow was selected for the bounded prototype: a fast measured PNG/mask backend, existing Python
workflow, explicit alpha/threshold semantics and small standard-library grouping core. A offers
ready-made perceptual clusters; B needs grouping; C needs platform-specific native packages and
its bounds/lines are not component clusters. All require distinct policy review. Pillow and npm
tools were exercised only on this macOS arm64 host; Linux/Windows portability remains untested.
No NumPy, scikit-image, OpenCV or SSIM dependency was added.

Raw output: `artifacts/ui-evidence/20261015T045035Z-1792039835321752000-visual-spike/comparison/benchmark.json`;
commands/logs: `artifacts/agent-runs/visual-compare/`. Rerun instructions and manifest format are
in the tool contract. Original capture and controlled-mutation scripts remain local in the
calibration directory; they are throwaway fixture preparation, not a new Web harness.

## MVP observations and same-scenario recheck

`cargo xtask visual-compare` ran for all six real-PNG pairs, with explicit threshold 8 for this
calibration. The full command, including Cargo/interpreter startup, grouping, crops and reports,
took approximately 0.58–0.63 s for non-identical pairs on this host (first invocation 0.89 s).
Those times are separate from the backend microbenchmark.

Opened with `view_image` by Codex: HTML reference/actual, numbered overlay, diff, mask, heatmap,
three region crops, freshly recaptured restored HTML and its full overlay; also the original CMP
capture and injected-displacement overlay. Concrete observations:

- HTML player is 24 px higher; the red box covers both the old/new pill and duplicated text in
  the 50/50 overlay. Region 1 is exactly `(24,715,342,101)`, 18860 changed pixels. The binary mask
  confines changes to that player footprint; the rest of the page stays black.
- The reference/actual detail crops show the bottom-space difference; the overlay crop shows
  two vertical text positions inside the numbered box. The diff highlights that footprint;
  the heatmap is dim over low-contrast fill and brighter at text/transport edges.
- The CMP overlay's box `(16,676,358,80)`, 17601 changed pixels, surrounds the injected shifted
  mini-player. This does not diagnose any Kotlin source cause.
- A new Chrome capture of the restored original HTML produces zero changed pixels with the
  same scenario, reference hash, threshold, tile policy, masks and comparator version/source.
  Opened actual/overlay show the single player back at its original bottom position. This is a
  **fixture-only repair/recheck demonstration**, not a production UI fix or Leptos/CMP acceptance.

The local inspector finding/report records hashes and observations separately from the unchanged
metric report (`images_inspected:false`). The existing UI-fix workflow consumes the bounded
region and rechecks with the same policy. No baseline/mask/threshold was edited to improve a score.

## Claim and layer ledger

| Claim / failure mode | Oracle / held evidence | Boundary |
| --- | --- | --- |
| Pixel accounting loses or double-counts changes/masks | Exact masks/rectangles, overlap union, deterministic seeded-set differential checks | Standard-library core; example-/differentially-tested |
| Alpha or threshold semantics silently change | Real PNG hidden RGB / alpha and exact 8/9 boundary regressions; 16-bit PNG refused before quantization | Pinned Pillow adapter; example-tested |
| Size/state mismatch silently resizes or scores | Mismatch reports omit metrics/overlay; scenario mismatch asserts theme conflict | Local file I/O; example-tested |
| Mask/baseline evidence is forged by omission | Review fields, exact pinned baseline hash, full-mask/duplicate-key/non-finite rejection, stale output refusal | Caller attribution only; no authenticated design approval |
| Drift cannot be located | Exact synthetic 3×2 PNG box/mask; opened HTML/CMP overlays and crops | Local PNG adapter; screenshot-inspected |
| A repair changes its oracle | Reference/configuration/implementation policy hash retained on fixture recheck | Tooling demonstration, no production fix |
| Tool silently provisions or changes releases | Lazy optional Pillow import; isolated opt-in spike; inward xtask/script dependencies | No app/runtime dependency, network/install action or public handler |

L1: documented modular boundary; existing entrypoints compose one visual method, no new skill.
L2: immutable rectangle values, bounded dimensions/masks/tile grid and explicit rejection.
L3: focused comparator tests and Rust CLI planning tests; see current command receipts below.
L4: actual CLI → Python → real local PNG read/write, independent fixtures and hashes; no mocked
provider, database or service. L5: HTML browser and JVM/Skia calibration captures plus opened
images only. Product browser journeys, real design parity, native devices/readers and CI pixel
gates are outside this PR.

## Command receipts and blockers

- `RUSTUP_TOOLCHAIN=1.98.1 TMPDIR=/private/tmp cargo xtask visual-compare --self-test`: PASS;
  12 tests, including seven core and five optional real-PNG tests on installed Pillow.
- `cargo xtask theatre-demo verify`: PASS with the explicitly selected existing Gradle/JDK;
  actual mock captures, no Android/iOS/device claim.
- `cargo xtask visual-compare --spike-pairs ...` and all six MVP comparisons: PASS (tool completion).
- `cargo xtask verify repository`: FAIL, reproduced on a clean disposable snapshot of the exact
  source base. Six Leptos preview qualifications have stale source/token digests; two Membership
  import sites require fresh both-renderer qualification. No qualification/baseline was rewritten.
  The same eight failure messages exist before this change; default Python-suite execution is
  not reached after that failure.
- `cargo xtask verify rust`: PASS (format, workspace lint and workspace tests). Final focused
  `cargo fmt --all -- --check`, `cargo clippy -p xtask --all-targets --locked -- -D warnings` and
  `cargo test -p xtask --locked`: PASS; 161 xtask tests, including missing-interpreter refusal.
- `cargo xtask verify contracts`: UNSUPPORTED (`audio-inspection`: missing ffmpeg/ffprobe);
  its other selected steps are NOT_RUN by prerequisite precheck. This is not a passed suite.
- `cargo xtask preflight --changed-only`: UNSUPPORTED (`audio-inspection`, `postgres`: missing
  ffmpeg/ffprobe); other selected gates NOT_RUN by precheck.
- `cargo xtask preflight --changed-only --skip repository --skip audio-inspection --skip postgres`:
  PASS **with three explicitly skipped NOT_RUN steps**. Available rust-format, Script IR and
  Protobuf references, workspace lint/tests, DBSP, WAV reference, Web lint/both builds, UI preview,
  CMP preview, Theatre CMP and mutation gates all PASS. Mutation killed 9/9 bounded mutants.
  This independent run retains the original repository FAIL and missing-media UNSUPPORTED
  receipts above; skips do not resolve or accept them. Gradle/JDK were selected as in the capture.

Deferred acceptance: design/tooling owner review of ADR 0029 and actual product baseline
approval; representative real AA/font/platform pairs; native and screen-reader acceptance where
the consumer requires it. Next owner is unassigned; assignment is needed before product
acceptance. Requalifying the stale shared components belongs to their existing admission owner.
No production/publication behavior can activate through this optional local command.

Residual risk: occupied tiles can merge neighboring components and up to 256 regions are listed;
all pixels remain counted/masked and omitted region count is explicit. Thresholds can suppress
real low-contrast changes. Metadata/approval are caller assertions, not authentication. Encoded
channels are not color-managed perceptual distance. Cross-OS/renderers/fonts, approved design
parity, a native device, keyboard/focus, accessibility and motion were not accepted by this run.
