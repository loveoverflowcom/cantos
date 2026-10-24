package com.cantos.ui

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.AnimationVector1D
import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.animation.core.FiniteAnimationSpec
import androidx.compose.animation.core.snap
import androidx.compose.animation.core.tween
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import com.cantos.ui.generated.CantosTokens

/** Why something moves. Names follow the canonical `motion.*` token intents; the mapping decides how. */
enum class MotionIntent { Feedback, Navigation, Emphasis }

/** Spatial properties (position, size, rotation, shape) may be expressive; effects (opacity, color) never overshoot. */
enum class MotionProperty { Spatial, Effects }

/**
 * The resolved fact. A composition root resolves the OS signal or a person's choice; components
 * only ever see this value, through [UiPreferences.motion], and never read the operating system.
 */
enum class MotionPreference { Full, Reduced }

/**
 * Cubic Bézier easing, control points as dimensionless fractions of progress. Both x values must lie
 * in 0..1 so the curve is a function of time.
 */
data class MotionEasing(val x1: Float, val y1: Float, val x2: Float, val y2: Float) {
    init {
        require(listOf(x1, y1, x2, y2).all { it.isFinite() }) { "Easing control points must be finite" }
        require(x1 in 0f..1f && x2 in 0f..1f) { "Easing x control points must be within 0..1" }
    }

    /** Conservative: a y control point outside 0..1 can drive the curve past its end value. */
    val mayOvershoot: Boolean get() = y1 !in 0f..1f || y2 !in 0f..1f
}

/** What one animated property does. There is no spring variant until a token source and a target observation exist. */
sealed interface MotionSpec {
    /** The final state at once: no duration, no travel, no overshoot. */
    data object Instant : MotionSpec

    /** [durationMillis] is whole milliseconds, taken from a canonical `motion.*.duration` token. */
    data class Timed(val durationMillis: Int, val easing: MotionEasing) : MotionSpec {
        init {
            require(durationMillis in 1..MaxMotionDurationMillis) {
                "Motion duration must be 1..$MaxMotionDurationMillis ms, got $durationMillis"
            }
        }
    }
}

/** Upper bound for any single transition, in milliseconds; the longest canonical token is 260. */
const val MaxMotionDurationMillis: Int = 1_000

/**
 * Easing of spatial changes, in dimensionless control points. This is Compose's `FastOutSlowIn`, the
 * easing every default `tween` used before this policy, named so adopting the policy is
 * behavior-preserving. The canonical tokens define no spatial easing or spring yet; a reviewed token
 * replaces this constant rather than a second constant being added.
 */
val SpatialStandardEasing: MotionEasing = MotionEasing(0.4f, 0f, 0.2f, 1f)

/**
 * Pure: the spec for one intent and property under one resolved preference. Reduced motion is the
 * final state at once for every intent and property, which the token source mirrors with zero
 * durations. Effects use the canonical effects easing and cannot overshoot.
 */
fun motionFor(intent: MotionIntent, property: MotionProperty, preference: MotionPreference): MotionSpec =
    when (preference) {
        MotionPreference.Reduced -> MotionSpec.Instant
        MotionPreference.Full -> MotionSpec.Timed(
            durationMillis = CantosTokens.number(motionDurationRole(intent, property)).toInt(),
            easing = when (property) {
                MotionProperty.Effects -> effectsEasing
                MotionProperty.Spatial -> SpatialStandardEasing
            },
        )
    }

/** One full turn of the pending ring. No canonical continuous-motion token exists yet; a reviewed token replaces this constant. */
const val PendingRingTurnMillis: Int = 1_000

/** Pure: milliseconds per turn of the pending ring, or null when reduced motion keeps it stationary. */
fun pendingRingTurnMillis(preference: MotionPreference): Int? = when (preference) {
    MotionPreference.Full -> PendingRingTurnMillis
    MotionPreference.Reduced -> null
}

/** The canonical duration role each combination reads; a spatial feedback change is the shape role. */
internal fun motionDurationRole(intent: MotionIntent, property: MotionProperty): String = when (property) {
    MotionProperty.Effects -> when (intent) {
        MotionIntent.Feedback -> "motion.feedback.duration"
        MotionIntent.Navigation -> "motion.navigation.duration"
        MotionIntent.Emphasis -> "motion.emphasis.duration"
    }
    MotionProperty.Spatial -> when (intent) {
        MotionIntent.Feedback -> "motion.shape.duration"
        MotionIntent.Navigation -> "motion.navigation.duration"
        MotionIntent.Emphasis -> "motion.emphasis.duration"
    }
}

private val effectsEasing: MotionEasing by lazy { parseEasingToken(CantosTokens.text("motion.effects.easing")) }

/** Parses the generated `cubicBezier` token value, for example `[0.2,0,0,1]`. */
internal fun parseEasingToken(raw: String): MotionEasing {
    val points = raw.trim().removeSurrounding("[", "]").split(',').map {
        requireNotNull(it.trim().toFloatOrNull()) { "Not a cubic-bezier token value: $raw" }
    }
    require(points.size == 4) { "A cubic-bezier token has four control points: $raw" }
    return MotionEasing(points[0], points[1], points[2], points[3])
}

/** The Compose mapping of a spec; [MotionSpec.Instant] has no duration at all. */
fun MotionSpec.toAnimationSpec(): FiniteAnimationSpec<Float> = when (this) {
    MotionSpec.Instant -> snap()
    is MotionSpec.Timed -> tween(durationMillis, easing = CubicBezierEasing(easing.x1, easing.y1, easing.x2, easing.y2))
}

/** Where an entrance starts: an [MotionSpec.Instant] spec starts at its final value, so the first frame is already final. */
fun MotionSpec.entranceStart(): Float = when (this) {
    MotionSpec.Instant -> 1f
    is MotionSpec.Timed -> 0f
}

/** [MotionSpec.Instant] snaps in place, so the final frame needs no coroutine turn; a timed spec animates. */
suspend fun Animatable<Float, AnimationVector1D>.animateWith(target: Float, spec: MotionSpec) {
    when (spec) {
        MotionSpec.Instant -> snapTo(target)
        is MotionSpec.Timed -> animateTo(target, spec.toAnimationSpec())
    }
}

/** The spec for the current injected [UiPreferences.motion]; the component never reads the OS. */
@Composable
fun rememberMotionSpec(intent: MotionIntent, property: MotionProperty): MotionSpec {
    val preference = LocalUiPreferences.current.motion
    return remember(intent, property, preference) { motionFor(intent, property, preference) }
}
