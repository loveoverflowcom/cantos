package com.cantos.theatre.navigation

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse

class NavigationTest {
    @Test
    fun detail_and_player_back_return_to_the_originating_tab() {
        val initial = Navigation().tab(Screen.Search)
        val player = initial.open(Screen.Work).open(Screen.Player)
        assertEquals(listOf(Screen.Search, Screen.Work, Screen.Player), player.stack)
        assertEquals(Screen.Work, player.back().current)
        assertEquals(initial, player.back().back())
        assertEquals(listOf(Screen.Search), initial.stack)
    }

    @Test
    fun repeated_navigation_and_root_back_do_not_grow_or_empty_the_stack() {
        val root = Navigation()
        assertEquals(root, root.open(Screen.Discover))
        assertEquals(root, root.back())
        assertFalse(root.canGoBack)
        assertEquals(DemoState.NotImplemented, PlaybackBoundary.state)
    }
}
