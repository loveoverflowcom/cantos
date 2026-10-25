package com.cantos.ui

import androidx.compose.foundation.layout.size
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertContentDescriptionEquals
import androidx.compose.ui.test.assertHeightIsAtLeast
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.assertWidthIsAtLeast
import androidx.compose.ui.test.assertWidthIsEqualTo
import androidx.compose.ui.test.assertHeightIsEqualTo
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performSemanticsAction
import androidx.compose.ui.test.performTextInputSelection
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.unit.dp
import com.cantos.ui.generated.CantosTokens
import kotlin.test.assertEquals
import kotlin.test.assertNotEquals
import kotlin.test.assertTrue
import org.junit.Rule
import org.junit.Test

/**
 * The Theatre expression, the Floating field with its trailing action, the tonal/docked icon
 * buttons and the Theatre chip as production wrappers on the JVM Compose renderer. Not Android/iOS,
 * TalkBack/VoiceOver or a real IME.
 */
class TheatreFoundationTest {
    @get:Rule
    val compose = createComposeRule()

    private val theatre = UiPreferences(expression = CantosExpression.Theatre)

    @Test
    fun the_expression_remaps_roles_and_weights_only_inside_its_own_subtree() {
        var outerPrimary: Color? = null
        var innerPrimary: Color? = null
        var restoredPrimary: Color? = null
        val weights = mutableMapOf<String, FontWeight?>()
        compose.setContent {
            UiRoot {
                outerPrimary = tokenColor("color.primary")
                weights["outer-headline"] = MaterialTheme.typography.headlineMedium.fontWeight
                weights["outer-label"] = MaterialTheme.typography.labelLarge.fontWeight
                UiRoot(preferences = theatre) {
                    innerPrimary = tokenColor("color.primary")
                    weights["inner-headline"] = MaterialTheme.typography.headlineMedium.fontWeight
                    weights["inner-label"] = MaterialTheme.typography.labelLarge.fontWeight
                    weights["inner-body"] = MaterialTheme.typography.bodyMedium.fontWeight
                    UiRoot(preferences = UiPreferences()) { restoredPrimary = tokenColor("color.primary") }
                }
            }
        }
        compose.waitForIdle()
        assertEquals(parseTokenColor(CantosTokens.color("color.primary", "light")), outerPrimary)
        assertEquals(parseTokenColor(CantosTokens.color("color.theatrePrimary", "light")), innerPrimary)
        assertEquals(outerPrimary, restoredPrimary, "a nested standard theme leaves the expression")
        assertNotEquals(outerPrimary, innerPrimary)
        assertEquals(FontWeight(500), weights["outer-headline"])
        assertEquals(FontWeight(700), weights["inner-headline"])
        assertEquals(FontWeight(600), weights["outer-label"])
        assertEquals(FontWeight(700), weights["inner-label"])
        assertEquals(FontWeight(400), weights["inner-body"], "body copy is not an emphasised role")
    }

    private fun androidx.compose.ui.test.SemanticsNodeInteraction.rect() = fetchSemanticsNode().boundsInRoot

    @Test
    fun a_trailing_eye_is_a_named_48_unit_control_inside_the_field_with_one_callback_per_press() {
        var visible by mutableStateOf(false)
        var presses = 0
        var edits = 0
        var value by mutableStateOf("mật-khẩu-Ỗ-Ữ")
        compose.setContent {
            UiRoot(preferences = theatre) {
                TextField("Mật khẩu", value, { edits += 1; value = it }, variant = TextFieldVariant.Floating,
                    visualTransformation = if (visible) VisualTransformation.None else PasswordVisualTransformation(),
                    controlModifier = Modifier.testTag("password"),
                    trailing = {
                        IconButton(
                            AccessibleName.of(if (visible) "Ẩn mật khẩu" else "Hiện mật khẩu"),
                            if (visible) "visibility_off" else "visibility",
                            { presses += 1; visible = !visible },
                            controlModifier = Modifier.testTag("eye"),
                        )
                    })
            }
        }
        val eye = compose.onNodeWithTag("eye")
        eye.assertContentDescriptionEquals("Hiện mật khẩu").assertWidthIsAtLeast(48.dp).assertHeightIsAtLeast(48.dp)
        val field = compose.onNodeWithTag("password").rect()
        val action = eye.rect()
        assertTrue(action.left >= field.left && action.right <= field.right, "the action stays inside the shell: $action in $field")
        assertTrue(action.top >= field.top && action.bottom <= field.bottom, "the action stays inside the shell: $action in $field")
        assertEquals(field.center.y, action.center.y, 1.5f, "the action is centered on the shell's axis")
        eye.performClick()
        compose.waitForIdle()
        assertEquals(1, presses)
        assertEquals(0, edits, "the action never edits the value")
        assertEquals("mật-khẩu-Ỗ-Ữ", value)
        compose.onNodeWithTag("eye").assertContentDescriptionEquals("Ẩn mật khẩu")
        compose.onNodeWithTag("eye").performClick()
        compose.waitForIdle()
        assertEquals(2, presses)
        compose.onNodeWithTag("eye").assertContentDescriptionEquals("Hiện mật khẩu")
    }

    @Test
    fun the_rich_value_path_keeps_selection_and_composition_across_a_trailing_press() {
        var value by mutableStateOf(TextFieldValue("Người dẫn chuyện", TextRange(3, 7), TextRange(3, 7)))
        var presses = 0
        compose.setContent {
            UiRoot(preferences = theatre) {
                TextField("Tên hiển thị", value, { value = it }, variant = TextFieldVariant.Floating,
                    controlModifier = Modifier.testTag("name"),
                    trailing = {
                        IconButton(AccessibleName.of("Xóa tên"), "close", { presses += 1 }, controlModifier = Modifier.testTag("clear"))
                    })
            }
        }
        compose.onNodeWithTag("name").assert(SemanticsMatcher.expectValue(SemanticsProperties.TextSelectionRange, TextRange(3, 7)))
        compose.onNodeWithTag("clear").performClick()
        compose.waitForIdle()
        assertEquals(1, presses)
        assertEquals(TextRange(3, 7), value.selection)
        assertEquals(TextRange(3, 7), value.composition)
        assertEquals("Người dẫn chuyện", value.text)
        compose.onNodeWithTag("name").performTextInputSelection(TextRange(0, 2))
        assertEquals(TextRange(0, 2), value.selection)
    }

    @Test
    fun the_floating_field_keeps_the_native_error_and_description_contract_with_help_below_the_shell() {
        val help = "Dùng email đã đăng ký."
        val error = "Kiểm tra địa chỉ email."
        compose.setContent {
            UiRoot(preferences = theatre) {
                TextField("Email", "ban@", {}, variant = TextFieldVariant.Floating, help = help, error = error,
                    controlModifier = Modifier.testTag("email"))
            }
        }
        compose.onNodeWithTag("email")
            .assert(SemanticsMatcher.expectValue(SemanticsProperties.Error, error))
            .assert(SemanticsMatcher.expectValue(SemanticsProperties.StateDescription, "$help. $error"))
        compose.onNodeWithText(help).assertExists()
        compose.onNodeWithText(error).assertExists()
        // Messages are outside the painted shell, so the perimeter never wraps them.
        val shell = compose.onNodeWithTag("email").rect()
        assertTrue(compose.onNodeWithText(help).rect().top >= shell.bottom - 0.5f)
        assertTrue(compose.onNodeWithText(error).rect().top >= shell.bottom - 0.5f)
    }

    @Test
    fun a_floating_field_grows_with_text_scale_and_a_long_label_instead_of_clipping_or_overlapping() {
        var scale by mutableStateOf(1f)
        compose.setContent {
            UiRoot(preferences = theatre.copy(textScale = scale)) {
                TextField("Mật khẩu của Nguyễn Thị Ánh Nguyệt để tiếp tục câu chuyện", "Ỗ, Ữ", {},
                    variant = TextFieldVariant.Floating, controlModifier = Modifier.testTag("long"))
            }
        }
        val normal = compose.onNodeWithTag("long").rect()
        assertTrue(normal.height >= FloatingFieldMinHeight.value * 1f - 0.5f, "never below the 68-unit minimum: ${normal.height}")
        compose.runOnIdle { scale = 2f }
        compose.waitForIdle()
        val large = compose.onNodeWithTag("long").rect()
        assertTrue(large.height >= normal.height, "text scale grows the shell: ${normal.height} -> ${large.height}")
    }

    @Test
    fun a_blocked_floating_field_refuses_native_edits_and_keeps_its_reason_and_value_visible() {
        var state by mutableStateOf<FieldState>(FieldState.Editable)
        var calls = 0
        compose.setContent {
            UiRoot(preferences = theatre) {
                TextField("Tên hiển thị", "Nguyễn Thị Minh Phương", { calls += 1 }, state = state,
                    variant = TextFieldVariant.Floating, controlModifier = Modifier.testTag("blocked"))
            }
        }
        listOf(
            FieldState.Disabled(DisabledReason.of("Chưa thể chỉnh sửa.")),
            FieldState.Loading(DisabledReason.of("Đang nạp thông tin.")),
        ).forEach { blocked ->
            compose.runOnIdle { state = blocked }
            val field = compose.onNodeWithTag("blocked").assertIsNotEnabled()
            if (field.fetchSemanticsNode().config.contains(SemanticsActions.SetText)) {
                field.performSemanticsAction(SemanticsActions.SetText) { edit ->
                    assertEquals(false, edit(AnnotatedString("Không được thay thế")))
                }
            }
            compose.onNodeWithText(requireNotNull(blocked.reason())).assertExists()
            assertEquals(0, calls)
        }
    }

    @Test
    fun tonal_and_docked_icon_buttons_keep_a_48_unit_target_a_name_and_never_fire_while_blocked() {
        var presses = 0
        val pressed = mutableListOf<String>()
        var state by mutableStateOf<ButtonState>(ButtonState.Enabled)
        compose.setContent {
            UiRoot(preferences = theatre) {
                // Side by side: a hovered button's tooltip opens below it and must not cover its neighbour.
                androidx.compose.foundation.layout.Row {
                    IconButton(AccessibleName.of("Chuyển sang giao diện tối"), "dark_mode", { presses += 1; pressed += "theme" },
                        variant = ButtonVariant.Tonal, state = state, controlModifier = Modifier.testTag("theme"))
                    IconButton(AccessibleName.of("Tìm trong danh mục"), "search", { presses += 1; pressed += "search" },
                        variant = ButtonVariant.Primary, circular = true, state = state,
                        controlModifier = Modifier.size(tokenDp("size.searchAction")).testTag("search"))
                }
            }
        }
        compose.onNodeWithTag("theme").assertContentDescriptionEquals("Chuyển sang giao diện tối")
            .assertWidthIsEqualTo(48.dp).assertHeightIsEqualTo(48.dp)
        compose.onNodeWithTag("search").assertContentDescriptionEquals("Tìm trong danh mục")
            .assertWidthIsEqualTo(52.dp).assertHeightIsEqualTo(52.dp)
        compose.onNodeWithTag("theme").performClick()
        compose.onNodeWithTag("search").performClick()
        assertEquals(listOf("theme", "search"), pressed)
        compose.runOnIdle { state = ButtonState.Loading(DisabledReason.of("Đang xử lý.")) }
        compose.onNodeWithTag("theme").performClick()
        compose.onNodeWithTag("search").performClick()
        assertEquals(2, presses, "a blocked action dispatches nothing")
    }

    @Test
    fun the_default_icon_button_is_unchanged_a_plain_muted_glyph() {
        var presses = 0
        compose.setContent {
            UiRoot { IconButton(AccessibleName.of("Đóng"), "close", { presses += 1 }, controlModifier = Modifier.testTag("plain")) }
        }
        compose.onNodeWithTag("plain").assertContentDescriptionEquals("Đóng").assertWidthIsEqualTo(48.dp)
        compose.onNodeWithTag("plain").performClick()
        assertEquals(1, presses)
    }

    private data class ChipBounds(val before: androidx.compose.ui.geometry.Rect, val after: androidx.compose.ui.geometry.Rect)

    @OptIn(ExperimentalTestApi::class)
    private fun chipBounds(preferences: UiPreferences): ChipBounds {
        lateinit var result: ChipBounds
        runDesktopComposeUiTest {
            var selected by mutableStateOf(false)
            setContent {
                UiRoot(preferences = preferences) {
                    FilterChip("Tiểu thuyết", selected, { selected = it }, controlModifier = Modifier.testTag("chip"))
                }
            }
            val before = onNodeWithTag("chip").fetchSemanticsNode().boundsInRoot
            onNodeWithTag("chip").performClick()
            waitForIdle()
            val node = onNodeWithTag("chip")
            node.assert(SemanticsMatcher.expectValue(SemanticsProperties.Selected, true))
            assertEquals(listOf(AnnotatedString("Tiểu thuyết")), node.fetchSemanticsNode().config.getOrNull(SemanticsProperties.Text),
                "the check is decorative and never joins the accessible text")
            result = ChipBounds(before, node.fetchSemanticsNode().boundsInRoot)
        }
        return result
    }

    @Test
    fun a_theatre_chip_keeps_its_start_edge_and_48_unit_height_when_selected_and_only_grows_by_the_check() {
        val bounds = chipBounds(theatre)
        assertEquals(bounds.before.left, bounds.after.left, 0.5f, "the press point stays inside the chip")
        assertEquals(bounds.before.top, bounds.after.top, 0.5f)
        assertEquals(bounds.before.height, bounds.after.height, 0.5f)
        assertTrue(bounds.after.height >= 48f - 0.5f, "hit area stays at least 48 units: ${bounds.after.height}")
        assertTrue(bounds.after.width > bounds.before.width, "the selected chip gains its check")
    }

    @Test
    fun the_control_avatar_is_48_units_and_the_accent_does_not_change_its_name_or_initials() {
        compose.setContent {
            UiRoot(preferences = theatre) {
                Avatar("Cantos", size = AvatarSize.Control, accent = true,
                    accessibleName = AccessibleName.of("Tài khoản Cantos"), modifier = Modifier.testTag("avatar"))
            }
        }
        compose.onNodeWithTag("avatar").assertWidthIsEqualTo(48.dp).assertHeightIsEqualTo(48.dp)
            .assertContentDescriptionEquals("Tài khoản Cantos")
        assertEquals("C", avatarInitials("Cantos"))
    }
}
