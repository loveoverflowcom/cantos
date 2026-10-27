package com.cantos.ui

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.assertIsNotSelected
import androidx.compose.ui.test.assertIsSelected
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import org.junit.Rule
import org.junit.Test
import kotlin.test.assertEquals

/** Production NavItem on the JVM renderer; pointer injection is not native OS input. */
class NavItemTest {
    @get:Rule
    val compose = createComposeRule()

    @Test
    fun selection_is_a_callback_request_and_the_caller_owns_the_selected_state() {
        var selected by mutableStateOf(false)
        val calls = mutableListOf<String>()
        compose.setContent {
            UiRoot {
                NavItem("Tác phẩm của tôi", selected, { calls += "select" }, controlModifier = Modifier.testTag("item"))
            }
        }
        compose.onNodeWithTag("item").assertIsNotSelected().performClick()
        assertEquals(listOf("select"), calls)
        compose.onNodeWithTag("item").assertIsNotSelected()
        selected = true
        compose.waitForIdle()
        compose.onNodeWithTag("item").assertIsSelected()
    }

    @Test
    fun blocked_states_refuse_activation_and_expose_the_reason() {
        var calls = 0
        compose.setContent {
            UiRoot {
                Stack {
                    NavItem("Đang nạp", false, { calls++ }, state = ButtonState.Loading(DisabledReason.of("Đang nạp")),
                        controlModifier = Modifier.testTag("loading"))
                    NavItem("Chưa mở", false, { calls++ }, state = ButtonState.Disabled(DisabledReason.of("Cần quyền biên tập")),
                        controlModifier = Modifier.testTag("disabled"))
                }
            }
        }
        compose.onNodeWithTag("loading").assertIsNotEnabled().performClick()
        compose.onNodeWithTag("disabled").assertIsNotEnabled().performClick()
        assertEquals(0, calls)
        compose.onNodeWithText("Cần quyền biên tập").assertExists()
    }

    @Test
    fun enabled_item_is_enabled_with_a_long_vietnamese_label() {
        compose.setContent {
            UiRoot {
                NavItem("Nguyễn Thị Minh Phương · Người dẫn chuyện Ánh đèn cuối sân khấu · Ỗ, Ữ", true, {},
                    controlModifier = Modifier.testTag("long"))
            }
        }
        compose.onNodeWithTag("long").assertIsEnabled().assertIsSelected()
    }
}
