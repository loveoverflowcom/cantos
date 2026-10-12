package com.cantos.ui

import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.input.TextFieldValue
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class FieldContractTest {
    @Test
    fun empty_messages_have_no_error_or_description() {
        listOf(null, "", " ", "\n\t").forEach { blank ->
            assertEquals(FieldMessages(null, null, null), fieldMessages(FieldState.Editable, blank, blank))
        }
        val blocked = fieldMessages(FieldState.Disabled(DisabledReason.of("Chưa chọn bản nháp")), " ", "")
        assertEquals("Chưa chọn bản nháp", blocked.description)
        val messages = fieldMessages(FieldState.Editable, "Giữ nguyên dấu", "Cần chọn tên")
        assertEquals("Giữ nguyên dấu. Cần chọn tên", messages.description)
        assertEquals("Cần chọn tên", messages.error)
    }

    @Test
    fun readonly_accepts_only_selection_changes_and_unavailable_never_dispatches() {
        val previous = TextFieldValue("Người dẫn chuyện · Ỗ, Ữ", TextRange(5), TextRange(0, 5))
        val selection = previous.copy(selection = TextRange(0, 5))
        val edit = previous.copy(text = "Nguyễn Thị Minh Phương")
        val commit = previous.copy(composition = null)
        assertTrue(FieldState.ReadOnly.acceptsValueChange(previous, selection))
        assertFalse(FieldState.ReadOnly.acceptsValueChange(previous, edit))
        assertFalse(FieldState.ReadOnly.acceptsValueChange(previous, commit))
        listOf(
            FieldState.Disabled(DisabledReason.of("Chưa chọn bản nháp")),
            FieldState.Loading(DisabledReason.of("Đang nạp bản nháp")),
        ).forEach { blocked ->
            assertFalse(blocked.acceptsValueChange(previous, selection))
            assertFalse(blocked.acceptsValueChange(previous, edit))
        }
        listOf(selection, edit, commit).forEach { next ->
            assertTrue(FieldState.Editable.acceptsValueChange(previous, next))
        }
    }
}
