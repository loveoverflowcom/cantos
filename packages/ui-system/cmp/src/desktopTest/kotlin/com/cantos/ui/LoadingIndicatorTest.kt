package com.cantos.ui

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.ComposeUiTest
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.runDesktopComposeUiTest
import org.junit.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

/**
 * Issue #528 on the production components, JVM Desktop Compose only: the shared indicator turns
 * under full motion over a manual clock, is a stationary visible arc under reduced motion, is
 * decorative (the text is the status), and stops with its composition. Not Android/iOS runtime,
 * TalkBack or a recording.
 */
@OptIn(ExperimentalTestApi::class)
class LoadingIndicatorTest {
    private val label = "Đang tải danh sách bản nháp của Nguyễn Thị Ánh Nguyệt"

    private fun ComposeUiTest.advance(millis: Long) {
        val until = mainClock.currentTime + millis
        while (mainClock.currentTime < until) mainClock.advanceTimeByFrame()
    }

    private fun frames(reducedMotion: Boolean, times: List<Long>): List<ImageBitmap> {
        val captured = mutableListOf<ImageBitmap>()
        runDesktopComposeUiTest(width = 360, height = 120) {
            mainClock.autoAdvance = false
            setContent {
                BundledPreviewCaptureRoot(preferences = UiPreferences(reducedMotion = reducedMotion)) {
                    Box(Modifier.padding(tokenDp("space.lg"))) {
                        LoadingState(label, Modifier.testTag("loading"), announcement = Announcement.Polite)
                    }
                }
            }
            mainClock.advanceTimeByFrame()
            var now = 0L
            times.forEach { at ->
                advance(at - now)
                now = at
                captured += onNodeWithTag("loading").captureToImage()
            }
        }
        return captured
    }

    @Test
    fun loading_state_indicator_turns_under_full_motion() {
        val (start, quarter, half, full) = frames(false, listOf(0L, 250L, 500L, 1_008L))
        assertTrue(imageDistance(start, quarter) > 0.0, "a quarter turn repaints the indicator")
        assertTrue(imageDistance(start, half) > imageDistance(start, quarter), "a half turn is further from rest")
        assertTrue(imageDistance(start, full) < imageDistance(start, quarter) / 4, "one whole turn is back near rest")
    }

    @Test
    fun loading_state_indicator_is_a_stationary_arc_under_reduced_motion() {
        val (start, later, muchLater) = frames(true, listOf(0L, 400L, 1_700L))
        assertEquals(0.0, imageDistance(start, later), 1e-6)
        assertEquals(0.0, imageDistance(start, muchLater), 1e-6)
    }

    @Test
    fun the_label_is_the_status_and_the_indicator_adds_no_semantics_of_its_own() {
        runDesktopComposeUiTest(width = 360, height = 120) {
            mainClock.autoAdvance = false
            setContent {
                BundledPreviewCaptureRoot(preferences = UiPreferences()) {
                    LoadingState(label, Modifier.testTag("loading"), announcement = Announcement.Polite)
                }
            }
            val config = onNodeWithTag("loading").fetchSemanticsNode().config
            assertEquals(label, config[SemanticsProperties.StateDescription])
            assertEquals(LiveRegionMode.Polite, config[SemanticsProperties.LiveRegion])
            assertTrue(SemanticsProperties.ContentDescription !in config, "the arc is decorative, not a named glyph")
            onNodeWithText(label).fetchSemanticsNode()
        }
    }

    @Test
    fun leaving_composition_ends_the_loop_and_the_clock_reaches_idle() {
        runDesktopComposeUiTest(width = 360, height = 120) {
            var pending by mutableStateOf(true)
            setContent {
                BundledPreviewCaptureRoot(preferences = UiPreferences()) {
                    if (pending) Box(Modifier.testTag("pending")) { LoadingIndicator() }
                }
            }
            onAllNodesWithTag("pending").assertCountEquals(1)
            pending = false
            waitForIdle()
            onAllNodesWithTag("pending").assertCountEquals(0)
        }
    }
}
