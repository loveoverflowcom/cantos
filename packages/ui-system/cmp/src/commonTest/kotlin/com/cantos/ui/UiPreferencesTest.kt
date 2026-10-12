package com.cantos.ui

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

class UiPreferencesTest {
    @Test
    fun finite_positive_text_scales_are_preserved_by_construction_and_copy() {
        listOf(Float.MIN_VALUE, 0.75f, 1f, 1.3f, 2f, Float.MAX_VALUE).forEach { scale ->
            assertEquals(scale, UiPreferences(textScale = scale).textScale)
            assertEquals(scale, UiPreferences().copy(textScale = scale).textScale)
        }
    }

    @Test
    fun nonpositive_or_nonfinite_text_scales_are_rejected_at_construction() {
        invalidScales.forEach { scale ->
            val error = assertFailsWith<IllegalArgumentException> { UiPreferences(textScale = scale) }
            assertEquals("Text scale must be finite and positive", error.message)
        }
    }

    @Test
    fun copying_preferences_cannot_bypass_text_scale_validation() {
        val preferences = UiPreferences(theme = Theme.Dark, density = Density.Comfortable)
        invalidScales.forEach { scale ->
            val error = assertFailsWith<IllegalArgumentException> { preferences.copy(textScale = scale) }
            assertEquals("Text scale must be finite and positive", error.message)
        }
        assertEquals(UiPreferences(theme = Theme.Dark, density = Density.Comfortable), preferences)
    }

    private val invalidScales = listOf(0f, -0f, -1f, Float.NaN, Float.NEGATIVE_INFINITY, Float.POSITIVE_INFINITY)
}
