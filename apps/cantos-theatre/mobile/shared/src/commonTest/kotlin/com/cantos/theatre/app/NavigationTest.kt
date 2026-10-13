package com.cantos.theatre.app

import com.cantos.theatre.domain.publication.WorkKey
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertFailsWith

class NavigationTest {
    @Test
    fun repeated_navigation_back_and_queue_selection_preserve_one_player_route() {
        val initial = NavigationState()
        assertFalse(initial.canGoBack)
        assertEquals(initial, reduceNavigation(initial, NavigationEvent.Back))
        val work = TheatreRoute.Work(WorkKey("son-ha-ky"))
        val detail = reduceNavigation(initial, NavigationEvent.Navigate(work))
        assertEquals(detail, reduceNavigation(detail, NavigationEvent.Navigate(work)))
        val player = reduceNavigation(detail, NavigationEvent.OpenPlayer)
        val queue = reduceNavigation(player, NavigationEvent.Navigate(TheatreRoute.Queue))
        assertEquals(player, reduceNavigation(queue, NavigationEvent.OpenPlayer))
        val settings = reduceNavigation(player, NavigationEvent.Navigate(TheatreRoute.Settings))
        assertEquals(player, reduceNavigation(settings, NavigationEvent.Back))
        assertEquals(detail, reduceNavigation(player, NavigationEvent.Back))
        assertEquals(NavigationState(listOf(TheatreRoute.Library)), reduceNavigation(settings, NavigationEvent.Navigate(TheatreRoute.Library)))
        assertEquals(initial, reduceNavigation(settings, NavigationEvent.Reset))
    }

    @Test
    fun search_history_and_account_are_secondary_to_the_two_primary_destinations() {
        val discovery = NavigationState()
        val search = reduceNavigation(discovery, NavigationEvent.Navigate(TheatreRoute.Search))
        assertEquals(listOf(TheatreRoute.Discovery, TheatreRoute.Search), search.stack)
        assertEquals(discovery, reduceNavigation(search, NavigationEvent.Back))
        val history = reduceNavigation(search, NavigationEvent.Navigate(TheatreRoute.History))
        assertEquals(listOf(TheatreRoute.Library, TheatreRoute.History), history.stack)
        val account = reduceNavigation(history, NavigationEvent.Navigate(TheatreRoute.Account))
        val settings = reduceNavigation(account, NavigationEvent.Navigate(TheatreRoute.Settings))
        assertEquals(account, reduceNavigation(settings, NavigationEvent.Back))
        assertEquals(history, reduceNavigation(account, NavigationEvent.Back))
        assertEquals(NavigationState(listOf(TheatreRoute.Library)), reduceNavigation(history, NavigationEvent.Back))
        assertEquals(discovery, reduceNavigation(settings, NavigationEvent.Navigate(TheatreRoute.Discovery)))
    }

    @Test
    fun navigation_copies_collection_authority_and_rejects_empty_root() {
        val stack = mutableListOf<TheatreRoute>(TheatreRoute.Discovery, TheatreRoute.Player)
        val state = NavigationState(stack)
        stack.clear()
        (state.stack as MutableList<*>).clear()
        assertEquals(listOf(TheatreRoute.Discovery, TheatreRoute.Player), state.stack)
        assertEquals(TheatreRoute.Player, state.route)
        assertFailsWith<IllegalArgumentException> { NavigationState(emptyList()) }
    }
}
