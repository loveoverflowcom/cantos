package com.cantos.ui

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.requiredWidth
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.Text
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithContentDescription
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performSemanticsAction
import androidx.compose.ui.test.performTextReplacement
import androidx.compose.ui.test.performTextInputSelection
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.unit.dp
import com.cantos.ui.generated.CantosTokens
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import org.junit.Rule
import org.junit.Test

/** Actual shared wrappers on the JVM Compose renderer, not Android/iOS or TalkBack. */
class AuthAccountComponentTest {
    @get:Rule
    val compose = createComposeRule()

    @Test
    fun auth_action_emits_one_intent_before_host_loading_blocks_repeated_activation() {
        var state by mutableStateOf<ButtonState>(ButtonState.Enabled)
        var calls = 0
        val label = "Đăng nhập với Cantos ID"
        compose.setContent {
            UiRoot {
                Button(AccessibleName.of(label), {
                    calls += 1
                    state = ButtonState.Loading(DisabledReason.of("Đang mở Cantos ID, không gửi lần hai."))
                }, state = state, variant = ButtonVariant.AuthPrimary, icon = "arrow_forward") {
                    Text(label)
                }
            }
        }
        val action = compose.onNodeWithContentDescription(label)
        compose.onAllNodesWithContentDescription(label, useUnmergedTree = true).assertCountEquals(1)
        action.assertIsEnabled().performClick().assertIsNotEnabled()
        action.performSemanticsAction(SemanticsActions.OnClick) { it() }
        compose.onNodeWithText("Đang mở Cantos ID, không gửi lần hai.").assertExists()
        action.assert(SemanticsMatcher.expectValue(SemanticsProperties.StateDescription, "Đang mở Cantos ID, không gửi lần hai."))
        assertEquals(1, calls)
    }

    @Test
    fun every_auth_variant_keeps_native_role_and_refuses_disabled_callback() {
        val reason = "Kết nối tạm ngừng, nội dung vẫn được giữ lại."
        val calls = mutableListOf<ButtonVariant>()
        compose.setContent {
            UiRoot {
                Column {
                    listOf(ButtonVariant.AuthPrimary, ButtonVariant.AuthSecondary, ButtonVariant.AuthOutlined).forEach { variant ->
                        Button("Mẫu $variant", { calls += variant }, variant = variant,
                            state = ButtonState.Disabled(DisabledReason.of(reason)))
                    }
                }
            }
        }
        listOf(ButtonVariant.AuthPrimary, ButtonVariant.AuthSecondary, ButtonVariant.AuthOutlined).forEach { variant ->
            compose.onNodeWithContentDescription("Mẫu $variant").assertIsNotEnabled()
                .assert(SemanticsMatcher.expectValue(SemanticsProperties.Role, Role.Button))
                .performSemanticsAction(SemanticsActions.OnClick) { it() }
        }
        assertEquals(emptyList(), calls)
    }

    @Test
    fun back_only_dispatches_its_host_intent_and_has_one_named_focusable_target() {
        val name = "Quay lại Tài khoản, giữ nguyên nội dung đã nhập"
        val focus = FocusRequester()
        var calls = 0
        var state by mutableStateOf<ButtonState>(ButtonState.Enabled)
        compose.setContent {
            UiRoot {
                BackButton(AccessibleName.of(name), { calls += 1 }, state = state,
                    controlModifier = Modifier.focusRequester(focus))
            }
        }
        val back = compose.onNodeWithContentDescription(name)
        compose.onAllNodesWithContentDescription(name, useUnmergedTree = true).assertCountEquals(1)
        val before = back.fetchSemanticsNode().boundsInRoot
        assertTrue(before.width >= 48f && before.height >= 48f)
        compose.runOnIdle { focus.requestFocus() }
        back.assertIsFocused().performClick()
        assertEquals(before, back.fetchSemanticsNode().boundsInRoot)
        assertEquals(1, calls)
        listOf(
            ButtonState.Loading(DisabledReason.of("Đang quay lại, không gửi lần hai.")),
            ButtonState.Disabled(DisabledReason.of("Chưa thể quay lại, nội dung vẫn được giữ.")),
        ).forEach { blocked ->
            compose.runOnIdle { state = blocked }
            back.assertIsNotEnabled().performSemanticsAction(SemanticsActions.OnClick) { it() }
            compose.onNodeWithText(requireNotNull(blocked.reason())).assertExists()
            assertEquals(1, calls)
        }
    }

    @Test
    fun tonal_field_retains_raw_vietnamese_text_and_exposes_exact_error_without_erasing_it() {
        val help = "Giữ nguyên nội dung đã nhập."
        val error = "Tên hiển thị cần được kiểm tra."
        var value by mutableStateOf("Nguyễn Thị Minh Phương · Ỗ, Ữ")
        compose.setContent {
            UiRoot {
                TextField("Tên hiển thị", value, { value = it }, variant = TextFieldVariant.AuthTonal,
                    help = help, error = error, controlModifier = Modifier.testTag("auth-field"))
            }
        }
        val field = compose.onNodeWithTag("auth-field")
        field.assert(SemanticsMatcher.expectValue(SemanticsProperties.Error, error))
            .assert(SemanticsMatcher.expectValue(SemanticsProperties.StateDescription, "$help. $error"))
        compose.onNodeWithText(help).assertExists()
        compose.onNodeWithText(error).assertExists()
        field.performTextReplacement("  Đỗ Văn Ánh · Ỗ, Ữ  ")
        compose.runOnIdle { assertEquals("  Đỗ Văn Ánh · Ỗ, Ữ  ", value) }
    }

    @Test
    fun tonal_rich_field_keeps_external_composition_and_readonly_selection_without_edit_intents() {
        var value by mutableStateOf(TextFieldValue("Người dẫn chuyện · Ỗ, Ữ", TextRange(0, 5), TextRange(0, 5)))
        var theme by mutableStateOf(Theme.Light)
        var state by mutableStateOf<FieldState>(FieldState.Editable)
        compose.setContent {
            UiRoot(preferences = UiPreferences(theme = theme)) {
                TextField("Tên hiển thị minh họa", value, { value = it }, state = state,
                    variant = TextFieldVariant.AuthTonal, controlModifier = Modifier.testTag("rich-auth-field"))
            }
        }
        val field = compose.onNodeWithTag("rich-auth-field")
        field.assert(SemanticsMatcher.expectValue(SemanticsProperties.TextSelectionRange, TextRange(0, 5)))
        compose.runOnIdle { theme = Theme.Dark }
        assertEquals(TextRange(0, 5), value.composition)
        field.assert(SemanticsMatcher.expectValue(SemanticsProperties.TextSelectionRange, TextRange(0, 5)))
        field.performTextInputSelection(TextRange(6, 9))
        assertEquals(TextRange(6, 9), value.selection)
        compose.runOnIdle {
            value = TextFieldValue("Nguyễn Thị Minh Phương · Ỗ, Ữ")
            state = FieldState.ReadOnly
        }
        field.assertIsEnabled().assert(hasSetTextAction().not()).performTextInputSelection(TextRange(0, 7))
        assertEquals("Nguyễn Thị Minh Phương · Ỗ, Ữ", value.text)
        assertEquals(TextRange(0, 7), value.selection)
    }

    @Test
    fun tonal_field_disabled_and_loading_block_native_set_text_and_keep_reason_visible() {
        var state by mutableStateOf<FieldState>(FieldState.Editable)
        var calls = 0
        compose.setContent {
            UiRoot {
                TextField("Tên hiển thị", "Nguyễn Thị Minh Phương", { calls += 1 }, state = state,
                    variant = TextFieldVariant.AuthTonal, controlModifier = Modifier.testTag("auth-field"))
            }
        }
        listOf(
            FieldState.Disabled(DisabledReason.of("Chưa thể chỉnh sửa.")),
            FieldState.Loading(DisabledReason.of("Đang nạp thông tin.")),
        ).forEach { blocked ->
            compose.runOnIdle { state = blocked }
            val field = compose.onNodeWithTag("auth-field").assertIsNotEnabled()
            if (!field.fetchSemanticsNode().config.contains(SemanticsActions.SetText)) {
                field.assert(SemanticsMatcher.keyNotDefined(SemanticsActions.SetText))
            } else {
                // Exercise an exposed native edit even though input events cannot reach a disabled field.
                field.performSemanticsAction(SemanticsActions.SetText) { edit ->
                    assertEquals(false, edit(AnnotatedString("Không được thay thế nội dung")))
                }
            }
            compose.onNodeWithText(requireNotNull(blocked.reason())).assertExists()
            compose.onNodeWithText("Nguyễn Thị Minh Phương").assertExists()
            assertEquals(0, calls)
        }
        assertEquals(0, calls)
        compose.onNodeWithText("Nguyễn Thị Minh Phương").assertExists()
    }

    @Test
    fun auth_long_text_reflows_inside_intrinsic_control_bounds_at_two_times_scale() {
        val label = "Đăng nhập với Cantos ID để giữ tủ sách của Nguyễn Thị Minh Phương · Ỗ, Ữ"
        compose.setContent {
            UiRoot(preferences = UiPreferences(textScale = 2f)) {
                Box(Modifier.requiredWidth(324.dp).testTag("viewport")) {
                    Button(label, {}, variant = ButtonVariant.AuthPrimary, controlModifier = Modifier.fillMaxWidth())
                }
            }
        }
        val button = compose.onNodeWithContentDescription(label).fetchSemanticsNode().boundsInRoot
        val text = compose.onNodeWithText(label, useUnmergedTree = true).fetchSemanticsNode().boundsInRoot
        val viewport = compose.onNodeWithTag("viewport").fetchSemanticsNode().boundsInRoot
        assertTrue(button.height > 60f, "Large text must grow beyond the proposed minimum rather than clip")
        assertTrue(text.left >= button.left && text.right <= button.right)
        assertTrue(text.top >= button.top && text.bottom <= button.bottom)
        assertTrue(button.left >= viewport.left && button.right <= viewport.right)
    }

    @Test
    fun profile_surface_inherits_the_primary_container_foreground_in_both_themes() {
        var foreground = Color.Unspecified
        var theme by mutableStateOf(Theme.Light)
        compose.setContent {
            UiRoot(preferences = UiPreferences(theme = theme)) {
                Surface(tone = SurfaceTone.PrimaryContainer, variant = SurfaceVariant.Profile) {
                    foreground = LocalContentColor.current
                    Text("Chào, Nguyễn Thị Minh Phương.")
                }
            }
        }
        Theme.entries.forEach { value ->
            compose.runOnIdle { theme = value }
            compose.waitForIdle()
            assertEquals(parseTokenColor(CantosTokens.color("color.onPrimaryContainer", value.tokenName)), foreground)
        }
    }

    @Test
    fun primary_container_help_and_disabled_reason_use_the_paired_foreground_without_changing_regular_metadata() {
        var theme by mutableStateOf(Theme.Light)
        val help = "Thông tin minh họa trên nền hồ sơ."
        val reason = "Chưa kết nối tài khoản, không gửi yêu cầu đồng bộ."
        compose.setContent {
            UiRoot(preferences = UiPreferences(theme = theme)) {
                Column {
                    Surface(tone = SurfaceTone.PrimaryContainer, variant = SurfaceVariant.Profile) {
                        Column {
                            FieldHelp(help)
                            Button("Đồng bộ minh họa", {}, variant = ButtonVariant.AuthSecondary,
                                state = ButtonState.Disabled(DisabledReason.of(reason)))
                            Surface(tone = SurfaceTone.Container) { FieldHelp("Metadata trong surface thường") }
                        }
                    }
                    FieldHelp("Metadata ngoài surface")
                }
            }
        }
        Theme.entries.forEach { value ->
            compose.runOnIdle { theme = value }
            listOf(help, reason, "Metadata trong surface thường", "Metadata ngoài surface").forEach { text ->
                val role = if (text == help || text == reason) "color.onPrimaryContainer" else "color.onSurfaceVariant"
                val expected = parseTokenColor(CantosTokens.color(role, value.tokenName))
                compose.onNodeWithText(text).performSemanticsAction(SemanticsActions.GetTextLayoutResult) { getLayout ->
                    val layouts = mutableListOf<TextLayoutResult>()
                    assertTrue(getLayout(layouts))
                    assertEquals(expected, layouts.single().layoutInput.style.color, "$text / $value")
                }
            }
        }
    }
}
