package com.cantos.ui

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.assertIsOff
import androidx.compose.ui.test.assertIsOn
import androidx.compose.ui.test.assertTextContains
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextReplacement
import org.junit.Rule
import org.junit.Test
import kotlin.test.assertEquals

/** Pending JVM renderer evidence: these tests must actually run before semantics are claimed. */
class ComponentSemanticsTest {
    @get:Rule
    val compose = createComposeRule()

    @Test
    fun enabled_button_dispatches_once_and_disabled_reason_remains_visible() {
        var calls = 0
        compose.setContent {
            UiRoot {
                Stack {
                    Button("Lưu bản nháp", { calls += 1 })
                    Button("Xuất bản", { calls += 100 }, state = ButtonState.Disabled(DisabledReason.of("Chưa duyệt bản nháp")))
                }
            }
        }
        compose.onNodeWithText("Lưu bản nháp").assertIsEnabled().performClick()
        compose.onNodeWithText("Xuất bản").assertIsNotEnabled()
        compose.onNodeWithText("Chưa duyệt bản nháp").assertExists()
        assertEquals(1, calls)
    }

    @Test
    fun icon_button_has_localized_accessible_name() {
        var calls = 0
        compose.setContent {
            UiRoot {
                IconButton(AccessibleName.of("Thêm cảnh cho Ánh đèn cuối sân khấu"), "add", { calls += 1 })
            }
        }
        compose.onNodeWithContentDescription("Thêm cảnh cho Ánh đèn cuối sân khấu").performClick()
        assertEquals(1, calls)
    }

    @Test
    fun checkbox_keeps_real_native_checked_state_and_callback() {
        compose.setContent {
            UiRoot {
                var checked by remember { mutableStateOf(false) }
                Checkbox("Giữ ghi chú phát âm", checked, { checked = it })
            }
        }
        compose.onNodeWithContentDescription("Giữ ghi chú phát âm").assertIsOff().performClick().assertIsOn()
    }

    @Test
    fun controlled_field_preserves_vietnamese_input() {
        compose.setContent {
            UiRoot {
                var value by remember { mutableStateOf("Ánh đèn") }
                TextField("Tên tác phẩm", value, { value = it })
            }
        }
        compose.onNodeWithText("Ánh đèn").performTextReplacement("Nguyễn Thị Minh Phương · Ỗ, Ữ")
        compose.onNodeWithText("Nguyễn Thị Minh Phương · Ỗ, Ữ").assertTextContains("Nguyễn Thị Minh Phương · Ỗ, Ữ")
    }
}
