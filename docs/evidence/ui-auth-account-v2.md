# Shared auth/account expressive foundation — issue 127

## Scope and invariant

Auth/account expression is opt-in in the shared UI system. Regular listening/editor controls,
mini-player dimensions, existing color/font/density roles and app navigation do not change.
Controlled actions and fields retain host-owned values, names, reasons and callbacks; loading
or disabled input cannot produce another intent. No authentication, membership activation,
billing, authorization, grant, provider or service configuration is implemented here.

Owner: `packages/ui-system/shared` canonical semantics, Leptos/CMP renderer adapters and their
package-local diagnostics. Public `BackButton` and `Logo` plus scoped Button/TextField/Surface
variants are **not admitted for application adoption** by this draft. L1–L3 source/semantic
verification is separate from L4–L5 rendered interaction/platform acceptance. There is no new
durable domain core to extract: existing pure state guards still own dispatch admission.

## Source and design provenance

- Fresh base: `develop@b6066acc659b6d4cd030badb9c1abdad08b6dcf2`, tree
  `da9481756cc472a5ac8c4431de7663c402f0e142`; fetched on 2026-10-14.
- Implementation branch: `feat/issue127-auth-account-ui-system`, created directly from that base.
  Both existing worktrees/refs and historical evidence are preserved. This executor had no
  reproducible Cantos build/target/dist cache, so cleanup removed no files and reports no savings.
- [Issue 127](https://github.com/loveoverflowcom/cantos/issues/127) and its complete empty comment
  thread were read, then the actual design/source/logo ZIP bytes were materialized and inspected.
- [Design v2](https://drive.google.com/drive/folders/1oJ8Y8NzmjfFh_qSAx_qV6tNH9JIKlERx),
  [handoff](https://drive.google.com/file/d/1ETmdL9w1ywC10OqjPIrhV9ERy2l59tvJ/view),
  [Logo Kit](https://drive.google.com/drive/folders/1nIIehbnR1t2sXD0tKsYTXyHwekNtVly7).
  The design's historical source SHA `1c1d2107` is not the implementation baseline.
- Inspected reference pixels: Web board and Logo Kit preview; renderer implementers also read
  actual individual mobile/Web auth and account PNGs. References are prototype Chromium pixels,
  never evidence that Leptos, CMP, native auth or payment ran.
- Handoff/source token proposal, `auth-design.css`, motion/runtime boundary and Logo Guide were
  read rather than imported as production CSS/JS. Local official logo provenance/checksums and
  font-derived licensing are recorded with the [assets](../../packages/ui-system/shared/assets/cantos-logo-kit-v2/README.md).

## Mapping and unchanged boundaries

The [v0.4.0 canonical contract](../../packages/ui-system/shared/README.md#scoped-authaccount-mapping-v040)
is generated to scoped CSS/typed Rust/Kotlin and both preview inventories. Values are Cantos's
reference proposal, not mandatory Google measurements. Web rem and native dp preserve semantic
relationships, fractional `size.authOutline=1.5` and intrinsic layout.

| Reference intent | Shared implementation | Verification boundary |
| --- | --- | --- |
| Primary filled 60 / secondary tonal 56, capsule / pressed 16 | Scoped Button variants; regular variants unchanged | CMP desktop intent/state tests and opened focus/pressed/large-text pixels; Web runtime still pending |
| Tonal field 56 / radius 20 | Optional TextField auth variant, controlled values and external messages | Semantic tests; actual input/focus/IME still renderer-specific |
| Auth/profile 36, membership 32, account group 28 | Surface variant separate from semantic color tone | Paired foreground, no decorative outline; CMP pixels inspected, Web still pending |
| Back target 48, contextual name | Typed nonblank AccessibleName, guarded host callback | Destination/history/system Back/cancellation stay in consumer issues |
| 1.5 functional outline, 3 focus | Fractional scoped outline and existing focus tokens | Exact no-rounding generator regression and CMP focus pixels; Web/device rendering pending |
| Interrupted shape feedback / reduced motion | Shape duration 220, explicit zero mapping | No CSS/native spring claim; runtime interruption remains pending |
| Official Cantos/Cantos Studio/symbol, light/dark/mono | Local path vectors, typed role/tone, optional standalone name | SVG/alpha/hash/anatomy tests and five opened CMP logo captures; Web/full nine-mark visual acceptance pending |

No app imports the new components or opts into the variants. Existing catalogue compatibility
sites are not expanded and no qualifications are fabricated. Registration/source parity is not
admission. Both package previews provide actual anatomy and host-driven state specimens; future
consumers #128–#131 retain their own permissions, business semantics and product navigation.

## Execution environment

The fresh executor did not contain historical issue120/125 tool directories. Exact official
Rust/Cargo1.98.1, rustfmt1.9.0, Clippy0.1.98, wasm32-unknown-unknown, Trunk0.21.14, Temurin
JDK17.0.20.1+1 and Gradle8.14.3 were restored into an isolated task-local tool directory with
checksums; no global settings or credentials were created. Tool doctor rows are capability
observations only, never application PASS claims.

Chromium154/Playwright exist, but browser launch failed at its local process socket with
`Operation not permitted`; escalated execution failed in environment setup before the launch.
No permitted separate dot-cloud browser route was available. User-computer/Opera/CUA and Mac
were not used as a fallback. Browser runtime, DOM/input, focus, 320 CSS px reflow, 200% text,
light/dark screenshots and motion observation remain **BLOCKED / NOT_RUN**.

Android SDK, official Android CLI and Android Studio are unavailable. Android host capture and
native device/screen-reader acceptance remain unsupported/unrun; nothing is provisioned solely
for a green capture. JVM desktop tests cannot establish Android/iOS device behavior.

## Gate ledger

Commands ran from the repository root on the base plus this draft's uncommitted source,
using the isolated tool versions above. Full logs remain in ignored
`artifacts/agent-runs/issue127/`; no remote CI was used or checked.

| Command / obligation | Actual result | Evidence and boundary |
| --- | --- | --- |
| `cargo xtask verify repository` | PASS | 398 Python tests, repository/source checks and whitespace, repeated on the final staged source/evidence; `repository-first.log`, `repository-frozen.log`, `repository-publish.log` |
| `cargo xtask ui-preview leptos verify` | PASS | 71 native tests, 5 compile-fail doctests, warning-denied WASM clippy and Trunk gallery build on the frozen source including logo-spacing correction; `leptos-frozen.log`. This is not a browser test. |
| `cargo xtask preflight --changed-only` | UNSUPPORTED / NOT_RUN | PostgreSQL prerequisites (`initdb`, `pg_ctl`, `psql`) missing; every independent gate not started by precheck; `preflight-initial.log` |
| `cargo xtask preflight --changed-only --skip postgres` | FAIL | Repository, formatting, Script IR reference, protobuf reference and workspace clippy PASS; workspace Rust tests FAIL; remaining gates NOT_RUN; `preflight-available-second.log` |
| `cargo test --locked -p xtask shared_ui_inputs_reach_both_web_consumers_and_the_gallery` | PASS | One focused selector regression selected, 157 filtered; `selector-focused.log`. Not a substitute for the failed full suite. |
| `cargo xtask ui-preview cmp verify`, then final `cmp-ui-preview` gate | PASS | 111 tests, zero failed/errors/skipped with actual desktop semantics and PNG generation; `cmp-runtime.log`, then the final strengthened capture suite in `preflight-independent.log`. The first source/test compilation passed but runtime had 36 passed/75 failed from Skiko cache initialization; `cmp-final.log`. |
| `cargo xtask preflight --changed-only --skip postgres --skip rust-tests` | FAIL | 13 gates PASS: repository, rust-format, Script IR/protobuf references, workspace clippy, DBSP, WAV reference, audio inspection, Web lint, both app Web builds, Leptos gallery build and final CMP suite. Untouched Theatre mock failed before compilation at official Kotlin serialization plugin resolution; mutation not reached; Rust tests/PostgreSQL explicitly NOT_RUN by skip. `preflight-independent.log`. Original full Rust FAIL remains above. |
| `cargo xtask verify mutation` | FAIL | The unchanged baseline Cargo invocation timed out at its configured 60 seconds, before any mutant result; `mutation-final.log`. Do not accept this as nine killed mutants or extend the boundary to manufacture acceptance. |
| `cargo xtask theatre-demo verify` | PASS | One bounded retry compiled the unchanged mock and passed 248 desktop tests, zero failed/skipped; `theatre-cmp-retry.log`. Official plugin/POM routes returned HTTP200 and the retry resolved normally; the first generic resolution failure's root cause is not proven. No app/dependency/repository settings changed. |
| Leptos browser interaction / opened captures | BLOCKED / NOT_RUN | Verified browser-launch environment failure above; no screenshots or DOM runtime evidence generated. |
| `cargo xtask ui-preview cmp android-build` | FAIL | Initial AGP application failed at read-only home settings; isolated Android config/cache retry reached the missing SDK location before compilation. `android-build.log`, `android-build-local-cache.log`; no SDK installed and no source/test changes. |
| `cargo xtask compose-preview ui-system-cmp --file packages/ui-system/cmp/android-preview/src/androidMain/kotlin/com/cantos/ui/preview/AndroidPreviews.kt --composable AndroidAuthAccountLightPreview --label auth-light-390-vi` | UNSUPPORTED | Official Android CLI absent; `android-capture.log`. Actual registered wrapper selected, no host capture fabricated. |
| Native device and named screen-reader acceptance | NOT_RUN | Requires authorized Android/iOS runtime and TalkBack/VoiceOver reviewer; desktop/native source is not this evidence. |

The failed full workspace run obtained 1392 passed, 18 failed and 293 ignored tests before
stopping; documentation tests were not reached. Seventeen failures are unchanged service
private-input fixtures: this sandbox's read-only synthetic `/tmp/.git` makes disposable
temporary paths fail the existing checkout-exclusion guard. The remaining unchanged
`probe::tests::port_ownership_reflects_an_existing_listener` failed the final post-release
`port_is_free` assertion; its root cause is not proven. The tests, private-input configuration
and probe files are byte-identical to the fresh base. No unrelated fixture, mount or security
guard was changed. These actual FAIL results remain failures, not acceptance or merely skipped
checks. The first available preflight's formatting failure was corrected with the exact
rustfmt and passed on the next run; it is retained in `preflight-available-first.log`.

The first Leptos build failed because its inherited cache location was read-only. Moving only
`XDG_CACHE_HOME` to this task's writable isolated cache allowed the final build. The first CMP
attempt used a stale inherited proxy; the current process-local helper selects the current
proxy and existing OS Java truststore while keeping normal TLS verification, official plugin
and Maven repositories, and unchanged global settings. Neither correction fabricates rendered
evidence or expands credentials/access.

The first compiled CMP suite's 75 runtime failures occurred before assertions because Skiko
0.9.22.2 extracts its bundled native to `skiko.data.path`, defaulting under the read-only home
directory. Cached bytecode and an actual native-load probe established this exact path owner;
it is not `java.io.tmpdir`. Setting only process-local `JAVA_TOOL_OPTIONS=-Dskiko.data.path=...`
to this task's writable cache allowed the full 111-test suite to pass with its original tests.
Normal bundled checksum verification and native loading are unchanged.

### Frozen source checks

`python3 scripts/ui_preview_catalog.py --digests` reports:

- tokens: `15ab3bca01d29c3e8c98a938ffcddc2583f1c15c472b37656d3fe76de7938a42`
- Leptos: `93b8c38368e5236883e3a4f8a5c52b5b9d3e5c0f7b4a7558348bfeed998017ab`
- CMP: `850afe4c4a89b007615fefc92f64a179d63a5246e5f2bd62b4dbc0b93d8c76ff`

Independent read-only source review confirmed those digests, unchanged historical token and
regular geometry values, byte-identical official assets, callback/name guards and shared icon
parity. The profile supporting-text correction uses `onPrimaryContainer`, not muted
`onSurfaceVariant` (the latter would produce only 4.35:1 in light theme). Computed current
light/dark role-pair contrast is 6.92/8.67 for primary, 10.69/7.69 for primary-container,
12.65/10.79 for field surface-container, and 4.88/6.75 for muted supporting text on its regular
surface-container. Numeric contrast and source review do not establish rendered text, focus,
screen-reader behavior or preview qualification.

### Actual JVM pixel inspection

Seven auth/account and five Logo PNGs from the real desktop tests were opened, not inferred
from declarations. The initial `390×3000`, text2× dark account capture visibly clipped the
membership card and omitted its last account group despite a passing bounds-only assertion.
That evidence is invalid for full-scene visibility and is preserved byte-for-byte with all
38 initial test captures in `artifacts/ui-evidence/issue127-cmp-initial-3000/`. The corrected
diagnostic uses unbounded vertical measurement, `390×4800`, and requires a displayed nonzero
footer, complete text layout and in-image bounds; it does not shrink production typography.
The complete replacement scene is required instead of that initial clipped evidence.

The strengthened final suite passed all 111 tests. The replacement `390×4800` dark2× PNG was
opened independently at original resolution: full membership copy/button and the complete
account-group footer are visible with bottom breathing room. Its SHA256 is
`88985d0fe79e01e69edbcc849e28133109c5626f6117ea0201072b60fa4ae3db`.
Both remeasured `390×1600`, text1× light/dark captures were reopened with complete anatomy.

The other opened auth captures show intact Vietnamese marks, readable paired profile metadata,
visible blocked reasons, separate focus/error/supporting-text anatomy and capsule/pressed16
static shapes. Native single-line input uses its normal horizontal value viewport; this is
not an assertion that its entire long value is visible at once. Still images cannot establish
interruption/release animation, live accessibility or native device behavior.

Opened Logo captures show real caller-background knockouts on two backgrounds, exact official
light/dark symbol ink, transparent corners and unstretched Studio fit in a `160×160` slot.
The tight `64×64` symbol roots do not establish surrounding guide clear space or full nine-mark
visual acceptance. Source/hash checks cover all nine originals; neither source checks nor these
five inspected JVM images qualify the Web renderer.

## Admission and remaining acceptance

- [ ] On a permitted Leptos runtime, drive idle/focus/pressed/loading/disabled/error using actual
  package specimens, long Vietnamese labels, light/dark, 320/wide viewports and 200% text. Capture
  and open pixels, assert DOM names/roles/description/intent counts, keyboard focus and no clipping.
- [x] On CMP desktop, run the actual component/state semantics and inspect auth/account
  light/dark, native text scale2×, focus/error/pressed/blocked and intrinsic scene pixels.
  Desktop pixels do not replace Android host, device, motion or screen-reader acceptance.
- [ ] On the Android host, repeat the same registered actual fixtures and inspect pixels and
  semantics with native font scale2× and both themes.
- [ ] Run `cargo xtask ui-preview cmp android-build` with a real local SDK; use
  `cargo xtask compose-preview ui-system-cmp` with registered actual Android preview wrappers
  and official CLI/ready Quail2+ IDE. Inspect PNG/semantics and retain fresh source SHA/config.
- [ ] Observe shape interruption/release and reduced motion in each real renderer; no animation
  may delay dispatch, move a hit target or claim spring/native behavior from a still image.
- [ ] Name a reviewer and create fresh structural/runtime/accessibility/visual qualification
  records for both renderers, matching current token/source digests, before any app adoption.
- [ ] Re-run `cargo xtask verify rust` in an environment with real out-of-checkout disposable
  paths; all workspace tests must pass. Keep the existing private-path security guard and
  investigate the separate unknown post-listener-release port assertion without bypassing it.
- [x] `cargo xtask theatre-demo verify` resolved the official serialization2.2.20 plugin,
  compiled the actual unchanged mock and passed all 248 desktop tests on the bounded retry.
- [ ] Re-run `cargo xtask verify mutation`; its unchanged 60-second baseline and nine mutant
  checks must complete normally. This run has no mutation score or accepted mutant result.
- [ ] Re-run `cargo xtask verify postgres` with disposable local PostgreSQL binaries plus
  ffmpeg/ffprobe; require real fixture checks. Prior remote artwork Storage(Unavailable)
  failures are not accepted, explained or repaired by this draft.
- [ ] Complete Android/iOS large text, system Back/cancellation and named TalkBack/VoiceOver
  acceptance in the separate native/consumer scope; no auth integration is implied here.

Next owner: unassigned — assignment required before admission. Existing PR126 PostgreSQL artwork
fixture failures and the 60-second mutation baseline timeout are not accepted or silently fixed
by this UI foundation. No merge, issue closure, remote workflow rerun or production activation
is requested by this draft.
