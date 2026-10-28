package com.cantos.ui

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.ComposeUiTest
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import org.junit.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

/**
 * Issue #585 on the production SurfacePulse, JVM Desktop Compose over a manual clock only: the fill
 * beats on the canonical cycle under full motion, holds one static frame under reduced motion and
 * while inactive, is decorative, and ends with its composition. Not Android/iOS runtime, TalkBack
 * or a recording.
 */
@OptIn(ExperimentalTestApi::class)
class SurfacePulseTest {
    private fun ComposeUiTest.advance(millis: Long) {
        val until = mainClock.currentTime + millis
        while (mainClock.currentTime < until) mainClock.advanceTimeByFrame()
    }

    private fun frames(reducedMotion: Boolean, times: List<Long>, active: Boolean = true): List<ImageBitmap> {
        val captured = mutableListOf<ImageBitmap>()
        runDesktopComposeUiTest(width = 160, height = 120) {
            mainClock.autoAdvance = false
            setContent {
                BundledPreviewCaptureRoot(preferences = UiPreferences(reducedMotion = reducedMotion)) {
                    Box(Modifier.padding(tokenDp("space.md"))) {
                        SurfacePulse(Modifier.testTag("pulse").width(96.dp).height(64.dp), active, RoundedCornerShape(12.dp))
                    }
                }
            }
            mainClock.advanceTimeByFrame()
            var now = 0L
            times.forEach { at ->
                advance(at - now)
                now = at
                captured += onNodeWithTag("pulse").captureToImage()
            }
        }
        return captured
    }

    @Test
    fun the_fill_beats_over_1400_ms_and_returns_to_rest_after_a_whole_cycle() {
        val (start, quarter, half, full) = frames(false, listOf(0L, 350L, 700L, 1_400L))
        assertTrue(imageDistance(start, quarter) > 0.0, "a quarter cycle repaints the fill")
        assertTrue(imageDistance(start, half) > imageDistance(start, quarter), "the half cycle is the peak alpha")
        assertTrue(imageDistance(start, full) < imageDistance(start, quarter) / 4, "one whole cycle is back near rest")
    }

    @Test
    fun reduced_motion_holds_one_static_frame() {
        val (start, later, muchLater) = frames(true, listOf(0L, 700L, 2_800L))
        assertEquals(0.0, imageDistance(start, later), 1e-6)
        assertEquals(0.0, imageDistance(start, muchLater), 1e-6)
    }

    @Test
    fun an_inactive_pulse_keeps_its_box_but_runs_no_animation() {
        val (start, later) = frames(false, listOf(0L, 700L), active = false)
        assertEquals(0.0, imageDistance(start, later), 1e-6)
        assertTrue(start.width > 0 && start.height > 0, "the fill keeps the caller's box")
    }

    @Test
    fun the_fill_is_decorative_and_adds_no_semantics() {
        runDesktopComposeUiTest(width = 160, height = 120) {
            mainClock.autoAdvance = false
            setContent {
                BundledPreviewCaptureRoot(preferences = UiPreferences()) { SurfacePulse(Modifier.testTag("pulse").width(96.dp).height(64.dp)) }
            }
            val config = onNodeWithTag("pulse").fetchSemanticsNode().config
            assertTrue(SemanticsProperties.ContentDescription !in config && SemanticsProperties.StateDescription !in config)
            assertTrue(SemanticsProperties.Role !in config && SemanticsProperties.LiveRegion !in config)
        }
    }

    @Test
    fun toggling_inactive_or_leaving_composition_ends_the_loop_and_the_clock_reaches_idle() {
        runDesktopComposeUiTest(width = 160, height = 120) {
            var active by mutableStateOf(true)
            var mounted by mutableStateOf(true)
            setContent {
                BundledPreviewCaptureRoot(preferences = UiPreferences()) {
                    if (mounted) SurfacePulse(Modifier.testTag("pulse").width(96.dp).height(64.dp), active)
                }
            }
            active = false
            waitForIdle() // an inactive pulse owns no infinite transition, so the clock can settle
            mounted = false
            waitForIdle()
        }
    }
}
