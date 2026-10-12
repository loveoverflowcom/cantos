package com.cantos.ui

import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Text
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performImeAction
import androidx.compose.ui.test.performTextInputSelection
import androidx.compose.ui.test.performTextReplacement
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.TextFieldValue
import com.cantos.ui.preview.ComponentExample
import org.junit.Rule
import org.junit.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

/** Real production wrappers on the JVM Compose test renderer, not OS input or mobile evidence. */
class FoundationComponentTest {
    @get:Rule
    val compose = createComposeRule()

    @Test
    fun composed_preview_accessible_names_begin_with_each_complete_visible_label() {
        compose.setContent {
            UiRoot { ComponentExample(PreviewExample.Button, "composed-content", {}, Modifier) }
        }
        listOf("Lưu · Primary", "Lưu · Tonal", "Lưu · Secondary", "Lưu · Text", "Lưu · Danger", "Thêm cảnh · Ỗ, Ữ").forEach { visible ->
            val name = compose.onNodeWithText(visible).fetchSemanticsNode().config[SemanticsProperties.ContentDescription].single()
            assertTrue(name.startsWith(visible), "Visible label must remain intact in its accessible name: $visible")
        }
    }

    @Test
    fun composed_button_has_one_named_control_focus_target_and_controlled_blocking() {
        var state by mutableStateOf<ButtonState>(ButtonState.Enabled)
        val focus = FocusRequester()
        var calls = 0
        compose.setContent {
            UiRoot {
                Button(
                    name = AccessibleName.of("Thêm cảnh cho Ánh đèn cuối sân khấu"),
                    onPress = { calls += 1 },
                    modifier = Modifier.testTag("whole-action"),
                    controlModifier = Modifier.testTag("native-action").focusRequester(focus),
                    state = state,
                ) {
                    Icon("add")
                    Spacer(Modifier.size(tokenDp("space.sm")))
                    Text("Thêm cảnh")
                }
            }
        }
        compose.runOnIdle { focus.requestFocus() }
        val node = compose.onNodeWithContentDescription("Thêm cảnh cho Ánh đèn cuối sân khấu")
        node.assertIsFocused().performClick().performClick()
        compose.onNodeWithTag("native-action").assertIsFocused()
        assertEquals(2, calls)
        compose.runOnIdle { state = ButtonState.Loading(DisabledReason.of("Đang thêm cảnh")) }
        node.assertIsNotEnabled().assert(SemanticsMatcher.expectValue(SemanticsProperties.StateDescription, "Đang thêm cảnh"))
        compose.onNodeWithText("Đang thêm cảnh").assertExists()
        assertEquals(2, calls)
    }

    @Test
    fun rich_text_keeps_controlled_selection_and_external_composition_during_recomposition() {
        var value by mutableStateOf(TextFieldValue("Người dẫn chuyện · Ỗ, Ữ", TextRange(0, 5), TextRange(0, 5)))
        var theme by mutableStateOf(Theme.Light)
        compose.setContent {
            UiRoot(preferences = UiPreferences(theme = theme)) {
                TextField("Nhân vật", value, { value = it })
            }
        }
        val node = compose.onNode(hasSetTextAction() and hasText("Nhân vật"))
        node.assert(SemanticsMatcher.expectValue(SemanticsProperties.TextSelectionRange, TextRange(0, 5)))
        compose.runOnIdle { theme = Theme.Dark }
        assertEquals(TextRange(0, 5), value.composition)
        node.assert(SemanticsMatcher.expectValue(SemanticsProperties.TextSelectionRange, TextRange(0, 5)))
        node.performTextInputSelection(TextRange(6, 9))
        assertEquals(TextRange(6, 9), value.selection)
        node.performTextReplacement("Nguyễn Thị Minh Phương · Ỗ, Ữ")
        assertEquals("Nguyễn Thị Minh Phương · Ỗ, Ữ", value.text)
    }

    @Test
    fun rich_readonly_reports_selection_without_edit_action() {
        var value by mutableStateOf(TextFieldValue("Ánh đèn cuối sân khấu"))
        compose.setContent {
            UiRoot { TextArea("Lời thoại", value, { value = it }, state = FieldState.ReadOnly) }
        }
        val node = compose.onNodeWithText("Ánh đèn cuối sân khấu")
        node.assert(hasSetTextAction().not()).performTextInputSelection(TextRange(0, 7))
        assertEquals("Ánh đèn cuối sân khấu", value.text)
        assertEquals(TextRange(0, 7), value.selection)
    }

    @Test
    fun rich_number_and_string_number_preserve_zero_empty_and_incomplete_decimal_text() {
        var rich by mutableStateOf(TextFieldValue("0"))
        var plain by mutableStateOf("0")
        compose.setContent {
            UiRoot {
                Stack {
                    NumberField("Số có selection", rich, { rich = it })
                    NumberField("Số đơn giản", plain, { plain = it })
                }
            }
        }
        listOf("", "-", "1,", "1,25", "0").forEach { raw ->
            compose.onNode(hasSetTextAction() and hasText("Số có selection")).performTextReplacement(raw)
            compose.onNode(hasSetTextAction() and hasText("Số đơn giản")).performTextReplacement(raw)
            assertEquals(raw, rich.text)
            assertEquals(raw, plain)
        }
    }

    @Test
    fun all_text_wrappers_forward_native_ime_action_once_and_block_readonly_submission() {
        var state by mutableStateOf<FieldState>(FieldState.Editable)
        val calls = IntArray(3)
        compose.setContent {
            UiRoot {
                var text by remember { mutableStateOf("Ỗ, Ữ") }
                var area by remember { mutableStateOf(TextFieldValue("Ngày mai, mình có diễn tiếp không?")) }
                var number by remember { mutableStateOf(TextFieldValue("0")) }
                Stack {
                    TextField("Tên", text, { text = it }, state = state,
                        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
                        keyboardActions = KeyboardActions(onDone = { calls[0] += 1 }))
                    TextArea("Lời thoại", area, { area = it }, state = state,
                        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
                        keyboardActions = KeyboardActions(onDone = { calls[1] += 1 }), minLines = 2, maxLines = 4)
                    NumberField("Số", number, { number = it }, state = state,
                        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
                        keyboardActions = KeyboardActions(onDone = { calls[2] += 1 }))
                }
            }
        }
        val labels = listOf("Tên", "Lời thoại", "Số")
        labels.forEach { label -> compose.onNode(hasSetTextAction() and hasText(label)).performImeAction() }
        assertEquals(listOf(1, 1, 1), calls.toList())
        compose.runOnIdle { state = FieldState.ReadOnly }
        labels.forEach { label -> compose.onNodeWithText(label).performImeAction() }
        assertEquals(listOf(1, 1, 1), calls.toList())
    }

    @Test
    fun blank_errors_have_no_native_error_semantics_while_visible_errors_do() {
        var message by mutableStateOf<String?>(null)
        compose.setContent {
            UiRoot { TextField("Tên", "Ánh đèn", {}, help = " ", error = message) }
        }
        val node = compose.onNode(hasSetTextAction() and hasText("Tên"))
        listOf(null, "", " \n\t").forEach { blank ->
            compose.runOnIdle { message = blank }
            node.assert(SemanticsMatcher.keyNotDefined(SemanticsProperties.Error))
            node.assert(SemanticsMatcher.keyNotDefined(SemanticsProperties.StateDescription))
        }
        compose.runOnIdle { message = "Chưa chọn bản nháp" }
        node.assert(SemanticsMatcher.expectValue(SemanticsProperties.Error, "Chưa chọn bản nháp"))
        compose.onNodeWithText("Chưa chọn bản nháp").assertExists()
    }
}
