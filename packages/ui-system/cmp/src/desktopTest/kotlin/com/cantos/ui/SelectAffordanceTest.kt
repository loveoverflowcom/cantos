package com.cantos.ui

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.asSkiaBitmap
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.assertIsSelected
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.isPopup
import androidx.compose.ui.test.isSelected
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performKeyInput
import androidx.compose.ui.test.pressKey
import androidx.compose.ui.test.printToString
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import java.io.File
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import org.jetbrains.skia.EncodedImageFormat
import org.jetbrains.skia.Image
import org.junit.Test

/** Native Material popup on the JVM renderer, never Android/iOS or screen-reader evidence. */
@OptIn(ExperimentalTestApi::class)
class SelectAffordanceTest {
    private val longLabel = "Nguyễn Thị Minh Phương · Người dẫn chuyện Ánh đèn cuối sân khấu · Ỗ, Ữ"

    @Test
    fun keyboard_dismissal_returns_focus_and_zero_selection_dispatches_once() = runDesktopComposeUiTest {
        val focus = FocusRequester()
        val calls = mutableListOf<String>()
        var value by mutableStateOf<String?>("0")
        setContent {
            UiRoot {
                Select("Nhân vật", listOf(SelectOption("0", "Người dẫn chuyện", icon = "mic"), SelectOption("1", "Minh Phương", icon = "person")), value,
                    { calls += it; value = it }, expandedDescription = "Mở", collapsedDescription = "Đóng",
                    placeholder = "Chọn nhân vật", emptyDescription = "Chưa có nhân vật",
                    controlModifier = Modifier.focusRequester(focus).testTag("select-trigger"))
            }
        }
        runOnIdle { focus.requestFocus() }
        val trigger = onNodeWithTag("select-trigger")
        trigger.assertIsFocused().performKeyInput { pressKey(Key.Enter) }
        onNodeWithText("Minh Phương").assertExists()
        onNode(hasText("Người dẫn chuyện") and isSelected()).assertIsSelected()
        onNode(isPopup()).performKeyInput { pressKey(Key.Escape) }
        trigger.assertIsFocused()
        onNodeWithText("Minh Phương").assertDoesNotExist()
        assertEquals(emptyList(), calls)
        trigger.performClick()
        onNodeWithText("Minh Phương").performClick()
        trigger.assertIsFocused()
        assertEquals(listOf("1"), calls)
        trigger.performClick()
        onNodeWithText("Người dẫn chuyện").performClick()
        assertEquals(listOf("1", "0"), calls)
        onNodeWithContentDescription("Nhân vật: Người dẫn chuyện").assertExists()
    }

    @Test
    fun loading_closes_popup_keeps_name_reason_and_never_reopens_on_recovery() = runDesktopComposeUiTest {
        var state by mutableStateOf<ButtonState>(ButtonState.Enabled)
        var calls = 0
        setContent {
            UiRoot {
                Select("Nhân vật", listOf(SelectOption("0", longLabel, icon = "mic")), "0", { calls += 1 }, state = state,
                    expandedDescription = "Mở", collapsedDescription = "Đóng", placeholder = "Chọn nhân vật",
                    emptyDescription = "Chưa có nhân vật", controlModifier = Modifier.testTag("select-trigger"))
            }
        }
        val trigger = onNodeWithTag("select-trigger")
        trigger.performClick()
        onNode(isPopup()).assertExists()
        runOnIdle { state = ButtonState.Loading(DisabledReason.of("Đang tải danh sách, giữ nguyên lựa chọn")) }
        onNode(isPopup()).assertDoesNotExist()
        trigger.assertIsNotEnabled().assert(SemanticsMatcher.expectValue(SemanticsProperties.ContentDescription, listOf("Nhân vật: $longLabel")))
        onNodeWithText("Đang tải danh sách, giữ nguyên lựa chọn").assertExists()
        runOnIdle { state = ButtonState.Enabled }
        onNode(isPopup()).assertDoesNotExist()
        assertEquals(0, calls)
    }

    @Test
    fun native_select_states_are_captured_at_narrow_large_text_and_both_themes() {
        capture(320, Theme.Light, 1f)
        capture(390, Theme.Dark, 2f)
        capture(840, Theme.Light, 1f)
    }

    private fun capture(width: Int, theme: Theme, textScale: Float) = runDesktopComposeUiTest(width = width, height = 1000) {
        var state by mutableStateOf<ButtonState>(ButtonState.Enabled)
        var value by mutableStateOf<String?>("0")
        val options = listOf(SelectOption("0", longLabel, icon = "mic"), SelectOption("1", "Narrator — Nguyễn Thị Minh Phương", icon = "person"),
            SelectOption("2", "Giọng chưa được duyệt", ButtonState.Disabled(DisabledReason.of("Thiếu quyền dùng giọng"))))
        setContent {
            Box(Modifier.requiredSize(width.dp, 1000.dp).testTag("select-root")) {
                UiRoot(preferences = UiPreferences(theme = theme, textScale = textScale)) {
                    Column(Modifier.padding(tokenDp("space.lg"))) {
                        Select("Nhân vật / Character", options, value, { value = it }, state = state,
                            expandedDescription = "Mở", collapsedDescription = "Đóng", placeholder = "Chọn nhân vật / Choose a character",
                            emptyDescription = "Chưa có nhân vật", error = "Lựa chọn cần được duyệt / Review required",
                            controlModifier = Modifier.fillMaxWidth().testTag("select-trigger"))
                    }
                }
            }
        }
        val output = File(System.getenv("CANTOS_UI_PREVIEW_CAPTURE_DIR") ?: "build/reports/cantos-preview")
        check(output.isDirectory || output.mkdirs())
        val prefix = "cmp-select-229-${theme.tokenName}-text$textScale-$width"
        fun save(tag: String, popup: Boolean = false) {
            waitForIdle()
            val node = if (popup) onNode(isPopup()) else onNodeWithTag("select-root")
            val bitmap = node.captureToImage()
            File(output, "$prefix-$tag.png").writeBytes(requireNotNull(Image.makeFromBitmap(bitmap.asSkiaBitmap())
                .encodeToData(EncodedImageFormat.PNG)).bytes)
            File(output, "$prefix-$tag.semantics.txt").writeText(node.printToString())
        }
        val trigger = onNodeWithTag("select-trigger")
        assertTrue(trigger.fetchSemanticsNode().boundsInRoot.height >= 48f)
        save("selected-error")
        trigger.performClick()
        onNodeWithText("Giọng chưa được duyệt").assertIsNotEnabled()
        assertTrue(onNode(isPopup()).fetchSemanticsNode().boundsInRoot.width >= trigger.fetchSemanticsNode().boundsInRoot.width)
        save("open", popup = true)
        onNode(isPopup()).performKeyInput { pressKey(Key.Escape) }
        runOnIdle { value = "removed" }
        save("missing")
        runOnIdle { state = ButtonState.Loading(DisabledReason.of("Đang tải / Loading")) }
        save("loading")
    }
}
