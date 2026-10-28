package com.cantos.theatre.features.shell.presentation

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import kotlin.test.Test
import kotlin.test.assertEquals

class SearchOverlayTest {
    @Test
    fun only_the_bounded_top_overlay_dismisses_on_an_outside_press() {
        val registry = BackOverlayRegistry()
        var sheets = 0
        val sheet = registry.open { sheets++ }
        registry.dismissOutside(Offset.Zero)
        assertEquals(0, sheets, "unbounded confirmation sheets retain their own policy")
        var searches = 0
        val search = registry.open(bounds = { Rect(10f, 10f, 100f, 200f) }) { searches++ }
        registry.dismissOutside(Offset(20f, 20f))
        assertEquals(0, searches)
        registry.dismissOutside(Offset(101f, 20f))
        assertEquals(1, searches)
        sheet() // a stale disposer cannot remove the search overlay
        registry.dismiss()
        assertEquals(2, searches)
        search()
    }
}
