package com.cantos.ui

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.toPixelMap
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertHeightIsEqualTo
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import org.junit.Test
import kotlin.math.abs
import kotlin.test.assertEquals
import kotlin.test.assertTrue

/** Production component, manual JVM Compose clock. No claim about device frame pacing or OS lifecycle adapters. */
@OptIn(ExperimentalTestApi::class)
class EdgeProgressTest {
    private val name = AccessibleName.of("Đang cập nhật danh sách")
    private fun Color.near(other: Color): Boolean = abs(red - other.red) + abs(green - other.green) + abs(blue - other.blue) < 0.03f

    @Test
    fun the_quarter_segment_crosses_linearly_over_the_token_cycle_without_relayout() {
        runDesktopComposeUiTest(width = 400, height = 120) {
            mainClock.autoAdvance = false
            var ink = Color.Unspecified
            setContent {
                BundledPreviewCaptureRoot {
                    ink = tokenColor("color.primary")
                    EdgeProgress(true, true, true, name, Modifier.testTag("edge"))
                }
            }
            mainClock.advanceTimeByFrame()
            fun leftAtNextQuarter(): Int {
                mainClock.advanceTimeBy(304L) // 19 frames at the test clock's 16 ms quantum
                val pixels = onNodeWithTag("edge").captureToImage().toPixelMap()
                val painted = (0 until pixels.width).filter { pixels[it, 0].near(ink) }
                assertTrue(painted.size in 99..101, "quarter of the 400-unit track, not a percentage: ${painted.size}")
                return painted.first()
            }
            onNodeWithTag("edge").assertHeightIsEqualTo(2.dp)
            val left = leftAtNextQuarter()
            val middle = leftAtNextQuarter()
            val right = leftAtNextQuarter()
            assertTrue(left in 20..40 && middle in 145..165 && right in 270..295, "$left/$middle/$right over 1200 ms")
            assertTrue(abs((middle - left) - (right - middle)) <= 2, "linear equal-time displacement")
        }
    }

    @Test
    fun reduced_motion_is_a_static_quarter_segment_with_one_status_and_no_percentage() {
        runDesktopComposeUiTest(width = 400, height = 120) {
            mainClock.autoAdvance = false
            setContent {
                BundledPreviewCaptureRoot(UiPreferences(reducedMotion = true)) {
                    EdgeProgress(true, true, true, name, Modifier.testTag("edge"), Announcement.Polite)
                }
            }
            mainClock.advanceTimeByFrame()
            val first = onNodeWithTag("edge").captureToImage()
            mainClock.advanceTimeBy(2400L)
            assertEquals(0.0, imageDistance(first, onNodeWithTag("edge").captureToImage()), 1e-6)
            val semantics = onNodeWithTag("edge").fetchSemanticsNode().config
            assertEquals(listOf(name.text), semantics[SemanticsProperties.ContentDescription])
            assertTrue(SemanticsProperties.ProgressBarRangeInfo !in semantics)
        }
    }

    @Test
    fun terminal_hidden_background_and_disposal_end_the_ticker_and_keep_field_focus_and_geometry() {
        runDesktopComposeUiTest(width = 400, height = 300) {
            var active by mutableStateOf(true)
            var visible by mutableStateOf(true)
            var foreground by mutableStateOf(true)
            var mounted by mutableStateOf(true)
            setContent {
                BundledPreviewCaptureRoot {
                    Column {
                        TextField("Tìm trong danh mục", "Ánh đèn cuối sân khấu", {}, modifier = Modifier.testTag("field"))
                        if (mounted) EdgeProgress(active, visible, foreground, name, Modifier.testTag("edge"))
                        Button("Nội dung cũ", {}, controlModifier = Modifier.testTag("retained"))
                    }
                }
            }
            mainClock.autoAdvance = false
            onNodeWithText("Ánh đèn cuối sân khấu").performClick()
            mainClock.advanceTimeByFrame()
            val bounds = onNodeWithTag("retained").fetchSemanticsNode().boundsInRoot
            val fieldBounds = onNodeWithTag("field").fetchSemanticsNode().boundsInRoot
            // Completing, failing and canceling are the same controlled active=false fact.
            for (stop in listOf("success", "error", "cancel", "hidden", "background")) {
                runOnIdle { active = stop in listOf("hidden", "background"); visible = stop != "hidden"; foreground = stop != "background" }
                mainClock.autoAdvance = true
                waitForIdle() // no infinite transition is composed while any fact is false
                val image = onNodeWithTag("edge").captureToImage()
                mainClock.advanceTimeBy(1600L)
                assertEquals(0.0, imageDistance(image, onNodeWithTag("edge").captureToImage()), 1e-6, stop)
                assertEquals(bounds, onNodeWithTag("retained").fetchSemanticsNode().boundsInRoot, stop)
                assertEquals(fieldBounds, onNodeWithTag("field").fetchSemanticsNode().boundsInRoot, stop)
                onNodeWithText("Ánh đèn cuối sân khấu").assertIsFocused()
                assertTrue(SemanticsProperties.ContentDescription !in onNodeWithTag("edge").fetchSemanticsNode().config)
                mainClock.autoAdvance = false
                runOnIdle { active = true; visible = true; foreground = true }
                mainClock.advanceTimeByFrame()
            }
            runOnIdle { mounted = false }
            mainClock.autoAdvance = true
            waitForIdle()
        }
    }

    @Test
    fun paired_theme_width_and_text_scale_specimens_are_captured_with_real_component_geometry() {
        for (theme in Theme.entries) for (width in listOf(320, 840)) {
            val preferences = UiPreferences(theme = theme, textScale = if (width == 320) 2f else 1f, reducedMotion = true)
            runDesktopComposeUiTest(width = width, height = 900) {
                var density = 1f
                var scale = 1f
                setContent {
                    density = LocalDensity.current.density
                    scale = LocalDensity.current.fontScale
                    BundledPreviewCaptureRoot(preferences) {
                        Column(Modifier.testTag("capture").padding(16.dp)) {
                            com.cantos.ui.preview.ComponentExample(PreviewExample.EdgeProgress, "pending", {}, Modifier)
                        }
                    }
                }
                waitForIdle()
                onNodeWithTag("edge-search").assertHeightIsEqualTo(2.dp)
                writeCapture(CaptureScenario("edge-progress-${theme.tokenName}-$width", "edge-progress", "edge-progress", "pending",
                    themeMode = if (theme == Theme.Dark) ThemeMode.Dark else ThemeMode.Light,
                    systemIsDark = false, preferences = preferences, viewportWidthDp = width, viewportHeightDp = 900,
                    hostDensity = density, hostFontScale = scale, fontSource = BundledPreviewFontSource), onNodeWithTag("capture").captureToImage())
            }
        }
    }
}
