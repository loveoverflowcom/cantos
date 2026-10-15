package com.cantos.ui

import kotlin.math.abs
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import org.junit.Test

/**
 * The production Auth action under real touch dispatch and a manual clock: intents, hit box and a
 * plain image distance per frame, with labelled frame captures. The shape itself is asserted on its
 * exact outline in [AuthShapeTimelineTest]; pixels here are confounded by the ripple and focus ring.
 * The existing [AuthActionMotionTest] is kept as it was. JVM Desktop Compose only: no recording, OS
 * reduce-motion signal, Android or iOS behavior.
 */
class AuthButtonReplayTest {
    /** Frames plus named marks in time order; at equal times the frame comes first (the sort is stable). */
    private fun inOrder(frames: List<MotionStep>, vararg named: MotionStep): List<MotionStep> =
        (frames + named).sortedBy { it.atMillis }

    private val pressHold = MotionRecipe(
        "press-hold",
        listOf(MotionStep.Mark(0, "idle", capture = true), MotionStep.Input(0, MotionInput.Down), MotionStep.Mark(0, "frame0", capture = true)) +
            inOrder(everyFrame(16, 480), MotionStep.Mark(112, "mid", capture = true), MotionStep.Mark(480, "end", capture = true)) +
            MotionStep.Input(480, MotionInput.Up) +
            inOrder(everyFrame(496, 960), MotionStep.Mark(960, "released", capture = true)),
    )

    private val releaseIntent = MotionRecipe(
        "release-intent",
        listOf(MotionStep.Input(0, MotionInput.Down), MotionStep.Mark(0, "pressed-before-release")) +
            everyFrame(16, 480) +
            // The mark right after the release runs before the clock advances any frame.
            listOf(MotionStep.Input(480, MotionInput.Up), MotionStep.Mark(480, "released-immediately")) +
            everyFrame(496, 704),
    )

    private val cancelled = MotionRecipe(
        "press-cancel",
        listOf(MotionStep.Input(0, MotionInput.Down)) + everyFrame(16, 96) +
            listOf(MotionStep.Input(96, MotionInput.Cancel), MotionStep.Mark(96, "cancelled-immediately")) + everyFrame(112, 640),
    )

    private val fastTaps = MotionRecipe(
        "five-fast-taps",
        listOf(MotionStep.Mark(0, "idle")) + (0 until 5).flatMap { tap ->
            val start = tap * 64L
            listOf(
                MotionStep.Input(start, MotionInput.Down),
                MotionStep.Input(start + 16, MotionInput.Up),
                MotionStep.Mark(start + 16, "tap${tap + 1}"),
            )
        } + everyFrame(336, 960),
    )

    private fun intent(run: Pair<MotionReplayResult, List<Int>>, mark: String): Int =
        run.second[run.first.samples.indexOfLast { it.name == mark }]

    @Test
    fun frame0_is_the_rest_frame_and_mid_and_end_are_distinct_labelled_frames() {
        val result = replayAuthButton(pressHold, reducedMotion = false, captureName = "auth-action").first
        assertEquals(0.0, result.at("idle"))
        assertEquals(0.0, result.at("frame0"), "the first frame after the press must not change before the clock advances")
        val distances = "mid=${result.at("mid")} end=${result.at("end")} released=${result.at("released")}"
        assertTrue(result.at("mid") > 0.0, "mid must differ from rest: $distances")
        assertTrue(result.at("end") > 0.0, "end must differ from rest: $distances")
        assertTrue(result.at("mid") != result.at("end"), "mid must not be the settled frame: $distances")
    }

    @Test
    fun release_dispatches_exactly_one_intent_before_the_clock_advances_a_frame() {
        val run = replayAuthButton(releaseIntent, reducedMotion = false)
        assertEquals(0, intent(run, "pressed-before-release"))
        assertEquals(0, intent(run, "t480"), "a held press must not dispatch")
        assertEquals(1, intent(run, "released-immediately"), "the intent must not wait for the animation or the next frame")
        assertEquals(1, intent(run, "t704"), "the release must dispatch exactly once")
    }

    @Test
    fun a_cancelled_press_never_dispatches() {
        val run = replayAuthButton(cancelled, reducedMotion = false)
        assertTrue(run.second.all { it == 0 }, "a cancelled press dispatched")
        assertTrue(run.first.frames().first { it.atMillis == 96L }.value > 0.0, "the press must have changed the frame before it was cancelled")
    }

    @Test
    fun five_fast_taps_each_dispatch_immediately_without_waiting_for_the_animation() {
        val run = replayAuthButton(fastTaps, reducedMotion = false)
        for (tap in 1..5) assertEquals(tap, intent(run, "tap$tap"), "tap $tap was delayed or lost")
        assertEquals(5, run.second.last(), "no tap may dispatch twice")
    }

    @Test
    fun reduced_motion_dispatches_once_and_settles_on_the_same_frame_as_full_motion() {
        val reduced = replayAuthButton(pressHold, reducedMotion = true, captureName = "auth-action")
        val full = replayAuthButton(pressHold, reducedMotion = false)
        assertTrue(reduced.first.at("end") > 0.0)
        assertEquals(full.first.at("end"), reduced.first.at("end"), "reduced and full motion must settle on the same pressed frame")
        assertEquals(1, reduced.second.last(), "the release must dispatch exactly once")
    }

    @Test
    fun replaying_the_same_recipe_gives_the_same_log_intents_and_sample_times_and_near_equal_frames() {
        for (recipe in listOf(releaseIntent, cancelled, fastTaps)) {
            for (reduced in listOf(false, true)) {
                val first = replayAuthButton(recipe, reduced)
                val second = replayAuthButton(recipe, reduced)
                val label = "${recipe.id} reduced=$reduced"
                assertEquals(first.first.log, second.first.log, label)
                assertEquals(first.second, second.second, label)
                assertEquals(first.first.samples.map { it.name to it.atMillis }, second.first.samples.map { it.name to it.atMillis }, label)
                // Overlapping Material ripples are not bit-reproducible under the manual clock (observed
                // below 2% between runs), so frames are compared with a bounded tolerance. The exact
                // shape is replayed bit-for-bit in AuthShapeTimelineTest.
                first.first.samples.zip(second.first.samples).forEach { (a, b) ->
                    assertTrue(abs(a.value - b.value) <= 0.05 * maxOf(a.value, b.value) + 1.0, "$label ${a.name}: ${a.value} vs ${b.value}")
                }
            }
        }
    }
}
