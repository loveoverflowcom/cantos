package com.cantos.ui

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.toPixelMap
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
import androidx.compose.ui.test.assertHeightIsEqualTo
import androidx.compose.ui.test.assertWidthIsEqualTo
import androidx.compose.ui.test.getUnclippedBoundsInRoot
import androidx.compose.ui.test.performClick
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.width
import com.cantos.ui.generated.CantosTokens
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

    @Test
    fun micro_arc_frames_are_16_and_20_and_a_slot_keeps_the_glyph_footprint() {
        runDesktopComposeUiTest(width = 200, height = 120) {
            mainClock.autoAdvance = false
            setContent {
                BundledPreviewCaptureRoot(preferences = UiPreferences(reducedMotion = true)) {
                    Row {
                        Box(Modifier.testTag("compact")) { LoadingIndicator() }
                        Box(Modifier.testTag("standard")) { LoadingIndicator(size = LoadingSize.Standard) }
                        Box(Modifier.testTag("slot")) { LoadingIndicator(slot = Dp(24f)) }
                    }
                }
            }
            val compact = onNodeWithTag("compact")
            compact.assertWidthIsEqualTo(Dp(16f)).assertHeightIsEqualTo(Dp(16f))
            onNodeWithTag("standard").assertWidthIsEqualTo(Dp(20f)).assertHeightIsEqualTo(Dp(20f))
            // The 16 unit arc replaces a 24 unit glyph, so the slot, not the arc, sets the footprint.
            onNodeWithTag("slot").assertWidthIsEqualTo(Dp(24f)).assertHeightIsEqualTo(Dp(24f))
            assertEquals(16.0, CantosTokens.number("size.loadingArcCompact"))
            assertEquals(20.0, CantosTokens.number("size.loadingArc"))
            assertEquals(2.0, CantosTokens.number("size.loadingArcStroke"))
            assertEquals(24.0, CantosTokens.number("size.icon"))
            assertEquals(270f, LoadingArcSweepDegrees)
        }
    }

    @Test
    fun micro_arc_ends_are_round_and_the_arc_is_one_270_degree_stroke() {
        runDesktopComposeUiTest(width = 120, height = 120) {
            mainClock.autoAdvance = false
            setContent {
                BundledPreviewCaptureRoot(preferences = UiPreferences(reducedMotion = true)) {
                    Box(Modifier.padding(tokenDp("space.lg"))) {
                        Box(Modifier.testTag("arc")) { LoadingIndicator(size = LoadingSize.Standard) }
                    }
                }
            }
            mainClock.advanceTimeByFrame()
            val pixels = onNodeWithTag("arc").captureToImage().toPixelMap()
            val tint = pixels[10, 0]
            val hollow = pixels[10, 10]
            fun filled(x: Int, y: Int) = imageDistance(pixels[x, y], hollow) > 0.5
            assertTrue(filled(10, 1), "the arc starts at twelve o'clock")
            assertTrue(filled(19, 10), "the arc passes three o'clock")
            assertTrue(filled(10, 19), "the arc passes six o'clock")
            assertTrue(filled(1, 10), "the arc ends at nine o'clock")
            // Round caps extend past each end; a butt cap would leave these pixels empty.
            assertTrue(filled(9, 1), "the start cap is round, past twelve o'clock")
            assertTrue(filled(1, 9), "the end cap is round, past nine o'clock")
            assertTrue(!filled(3, 3), "the missing quarter between nine and twelve o'clock stays open")
            assertTrue(!filled(10, 10), "the arc is a ring, not a disc")
            assertTrue(tint != hollow)
        }
    }

    private fun imageDistance(a: androidx.compose.ui.graphics.Color, b: androidx.compose.ui.graphics.Color): Double =
        (kotlin.math.abs(a.red - b.red) + kotlin.math.abs(a.green - b.green) + kotlin.math.abs(a.blue - b.blue) +
            kotlin.math.abs(a.alpha - b.alpha)).toDouble()

    /** Ready and pending chip widths for one expression and selection. */
    private fun chipWidths(expression: CantosExpression, selected: Boolean): Pair<Dp, Dp> {
        var ready = Dp(0f)
        var pending = Dp(0f)
        for (loading in listOf(false, true)) {
            runDesktopComposeUiTest(width = 400, height = 160) {
                mainClock.autoAdvance = false
                var calls = 0
                setContent {
                    BundledPreviewCaptureRoot(preferences = UiPreferences(reducedMotion = true, expression = expression)) {
                        FilterChip(
                            label = label, selected = selected, onSelect = { calls++ },
                            state = if (loading) ButtonState.Loading(DisabledReason.of("Đang lọc; không gửi lại."))
                            else ButtonState.Enabled,
                            controlModifier = Modifier.testTag("chip"),
                        )
                    }
                }
                mainClock.advanceTimeByFrame()
                val width = onNodeWithTag("chip").getUnclippedBoundsInRoot().width
                if (loading) {
                    pending = width
                    onNodeWithTag("chip").performClick()
                    assertEquals(0, calls, "a pending chip dispatches nothing")
                    onNodeWithText("Đang lọc; không gửi lại.").fetchSemanticsNode()
                } else ready = width
            }
        }
        return ready to pending
    }

    @Test
    fun a_selected_chip_keeps_its_width_when_the_arc_replaces_the_check_in_the_same_slot() {
        for (expression in CantosExpression.entries) {
            val (ready, pending) = chipWidths(expression, selected = true)
            assertEquals(ready, pending, "$expression: the arc takes the check's slot, so the chip does not move")
        }
    }
}
