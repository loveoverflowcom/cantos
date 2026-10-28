# SurfacePulse and the Loading v1 token foundation (#585)

Source: `develop` at `623bb79ea65c9e86c174ae2777e23ccda0343c02` plus this PR. Work item 1 of 4 of Loading v1
(Pulse · Arc · Line). It adds the canonical continuous-loading tokens for all three primitives and ships
only the **SurfacePulse** primitive in both renderers; the arc ([#586](https://github.com/loveoverflowcom/cantos/issues/586)),
the edge line ([#587](https://github.com/loveoverflowcom/cantos/issues/587)) and application adoption
([#588](https://github.com/loveoverflowcom/cantos/issues/588)) stay separate. No application imports `SurfacePulse`.

## Contract

| Part | Leptos (`cantos-ui`) | CMP (`com.cantos.ui`) |
| --- | --- | --- |
| Primitive | `SurfacePulse { active, class }` → decorative `span.cui-surface-pulse` | `SurfacePulse(modifier, active, shape, color)` |
| Geometry | the caller's box and radius (the fill takes its parent's) | `modifier` and `shape` |
| Full motion | opacity keyframes 0.60 → 0.85 → 0.60 over `--cantos-motion-loading-pulse-cycle` (1400 ms), `--cantos-motion-loading-pulse-easing` | layer alpha, `RepeatMode.Reverse`, two 700 ms halves, `surfacePulseFor` |
| Reduced / inactive | `animation: none`, static opacity 0.70 | no infinite transition composed, `SurfacePulseRestingAlpha` 0.70 |
| Lifecycle | `data-pulse="idle"`, unmount or `display: none` ends the animation | `active = false` or leaving composition ends it; alpha is read in the layer block |
| Semantics | `aria-hidden`; the host names the region (`role="group"`, `aria-busy`) | cleared semantics; the host names the region once |
| Reveal | `--cantos-motion-loading-reveal-duration` 150 ms (0 under reduced) | `loadingRevealFor` (`Instant` under reduced) |

New canonical roles (tokens v0.10.0): `motion.loadingPulse|Arc|Edge.cycle` (1400/1000/1200),
`motion.loadingPulse.easing`, `motion.loadingReveal.duration` (150), `state.loadingPulse{Low,High,Reduced}Opacity`
(0.60/0.85/0.70), `size.loadingArc{Small,Regular}` (16/20) and `size.loadingEdge` (2). A `.cycle` is a duration with no
reduced-motion mapping (the generator refuses a zero cycle and a reduced entry for one); `motion.feedback.duration`
(140 ms) and CMP's finite `MaxMotionDurationMillis` (1000 ms) are unchanged. The shipped arcs still use
`--cantos-web-loading-turn` / `PendingRingTurnMillis`; a CMP test pins the latter to the Arc cycle.

## Evidence (this PR, local, 28 October 2026)

| AC | Check | Result | Level / scope |
| --- | --- | --- | --- |
| AC-01 | `python3 scripts/ui_tokens.py --check`; `PYTHONPATH=scripts python3 -m unittest scripts/test_ui_tokens.py` (47, new cycle/reduced/drift test) | PASS | generated CSS, Rust and Kotlin agree with the canonical source |
| AC-01 | `MotionPolicyTest` (common): cycles, pulse spec, reveal, finite bound and 140 ms unchanged | PASS | pure core |
| AC-02, AC-03 | `cargo test --locked -p cantos-ui` (`layout_feedback.rs`: one decorative fill, opacity-only keyframes, allowlisted endless loops with reduced-motion removal; the old "one infinite animation" assertion is intentionally replaced by that allowlist) | PASS | source-structural over the shipped CSS |
| AC-02, AC-03 | `SurfacePulseTest` (JVM manual clock): beat over 1400 ms and back to rest, one frame under reduced and while inactive, no semantics, leaves no loop | PASS | JVM Desktop Compose only |
| AC-04 | `SurfacePulseSpecimenTest` (geometry identical for running/idle/revealed at ×1 and ×2 text scale, 150 ms crossfade, instant under reduced) and a browser measurement of the Leptos specimen (below) | PASS | JVM + Chromium preview |
| AC-05 | `gallery.rs` specimen test (one `role="group"`, `aria-busy` while loading, every fill `aria-hidden`, no `status`/`alert`); `SurfacePulseSpecimenTest` (one named node while loading, placeholders and unrevealed text absent from the merged tree) | PASS | source/semantics tree; no screen reader |
| AC-06 | `cargo xtask ui-preview leptos verify`, `cargo xtask ui-preview cmp verify` | PASS | existing previews; catalog entry `surface-pulse` registered, CMP `implemented-unverified` |
| AC-07 | all of the above; JVM clock tests use no static PNG as motion evidence | PASS | see gaps |

Browser observation (in-app Chromium, Leptos preview at `127.0.0.1:8083`, `theme=dark`, no OS reduced motion): computed style of a
running fill was `animation-name: cui-surface-pulse`, `1.4s`, `cubic-bezier(0.42, 0, 0.58, 1)`, `infinite`, opacity between 0.6 and 0.85
(sampled 0.7745); the fill colour was `outlineVariant`. An idle fill had `animation: none`, opacity 0.7. Specimen rows measured
`107/78/40/107` px for running, idle and revealed alike, `107/106/40/107` at a 375 px viewport and exactly 2× at a 200 % root font size,
with no clipped text and no overflow inside the specimen. Screenshots were opened for Light (running) and Dark (running and revealed); they are not committed.

## Not covered

- Light/Dark × VI/EN × narrow/wide × font scale × Full/Reduced was not exercised as a full matrix: the specimen's fixed Vietnamese and English
  rows were opened in Light and Dark at a wide and a 375 px viewport and at 200 % root font size by measurement, not every combination by
  screenshot. **`prefers-reduced-motion` was not observed in a browser** (the pane cannot force it): the Web reduced-motion
  rule is verified in the built stylesheet and source tests only.
- No recording or frame profile of the Web pulse; CMP motion is a manual clock over the JVM, not Android/iOS or a screen reader (TalkBack, VoiceOver, NVDA).
- Contrast of the fill is not measured (decorative, non-text): `outlineVariant` at 0.60–0.85 alpha is a design seed, not a token contract.
- No per-component qualification record is added: `surface-pulse` has none and nothing may import it before both renderers' PASS
  records exist. The repository's existing `scripts/ui_preview_catalog.py --check` failure (stale Leptos qualifications) is **pre-existing at the base**
  and unchanged by this PR (identical output on a clean `develop` tree).
- The design-v3 inventory (102 specimens, 32 baseline APIs) is immutable; `SurfacePulse` is the 33rd registered API with no design specimen yet.
