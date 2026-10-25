package com.cantos.ui

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.material3.Text
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.toPixelMap
import kotlin.math.abs
import kotlin.math.roundToInt
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.hasAnyDescendant
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.isRoot
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performKeyInput
import androidx.compose.ui.test.performTextReplacement
import androidx.compose.ui.test.pressKey
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import org.junit.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

/** Full rendered modal root and actual JVM input; no OS-window/device/AT claim. */
@OptIn(ExperimentalTestApi::class)
class DialogQualificationTest {
    @Test
    fun modal_reopens_with_caller_text_and_keeps_recovery_controls_accessible_at_narrow_large_text() {
        Theme.entries.forEach { theme ->
            listOf(320 to 1f, 320 to 2f, 840 to 1f).forEach { (width, scale) ->
                val preferences = UiPreferences(theme = theme, textScale = scale)
                runDesktopComposeUiTest(width = width, height = 844) {
                    var open by mutableStateOf(false)
                    var value by mutableStateOf("Lời hẹn vẫn còn")
                    var requests = 0
                    val trigger = FocusRequester()
                    var density = 0f
                    var fontScale = 0f
                    var focusColor = Color.Unspecified
                    var focusOffset = 0f
                    var focusWidth = 0f
                    setContent {
                        density = LocalDensity.current.density
                        fontScale = LocalDensity.current.fontScale
                        Box(Modifier.requiredSize(width.dp, 844.dp).testTag("host")) {
                            BundledPreviewCaptureRoot(preferences = preferences) {
                                focusColor = tokenColor("color.focus")
                                with(LocalDensity.current) {
                                    focusOffset = tokenDp("size.focusOffset").toPx()
                                    focusWidth = tokenDp("size.focusWidth").toPx()
                                }
                                Button("Mở bản nháp", { trigger.requestFocus(); open = true },
                                    controlModifier = Modifier.focusRequester(trigger))
                                Dialog(open, "Một lời hẹn dưới chân Thanh Vân Sơn — bản nháp của Nguyễn Thị Ánh Nguyệt",
                                    "Đóng", { requests += 1; open = false },
                                    modifier = Modifier.testTag("modal"), returnFocus = trigger) {
                                    Stack(gap = Space.Md) {
                                        Text("Nội dung vẫn được giữ khi đóng rồi mở lại. Không có yêu cầu mạng hay thay đổi bản nháp trên máy chủ.")
                                        TextField("Văn bản được giữ sau khi đóng", value, { value = it },
                                            controlModifier = Modifier.testTag("editor"))
                                    }
                                }
                            }
                        }
                    }
                    val opener = onNodeWithText("Mở bản nháp")
                    opener.performClick()
                    val close = onNodeWithText("Đóng")
                    close.assertIsFocused().performKeyInput { pressKey(Key.Tab) }
                    val editor = onNodeWithTag("editor").assertIsFocused()
                    val focusedRoot = onNode(isRoot() and hasAnyDescendant(hasTestTag("modal")), useUnmergedTree = true)
                    writeCapture(CaptureScenario(
                        "cmp-qualification-dialog-${theme.tokenName}-text$scale-$width-focused", "qualification-dialog",
                        "dialog", "focused-input", themeMode = if (theme == Theme.Dark) ThemeMode.Dark else ThemeMode.Light,
                        systemIsDark = false, preferences = preferences, viewportWidthDp = width, viewportHeightDp = 844,
                        hostDensity = density, hostFontScale = fontScale, fontSource = BundledPreviewFontSource,
                    ), focusedRoot.captureToImage())
                    editor.assertIsDisplayed()
                    val editorBounds = editor.fetchSemanticsNode().boundsInRoot
                    assertTrue(editorBounds.height > 0f && editorBounds.top >= 0f && editorBounds.bottom <= 844f,
                        "Focused content must remain visible in $width/$scale/$theme: $editorBounds")
                    val focusedPixels = focusedRoot.captureToImage().toPixelMap()
                    val x = editorBounds.center.x.roundToInt().coerceIn(0, focusedPixels.width - 1)
                    val focusRows = ((editorBounds.bottom + focusOffset).toInt()..
                        (editorBounds.bottom + focusOffset + focusWidth).toInt())
                    val visibleStroke = focusRows.count { y ->
                        if (y !in 0 until focusedPixels.height) false else {
                            val pixel = focusedPixels[x, y]
                            abs(pixel.red - focusColor.red) + abs(pixel.green - focusColor.green) +
                                abs(pixel.blue - focusColor.blue) < 0.1f
                        }
                    }
                    assertTrue(visibleStroke > 0,
                        "The scroll viewport must not clip the bottom focus stroke in $width/$scale/$theme; editor=$editorBounds")
                    editor.performTextReplacement("Nguyễn Thị Ánh Nguyệt · Ỗ, Ữ")
                    editor.performKeyInput { pressKey(Key.Tab) }
                    close.assertIsFocused().assertIsDisplayed()
                    val root = onNode(isRoot() and hasAnyDescendant(hasTestTag("modal")), useUnmergedTree = true)
                    val image = root.captureToImage()
                    println("modal-root $width/$scale/$theme bitmap=${image.width}x${image.height} bounds=${root.fetchSemanticsNode().boundsInRoot}")
                    assertEquals(width, image.width, "Capture must retain the full host width")
                    assertEquals(844, image.height, "Capture must retain the full modal viewport, not just the Surface")
                    writeCapture(CaptureScenario(
                        "cmp-qualification-dialog-${theme.tokenName}-text$scale-$width", "qualification-dialog",
                        "dialog", "retained-input", themeMode = if (theme == Theme.Dark) ThemeMode.Dark else ThemeMode.Light,
                        systemIsDark = false, preferences = preferences, viewportWidthDp = width, viewportHeightDp = 844,
                        hostDensity = density, hostFontScale = fontScale, fontSource = BundledPreviewFontSource,
                    ), image)
                    close.performKeyInput { pressKey(Key.Escape) }
                    assertEquals(1, requests)
                    opener.assertIsFocused().performClick()
                    onNodeWithText("Nguyễn Thị Ánh Nguyệt · Ỗ, Ữ").assertExists()
                    close.performClick()
                    assertEquals(2, requests)
                    opener.assertIsFocused()
                }
            }
        }
    }
}
