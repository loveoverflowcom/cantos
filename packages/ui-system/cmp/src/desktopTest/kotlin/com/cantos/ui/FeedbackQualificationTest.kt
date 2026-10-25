package com.cantos.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.Spacer
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.toPixelMap
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.test.hasAnyAncestor
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.hasText
import kotlin.math.abs
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performKeyInput
import androidx.compose.ui.test.performSemanticsAction
import androidx.compose.ui.test.pressKey
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.unit.dp
import org.junit.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

/** Executed JVM renderer matrix, not OS-window, screen-reader or Android/iOS acceptance. */
@OptIn(ExperimentalTestApi::class)
class FeedbackQualificationTest {
    private val title = "Không thể nạp bản nháp của Nguyễn Thị Ánh Nguyệt"
    private val message = "Ghi chú phát âm tiếng Việt vẫn còn nguyên: Ỗ, Ữ. Hãy chọn cách khôi phục bản nháp."

    @Test
    fun error_recovery_text_and_actions_remain_complete_at_phone_width_in_both_themes_and_text_scales() {
        Theme.entries.forEach { theme ->
            listOf(1f, 2f).forEach { scale ->
                val preferences = UiPreferences(theme = theme, textScale = scale)
                runDesktopComposeUiTest(width = 320, height = 844) {
                    var calls = 0
                    var announcement by mutableStateOf(Announcement.Off)
                    var hostDensity = 0f
                    var hostFontScale = 0f
                    setContent {
                        hostDensity = LocalDensity.current.density
                        hostFontScale = LocalDensity.current.fontScale
                        Box(Modifier.requiredSize(320.dp, 844.dp).testTag("capture")) {
                            BundledPreviewCaptureRoot(preferences = preferences) {
                                Column(Modifier.padding(tokenDp("space.lg"))) {
                                    ErrorState(title, message, Modifier.testTag("feedback"), announcement) {
                                        Stack(direction = StackDirection.Horizontal, gap = Space.Sm) {
                                            Button("Thử lại", { calls += 1 }, variant = ButtonVariant.Secondary)
                                            Button("Xem ghi chú bản nháp", { calls += 1 }, variant = ButtonVariant.Text)
                                        }
                                    }
                                }
                            }
                        }
                    }
                    waitForIdle()
                    val root = onNodeWithTag("capture").fetchSemanticsNode().boundsInRoot
                    listOf(title, message, "Thử lại", "Xem ghi chú bản nháp").forEach { text ->
                        val node = onNodeWithText(text).assertIsDisplayed()
                        val bounds = node.fetchSemanticsNode().boundsInRoot
                        assertTrue(bounds.left >= root.left && bounds.right <= root.right, "$text stays in the phone width")
                        assertTrue(bounds.top >= root.top && bounds.bottom <= root.bottom, "$text remains visible")
                        node.performSemanticsAction(SemanticsActions.GetTextLayoutResult) { getLayout ->
                            val layouts = mutableListOf<TextLayoutResult>()
                            assertTrue(getLayout(layouts))
                            val layout = layouts.single()
                            assertFalse(layout.didOverflowHeight, "$text retains every line")
                            assertEquals(text.length, layout.getLineEnd(layout.lineCount - 1))
                            assertTrue(bounds.height + 0.5f >= layout.size.height, "$text is not vertically cropped")
                            repeat(layout.lineCount) { line -> assertFalse(layout.isLineEllipsized(line)) }
                        }
                    }
                    val feedback = onNodeWithTag("feedback")
                    feedback.assert(SemanticsMatcher.keyNotDefined(SemanticsProperties.LiveRegion))
                    onNodeWithText("Thử lại").performClick()
                    onNodeWithText("Xem ghi chú bản nháp")
                        .performSemanticsAction(SemanticsActions.RequestFocus) { it() }
                        .performKeyInput { pressKey(Key.Enter) }
                    assertEquals(2, calls, "Each independent pointer/key activation dispatches once")
                    runOnIdle { announcement = Announcement.Assertive }
                    feedback.assert(SemanticsMatcher.expectValue(SemanticsProperties.LiveRegion, LiveRegionMode.Assertive))
                    assertEquals(2, calls, "Changing announcement must not replay recovery")
                    writeCapture(CaptureScenario(
                        "cmp-qualification-error-state-${theme.tokenName}-text$scale-320", "qualification-feedback",
                        "error-state", "recovery-slot", themeMode = if (theme == Theme.Dark) ThemeMode.Dark else ThemeMode.Light,
                        systemIsDark = false, preferences = preferences, viewportWidthDp = 320, viewportHeightDp = 844,
                        hostDensity = hostDensity, hostFontScale = hostFontScale, fontSource = BundledPreviewFontSource,
                    ), onNodeWithTag("capture").captureToImage())
                }
            }
        }
    }

    @Test
    fun a_wrapped_badge_does_not_clip_letter_ink_that_still_fits_its_text_layout() {
        val label = "Không thể nạp bản nháp của Nguyễn Thị Ánh Nguyệt"
        runDesktopComposeUiTest(width = 320, height = 600) {
            var expectedWidth by mutableStateOf(248.dp)
            var foreground = Color.Unspecified
            var hostDensity = 1f
            setContent {
                Box(Modifier.requiredSize(320.dp, 600.dp).testTag("paint-root")) {
                    BundledPreviewCaptureRoot(preferences = UiPreferences(textScale = 2f)) {
                        foreground = tokenColor("color.error")
                        hostDensity = LocalDensity.current.density
                        Column(Modifier.padding(tokenDp("space.lg"))) {
                            Badge(label, Modifier.testTag("badge"), StatusTone.Error)
                            Spacer(Modifier.height(32.dp))
                            // Independent native text, with the same width/font/ink but no shaped
                            // ancestor. Layout assertions alone cannot see Surface's paint clipping.
                            Box(Modifier.background(tokenColor("color.errorContainer"))
                                .padding(horizontal = tokenDp("space.inset"), vertical = tokenDp("space.xs"))) {
                                Text(label, Modifier.width(expectedWidth).testTag("reference"),
                                    color = foreground, style = MaterialTheme.typography.labelSmall)
                            }
                        }
                    }
                }
            }
            waitForIdle()
            val actual = onNode(hasText(label) and hasAnyAncestor(hasTestTag("badge")))
            val actualBounds = actual.fetchSemanticsNode().boundsInRoot
            runOnIdle { expectedWidth = (actualBounds.width / hostDensity).dp }
            waitForIdle()
            val referenceBounds = onNodeWithTag("reference").fetchSemanticsNode().boundsInRoot
            assertEquals(actualBounds.width, referenceBounds.width, 0.5f)
            assertEquals(actualBounds.height, referenceBounds.height, 0.5f)
            val pixels = onNodeWithTag("paint-root").captureToImage().toPixelMap()
            fun ink(bounds: Rect): Int {
                var count = 0
                for (y in bounds.top.toInt() until bounds.bottom.toInt()) {
                    for (x in bounds.left.toInt() until bounds.right.toInt()) {
                        val p = pixels[x, y]
                        if (abs(p.red - foreground.red) + abs(p.green - foreground.green) +
                            abs(p.blue - foreground.blue) < 0.15f) count += 1
                    }
                }
                return count
            }
            val expectedInk = ink(referenceBounds)
            val actualInk = ink(actualBounds)
            assertTrue(expectedInk > 100, "The independent reference must contain actual glyph ink")
            assertTrue(actualInk >= expectedInk * 0.99f,
                "The badge clips glyph ink despite an intact layout: actual=$actualInk reference=$expectedInk")
        }
    }

    @Test
    fun every_status_tone_keeps_long_vietnamese_text_and_explicit_announcement_at_narrow_and_wide_constraints() {
        Theme.entries.forEach { theme ->
            listOf(320 to 1f, 320 to 2f, 1100 to 1f).forEach { (width, scale) ->
                StatusTone.entries.forEach { tone ->
                    val preferences = UiPreferences(theme = theme, textScale = scale)
                    val label = "Nguyễn Thị Ánh Nguyệt — lời hẹn dưới chân Thanh Vân Sơn · Ỗ, Ữ"
                    runDesktopComposeUiTest(width = width, height = 400) {
                        var announcement by mutableStateOf(Announcement.Off)
                        var hostDensity = 0f
                        var hostFontScale = 0f
                        setContent {
                            hostDensity = LocalDensity.current.density
                            hostFontScale = LocalDensity.current.fontScale
                            Box(Modifier.requiredSize(width.dp, 400.dp).testTag("capture")) {
                                BundledPreviewCaptureRoot(preferences = preferences) {
                                    Column(Modifier.padding(tokenDp("space.lg")), verticalArrangement = Arrangement.spacedBy(tokenDp("space.md"))) {
                                        Status(label, Modifier.testTag("status"), tone, announcement = announcement)
                                    }
                                }
                            }
                        }
                        waitForIdle()
                        val node = onNodeWithText(label).assertIsDisplayed()
                        val bounds = node.fetchSemanticsNode().boundsInRoot
                        assertTrue(bounds.left >= 0f && bounds.right <= width && bounds.bottom <= 400f)
                        node.performSemanticsAction(SemanticsActions.GetTextLayoutResult) { getLayout ->
                            val layouts = mutableListOf<TextLayoutResult>()
                            assertTrue(getLayout(layouts))
                            val layout = layouts.single()
                            assertFalse(layout.didOverflowHeight)
                            assertEquals(label.length, layout.getLineEnd(layout.lineCount - 1))
                            assertTrue(bounds.height + 0.5f >= layout.size.height)
                            repeat(layout.lineCount) { line -> assertFalse(layout.isLineEllipsized(line)) }
                            if (width == 320 && scale == 2f) assertTrue(layout.lineCount > 1)
                        }
                        val status = onNodeWithTag("status")
                        status.assert(SemanticsMatcher.keyNotDefined(SemanticsProperties.LiveRegion))
                        runOnIdle { announcement = Announcement.Polite }
                        status.assert(SemanticsMatcher.expectValue(SemanticsProperties.LiveRegion, LiveRegionMode.Polite))
                        runOnIdle { announcement = Announcement.Assertive }
                        status.assert(SemanticsMatcher.expectValue(SemanticsProperties.LiveRegion, LiveRegionMode.Assertive))
                        writeCapture(CaptureScenario(
                            "cmp-qualification-status-${tone.name.lowercase()}-${theme.tokenName}-text$scale-$width", "qualification-feedback",
                            "status", tone.name.lowercase(), themeMode = if (theme == Theme.Dark) ThemeMode.Dark else ThemeMode.Light,
                            systemIsDark = false, preferences = preferences, viewportWidthDp = width, viewportHeightDp = 400,
                            hostDensity = hostDensity, hostFontScale = hostFontScale, fontSource = BundledPreviewFontSource,
                        ), onNodeWithTag("capture").captureToImage())
                    }
                }
            }
        }
    }
}
