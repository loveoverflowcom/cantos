package com.cantos.ui

import androidx.compose.ui.input.key.Key
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class ChoiceContractTest {
    @Test
    fun ranges_reject_nan_infinity_empty_descending_outside_and_tick_overflow() {
        for (value in listOf(Float.NaN, Float.POSITIVE_INFINITY, -1f, 101f)) {
            assertFailsWith<IllegalArgumentException> { validateRange(value, 0f..100f, 0) }
        }
        for (range in listOf(1f..1f, 2f..1f, Float.NaN..1f, 0f..Float.POSITIVE_INFINITY, -Float.MAX_VALUE..Float.MAX_VALUE)) {
            assertFailsWith<IllegalArgumentException> { validateRange(0f, range, 0) }
        }
        for (steps in listOf(-1, 1001, Int.MAX_VALUE)) {
            assertFailsWith<IllegalArgumentException> { validateRange(0f, 0f..100f, steps) }
        }
        assertFailsWith<IllegalArgumentException> { validateRange(45f, 0f..100f, 3) }
        validateRange(50f, 0f..100f, 3)
        validateRange(1.25f, 0f..2f, 0)
    }

    @Test
    fun option_constructor_and_copy_preserve_nonblank_identity_and_label() {
        assertFailsWith<IllegalArgumentException> { SelectOption("", "Người dẫn chuyện") }
        assertFailsWith<IllegalArgumentException> { SelectOption("n", " ") }
        val option = SelectOption("n", "Người dẫn chuyện")
        assertFailsWith<IllegalArgumentException> { option.copy(value = " ") }
        assertFailsWith<IllegalArgumentException> { option.copy(label = " ") }
        assertEquals("n", option.copy(label = "Lời dẫn").value)
    }

    @Test
    fun link_dispatch_contract_accepts_local_and_https_without_network_path_or_control_characters() {
        for (uri in listOf("/draft/example", "#ghi-chu", "https://example.com/draft?q=1#note")) assertTrue(supportedLink(uri))
        for (uri in listOf("", "#", "//example.com", "https:///draft", "http://example.com", "javascript:alert(1)", "/draft\nnew", "/draft\\new")) assertFalse(supportedLink(uri))
    }

    @Test
    fun tab_keys_have_bounded_targets_and_do_not_intercept_typing_keys() {
        assertEquals(0, tabKeyTarget(0, 3, Key.DirectionLeft))
        assertEquals(2, tabKeyTarget(2, 3, Key.DirectionRight))
        assertEquals(0, tabKeyTarget(2, 3, Key.MoveHome))
        assertEquals(2, tabKeyTarget(0, 3, Key.MoveEnd))
        assertEquals(null, tabKeyTarget(0, 3, Key.A))
    }

    @Test
    fun range_keyboard_uses_interior_ticks_finite_endpoints_and_continuous_hundredths() {
        assertEquals(25f, rangeKeyValue(0f, 0f..100f, 3, Key.DirectionRight))
        assertEquals(0f, rangeKeyValue(0f, 0f..100f, 3, Key.DirectionLeft))
        assertEquals(100f, rangeKeyValue(75f, 0f..100f, 3, Key.MoveEnd))
        assertEquals(1f, rangeKeyValue(0f, 0f..100f, 0, Key.DirectionRight))
        assertEquals(null, rangeKeyValue(0f, 0f..100f, 0, Key.A))
    }


    @Test
    fun stepped_ranges_reject_collapsed_float_ticks_before_dispatch() {
        for ((range, steps) in listOf(1_000_000f..1_000_001f to 19, 1_048_575.875f..1_048_576.5f to 6)) {
            assertFailsWith<IllegalArgumentException> { validateRange(range.start, range, steps) }
        }
    }

    @Test
    fun accepted_stepped_ranges_accept_every_forward_and_reverse_keyboard_output() {
        for ((range, steps) in listOf(0f..100f to 3, 0f..2f to 19, 1_048_575.875f..1_048_576.5f to 3)) {
            var value = range.start
            repeat(steps + 2) {
                validateRange(value, range, steps)
                value = requireNotNull(rangeKeyValue(value, range, steps, Key.DirectionRight))
            }
            assertEquals(range.endInclusive, value)
            repeat(steps + 2) {
                validateRange(value, range, steps)
                value = requireNotNull(rangeKeyValue(value, range, steps, Key.DirectionLeft))
            }
            assertEquals(range.start, value)
        }
    }

    @Test
    fun fractional_controlled_values_and_native_tick_callbacks_share_the_pinned_float_interpolation() {
        val range = 0.1f..0.9f
        validateRange(0.3f, range, 3)
        validateRange(0.7f, range, 3)
        val next = rangeKeyValue(0.1f, range, 3, Key.DirectionRight)
        assertEquals(0.3f, next)
        validateRange(requireNotNull(next), range, 3)
        assertFailsWith<IllegalArgumentException> { validateRange(0.45f, range, 3) }
    }
}
