# Theatre CMP shape normalization

## Input, invariant and ownership

User request: audit every Theatre CMP corner and normalize ordinary containers without changing
fonts, colors, spacing, layout, navigation or mock playback. The input screenshots document the
reported excessive rounding; the fresh baseline below is the current mounted app, not those
historical screenshots. UI Fix, UI Design, CMP, Leptos and Engineering skills were compared with
`origin/develop` at `7e9a7ec`; their owning SKILL.md files are identical to this checkout.
Implementation preserves the previous local UI repair at `f0fd9a8`.

Invariant: one canonical shape contract supplies the five Material slots (4/8/12/16/20),
mini-player 12, thumbnail 10, story/continue cards and large covers 16; Full is only for actual
circles/pills. The 10-unit thumbnail is the user's explicit exception to the five-step scale.
Owner: `packages/ui-system/shared/tokens.json` v0.2.2 → generated CSS/Rust/Kotlin →
`CantosShapes` / `CantosTheme` → mounted Theatre composables. No new component library,
dependency, spacing token set or playback authority is introduced.

## Audit and minimal repair

| Consumer | Baseline | Result / role |
| --- | --- | --- |
| Material Shapes extraSmall/small/medium/large/extraLarge | 12/12/18/24/24 | 4/8/12/16/20 from detail/control/reading/container/overlay |
| MiniPlayer outer Surface | Material large, 24 | medium / reading, 12 |
| Continue Listening and history ListeningPoint | Material large, 24 | large / container, 16 |
| Story Card | explicit Material large, 24 | large / container, 16 |
| WorkArtwork large banner/cover | Material medium, 18 | large / container, 16 |
| Compact artwork in card, continue/history and mini | Material medium, 18 | thumbnail, 10 |
| Episode rows | Material medium, 18 | medium / reading, 12 |
| Outlined search field | Material extraSmall default, 12 | explicit medium / reading, 12 |
| Regular filled/tonal actions | Material default Full | explicit medium / reading, 12 |
| Text actions | Material default Full | explicit small / control, 8 |
| Preference/history AlertDialog | Material default extraLarge, 24 | explicit extraLarge / overlay, 20 |
| Mini and full player transport icon buttons | default circle | explicit CantosShapes.full (CircleShape) |
| Navigation indicator, slider thumb, radio/switch anatomy | native Material shape | retained platform circle/pill anatomy |
| Uncontained rows and structural layout | rectangle / no clip | retained; no decorative rounding added |
| Shared CMP Action/Field/Surface/Dialog | locally constructed token shapes | centralized CantosShapes adapter over existing semantic roles |
| Shared badge | token pill sentinel | Full proportional CMP mapping; genuine status badge |

No `CardDefaults.shape`, `elevatedShape` or `outlinedShape` consumer exists in the app;
`CardDefaults.cardColors` remains unchanged. No `RoundedCornerShape` or `CircleShape` constructor,
raw dp spacing/radius, or numeric corner literal remains in Theatre commonMain. CircleShape
exists once in the shared Full adapter. The shared token gallery constructs shapes dynamically
from each declared token to display the catalog; it is not a production container policy.
Material default geometry for radio, switch and navigation is deliberate native anatomy.

Existing semantic role values change, so shared Web action/navigation corners become 8,
containers 16 and overlays 20. Both renderer outputs and catalog inventory were regenerated;
shape.field remains a direct alias of shape.reading. No token family outside shape changed,
and existing spacing values, density overrides, colors, typography and motion are untouched.

## Layered evidence

Full logs and PNG/JSON/semantics remain ignored under `artifacts/agent-runs/cmp-shape-tokens/`.
The mounted entrypoint is `CantosDemoApp`, with memory-only fixture catalog/state, real resource
fonts/artwork and no server/audio/native playback integration.

| Layer | Evidence and limit |
| --- | --- |
| L1 architecture | Source trace and token audit above; shared semantic owner and renderer adapters only |
| L2 domain | Inapplicable to radius decisions; existing state/controller/event contracts unchanged |
| L3 regression | `material_shapes_follow_the_shared_five_step_radius_scale` failed on baseline [12,12,18,24,24], then passed on [4,8,12,16,20]. Thumbnail/Full outline test covers 2× density/font scale and three widths. A deliberate mini-player assignment to large/16 was rejected by its mounted pixel test; restored afterward |
| L4 mocked UI | Existing presentation, interaction, layout, preference focus and capture suites plus mounted MiniPlayerShapeTest; final gate results recorded below |
| L5 renderer / native | Same-scenario JVM images and Web browser observations below; Android/iOS runtime and screen readers remain separate deferred acceptance |

Before/after fixture matrix: 34 matching captures, density 1, base font scale 1, vi-VN or the
existing English library fixture, light/dark; 320×568, 320×800, 390×844 and 840×840. New mini
fixtures include both themes at 320/390/840 and both themes at text scale 2 on 320×800.
All 34 unmerged semantics dumps retain geometry, text, roles and actions after normalization
of unstable node IDs/tree indentation and the new mini-player test tag. The comparison is in
`semantics-comparison.json`; it does not imply device accessibility or animation evidence.

Opened baseline and repaired discovery/mini at light 390/840 and dark 320: corners tighten,
cover clipping changes, and text/control positions remain identical. Repaired dark text-2 mini
keeps one ellipsized title, time and circular play control above the bottom navigation. Additional opened before/after pairs:

- `jvm-light-library-390x844`: Continue and saved Story Card corners become 16, compact art 10;
  the saved title, description and independent save action stay at the same positions.
- `jvm-dark-player-390x844`: large cover becomes 16 and secondary actions 12;
  seek/play circles, progress and text remain unchanged.
- `jvm-light-work-390x844`: cover 16, primary/save actions 12; mini remains above navigation.
- `jvm-light-theme-dialog-390x844` and `jvm-dark-timer-dialog-text2-320x800`:
  overlay corners become 20; all options, selected state and dismiss action remain visible.
- `jvm-light-player-text2-320x800`: cover clipping tightens; long Vietnamese title,
  progress and transport remain in identical positions.

These named images were opened in both stages. Remaining captures have semantics comparison
and captured-pixel evidence only. `capture-index.json` records SHA-256 for all 68 before/after PNGs.

Web Leptos gallery on loopback port 8083: before/after action group, then light/dark actions,
layout containers and native dialog were opened. Regular controls have 8px computed radius;
chips remain Full, reading 12 and overlay 20. The browser gallery is development evidence,
not a Theatre/Studio production flow or native evidence. Temporary browser tab/server stopped.

Accessibility: unchanged actions/labels/geometry across captures; existing keyboard focus,
Escape and restoration checks run in JVM UI tests. TalkBack/VoiceOver not observed. Motion and
native inset/media behavior not observed or changed. No new motion claim is made.

## Commands and acceptance

Common environment: Rust 1.87.0, installed Gradle 8.14.3 on PATH, JAVA_HOME points to installed
Microsoft OpenJDK 17.0.19; TMPDIR=/private/tmp; Android build uses installed SDK via ANDROID_HOME.
`CANTOS_THEATRE_DEMO_CAPTURE_DIR` points to the before/after directory for each capture run.

- `cargo xtask theatre-demo verify`: baseline PASS; final theatre-cmp-demo PASS through preflight; 162 tests in 20 suites, zero failures/errors/skips.
- `cargo xtask ui-preview cmp verify`: red reproduction FAIL; repaired shared suite PASS; 80 tests in 11 suites, zero failures/errors/skips.
- `cargo xtask verify repository`: PASS including the final documentation rerun; 226 Python checks plus repository/service/link/diff checks.
- `cargo xtask preflight --changed-only`: PASS for repository, rust-format, selected Rust lint/tests, web-lint, both Web builds, ui-preview, cmp-ui-preview and theatre-cmp-demo.
- `cargo xtask verify web`: PASS (web-lint, both client builds and shared Leptos preview).
- `cargo xtask theatre-demo android-build`: PASS; installed Android SDK, unsigned debug APK compilation only.
- `cargo xtask compose-preview cantos-theatre --file apps/cantos-theatre/mobile/shared/src/androidMain/kotlin/com/cantos/theatre/app/TheatrePreviews.kt --composable TheatreDiscoveryLightPreview --label radius-review --timeout-seconds 30`: UNSUPPORTED, official CLI studio-check reports “No running Studio instances found”. Evidence: `artifacts/ui-evidence/compose-preview/cantos-theatre/20261013T094122Z-22b27649d52a`. JVM fallback captures above were inspected.
- `cargo xtask verify contracts`: audio-inspection UNSUPPORTED (missing trusted installed ffmpeg/ffprobe); script-ir-reference, contract-tests and wav-reference NOT_RUN by prerequisite precheck. No contract/media behavior changed.

- `cargo xtask verify rust`: first run rust-format/lint PASS, rust-tests FAIL at
  `xtask/src/probe.rs:139` (`port_ownership_reflects_an_existing_listener`, bind immediately
  after dropping a loopback listener). One bounded unchanged-source rerun: all three gates PASS.
  Classified flaky (1 FAIL / 2 runs); socket allocation/rebind is the nondeterministic boundary,
  exact contention actor unknown. `probe.rs` is unchanged since `1757334`; no unrelated fix,
  instrumentation, skip or new service was introduced. Logs: `rust.log`, `rust-recheck.log`.
- `python3 scripts/ui_tokens.py --check` and `python3 scripts/ui_preview_catalog.py --check`:
  PASS. A direct JSON comparison with HEAD confirms all non-shape values unchanged.

Deferred acceptance: device rendering, TalkBack/VoiceOver and native runtime require named
Android/iOS targets. JVM screenshots are screenshot-inspected/cross-viewport-inspected only.
The local mock remains a mock; no audio quality, durable state, release or production claim.
No CI wait. Commit/push/PR publication was requested separately after the radius verification;
this evidence records the implementation checks before that publication request.
