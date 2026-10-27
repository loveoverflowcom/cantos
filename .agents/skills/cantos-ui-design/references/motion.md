# Motion

> **Scope.** Add or change any animation, transition, gesture-driven movement or continuous
> indicator in Studio or Theatre on either renderer: intent tokens, springs, the rules that keep
> motion honest, reduced motion, Cantos motion patterns (play/pause, mini ↔ full player, seek,
> status updates) and the evidence that can prove any of it.

Requirements are owned by the [UI system](../../../../docs/design/ui-system.md#semantic-tokens)
(Motion row), [§ Accessibility and motion acceptance](../../../../docs/design/ui-system.md#accessibility-and-motion-acceptance)
and the Motion row of the [validation matrix](../../../../docs/design/ui-system.md#future-validation-matrix).
The canonical source supplies current duration/effects mappings; a proposed spring or gesture
mechanism needs a concrete consumer and target observations. A duration token alone does not
implement a spring or qualify motion.

## 1. Intent tokens

Motion is specified by **intent**, not by curve. Each token names why something moves; the
renderer mapping decides how.

| Token | Communicates | Cantos examples | Default mapping | Under reduced motion |
|---|---|---|---|---|
| `motion.feedback` | an input was received; a small state changed | play/pause morph, bookmark toggle, chip selection, saved indicator | fast spatial + fast effects | final state at once; effects-only change allowed |
| `motion.navigation` | where something came from or went | mini ↔ full player, inspector pane, sheets, route changes | default spatial + default effects | crossfade or instant swap; no travel |
| `motion.emphasis` | one important arrival on this screen | full-player transport settling on open, a resume prompt appearing | slow spatial, expressive scheme only in Theatre | no overshoot, no travel |
| `motion.reduced` | the substitution policy for every intent when the user asks for less motion | — | § 4 | — |

`motion.emphasis` is a budget: at most one emphasized arrival per screen, never in Studio's
editor, status, QC or publish surfaces.

## 2. Springs and schemes

Material 3 Expressive specifies motion as springs. **Spatial** springs animate position, size,
rotation and shape and may overshoot; **effects** springs animate color and opacity and never
overshoot. Each family has fast, default and slow speeds inside a **standard** scheme (calm,
little bounce) or an **expressive** scheme (visible overshoot).

| Surface | Scheme | Why |
|---|---|---|
| Studio, every region | standard | a reading and accountability tool; bounce reads as noise |
| Theatre library, detail, settings | standard | browsing and decisions |
| Theatre full-player transport, mini ↔ full transition | expressive spatial permitted | the Medium-High surface; still interruptible and reducible |

Two laws hold for every spec, whatever the values become: effects springs are critically damped
or more (damping ratio ≥ 1, so no overshoot); under reduced motion no spatial property travels.
Check them once on the token source so neither renderer can drift ([`tokens.md`](tokens.md)).

Renderer reality: Compose has spring specs (`spring`, `tween`). A Material motion scheme is
**not** available in the pinned CMP: `org.jetbrains.compose.material3:material3:1.8.2` (Compose
1.9.0) ships only its internal `MotionTokens`; `MotionScheme` is present in `material3` 1.9.0 and
the 1.12.0 alpha (inspected in the local Gradle cache) but not in the pin. CMP therefore maps
intent through the Cantos policy (§ 4) onto `tween` and `CubicBezierEasing`, a stable fallback that
is not the native Material Expressive spring. Moving the pin is a dependency decision (an ADR per
[`cmp-tooling.md`](../../cantos-cmp-mobile/references/cmp-tooling.md)), not a motion edit; re-check
the artifact when it moves. The Web has no native spring: CSS transitions cover effects and simple spatial
changes and retarget from the current value but drop velocity; CSS `linear()` can approximate a
spring curve; a small JavaScript animator is needed only where interruption must preserve
velocity, such as a dragged player sheet. Mechanics belong to the renderer skills.

## 3. Rules

| Rule | Failure mode | Good | Counterexample | Oracle |
|---|---|---|---|---|
| **Never delay an action** | the input feels ignored; rapid taps are lost | play/pause dispatches the command on press; the morph decorates a state already changed | starting playback in an animation-end callback; navigation waiting for an exit animation before loading | interaction test asserts the command and state immediately after input, before the clock advances |
| **Never move a target during interaction** | mis-taps; lost place while reading | status and download states reserve their space; time text uses tabular figures; the mini-player never appears under a finger mid-scroll | a job-status update reflowing the episode list while the pointer is down | driven test compares target bounds before and after an async update (proposed) |
| **Stay interruptible** | the user waits for animation or fights it | reversing the full-player expansion mid-flight retargets from the current value; a dragged sheet follows the finger and settles with its velocity | input locked until a transition ends | reverse at mid-flight under a controlled clock; assert final state equals the last intent |
| **State is never carried by motion alone** | a still frame or a screen reader misses the change | the final frame and the semantics show "Đã tạm dừng" / paused | a pulse as the only sign of a new QC finding | still capture plus semantics or DOM assertion of the end state |
| **Continuous motion only for ongoing activity** | distraction, battery drain, vestibular harm | a loading indicator while an episode opens; it stops when loaded, offscreen or backgrounded | an endless shimmer in Studio; a marquee title in the mini-player | recording shows the loop end; reduced-motion run shows none |
| **Static surfaces do not fake clickability** | people try to click explanations | press and hover feedback only on real controls | hover lift on an informational card | inspected hover/press states |

## 4. Reduced motion

| Platform | Signal | Note |
|---|---|---|
| Web | `prefers-reduced-motion: reduce` (CSS and `matchMedia` for script-driven animation) | emulate in browser developer tools for repeatable runs |
| Android | the system "remove animations" accessibility setting and animator duration scale | verify what the pinned Compose version honors by itself |
| iOS | Reduce Motion | reach it through a platform adapter |

CMP has no common reduced-motion API to rely on: expose a `MotionPreference` from a platform
adapter at the composition root and pass it down as a value. Never read the OS setting inside a
component.

The shared CMP theme already accepts a `reducedMotion` preference; its host owns how that value
is resolved. On Web, CSS may resolve media preferences at the scoped styling boundary; resolve
any script-driven gesture policy once at the host and inject values/intents. Do not add a DI
framework or component-owned service merely to pass preferences. A gesture has the same typed
intent as its button/keyboard alternative, with explicit cancellation, reversal, focus and input
semantics; no animation-end callback controls intent delivery.

Preview motion policy is explicit: System follows the real browser preference; Reduced may
further restrict it. A diagnostic force-full mode, if implemented, must be labelled, opt-in and
recorded, never a silent override of reduced motion. Use the same recipe and trusted input for
normal/reduced observations; replay/reset changes the host fixture, not a fake pressed class.
A dragged consumer needs named policy values with units and ownership for slop, axis, velocity
and snap thresholds, plus cancel/Escape/keyboard alternatives and no scroll-lock trap. Do not
add an animator, gesture abstraction or speculative sheet merely to demonstrate motion.

| Remove | Keep |
|---|---|
| travel, slide-ins, container transforms | the state change itself, immediately |
| overshoot and bounce | color and opacity changes (short or instant) |
| parallax and zooms | focus rings, pressed and selected states |
| shape-morph travel (swap to the final shape) | progress text and determinate progress updates |
| continuous loops and decorative indicators (replace with static indicator + text) | announcements of significant changes |

The mapping from intent and preference to a spec is a pure function: the functional core of
motion. On CMP it is implemented in `packages/ui-system/cmp/src/commonMain/kotlin/com/cantos/ui/MotionPolicy.kt`
(signatures only here):

```kotlin
enum class MotionIntent { Feedback, Navigation, Emphasis }
enum class MotionProperty { Spatial, Effects }
enum class MotionPreference { Full, Reduced }   // resolved by a host, carried as UiPreferences.motion

sealed interface MotionSpec {
    data object Instant : MotionSpec            // reduced: the final state at once
    data class Timed(val durationMillis: Int, val easing: MotionEasing) : MotionSpec
}

fun motionFor(intent: MotionIntent, property: MotionProperty, preference: MotionPreference): MotionSpec
```

| Rule of the mapping | Source |
|---|---|
| `Reduced` is `Instant` for every intent and property (no travel, no fade over time) | mirrors the zero `reduced_motion` durations in `tokens.json` |
| `Full` durations are whole milliseconds read from the canonical `motion.*.duration` roles; a spatial `Feedback` change reads `motion.shape.duration` | `tokens.json`; the table lives in `motionDurationRole` |
| effects use the canonical `motion.effects.easing`, whose y control points stay inside 0..1, so effects cannot overshoot | `tokens.json`; `MotionEasing.mayOvershoot` |
| spatial uses the named `SpatialStandardEasing` (Compose's `FastOutSlowIn`) until a reviewed spatial easing or spring token exists | a named constant, replaced by a token, never duplicated |
| there is **no spring variant**: add one only with a token source and an observation on the target | § 2 |

`MotionPolicyTest` is the bounded exhaustive test (3 intents × 2 properties × 2 preferences) and
guards drift: it fails when a canonical `motion.*.duration` role is not read by the policy, when a
role is mis-wired, or when a duration leaves its bound. The exhaustive `when` is the type-level
half of the proof: adding an intent, property or preference fails compilation until the mapping
handles it. A component calls `rememberMotionSpec(intent, property)`, which reads the injected
`UiPreferences.motion`; the Compose mapping is `MotionSpec.toAnimationSpec()` (`snap()` or
`tween`) and `Animatable.animateWith(target, spec)` (`snapTo` for `Instant`, so the final frame
needs no coroutine turn).

The Leptos renderer does not implement this mapping yet. When it does, generate both tables from
the token source rather than hand-maintaining two.

### Gesture policy values (planned, not implemented)

No CMP consumer drags today, so the package defines no gesture constants and must not gain a
gesture abstraction to demonstrate motion. The first dragged consumer (for example the mini ↔
full player) declares, next to its component, named values with units and an owner, never literals
in a pointer handler:

| Value | Unit | Purpose |
|---|---|---|
| touch slop | dp | movement before a drag claims the pointer |
| axis lock | ratio of the dominant axis | a vertical drag beside a scrolling list must not steal the scroll |
| fling velocity | dp/s | release velocity that completes instead of snapping back |
| snap threshold | fraction of travel (0..1) | where an unreleased drag settles |

with cancel, Escape and Back as explicit alternatives, the same typed intent as the button, focus
restored to the control that opened it, and no scroll-lock trap.

## 5. Cantos motion patterns

| Pattern | Full motion | Reduced motion | Never |
|---|---|---|---|
| **Play / pause** (feedback) | command on press; primary control morphs shape (fast spatial) while the icon crossfades (fast effects); the accessible name switches immediately (`Phát` ↔ `Tạm dừng`) | instant shape and icon swap | waiting for the morph; showing "playing" for buffering — intent and actual state differ ([`playback-semantics.md`](../../cantos-listening/references/playback-semantics.md)) |
| **Mini ↔ full player** (navigation) | the mini-player expands into the full player as one container, content crossfades; drag down collapses, following the finger; Back or Escape collapses; focus moves to the full-player heading and returns to the expand control on collapse | crossfade or instant swap, same focus behavior | re-creating the media session or audio element; moving the play control under a pressed finger; blocking input until settled |
| **Seek** | none while dragging: the thumb follows the pointer and the elapsed text updates; no spring-back on release | identical | animating the thumb with overshoot after a keyboard step |
| **Episode and download state updates** | effects-only color and icon change in reserved space | instant | reordering rows under the user; growing a row when "Đang tải xuống 42%" appears |
| **Studio job status** | effects-only change; determinate progress updates; announcement handled separately ([`accessibility.md`](accessibility.md#6-announcements-without-spam)) | instant | minute-long spinners; bouncing badges; motion inside the script text |
| **Loading and pending indicator** (feedback) | one shared `LoadingIndicator` arc per renderer (Leptos CSS, Compose `LoadingIndicator`) in buttons, selects and `LoadingState`; it turns once per 1000 ms only while mounted/composed for ongoing work; the localized label or reason is the status | the same arc, stationary and visible; CSS `animation: none`, and Compose composes no infinite transition | a second loader in the same region; a percentage or check for unknown progress; `expand_more` as a loader; a shimmer in a Studio reading area; a ticker under reduced motion |
| **Studio inspector pane** | navigation intent; the editor keeps its scroll anchor, selection and caret while the column resizes | instant | reflowing lines under the caret without restoring position |
| **Dialogs and sheets** | scrim fades (effects); sheet enters spatially | fade or instant | moving the confirm button after the dialog opens |

## 6. Evidence

A still image cannot demonstrate smoothness, interruption or reduced motion. Split the claim:

Name the component ID/variant and scenario ID in the admission/PR evidence. Record the same
fixture, theme, font readiness/scale, viewport/density/DPR, preference and target for full/reduced
runs. Include specimen frame width separately from actual browser viewport, plus source and
token/catalog digests. Follow the
[preview replay method](preview-catalog.md#replay-a-preview-scenario) for reset and same-scenario
recheck. Preview controls and static values are useful diagnostics, not motion observations.
For an affected transition, retain labelled beginning/middle/end frames and the cancellation or
reversal sequence in a recording or frame profile. A settled capture alone proves no movement,
timing, smoothness or cleanup. Observe visibility/background/offscreen pause and listener/observer
cleanup only when the consumer implements a loop or such resources; never infer them from CSS.

| Part of the claim | Evidence | Level |
|---|---|---|
| the right spec for each intent and preference | the exhaustive test in § 4 | `example-tested` (bounded exhaustive) |
| the command commits before animation; reversal lands on the last intent | controlled-clock UI test: Compose test clock with auto-advance off; on the Web, assert the state change synchronously after the event | `interaction-tested` |
| it looks and feels right on the target | screen recording at a known frame rate plus frame-timing data from the platform profiler (browser performance panel; Android and iOS frame and hitch tooling — candidates), with device or browser, build and scenario recorded | `interaction-tested` or `device-tested`; the recording is provenance |
| reduced motion works | the **same** scenario rerun with the OS or browser setting on; the recording shows no travel, overshoot or loops | as above |

Name the target of every motion observation with one of these labels; they never substitute for
one another:

| Label | What it is | What it can show | What it cannot |
|---|---|---|---|
| `jvm-frame` | JVM Desktop Compose on a manual clock, frames the test chose | ordering, retargeting, intent-before-clock, final state, no leaked animation | frame pacing, smoothness, any OS signal, Android, iOS |
| `android-layout-preview` | Android Studio layout renderer (static) | one static frame and its semantics | any transition or interaction |
| `android-runtime` | emulator or device, with the real OS reduce-motion/animation-scale setting | the above on that target, with a recording as provenance | other hardware, iOS |
| `ios-runtime` | simulator or device with Reduce Motion | the above on that target | Android; **not run on Linux hosts** |

A `UiPreferences(reducedMotion = …)` toggle, a gallery chip or a diagnostic host parameter proves
that the fixture reaches the component; it is never evidence that the OS Reduce Motion signal
does. State `NOT_RUN` or `UNSUPPORTED` for a target that was not observed.

## 7. Replay recipes on a virtual clock

A motion sample is declared as a **recipe**: ordered inputs (down, up, cancel) and observation
marks at virtual times, replayed on a manual clock owned by the test or fixture. Production UI
never advances that clock. Replaying the same recipe must give the same log and end state.

| Rule | Why |
|---|---|
| assert the dispatched intent before the clock advances any frame | the intent must not wait for the animation |
| assert the hit box on every sampled frame | no target moves under a finger |
| compare each frame of the animated property with an independent model of the declared duration and easing | a wrong duration or easing is invisible to "it ended in the right place" |
| reverse, cancel, press again before settled, and tap fast; assert the last intent wins and values stay in range | interruption must never lock input or overshoot |
| leave composition mid-animation, then let the clock run to idle | a leaked animation or coroutine never reaches idle |
| observe the animated property itself where a pixel metric is confounded by ripple, state layer or focus ring | pixels there prove only that frames differ |
| label every frame with recipe, mark, time, timebase and what it is not evidence of | a frame is provenance, not a verdict |

The CMP implementation is `MotionReplay.kt` with `AuthShapeTimelineTest` (exact outline against
the model) and `AuthButtonReplayTest` (real touch, intents, hit box, captures) in
`packages/ui-system/cmp`, and `LaunchMotionReplayTest` in the Theatre mock. A Compose
frame needs one frame to admit a state change and the next to start the animation clock: derive
the animation start from the observed frames instead of assuming it.

There is no "motion verified" level. A recording on one device says nothing about slower
hardware; a desktop CMP run says nothing about Android or iOS. Name what was observed and put the
rest on the residual-risk line.
