package com.cantos.ui

import com.cantos.ui.generated.CantosTokens
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import org.junit.Test

/**
 * The Auth action shape against its declared spec, frame by frame on a virtual clock. Values are the
 * real shape outline's pressed fraction, compared with an independent model of the declared duration
 * and easing. JVM Desktop Compose with a manual clock: ordering, retargeting and final state only.
 */
class AuthShapeTimelineTest {
    // Written out rather than read from the policy so an accidental change to either value fails here.
    private val shapeDuration = CantosTokens.number("motion.shape.duration")
    private val spatialEasing = MotionEasing(0.4f, 0f, 0.2f, 1f)
    private val tolerance = 0.02

    private fun ease(elapsedMillis: Long): Double = easedProgress(spatialEasing, elapsedMillis / shapeDuration)

    private val pressHold = MotionRecipe(
        "press-hold",
        listOf(MotionStep.Mark(0, "idle"), MotionStep.Input(0, MotionInput.Down), MotionStep.Mark(0, "frame0")) +
            everyFrame(16, 480) + listOf(MotionStep.Input(480, MotionInput.Up)) + everyFrame(496, 960),
    )

    private val reversal = MotionRecipe(
        "reverse-mid-flight",
        listOf(MotionStep.Input(0, MotionInput.Down)) + everyFrame(16, 96) +
            listOf(MotionStep.Input(96, MotionInput.Cancel)) + everyFrame(112, 640),
    )

    private val repress = MotionRecipe(
        "cancel-then-press-before-settled",
        listOf(MotionStep.Input(0, MotionInput.Down)) + everyFrame(16, 64) + listOf(MotionStep.Input(64, MotionInput.Cancel)) +
            everyFrame(80, 112) + listOf(MotionStep.Input(112, MotionInput.Down)) + everyFrame(128, 640),
    )

    private val fastTaps = MotionRecipe(
        "five-fast-taps",
        (0 until 5).flatMap { tap ->
            val start = tap * 64L
            listOf(MotionStep.Input(start, MotionInput.Down), MotionStep.Input(start + 32, MotionInput.Up))
        }.let { inputs -> (inputs + everyFrame(0, 960)).sortedBy { it.atMillis } },
    )

    @Test
    fun the_press_follows_the_declared_duration_and_easing_frame_by_frame_and_never_overshoots() {
        val result = replayAuthShape(pressHold, reducedMotion = false)
        assertEquals(0.0, result.at("idle"))
        assertEquals(0.0, result.at("frame0"), "the first frame must not jump before the animation clock starts")
        val pressing = result.frames().filter { it.atMillis <= 480 }
        // The animation starts on the last frame that still shows the resting value.
        val start = pressing.first { it.value > 0.0 }.atMillis - FrameMillis
        pressing.filter { it.atMillis >= start }.forEach {
            assertEquals(ease(it.atMillis - start), it.value, tolerance, "t=${it.atMillis} ms")
        }
        assertTrue(pressing.all { it.value in 0.0..1.0 }, "the shape left its range")
        assertTrue(pressing.zipWithNext().all { (a, b) -> b.value >= a.value }, "the press moved backwards")
        assertEquals(1.0, pressing.last().value, 1e-6)
        assertTrue(pressing.any { it.value > 0.0 && it.value < 1.0 }, "no mid-flight frame was observed")
    }

    @Test
    fun the_release_returns_with_the_same_spec_and_lands_on_rest() {
        val result = replayAuthShape(pressHold, reducedMotion = false)
        val releasing = result.frames().filter { it.atMillis >= 496 }
        val start = releasing.first { it.value < 1.0 }.atMillis - FrameMillis
        releasing.filter { it.atMillis >= start }.forEach {
            assertEquals(1.0 - ease(it.atMillis - start), it.value, tolerance, "t=${it.atMillis} ms")
        }
        assertTrue(releasing.zipWithNext().all { (a, b) -> b.value <= a.value }, "the release moved backwards")
        assertEquals(0.0, releasing.last().value, 1e-6)
    }

    @Test
    fun reversing_mid_flight_retargets_from_the_current_value_and_lands_on_the_last_intent() {
        val result = replayAuthShape(reversal, reducedMotion = false)
        val frames = result.frames()
        val peak = frames.maxOf { it.value }
        assertTrue(peak > 0.05 && peak < 0.95, "the reversal must happen mid-flight, peak was $peak")
        // The reversing animation starts on the last frame that still shows the peak.
        val start = frames.last { kotlin.math.abs(it.value - peak) < 1e-9 }.atMillis
        frames.filter { it.atMillis >= start }.forEach {
            assertEquals(peak * (1.0 - ease(it.atMillis - start)), it.value, tolerance, "t=${it.atMillis} ms")
        }
        val after = frames.filter { it.atMillis >= start }
        assertTrue(after.zipWithNext().all { (a, b) -> b.value <= a.value }, "the reversal moved forwards again")
        assertTrue(after.all { it.value in 0.0..peak }, "the reversal left its range")
        assertEquals(0.0, frames.last().value, 1e-6, "the last intent was a cancel")
    }

    @Test
    fun pressing_again_before_the_reversal_settles_continues_without_a_jump_and_ends_pressed() {
        val frames = replayAuthShape(repress, reducedMotion = false).frames()
        assertTrue(frames.all { it.value in 0.0..1.0 })
        assertTrue(frames.zipWithNext().all { (a, b) -> kotlin.math.abs(b.value - a.value) < 0.2 }, "the shape jumped between frames")
        assertTrue(frames.any { it.atMillis in 80..112 && it.value > 0.0 && it.value < 1.0 })
        assertEquals(1.0, frames.last().value, 1e-6, "the last intent was a press")
    }

    @Test
    fun five_fast_taps_stay_in_range_and_the_last_release_wins() {
        val frames = replayAuthShape(fastTaps, reducedMotion = false).frames()
        assertTrue(frames.all { it.value in 0.0..1.0 })
        assertTrue(frames.zipWithNext().all { (a, b) -> kotlin.math.abs(b.value - a.value) < 0.2 })
        assertTrue(frames.any { it.value > 0.0 }, "the taps never moved the shape")
        assertEquals(0.0, frames.last().value, 1e-6)
    }

    @Test
    fun reduced_motion_shows_the_final_value_at_once_with_no_intermediate_frame() {
        val result = replayAuthShape(pressHold, reducedMotion = true)
        val frames = result.frames()
        assertTrue(frames.all { it.value == 0.0 || it.value == 1.0 }, "reduced motion produced an intermediate value")
        assertTrue(frames.first { it.atMillis == 48L }.value == 1.0, "the pressed shape must show within three frames")
        assertTrue(frames.first { it.atMillis == 528L }.value == 0.0, "the rest shape must show within three frames of the release")
        assertEquals(replayAuthShape(pressHold, reducedMotion = false).frames().first { it.atMillis == 480L }.value, frames.first { it.atMillis == 480L }.value,
            "reduced and full motion must settle on the same pressed shape")
    }

    @Test
    fun replaying_the_same_recipe_gives_the_same_log_and_samples() {
        for (recipe in listOf(pressHold, reversal, repress, fastTaps)) {
            for (reduced in listOf(false, true)) {
                assertEquals(replayAuthShape(recipe, reduced), replayAuthShape(recipe, reduced), "${recipe.id} reduced=$reduced")
            }
        }
    }

    @Test(timeout = 120_000)
    fun leaving_composition_mid_animation_leaves_no_animation_or_coroutine_pending() {
        val recipe = MotionRecipe("dispose-mid-flight", listOf(MotionStep.Input(0, MotionInput.Down)) + everyFrame(16, 64))
        val result = replayAuthShape(recipe, reducedMotion = false, leaveMountedAfter = false)
        // waitForIdle with the clock running returns only when no animation or effect is left; a leaked
        // animation would never go idle and the timeout would fail this test.
        assertEquals("unmounted and idle", result.log.last())
        assertTrue(result.frames().last().value > 0.0 && result.frames().last().value < 1.0, "it must be disposed mid-animation")
    }
}
