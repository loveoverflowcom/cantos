# Issue 145 — CMP motion policy and virtual-clock replay

[Issue 145](https://github.com/loveoverflowcom/cantos/issues/145) asks for one consistent way to
declare, inject, replay and prove motion and gesture behavior on CMP. This record covers its first
slice only. It adds a pure motion policy to the shared CMP package, adopts it in the two real
motion consumers, and adds a test-owned virtual-clock replay with labelled frame captures. It
qualifies no new API, observes no Android or iOS runtime, adds no recording, and changes neither the
canonical tokens, the preview registry's components nor any product default.

```text
Item / issue:        016 CMP UI-system sequence / #145 — [P1][CMP][Motion & Gesture Preview] Policy
                     injection, replay timeline, interruption and reduced-motion evidence
This PR delivers:    a pure intent × property × preference motion policy fed by the canonical
                     motion tokens (MotionPolicy.kt), an injected Full/Reduced preference path
                     (UiPreferences.motion), its adoption by the Auth action press and the Theatre
                     launch entrance, test-owned virtual-clock replay recipes with an independent
                     model, labelled frame captures with a frame sidecar, a diagnostics injection
                     point on the Theatre mock, and the motion/gesture skill and doc updates
It does not deliver: a gallery or workbench replay UI (#144), a mini ↔ full player, sheet, dialog,
                     drag, seek or play/pause consumer (none exists), gesture constants (no dragged
                     consumer), a System motion mode, Android/iOS reduce-motion adapters, any
                     runtime recording, Leptos mapping, new tokens or a spring variant
Acceptance covered:  see "Issue acceptance"; the remaining items stay with the follow-up order in
                     docs/work-plan/016-cmp-ui-system.md
Closes the issue:    no — "Part of #145"
Decision record:     none needed — the slice selects nothing the docs leave open. Moving the pinned
                     Material3 (1.8.2 has no MotionScheme; 1.9.0 has) is a dependency decision that
                     needs an ADR and is not taken here
```

## Inventory of motion consumers

Every call site that animates, as found by searching `apps/cantos-theatre/mobile` and
`packages/ui-system/cmp` for animation and gesture APIs before this change:

| Consumer | Before | After |
| --- | --- | --- |
| Auth action press (`AuthActionShape`, Button/BackButton auth variants) | direct `motion.shape.duration` read, default `tween` easing | spec from the policy: spatial feedback, same 220 ms and the same easing |
| Theatre launch entrance (`CantosLocalLaunch`) | one `Animatable`, `motion.emphasis.duration` with a hard-coded `CubicBezierEasing(.2f, 0f, 0f, 1f)` and a separate `reducedMotion` branch driving opacity and geometry alike | two `Animatable`s: opacity from the effects spec, travel/scale/rotation from the spatial spec; no literal easing, no separate flag |
| Theatre mock presentation (`CantosDemoPresentation`) | `reducedMotion = true` hard-coded | the same default (`Reduced`) as an injectable parameter for diagnostics hosts |
| CMP gallery | a manual "Reduced motion" chip | unchanged |
| play/pause morph, mini ↔ full player, sheet, dialog, seek thumb, drag | none exist | none exist; no sample is claimed |

## Invariant, owner and decomposition

Motion is specified by intent; the mapping from intent, property and a resolved preference to a
spec is one pure function. Reduced motion is the final state at once; effects never overshoot; a
component never reads the operating system and never delays an action for an animation.

| Responsibility | Owner after this change |
| --- | --- |
| durations, effects easing, reduced zero mapping | `shared/tokens.json` (unchanged) |
| intent × property × preference → spec, named spatial easing, bounds | `packages/ui-system/cmp/.../MotionPolicy.kt` (new, `commonMain`, no Android/iOS type) |
| the resolved preference value | `UiPreferences.reducedMotion`, projected read-only as `UiPreferences.motion`; the host resolves any OS signal |
| the Compose mapping | `MotionSpec.toAnimationSpec()`, `Animatable.animateWith`, `rememberMotionSpec` |
| replay recipes, clock, model, frame sidecar | `desktopTest` only (`MotionReplay.kt`, `CaptureRecord.kt`); production UI never advances a clock |
| diagnostics injection | `CantosDemoPresentation(motionPreference = …)`, default `Reduced` |
| method, labels, gesture-value contract | `.agents/skills/cantos-ui-design/references/motion.md` |

Imports still point inward: Theatre → shared CMP → tokens. No dependency, module, service, port,
DI framework or mock owner was added. `MotionPolicy.kt` imports only Compose animation core and the
generated tokens. The preview-registry gate scans capitalised public composables; the new
composable `rememberMotionSpec` is lower-case and is not a component.

## Issue acceptance

| Acceptance item | Status | Evidence |
| --- | --- | --- |
| pure mapping tests over intents × properties × preferences; no effects overshoot; reduced spatial instant; constants valid with units | met | `MotionPolicyTest` (13 tests): 3 × 2 × 2 exhaustive, role table, bound, easing validity, token drift guards |
| preserve `AuthActionMotionTest`; frame0/mid/end, reverse/cancel and reduced final frame for the declared sample; intent asserted before the clock advances; hit box unchanged | met for the Auth action and the launch entrance | `AuthActionMotionTest` unchanged and passing; `AuthShapeTimelineTest`, `AuthButtonReplayTest`, `LaunchMotionReplayTest` |
| same recipe gives the same end state; an interrupted interaction never locks input, no coroutine or owner leak | met | determinism tests; `leaving_composition_mid_animation_…`; `readiness_mid_entrance_leaves_no_animation_or_coroutine_pending`. Pixels are compared with a bounded tolerance because overlapping ripples are not bit-reproducible (finding 2); the exact shape is replayed bit-for-bit |
| play intent is not "engine started"; seeking to the end is not complete; loading is not success after an animation | not in this PR | no play/pause, seek-thumb or loading consumer exists; `PlaybackController` tests keep owning those semantics |
| captured frames carry t, timebase, state, token, source and constraints and are opened; a runtime Full/Reduced and reversal recording has a reviewer | partial | JVM frames with a `frame` sidecar, opened (below). **Runtime recording `NOT_RUN`** |
| JVM frame, Android layout preview, Android runtime and iOS runtime labelled separately; a `UiPreferences` toggle is not OS proof | met in method | `motion.md` § 6 labels; every sidecar disclaims `runtime-recording` and `os-reduce-motion` |
| launch sample matches readiness and lifecycle, does not reopen #133; iOS stays `NOT_RUN` | met | no timer, hold or wait added; `readiness_interrupts_entrance_on_the_next_frame` unchanged; iOS `NOT_RUN` |
| API choice follows the pinned CMP/Material3; no Android-only dependency in `commonMain`; no test clock or debug toolbar in release | met | Material3 1.8.2 has no `MotionScheme` (jar inspected); policy uses `tween`/`CubicBezierEasing`; the replay lives in `desktopTest`; both Android builds compile |
| motion/gesture and component-first instructions updated in the shared reference and the CMP skill | met | `motion.md`, `cantos-ui-design/SKILL.md`, `preview-catalog.md`, `cantos-cmp-mobile` `SKILL.md`, `cmp-design-system.md`, `cmp-testing.md` |

Scope items 3 (workbench controls), 5 (gestures) and the System half of 6 are not delivered; the
reasons are in the block above and the order for them is in `016-cmp-ui-system.md`.

## Ledger

| Claim | Failure mode | Cheapest oracle | Evidence level | Live / mocked |
| --- | --- | --- | --- | --- |
| reduced motion is `Instant` for every intent and property | a reduced user sees travel or a fade over time | `MotionPolicyTest.reduced_preference_…`; token `reduced_motion` zero check | example-tested (bounded exhaustive) | real tokens |
| effects use the canonical easing and cannot overshoot; durations come from the canonical roles, bounded | a swapped role or an overshooting curve ships | role table, `mayOvershoot`, bound and drift guards | example-tested | real tokens |
| the Auth press follows the declared duration and easing frame by frame, retargets from the current value on reversal and never leaves 0..1 | a wrong duration or easing, a jump on cancel, an overshoot | `AuthShapeTimelineTest` against an independent bisection model on a manual clock | example-tested, interaction-tested (JVM) | real shape and theme |
| the press dispatches on release before any frame, a cancelled press never dispatches, five fast taps dispatch five times, the hit box never moves | an intent waits for the animation, or a target moves under a finger | `AuthButtonReplayTest` with real touch dispatch | interaction-tested (JVM) | real Button |
| reduced and full motion settle on the same final frame | a final state differs by preference | byte-identical PNGs (`cmp-motion-auth-action-full-end` = `…reduced-end`; launch settled = reduced) and tests | screenshot-inspected, example-tested (JVM) | real renderer |
| the launch entrance fades its copy in (effects) and moves it with the spatial spec, converges on the reduced final frame by 260 ms and stays there | opacity and geometry share one easing; the entrance never settles | `LaunchMotionReplayTest` (blank → fading → drawn; per-frame copy position against the model) | interaction-tested (JVM) | real splash |
| readiness during the entrance leaves nothing pending | a leaked animation or coroutine | `waitForIdle` with the clock running reaches idle | interaction-tested (JVM) | real splash |
| the mock keeps its Reduced default and a host can inject Full | a silent product-default change | `MotionPreferenceInjectionTest` | example-tested | real presentation |
| a frame sidecar states recipe, mark, time, timebase and disclaims recordings and the OS signal | a manual-clock PNG is read as smoothness proof | `CaptureRecordTest` (two tests) | example-tested | real files |
| pre-existing captures are unchanged | the refactor changes shipped pixels | SHA-256 comparison against clean develop: 57 CMP PNGs and 16 Theatre splash PNGs identical | screenshot-captured, byte-compared | real renderer |
| Android library and Theatre Android host still compile | the shared source breaks a consumer | `ui-preview cmp android-build`, `theatre-demo android-build` | compiled | real build |

## Executed evidence

Environment: macOS 26.5.2 arm64, installed Gradle 8.14.3, Microsoft JDK 17.0.19, Rust 1.98.1,
Homebrew Python 3.14.8 shim for the repository gates, Kotlin/CMP/Material3 2.2.20/1.9.0/1.8.2.
Nothing was installed or provisioned. Measured on base `ff5a86aa4f1c3e67901026dfa31ed284493dd3b5`
(develop after #152) plus this branch. Production composables ran on JVM Desktop Compose, not an OS
window, emulator or phone. The CMP source digest bound by the six re-bound records is
`518bb41cb1b36633e6335358eef3e5cbc7795c4d5f1708053563465a4f3cd77b`; the token digest
`25e99c49faa15a9112cd03e7e92f21360b808bbe68f0a2531017d9a0e21f0741` is unchanged.

Every command ran from the worktree with `RUSTUP_TOOLCHAIN=1.98.1`, Gradle on `PATH`, `LC_ALL=C`
and `TMPDIR=/private/tmp` (plus `ANDROID_HOME` for the Android builds). Cached test results and
capture directories were deleted first so every task executed. Gates are reported as xtask printed
them.

| Exact command | Result | Scope |
| --- | --- | --- |
| `cargo xtask ui-preview cmp verify` | PASS | `desktopTest`: 189 tests in 29 classes, 0 failed, errors or skipped (clean develop: 160 in 26) |
| `cargo xtask theatre-demo verify` | PASS | `:shared:desktopTest`: 279 tests in 41 classes, 0 failed, errors or skipped (clean develop: 273 in 39) |
| `cargo xtask ui-preview cmp android-build` | PASS | `compileDebugKotlinAndroid` of the isolated Android library; compiled, not rendered |
| `cargo xtask theatre-demo android-build` | PASS | `:androidApp:assembleDebug`; compiled, not launched |
| `cargo xtask theatre-demo ios-build` | `NOT_RUN` | needs `--kotlin-native-home` with Kotlin/Native 2.2.20, which is absent on this host |
| `python3 scripts/ui_preview_catalog.py --check` (Python 3.14) | PASS | after the re-binding; see "Repository gates" |
| Mutation ledger (hand-run, not an xtask gate) | 24 of 25 killed, 1 documented survivor | below |

New tests: `MotionPolicyTest` 13, `AuthShapeTimelineTest` 8, `AuthButtonReplayTest` 6 and
`CaptureRecordTest` +2 in the CMP package (+29); `LaunchMotionReplayTest` 4 and
`MotionPreferenceInjectionTest` 2 in the Theatre mock (+6).

## Mutation ledger

Each row is one hand-written change to a production or capture source, run against the tests that
could see it (`MotionPolicyTest`, `AuthShapeTimelineTest`, `AuthButtonReplayTest`,
`AuthActionMotionTest` and `CaptureRecordTest` for CMP; the launch, presentation and injection
tests for Theatre), then restored from Git. A mutant is killed when at least one test fails. The
first batch (A1–A11, B1, C1, C2, D1, D2, E2–E6) ran before the launch entrance derived its start from
its spec and before the per-frame travel model existed; its mutants sit on lines that did not change
and the tests only grew. E3 survived that batch because both easings converge on the same end; the
travel model was added and E3 re-run. E1 (launch ignores reduced motion) was withdrawn when the
separate flag it mutated was removed, and E1b replaced it.

| Mutant | Change | Outcome | Failing tests |
| --- | --- | --- | --- |
| A1 | Reduced preference returns a timed spec | KILLED | `MotionPolicyTest.reduced_preference_is_the_final_state_at_once_for_every_intent_and_property` |
| A2 | effects use the spatial easing | KILLED | `MotionPolicyTest.full_effects_use_the_canonical_duration_and_easing_and_cannot_overshoot` |
| A3 | spatial easing overshoots | KILLED | `AuthShapeTimelineTest.reversing_mid_flight_retargets_from_the_current_value_and_lands_on_the_last_intent`<br>`AuthShapeTimelineTest.the_press_follows_the_declared_duration_and_easing_frame_by_frame_and_never_overshoots`<br>`AuthShapeTimelineTest.the_release_returns_with_the_same_spec_and_lands_on_rest` |
| A4 | spatial feedback reads the effects duration role | KILLED | `MotionPolicyTest.the_roles_the_policy_reads_differ_exactly_where_the_token_values_differ`<br>`MotionPolicyTest.full_spatial_uses_a_canonical_duration_and_the_named_spatial_easing`<br>`MotionPolicyTest.no_canonical_motion_duration_role_is_left_unread_by_the_policy` (+4 more) |
| A5 | navigation effects read the emphasis role | KILLED | `MotionPolicyTest.the_roles_the_policy_reads_differ_exactly_where_the_token_values_differ` |
| A6 | duration bound removed | KILLED | `MotionPolicyTest.a_duration_outside_the_bound_is_rejected_at_construction_and_by_copy` |
| A7 | easing x range check removed | KILLED | `MotionPolicyTest.easing_values_must_be_finite_and_a_function_of_time` |
| A8 | Instant maps to a 100 ms tween | KILLED | `MotionPolicyTest.the_compose_mapping_snaps_for_instant_and_tweens_the_exact_spec_otherwise`<br>`AuthShapeTimelineTest.reduced_motion_shows_the_final_value_at_once_with_no_intermediate_frame` |
| A9 | timed animateWith snaps | KILLED | `LaunchMotionReplayTest.full_motion_converges_on_the_reduced_final_frame_by_the_canonical_duration_and_stays_there` |
| A10 | rememberMotionSpec ignores the injected preference | KILLED | `AuthActionMotionTest.reduced_motion_retains_immediate_press_feedback_and_one_release_intent`<br>`AuthShapeTimelineTest.reduced_motion_shows_the_final_value_at_once_with_no_intermediate_frame` |
| A11 | easing parser swaps y1 and x2 | KILLED | `MotionPolicyTest.the_easing_token_parser_accepts_the_canonical_value_and_rejects_malformed_ones` |
| A12 | tween easing control points transposed | KILLED | `MotionPolicyTest.the_compose_mapping_snaps_for_instant_and_tweens_the_exact_spec_otherwise`<br>`AuthShapeTimelineTest.reversing_mid_flight_retargets_from_the_current_value_and_lands_on_the_last_intent`<br>`AuthShapeTimelineTest.pressing_again_before_the_reversal_settles_continues_without_a_jump_and_ends_pressed` (+2 more) |
| B1 | UiPreferences.motion inverted | KILLED | `MotionPolicyTest.preferences_project_the_resolved_motion_fact_and_copy_cannot_desynchronize_it`<br>`AuthActionMotionTest.reduced_motion_retains_immediate_press_feedback_and_one_release_intent`<br>`AuthActionMotionTest.interrupted_press_restores_capsule_without_dispatch_or_target_movement` (+7 more) |
| C1 | Auth shape uses the effects property | KILLED | `AuthActionMotionTest.interrupted_press_restores_capsule_without_dispatch_or_target_movement`<br>`AuthShapeTimelineTest.reversing_mid_flight_retargets_from_the_current_value_and_lands_on_the_last_intent`<br>`AuthShapeTimelineTest.pressing_again_before_the_reversal_settles_continues_without_a_jump_and_ends_pressed` (+3 more) |
| C2 | Auth shape ignores the spec | KILLED | `AuthActionMotionTest.reduced_motion_retains_immediate_press_feedback_and_one_release_intent`<br>`AuthShapeTimelineTest.reduced_motion_shows_the_final_value_at_once_with_no_intermediate_frame` |
| D1 | sidecar drops the frame | KILLED | `CaptureRecordTest.a_motion_frame_states_its_recipe_mark_time_and_timebase_and_disclaims_recordings_and_the_os_signal` |
| D2 | sidecar drops the recording and OS-signal disclaimers | KILLED | `CaptureRecordTest.a_motion_frame_states_its_recipe_mark_time_and_timebase_and_disclaims_recordings_and_the_os_signal` |
| E2 | launch opacity snaps | KILLED | `LaunchMotionReplayTest.full_motion_converges_on_the_reduced_final_frame_by_the_canonical_duration_and_stays_there` |
| E3 | launch travel follows the effects progress | KILLED | `LaunchMotionReplayTest.the_copy_travels_with_the_declared_spatial_duration_and_easing_frame_by_frame` |
| E4 | launch geometry never animates | KILLED | `LaunchMotionReplayTest.full_motion_converges_on_the_reduced_final_frame_by_the_canonical_duration_and_stays_there` |
| E5 | mock default becomes Full | KILLED | `MotionPreferenceInjectionTest.the_mock_defaults_to_reduced_motion` |
| E6 | mock ignores the injected preference | KILLED | `MotionPreferenceInjectionTest.a_host_can_inject_full_motion_without_changing_the_default` |
| E7 | launch geometry animates with the effects spec | KILLED | `LaunchMotionReplayTest.the_copy_travels_with_the_declared_spatial_duration_and_easing_frame_by_frame` |
| F1 | an Instant spec starts at 0 | KILLED | `MotionPolicyTest.an_entrance_starts_at_its_final_value_only_when_the_spec_is_instant` |
| E1b | launch effects entrance always starts at 0 | SURVIVED | — |

E1b is a true survivor of the JVM harness, not an equivalent mutant: setting the initial value to 0
for a reduced entrance would show one unfinished frame before the first coroutine turn, but
`setContent` runs effects before the first capture, so no test can see that frame. The property is
covered by the pure `MotionSpec.entranceStart` test (killed F1) and by reading the one line that
applies it. A real first-frame observation needs a harness that withholds effects, which is not
added here.

## Opened captures and findings

`AuthButtonReplayTest` wrote ten PNGs with sidecars to the ignored
`packages/ui-system/cmp/build/reports/cantos-preview/`; `LaunchMotionReplayTest` wrote four PNGs
with sidecars to `apps/cantos-theatre/mobile/shared/build/reports/theatre-demo/`. Ten of them were
opened from the final run after the last source edit (the other four are byte-identical twins of an opened frame). They are JVM Desktop Compose test-root renders
at 1 px per dp on a manual clock (`compose-test-manual-clock-16ms-frames`), not an OS window,
emulator, phone or recording. Each sidecar names recipe, mark, time, timebase, theme, density,
text scale, constraint, token version and token-file digest, and lists what it is not evidence of.
`source_revision` reads `unrecorded` because `CANTOS_SOURCE_REVISION` was not set; the source is the
branch named above.

| Capture (PNG SHA-256 prefix) | Constraint and inputs | Observed after opening |
| --- | --- | --- |
| `cmp-motion-auth-action-full-frame0` and `…-idle` (`f71554fd`, identical) | 300×60 dp, light, Auth primary, vi-VN label "Đăng nhập với Cantos ID", t = 0 ms | The resting capsule, no focus ring. The frame after the press and the resting frame are the same bytes, so nothing moves before the clock does |
| `cmp-motion-auth-action-full-mid` (`b509d5a3`) | same, t = 112 ms of press-hold | The fill's corner radius is partway between the capsule and the pressed radius, the state layer is lighter, and a lavender focus ring of pill shape surrounds it |
| `cmp-motion-auth-action-full-end` and `reduced-end` (`664ca39e`, identical) | t = 480 ms | The settled pressed shape: a clearly less round fill, lighter state layer, pill-shaped focus ring with a visible gap at each corner. Full and reduced motion settle on the same bytes |
| `cmp-motion-auth-action-full-released` (`282f84f7`) | t = 960 ms, after the release | The capsule again; the focus ring remains because the control kept focus |
| `cmp-motion-auth-action-reduced-mid` (`4b408ca8`) | same recipe, reduced, t = 112 ms | The pressed radius already shows, at the same moment the full-motion frame is half way |
| `splash-motion-full-frame0` (`efdd1cfa`) | 390×844 dp, light, launch entrance, t = 0 ms | The halo is smaller and rotated, the symbol is drawn, the wordmark and tagline are absent (opacity 0); the footer category line is present |
| `splash-motion-full-mid` (`8fc817aa`) | t = 144 ms, the first 16 ms frame at or after half of 260 ms | The halo is near its final size and angle, the wordmark and tagline are fading in and sit slightly lower than their final place |
| `splash-motion-full-settled` and `splash-motion-reduced-frame0` (`c346a5e6`, identical) | t = 368 ms (260 ms plus six frames), and the first reduced frame at t = 0 | The settled entrance and the reduced first frame are the same bytes: halo, symbol, wordmark, tagline and footer all in place |

Findings from opening them and from the mutation work, none fixed in this PR:

1. **The focus ring does not follow the morphing shape.** `cantosFocus` draws the ring with the fixed
   `shape.pill` radius, so while an Auth action is pressed and focused (a keyboard press, or a pointer
   press on desktop where the press moves focus) the fill morphs to a 16 dp radius inside a ring that
   stays pill-shaped; the mid and end frames show the gap. Making the ring follow the shape is a
   Button implementation and qualification change with its own review, so it is reported, not
   changed here.
2. **Overlapping Material ripples are not bit-reproducible under the manual clock.** Two replays of
   the five-fast-taps recipe differ by under 2% in whole-frame image distance, so
   `AuthButtonReplayTest` compares logs, intents and sample times exactly and frames within 5%. The
   exact shape is replayed bit-for-bit by `AuthShapeTimelineTest`.
3. **A painted-corner scanline is not a safe shape probe once the ring appears.** The existing
   `AuthActionMotionTest` keeps working because it compares only coarse points; the new tests read
   the shape outline itself and use a plain image distance for pixels.
4. **A fade that also travels is not monotone in an image distance while it is faint and offset.** The
   launch test therefore asserts convergence from the midpoint and reads the copy position directly
   for the early frames.
5. **Material3 1.9.0 ships `MotionScheme`.** It is absent from the pinned 1.8.2. Native Expressive
   springs are therefore a dependency decision, not a motion edit.

The launch entrance changed slightly under Full motion: geometry now eases with the spatial easing
where it previously shared the effects easing, and the copy's opacity keeps the effects easing. The
reduced first frame and the settled frame are byte-identical to develop's reduced capture, and all
16 pre-existing splash captures (reduced) are byte-identical to clean develop's.

## Qualification re-binding

Any `.kt` file under `packages/ui-system/cmp` is part of the CMP source digest, so the six
qualified CMP records went stale (`ui_preview_catalog.py --check` named exactly those six and the
two Membership files that use them). They were not re-bound to clear the gate: the reviewed claim
is that nothing a qualified component renders changed. Button and BackButton use the Auth pressed
shape whose source changed; Badge, Logo, Surface and TextField do not. The evidence is that all 57
CMP capture PNGs taken on clean develop are byte-identical after the change, including every
component and logo capture and the existing Auth motion frames, that the whole existing CMP suite
passes unchanged (including `AuthActionMotionTest`, which killed four of the mutants), and the
mutation ledger above. New records `docs/evidence/issue145-<id>-cmp.json` bind the new digest and the
catalog now points at them; the issue 143 records stay as history. No design reference was
re-opened, and the Leptos records are fresh and untouched.

## Repository gates

| Exact command | Result | Scope |
| --- | --- | --- |
| `python3 scripts/ui_preview_catalog.py --check` (Python 3.14 shim) | PASS | "two preview registries, named tokens and guarded adoption agree". Before the re-binding it failed on exactly the six CMP records and the two Membership files that use them; the Leptos records stayed fresh |
| `python3 scripts/ui_preview_catalog.py` | PASS | regenerated `PreviewCatalog.kt` and `preview_catalog.rs`; only their header digest changed (the catalog's evidence paths moved) |
| `cargo xtask preflight --changed-only` (Python 3.14 shim, `CARGO_INCREMENTAL=0`, Gradle and `ANDROID_HOME` set) | PASS | ten gates, none skipped: `repository` (712 text files and 19 skills, local links, 442 Python tests OK with 5 skipped), `rust-format`, `rust-lint` and `rust-tests` (cantos-studio, cantos-theatre, cantos-ui), `web-lint` (Studio, Theatre), `web-build` (Studio, Theatre), `ui-preview`, `cmp-ui-preview`, `theatre-cmp-demo` |

The Rust and Web lanes were selected because the shared catalog and its generated Rust registry
changed; this PR changes no Rust behavior. Inside the preflight, `cmp-ui-preview` and
`theatre-cmp-demo` reported `desktopTest` UP-TO-DATE: the registry regeneration after the last Kotlin
edit changed only a header comment, so the compiled classes were identical and Gradle reused the
results. The test counts above therefore come from the dedicated runs executed after the last
Kotlin edit, not from the preflight. The python suite's printed `behavior: FAIL` lines are the
expected output of its own negative tests; the suite and the gate are OK.

## Deferred acceptance and residual risk

| Check / layer and status | Reason | Prerequisites and exact next command | Acceptance criteria | Next owner / residual risk |
| --- | --- | --- | --- | --- |
| Android reduce-motion adapter and `android-runtime` observation of Full and Reduced with a reversal recording — `NOT_RUN` | outside this PR: no adapter exists and no device or emulator run was made | an emulator or device, `ANDROID_HOME`, a composition-root adapter reading the animation scale, a screen recording with build and device recorded | recording shows no travel, overshoot or loop under Reduce/animator scale 0; the same recipe in Full shows reversal | follow-up 1 in `016-cmp-ui-system.md`; JVM frames say nothing about frame pacing or the OS signal |
| iOS Reduce Motion adapter and `ios-runtime` observation — `UNSUPPORTED` | Kotlin/Native 2.2.20 and a simulator run are absent on this host | `cargo xtask theatre-demo ios-build --kotlin-native-home <abs path>` then a simulator or device run | same as above on iOS | same; no iOS claim exists |
| first transition or drag consumer with a preview scenario and named gesture values — not started | no mini ↔ full, sheet, dialog, drag, seek or play/pause consumer exists | the consumer's own work item | gesture values with units, cancel/Escape/Back/keyboard alternative, scroll not stolen, focus restored | follow-up 2; do not add a gesture abstraction first |
| motion timeline controls (replay, pause, seek-to-frame, restart) in the gallery — not started | the scenario workbench is #144 | #144's seam; the recipe model here is the input | controls drive a preview-owned clock, never production UI | follow-up 3 |
| System motion mode and the OS signal at composition roots — not started | needs the platform adapters above | the adapters | `System` follows only an injected host fact, as `ThemeMode` does | follow-up 1 |
| first-frame observation under Reduced — not observable | the JVM harness runs effects during `setContent` (E1b) | a harness that withholds effects | a test fails when the first reduced frame is unfinished | low risk; pure `entranceStart` test covers the rule |
| focus ring following the morphing shape — finding 1 | a Button change needs its own review and re-qualification | Button owner | the ring corner radius follows the shape fraction | #141/#143 component owners |
| Leptos mapping — not started | the Web has no motion consumer in this scope | generate from the same token source | both mappings from one source | follow-up 4 |

Production and phase-exit acceptance remain separate. The slice is reachable only through the
existing Auth action and launch entrance; both behave the same with Reduced motion, and the Theatre
mock keeps its Reduced default.
