package com.cantos.ui.preview

import com.cantos.ui.generated.CantosTokens
import kotlin.test.Test
import kotlin.test.assertEquals

class PreviewGeometryTest {
    @Test
    fun the_page_scrolls_when_narrow_or_when_effective_text_is_enlarged_and_stays_fixed_only_when_expanded_at_normal_text() {
        val table = listOf(
            Triple(false, 1f, true),
            Triple(false, 2f, true),
            Triple(true, 1.3f, true),
            Triple(true, 2f, true),
            Triple(true, 1f, false),
        )
        table.forEach { (expanded, fontScale, expected) ->
            assertEquals(expected, scrollsWholePage(expanded, fontScale), "expanded=$expanded effectiveFontScale=$fontScale")
        }
    }

    @Test
    fun evidence_presets_are_the_named_dp_constraints_and_reuse_the_canonical_breakpoints() {
        assertEquals(listOf(320f, 390f, 600f, 840f, 1140f), ViewportPreset.entries.map { it.widthDp })
        assertEquals(listOf("320 dp", "390 dp", "600 dp", "840 dp", "1140 dp"), ViewportPreset.entries.map { it.label })
        assertEquals(CantosTokens.number("breakpoint.medium").toFloat(), ViewportPreset.Medium.widthDp)
        assertEquals(CantosTokens.number("breakpoint.expanded").toFloat(), ViewportPreset.Expanded.widthDp)
    }
}
