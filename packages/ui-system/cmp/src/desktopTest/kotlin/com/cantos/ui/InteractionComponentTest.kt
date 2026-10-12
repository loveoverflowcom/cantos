package com.cantos.ui

import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.setValue
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.requiredWidth
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.assertIsSelected
import androidx.compose.ui.test.assertIsOn
import androidx.compose.ui.test.assertIsOff
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performKeyInput
import androidx.compose.ui.test.pressKey
import androidx.compose.ui.test.performSemanticsAction
import androidx.compose.ui.test.performTextReplacement
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.DialogProperties
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.click
import org.junit.Rule
import org.junit.Test
import kotlin.test.assertEquals
import kotlin.math.nextDown

/** Production wrappers on the JVM renderer; pointer injection is not native OS input. */
@OptIn(androidx.compose.ui.test.ExperimentalTestApi::class)
class InteractionComponentTest {
    @get:Rule
    val compose = createComposeRule()

    @Test
    fun choice_labels_activate_one_controlled_target_without_changing_unaccepted_state() {
        val calls = mutableListOf<String>()
        compose.setContent {
            UiRoot {
                Stack {
                    Checkbox("Giữ ghi chú", false, { calls += "checkbox:$it" })
                    Switch("Hiện lời dẫn", false, { calls += "switch:$it" })
                    Radio("Bản nháp", false, { calls += "radio" })
                }
            }
        }
        listOf("Giữ ghi chú", "Hiện lời dẫn", "Bản nháp").forEach { label ->
            compose.onNodeWithText(label).performTouchInput { click() }
        }
        assertEquals(listOf("checkbox:true", "switch:true", "radio"), calls)
        compose.onNodeWithText("Giữ ghi chú").performTouchInput { click() }
        assertEquals("checkbox:true", calls.last())
    }

    @Test
    fun range_keeps_fractional_value_in_its_visible_label() {
        compose.setContent { UiRoot { RangeField("Tốc độ", 1.25f, {}, range = 0f..2f) } }
        compose.onNodeWithText("Tốc độ: 1.25").assertExists()
    }

    @Test
    fun choice_messages_leave_readable_space_beyond_the_focus_ring_without_focus_layout_shift() {
        var kind by mutableStateOf(0)
        var preferences by mutableStateOf(UiPreferences())
        val focus = FocusRequester()
        val label = "Giữ nguyên ghi chú phát âm tiếng Việt"
        val help = "Ghi chú được giữ nguyên khi duyệt lựa chọn."
        val error = "Cần duyệt lựa chọn trước khi tiếp tục."
        var focusOutsetPx = 0f
        var messageSpacePx = 0f
        compose.setContent {
            UiRoot(preferences = preferences) {
                with(LocalDensity.current) {
                    focusOutsetPx = (tokenDp("size.focusOffset") + tokenDp("size.focusWidth")).toPx()
                    messageSpacePx = tokenDp("space.xs").toPx()
                }
                Box(Modifier.requiredWidth(300.dp)) {
                    val control = Modifier.focusRequester(focus)
                    when (kind) {
                        0 -> Checkbox(label, false, {}, controlModifier = control, help = help, error = error)
                        1 -> Radio(label, false, {}, controlModifier = control, help = help, error = error)
                        2 -> Switch(label, false, {}, controlModifier = control, help = help, error = error)
                    }
                }
            }
        }
        for (theme in Theme.entries) {
            for (density in Density.entries) {
                for (scale in listOf(1f, 2f)) {
                    for (choice in 0..2) {
                        compose.runOnIdle {
                            kind = choice
                            preferences = UiPreferences(theme = theme, density = density, textScale = scale)
                        }
                        val control = compose.onNodeWithContentDescription(label)
                        val beforeFocus = control.fetchSemanticsNode().boundsInRoot
                        compose.runOnIdle { focus.requestFocus() }
                        control.assertIsFocused()
                        val bounds = control.fetchSemanticsNode().boundsInRoot
                        assertEquals(beforeFocus, bounds, "Focus must not move the control")
                        val helper = compose.onNodeWithText(help).fetchSemanticsNode().boundsInRoot
                        val failure = compose.onNodeWithText(error).fetchSemanticsNode().boundsInRoot
                        kotlin.test.assertTrue(
                            helper.top - bounds.bottom - focusOutsetPx >= messageSpacePx - 0.5f,
                            "Choice $choice / $theme / $density / $scale needs readable space outside the focus paint",
                        )
                        kotlin.test.assertTrue(
                            failure.top - helper.bottom >= messageSpacePx - 0.5f,
                            "Supporting messages must keep their own spacing",
                        )
                    }
                }
            }
        }
    }

    @Test
    fun choices_without_visible_messages_do_not_reserve_supporting_space() {
        compose.setContent {
            UiRoot {
                Checkbox(
                    "Giữ ghi chú", false, {}, modifier = Modifier.testTag("choice"),
                    controlModifier = Modifier.testTag("control"), help = " \n ", error = "\t",
                )
            }
        }
        val choice = compose.onNodeWithTag("choice").fetchSemanticsNode().boundsInRoot
        val control = compose.onNodeWithTag("control").fetchSemanticsNode().boundsInRoot
        assertEquals(control, choice)
    }

    @Test
    fun choices_have_one_named_role_focus_and_disabled_error_reason_at_each_density() {
        var density by mutableStateOf(Density.Compact)
        var state by mutableStateOf<ButtonState>(ButtonState.Enabled)
        var checked by mutableStateOf(false)
        val focus = FocusRequester()
        var calls = 0
        compose.setContent {
            UiRoot(preferences = UiPreferences(density = density)) {
                Checkbox("Giữ lời dẫn", checked, { checked = it; calls += 1 }, state = state,
                    controlModifier = Modifier.focusRequester(focus), error = "Cần duyệt ghi chú")
            }
        }
        val node = compose.onNodeWithContentDescription("Giữ lời dẫn")
        for (profile in Density.entries) {
            compose.runOnIdle { density = profile; checked = false }
            compose.onAllNodes(hasClickAction()).assertCountEquals(1)
            node.assert(SemanticsMatcher.expectValue(SemanticsProperties.Role, Role.Checkbox))
                .assert(SemanticsMatcher.expectValue(SemanticsProperties.Error, "Cần duyệt ghi chú"))
            compose.runOnIdle { focus.requestFocus() }
            node.assertIsFocused().performKeyInput { pressKey(Key.Spacebar) }
            node.assertIsOn()
            val bounds = node.fetchSemanticsNode().boundsInRoot
            kotlin.test.assertTrue(bounds.height >= if (profile == Density.Compact) 36f else 48f)
        }
        assertEquals(2, calls)
        compose.runOnIdle { state = ButtonState.Loading(DisabledReason.of("Đang giữ ghi chú")) }
        node.assertIsNotEnabled().assert(SemanticsMatcher.expectValue(
            SemanticsProperties.StateDescription, "Cần duyệt ghi chú. Đang giữ ghi chú"))
        node.performClick()
        assertEquals(2, calls)
        compose.onNodeWithText("Đang giữ ghi chú").assertExists()
    }

    @Test
    fun radio_repeat_does_not_toggle_selection_off_and_switch_requests_only_caller_values() {
        var selected by mutableStateOf(false)
        var checked by mutableStateOf(false)
        val calls = mutableListOf<String>()
        compose.setContent {
            UiRoot { Stack {
                Radio("Lời dẫn", selected, { selected = true; calls += "radio" })
                Switch("Hiện ghi chú", checked, { calls += "switch:$it" })
            } }
        }
        compose.onNodeWithText("Lời dẫn").performClick().performClick().assertIsSelected()
        compose.onNodeWithText("Hiện ghi chú").performClick().performClick().assertIsOff()
        assertEquals(listOf("radio", "radio", "switch:true", "switch:true"), calls)
        compose.runOnIdle { checked = true }
        compose.onNodeWithText("Hiện ghi chú").performClick().assertIsOn()
        assertEquals("switch:false", calls.last())
    }

    @Test
    fun stable_select_preserves_identity_rejects_unsupported_and_does_not_invent_empty_values() {
        val narrator = SelectOption("n", "Người dẫn chuyện")
        val actor = SelectOption("m", "Nguyễn Thị Minh Phương")
        val blocked = SelectOption("x", "Giọng chưa được duyệt", ButtonState.Disabled(DisabledReason.of("Thiếu quyền dùng giọng")))
        var options by mutableStateOf(listOf(narrator, actor, blocked))
        var value by mutableStateOf<String?>("n")
        val values = mutableListOf<String>()
        compose.setContent {
            UiRoot { Select("Nhân vật", options, value, { values += it },
                expandedDescription = "Mở", collapsedDescription = "Đóng", placeholder = "Chọn nhân vật",
                emptyDescription = "Chưa có nhân vật", error = "Cần duyệt lựa chọn") }
        }
        val trigger = compose.onNodeWithContentDescription("Nhân vật: Người dẫn chuyện")
        trigger.performClick()
        compose.onNodeWithText("Giọng chưa được duyệt").assertIsNotEnabled().performClick()
        assertEquals(emptyList(), values)
        compose.runOnIdle { options = options.reversed() }
        trigger.assertExists()
        compose.onNodeWithText("Nguyễn Thị Minh Phương").assertDoesNotExist()
        trigger.performClick()
        compose.onNodeWithText("Nguyễn Thị Minh Phương").performClick()
        assertEquals(listOf("m"), values)
        trigger.assertExists() // caller did not accept the request
        compose.runOnIdle { value = "m" }
        compose.onNodeWithContentDescription("Nhân vật: Nguyễn Thị Minh Phương").assertExists()
        compose.runOnIdle { options = emptyList() }
        compose.onNodeWithContentDescription("Nhân vật: Chọn nhân vật").assertIsNotEnabled()
            .assert(SemanticsMatcher.expectValue(SemanticsProperties.Error, "Cần duyệt lựa chọn"))
        compose.onNodeWithText("Chưa có nhân vật").assertExists()
        assertEquals(listOf("m"), values)
        compose.runOnIdle { options = listOf(narrator); value = "removed" }
        compose.onNodeWithContentDescription("Nhân vật: Chọn nhân vật").assertExists()
        assertEquals(listOf("m"), values)
    }

    @Test
    fun select_preserves_open_menu_for_equivalent_rebuilt_options_but_closes_on_availability_changes() {
        var parentRevision by mutableStateOf(0)
        var blockedReason by mutableStateOf("Thiếu quyền dùng giọng")
        var loading by mutableStateOf(false)
        var calls = 0
        compose.setContent {
            UiRoot { Stack {
                androidx.compose.material3.Text("Bản nháp: $parentRevision")
                val unavailable = if (loading) ButtonState.Loading(DisabledReason.of(blockedReason))
                    else ButtonState.Disabled(DisabledReason.of(blockedReason))
                // Rebuild identical values on an unrelated parent recomposition, as a caller may do.
                val options = listOf(
                    SelectOption("n", "Người dẫn chuyện"),
                    SelectOption("x", "Giọng chưa được duyệt", unavailable),
                )
                Select("Nhân vật", options, "n", { calls += 1 },
                    expandedDescription = "Mở", collapsedDescription = "Đóng",
                    placeholder = "Chọn nhân vật", emptyDescription = "Chưa có nhân vật")
            } }
        }
        val trigger = compose.onNodeWithContentDescription("Nhân vật: Người dẫn chuyện")
        trigger.performClick()
        compose.onNodeWithText("Giọng chưa được duyệt").assertIsNotEnabled()
        compose.runOnIdle { parentRevision += 1 }
        compose.onNodeWithText("Bản nháp: 1").assertExists()
        compose.onNodeWithText("Giọng chưa được duyệt").assertExists()
        trigger.assert(SemanticsMatcher.expectValue(SemanticsProperties.StateDescription, "Mở"))

        compose.runOnIdle { blockedReason = "Đang chờ quyền dùng giọng" }
        compose.onNodeWithText("Giọng chưa được duyệt").assertDoesNotExist()
        trigger.assert(SemanticsMatcher.expectValue(SemanticsProperties.StateDescription, "Đóng"))
        trigger.performClick()
        compose.onNodeWithText("Đang chờ quyền dùng giọng").assertExists()
        compose.runOnIdle { loading = true }
        compose.onNodeWithText("Giọng chưa được duyệt").assertDoesNotExist()
        trigger.assert(SemanticsMatcher.expectValue(SemanticsProperties.StateDescription, "Đóng"))
        assertEquals(0, calls)
    }

    @Test
    fun range_semantics_requests_controlled_steps_and_completion_and_blocks_unavailable_changes() {
        var value by mutableStateOf(0f)
        var state by mutableStateOf<ButtonState>(ButtonState.Enabled)
        val values = mutableListOf<Float>()
        var finishes = 0
        val focus = FocusRequester()
        compose.setContent {
            UiRoot { RangeField("Âm lượng", value, { values += it; value = it }, state = state,
                steps = 3, valueText = "${value.toInt()} trên 100", onValueChangeFinished = { finishes += 1 },
                controlModifier = Modifier.focusRequester(focus)) }
        }
        val node = compose.onNodeWithContentDescription("Âm lượng")
        compose.runOnIdle { focus.requestFocus() }
        node.assertIsFocused().performKeyInput { pressKey(Key.DirectionRight) }
        assertEquals(listOf(25f), values)
        assertEquals(1, finishes)
        node.performSemanticsAction(SemanticsActions.SetProgress) { it(75f) }
        assertEquals(listOf(25f, 75f), values)
        assertEquals(2, finishes)
        node.assert(SemanticsMatcher.expectValue(SemanticsProperties.StateDescription, "75 trên 100"))
        compose.runOnIdle { state = ButtonState.Disabled(DisabledReason.of("Không đổi âm lượng lúc này")) }
        node.assertIsNotEnabled()
        node.performSemanticsAction(SemanticsActions.SetProgress) { it(100f) }
        assertEquals(listOf(25f, 75f), values)
        assertEquals(2, finishes)
    }

    @Test
    fun negative_stepped_range_accessibility_requests_remain_valid_controlled_ticks() {
        var value by mutableStateOf(-100f)
        val values = mutableListOf<Float>()
        var finishes = 0
        compose.setContent {
            UiRoot { RangeField("Mức tín hiệu", value, { values += it; value = it }, range = -100f..0f,
                steps = 3, onValueChangeFinished = { finishes += 1 }) }
        }
        compose.onNodeWithContentDescription("Mức tín hiệu")
            .performSemanticsAction(SemanticsActions.SetProgress) { it(-40f) }
        compose.waitForIdle()
        assertEquals(listOf(-50f), values)
        assertEquals(1, finishes)
        compose.onNodeWithText("Mức tín hiệu: -50.0").assertExists()
    }

    @Test
    fun range_accessibility_rejects_nonfinite_and_unchanged_tick_requests_without_completion() {
        var value by mutableStateOf(-50f)
        val values = mutableListOf<Float>()
        var finishes = 0
        compose.setContent {
            UiRoot { RangeField("Mức tín hiệu", value, { values += it; value = it }, range = -100f..0f,
                steps = 3, onValueChangeFinished = { finishes += 1 }) }
        }
        val node = compose.onNodeWithContentDescription("Mức tín hiệu")
        for (request in listOf(-40f, Float.NaN, Float.POSITIVE_INFINITY, Float.NEGATIVE_INFINITY)) {
            node.performSemanticsAction(SemanticsActions.SetProgress) { kotlin.test.assertFalse(it(request)) }
        }
        assertEquals(emptyList(), values)
        assertEquals(0, finishes)
        node.performSemanticsAction(SemanticsActions.SetProgress) { kotlin.test.assertTrue(it(-200f)) }
        assertEquals(listOf(-100f), values)
        assertEquals(1, finishes)
    }

    @Test
    fun range_accessibility_keeps_the_caller_decimal_alias_when_its_tick_is_unchanged() {
        var calls = 0
        var finishes = 0
        compose.setContent {
            UiRoot { RangeField("Tốc độ", 0.7f, { calls += 1 }, range = 0.1f..0.9f, steps = 3,
                onValueChangeFinished = { finishes += 1 }) }
        }
        compose.onNodeWithContentDescription("Tốc độ")
            .performSemanticsAction(SemanticsActions.SetProgress) { kotlin.test.assertFalse(it(0.7f)) }
        assertEquals(0, calls)
        assertEquals(0, finishes)
        compose.onNodeWithText("Tốc độ: 0.7").assertExists()
    }

    @Test
    fun range_keyboard_keeps_a_decimal_alias_when_the_native_thumb_is_already_at_the_end() {
        var calls = 0
        var finishes = 0
        val focus = FocusRequester()
        compose.setContent {
            UiRoot { RangeField("Tốc độ", 0.9f.nextDown(), { calls += 1 }, range = 0.1f..0.9f, steps = 3,
                controlModifier = Modifier.focusRequester(focus), onValueChangeFinished = { finishes += 1 }) }
        }
        compose.runOnIdle { focus.requestFocus() }
        compose.onNodeWithContentDescription("Tốc độ").performKeyInput {
            pressKey(Key.MoveEnd); pressKey(Key.DirectionRight); pressKey(Key.PageUp)
        }
        assertEquals(0, calls)
        assertEquals(0, finishes)
    }

    @Test
    fun range_same_tick_comparison_matches_native_lower_tie_without_suppressing_a_new_tick() {
        val values = mutableListOf<Float>()
        var finishes = 0
        compose.setContent {
            UiRoot { RangeField("Mức tín hiệu", 1_000_000.0625f, { values += it },
                range = 1_000_000f..1_000_000.25f, steps = 1, onValueChangeFinished = { finishes += 1 }) }
        }
        val node = compose.onNodeWithContentDescription("Mức tín hiệu")
        node.assert(SemanticsMatcher.expectValue(SemanticsProperties.ProgressBarRangeInfo,
            androidx.compose.ui.semantics.ProgressBarRangeInfo(1_000_000f, 1_000_000f..1_000_000.25f, 1)))
        node.performSemanticsAction(SemanticsActions.SetProgress) { kotlin.test.assertFalse(it(1_000_000f)) }
        node.performSemanticsAction(SemanticsActions.SetProgress) { kotlin.test.assertTrue(it(1_000_000.125f)) }
        assertEquals(listOf(1_000_000.125f), values)
        assertEquals(1, finishes)
    }

    @Test
    fun range_preview_reports_the_complete_fraction_in_vietnamese_value_text() {
        compose.setContent { UiRoot {
            com.cantos.ui.preview.ComponentExample(PreviewExample.RangeField, "enabled", {}, Modifier)
        } }
        compose.onNodeWithContentDescription("Mức âm lượng thử")
            .performSemanticsAction(SemanticsActions.SetProgress) { it(43.5f) }
        compose.onNodeWithText("Mức âm lượng thử: 43,5 trên 100").assertExists()
    }

    @Test
    fun native_tabs_name_selection_keyboard_focus_overflow_and_repeated_callbacks() {
        var selected by mutableStateOf(0)
        var state by mutableStateOf<ButtonState>(ButtonState.Enabled)
        val values = mutableListOf<Int>()
        compose.setContent { UiRoot { Box(Modifier.requiredWidth(300.dp)) {
            NavTabs(listOf("Kịch bản", "Nguyễn Thị Minh Phương", "Lịch sử chỉnh sửa"), selected,
                { selected = it; values += it }, groupName = AccessibleName.of("Các phần bản nháp"), state = state)
        } } }
        compose.onNodeWithContentDescription("Các phần bản nháp").assertExists()
        val first = compose.onNodeWithText("Kịch bản")
        first.performSemanticsAction(SemanticsActions.RequestFocus) { it() }
        first.assertIsFocused().performKeyInput { pressKey(Key.DirectionRight) }
        val second = compose.onNodeWithText("Nguyễn Thị Minh Phương")
        second.assertIsSelected().assertIsFocused().performKeyInput { pressKey(Key.MoveEnd) }
        compose.onNodeWithText("Lịch sử chỉnh sửa").assertIsSelected().assertIsFocused()
            .performKeyInput { pressKey(Key.MoveHome) }
        first.assertIsSelected().assertIsFocused().performClick().performClick()
        assertEquals(listOf(1, 2, 0, 0, 0), values)
        compose.runOnIdle { state = ButtonState.Disabled(DisabledReason.of("Đang nạp bản nháp")) }
        first.assertIsNotEnabled().performClick()
        assertEquals(5, values.size)
    }

    @Test
    fun dialog_escape_close_and_reopen_restore_focus_and_keep_caller_text() {
        var open by mutableStateOf(false)
        var text by mutableStateOf("Lời dẫn")
        var requests = 0
        val trigger = FocusRequester()
        compose.setContent { UiRoot { Stack {
            Button("Mở bản nháp", { trigger.requestFocus(); open = true }, controlModifier = Modifier.focusRequester(trigger))
            Dialog(open, "Bản nháp Ánh đèn", "Đóng", { requests += 1; open = false }, returnFocus = trigger) {
                TextField("Ghi chú", text, { text = it })
            }
        } } }
        val opener = compose.onNodeWithText("Mở bản nháp")
        opener.performClick()
        val close = compose.onNodeWithText("Đóng")
        close.assertIsFocused().performKeyInput { pressKey(Key.Tab) }
        compose.onNodeWithText("Lời dẫn").assertIsFocused().performKeyInput { pressKey(Key.Tab) }
        close.assertIsFocused().performKeyInput { keyDown(Key.ShiftLeft); pressKey(Key.Tab); keyUp(Key.ShiftLeft) }
        compose.onNodeWithText("Lời dẫn").assertIsFocused().performTextReplacement("Nguyễn Thị Minh Phương · Ỗ, Ữ")
        compose.onNodeWithText("Nguyễn Thị Minh Phương · Ỗ, Ữ").performKeyInput { pressKey(Key.Escape) }
        assertEquals(1, requests)
        opener.assertIsFocused().performClick()
        compose.onNodeWithText("Nguyễn Thị Minh Phương · Ỗ, Ữ").assertExists()
        close.performClick()
        opener.assertIsFocused()
        assertEquals(2, requests)
    }

    @Test
    fun horizontal_stack_wraps_instead_of_clipping_the_second_intrinsic_child() {
        compose.setContent { UiRoot { Box(Modifier.requiredWidth(300.dp)) {
            Stack(direction = StackDirection.Horizontal, gap = Space.Lg) {
                Button("Người dẫn chuyện", {}, modifier = Modifier.testTag("first"))
                Button("Nguyễn Thị Minh Phương", {}, modifier = Modifier.testTag("second"))
            }
        } } }
        val first = compose.onNodeWithTag("first").fetchSemanticsNode().boundsInRoot
        val second = compose.onNodeWithTag("second").fetchSemanticsNode().boundsInRoot
        kotlin.test.assertTrue(second.top >= first.bottom, "Second child should wrap below the first")
        kotlin.test.assertTrue(second.right <= 300f)
    }

    @Test
    fun feedback_announcements_are_explicit_and_recovery_slot_keeps_native_callback() {
        var announcement by mutableStateOf(Announcement.Off)
        var calls = 0
        compose.setContent { UiRoot {
            ErrorState("Không thể lưu", "Ghi chú vẫn còn", modifier = Modifier.testTag("feedback"), announcement = announcement) {
                Button("Xem ghi chú", { calls += 1 })
            }
        } }
        val node = compose.onNodeWithTag("feedback")
        node.assert(SemanticsMatcher.keyNotDefined(SemanticsProperties.LiveRegion))
        compose.runOnIdle { announcement = Announcement.Assertive }
        node.assert(SemanticsMatcher.expectValue(SemanticsProperties.LiveRegion, androidx.compose.ui.semantics.LiveRegionMode.Assertive))
        compose.onNodeWithText("Xem ghi chú").performClick().performClick()
        assertEquals(2, calls)
    }

    @Test
    fun loading_preview_registered_announcement_is_explicit_and_default_stays_quiet() {
        var state by mutableStateOf("loading")
        compose.setContent { UiRoot {
            com.cantos.ui.preview.ComponentExample(PreviewExample.LoadingState, state, {}, Modifier.testTag("loading"))
        } }
        val node = compose.onNodeWithTag("loading")
        node.assert(SemanticsMatcher.keyNotDefined(SemanticsProperties.LiveRegion))
        compose.runOnIdle { state = "announcement" }
        node.assert(SemanticsMatcher.expectValue(SemanticsProperties.LiveRegion, androidx.compose.ui.semantics.LiveRegionMode.Polite))
    }

    @Test
    fun configured_dialog_initial_focus_and_controlled_dismissal_respect_properties() {
        var dismissOnEscape by mutableStateOf(false)
        var open by mutableStateOf(true)
        val initial = FocusRequester()
        var requests = 0
        compose.setContent { UiRoot {
            Dialog(open, "Bản nháp", "Đóng", { requests += 1 }, initialFocus = initial,
                properties = DialogProperties(dismissOnBackPress = dismissOnEscape, dismissOnClickOutside = false)) {
                TextField("Ghi chú", "Lời hẹn", {}, modifier = Modifier.focusRequester(initial))
            }
        } }
        val field = compose.onNodeWithText("Lời hẹn")
        field.assertIsFocused().performKeyInput { pressKey(Key.Escape) }
        assertEquals(0, requests)
        compose.runOnIdle { dismissOnEscape = true }
        field.performKeyInput { pressKey(Key.Escape) }
        assertEquals(1, requests)
        compose.onNodeWithText("Đóng").assertExists().performClick()
        assertEquals(2, requests)
        // Dismissal only requests; the caller deliberately kept the modal open.
        field.assertExists()
        compose.runOnIdle { open = false }
        field.assertDoesNotExist()
    }

    @Test
    fun select_escape_dismisses_without_callback_or_selection_change() {
        var calls = 0
        compose.setContent { UiRoot {
            Select("Nhân vật", listOf(SelectOption("n", "Người dẫn chuyện"), SelectOption("m", "Nguyễn Thị Minh Phương")), "n",
                { calls += 1 }, expandedDescription = "Mở", collapsedDescription = "Đóng",
                placeholder = "Chọn nhân vật", emptyDescription = "Chưa có nhân vật")
        } }
        val trigger = compose.onNodeWithContentDescription("Nhân vật: Người dẫn chuyện")
        trigger.performClick()
        compose.onNodeWithText("Nguyễn Thị Minh Phương").assertExists().performKeyInput { pressKey(Key.Escape) }
        compose.onNodeWithText("Nguyễn Thị Minh Phương").assertDoesNotExist()
        trigger.assert(SemanticsMatcher.expectValue(SemanticsProperties.StateDescription, "Đóng"))
        assertEquals(0, calls)
    }
}
