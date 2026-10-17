package com.cantos.ui

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.assertIsOff
import androidx.compose.ui.test.assertIsOn
import androidx.compose.ui.test.assertTextContains
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onAllNodesWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextReplacement
import androidx.compose.ui.unit.dp
import com.cantos.ui.preview.PreviewApp
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
    fun labeled_action_icons_keep_one_named_target_and_pending_never_dispatches() {
        var calls = 0
        var state by mutableStateOf<ButtonState>(ButtonState.Enabled)
        val label = "Lưu bản nháp của Nguyễn Thị Ánh Nguyệt / Save draft"
        compose.setContent {
            UiRoot(preferences = UiPreferences(textScale = 2f)) {
                Button(label, { calls += 1 }, state = state, icon = "save")
            }
        }
        val action = compose.onNodeWithContentDescription(label)
        compose.onAllNodesWithContentDescription(label, useUnmergedTree = true).assertCountEquals(1)
        action.assertIsEnabled().performClick()
        assertEquals(1, calls)
        compose.runOnIdle { state = ButtonState.Loading(DisabledReason.of("Đang chờ / Waiting")) }
        action.assertIsNotEnabled().assert(SemanticsMatcher.expectValue(
            SemanticsProperties.StateDescription, "Đang chờ / Waiting"))
        compose.onNodeWithText("Đang chờ / Waiting").assertExists()
        action.performClick()
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

    @Test
    fun select_trigger_keeps_label_value_and_actual_menu_state_on_its_interactive_node() {
        var calls = 0
        compose.setContent {
            UiRoot {
                var selected by remember { mutableStateOf(0) }
                Select("Nhân vật", listOf("Người dẫn chuyện", "Nguyễn Thị Minh Phương"), selected,
                    { selected = it; calls += 1 },
                    expandedDescription = "Danh sách nhân vật đang mở",
                    collapsedDescription = "Danh sách nhân vật đang đóng")
            }
        }
        val trigger = compose.onNodeWithContentDescription("Nhân vật: Người dẫn chuyện")
        trigger.assertIsEnabled().assert(SemanticsMatcher.expectValue(
            SemanticsProperties.StateDescription, "Danh sách nhân vật đang đóng"))
        trigger.performClick().assert(SemanticsMatcher.expectValue(
            SemanticsProperties.StateDescription, "Danh sách nhân vật đang mở"))
        compose.onNodeWithText("Nguyễn Thị Minh Phương").performClick()
        compose.onNodeWithContentDescription("Nhân vật: Nguyễn Thị Minh Phương")
            .assert(SemanticsMatcher.expectValue(SemanticsProperties.StateDescription, "Danh sách nhân vật đang đóng"))
        assertEquals(1, calls)
    }

    @Test
    fun disabled_select_associates_the_reason_with_its_named_trigger() {
        compose.setContent {
            UiRoot {
                Select("Nhân vật", listOf("Người dẫn chuyện"), 0, {},
                    state = ButtonState.Disabled(DisabledReason.of("Chưa chọn bản nháp")),
                    expandedDescription = "Danh sách nhân vật đang mở",
                    collapsedDescription = "Danh sách nhân vật đang đóng")
            }
        }
        compose.onNodeWithContentDescription("Nhân vật: Người dẫn chuyện").assertIsNotEnabled()
            .assert(SemanticsMatcher.expectValue(
                SemanticsProperties.StateDescription, "Danh sách nhân vật đang đóng. Chưa chọn bản nháp"))
        compose.onNodeWithText("Chưa chọn bản nháp").assertExists()
    }

    @Test
    fun disabling_or_loading_an_open_select_clears_its_menu_before_reenabling() {
        var state by mutableStateOf<ButtonState>(ButtonState.Enabled)
        compose.setContent {
            UiRoot {
                Select("Nhân vật", listOf("Người dẫn chuyện", "Nguyễn Thị Minh Phương"), 0, {},
                    state = state,
                    expandedDescription = "Danh sách nhân vật đang mở",
                    collapsedDescription = "Danh sách nhân vật đang đóng")
            }
        }
        val trigger = compose.onNodeWithContentDescription("Nhân vật: Người dẫn chuyện")
        listOf(
            ButtonState.Disabled(DisabledReason.of("Chưa chọn bản nháp")),
            ButtonState.Loading(DisabledReason.of("Đang nạp bản nháp")),
        ).forEach { blocked ->
            trigger.assertIsEnabled().performClick()
            compose.onNodeWithText("Nguyễn Thị Minh Phương").assertExists()
            compose.runOnIdle { state = blocked }
            trigger.assertIsNotEnabled()
            compose.onNodeWithText("Nguyễn Thị Minh Phương").assertDoesNotExist()
            compose.runOnIdle { state = ButtonState.Enabled }
            trigger.assertIsEnabled().assert(SemanticsMatcher.expectValue(
                SemanticsProperties.StateDescription, "Danh sách nhân vật đang đóng"))
            compose.onNodeWithText("Nguyễn Thị Minh Phương").assertDoesNotExist()
        }
    }

    @Test
    fun narrow_filter_keeps_component_picker_and_detail_in_agreement() {
        compose.setContent {
            Box(Modifier.requiredSize(420.dp, 840.dp)) { PreviewApp() }
        }
        val search = compose.onNode(hasSetTextAction() and hasText("Find component"))
        search.performTextReplacement("text-field")
        compose.onNodeWithContentDescription("Component: TextField").assertExists()
        compose.onNodeWithText("Lưu · Primary").assertDoesNotExist()
        compose.onNodeWithText("Tên tác phẩm").assertExists()
        search.performTextReplacement("không tồn tại")
        compose.onNodeWithText("Tên tác phẩm").assertDoesNotExist()
        compose.onNodeWithText("No results").assertExists()
        search.performTextReplacement("")
        compose.onNodeWithContentDescription("Component: Button").assertExists()
        compose.onNodeWithText("Lưu · Primary").assertExists()
    }

    @Test
    fun readonly_filled_text_remains_enabled_without_a_text_edit_action() {
        compose.setContent {
            UiRoot {
                TextField("Tên tác phẩm", "Ánh đèn cuối sân khấu · Ỗ, Ữ", {}, state = FieldState.ReadOnly)
            }
        }
        compose.onNodeWithText("Ánh đèn cuối sân khấu · Ỗ, Ữ").assertIsEnabled()
            .assert(hasSetTextAction().not())
    }
}
