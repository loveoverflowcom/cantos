package com.cantos.theatre.app

import com.cantos.theatre.domain.publication.WorkKey
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue

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

    private val home = NavigationState()
    private val account = reduceNavigation(home, NavigationEvent.Navigate(TheatreRoute.Account))

    @Test
    fun login_returns_to_its_origin_and_never_becomes_a_root() {
        val fromHome = reduceNavigation(home, NavigationEvent.OpenLogin)
        assertEquals(listOf(TheatreRoute.Discovery, TheatreRoute.Login), fromHome.stack)
        assertEquals(home, reduceNavigation(fromHome, NavigationEvent.Back))

        val fromAccount = reduceNavigation(account, NavigationEvent.OpenLogin)
        assertEquals(listOf(TheatreRoute.Discovery, TheatreRoute.Account, TheatreRoute.Login), fromAccount.stack)
        assertEquals(account, reduceNavigation(fromAccount, NavigationEvent.Back))

        assertFailsWith<IllegalArgumentException> { NavigationState(listOf(TheatreRoute.Login)) }
        assertFailsWith<IllegalArgumentException> { NavigationState(listOf(TheatreRoute.Register)) }
    }

    @Test
    fun register_is_one_step_above_login_and_back_unwinds_to_login_then_the_origin() {
        val login = reduceNavigation(account, NavigationEvent.OpenLogin)
        val register = reduceNavigation(login, NavigationEvent.OpenRegister)
        assertEquals(listOf(TheatreRoute.Discovery, TheatreRoute.Account, TheatreRoute.Login, TheatreRoute.Register), register.stack)
        assertEquals(login, reduceNavigation(register, NavigationEvent.Back))
        assertEquals(account, reduceNavigation(reduceNavigation(register, NavigationEvent.Back), NavigationEvent.Back))
        // Login is also reachable from Register without growing the stack.
        assertEquals(login, reduceNavigation(register, NavigationEvent.OpenLogin))
    }

    @Test
    fun repeated_or_contextless_auth_intents_are_idempotent_and_fall_back_to_the_home_origin() {
        val login = reduceNavigation(account, NavigationEvent.OpenLogin)
        assertEquals(login, reduceNavigation(login, NavigationEvent.OpenLogin))
        assertEquals(login, reduceNavigation(login, NavigationEvent.Navigate(TheatreRoute.Login)))
        val register = reduceNavigation(login, NavigationEvent.OpenRegister)
        assertEquals(register, reduceNavigation(register, NavigationEvent.OpenRegister))
        assertEquals(register, reduceNavigation(register, NavigationEvent.Navigate(TheatreRoute.Register)))

        // A registration deep link has no Login context: Login is supplied and Back reaches Home, with no loop.
        val deepLink = reduceNavigation(home, NavigationEvent.OpenRegister)
        assertEquals(listOf(TheatreRoute.Discovery, TheatreRoute.Login, TheatreRoute.Register), deepLink.stack)
        assertEquals(home, reduceNavigation(reduceNavigation(deepLink, NavigationEvent.Back), NavigationEvent.Back))
        assertEquals(home, reduceNavigation(home, NavigationEvent.Back))
    }

    @Test
    fun completion_closes_only_an_open_auth_intent_and_resumes_the_membership_origin() {
        val membership = reduceNavigation(account, NavigationEvent.Navigate(TheatreRoute.MembershipPreview))
        val login = reduceNavigation(membership, NavigationEvent.OpenLogin)
        val register = reduceNavigation(login, NavigationEvent.OpenRegister)
        assertEquals(membership, reduceNavigation(login, NavigationEvent.AuthCompleted))
        assertEquals(membership, reduceNavigation(register, NavigationEvent.AuthCompleted))

        // A completion that arrives after Back, or after leaving auth another way, is inert.
        assertEquals(membership, reduceNavigation(membership, NavigationEvent.AuthCompleted))
        assertEquals(account, reduceNavigation(account, NavigationEvent.AuthCompleted))
        assertEquals(home, reduceNavigation(home, NavigationEvent.AuthCompleted))
        val afterBack = reduceNavigation(login, NavigationEvent.Back)
        assertEquals(afterBack, reduceNavigation(afterBack, NavigationEvent.AuthCompleted))
    }

    @Test
    fun leaving_auth_by_another_destination_dismisses_it_and_never_stacks_above_it() {
        val register = reduceNavigation(reduceNavigation(account, NavigationEvent.OpenLogin), NavigationEvent.OpenRegister)
        assertEquals(account, reduceNavigation(register, NavigationEvent.Navigate(TheatreRoute.Account)))
        assertEquals(home, reduceNavigation(register, NavigationEvent.Navigate(TheatreRoute.Discovery)))
        assertEquals(home, reduceNavigation(register, NavigationEvent.Reset))
        assertEquals(NavigationState(listOf(TheatreRoute.Library)), reduceNavigation(register, NavigationEvent.Navigate(TheatreRoute.Library)))
        // Home -> Login -> Account has no Account below it, so Login is dropped before Account is pushed.
        val login = reduceNavigation(home, NavigationEvent.OpenLogin)
        assertEquals(account, reduceNavigation(login, NavigationEvent.Navigate(TheatreRoute.Account)))
        assertEquals(
            listOf(TheatreRoute.Discovery, TheatreRoute.Settings),
            reduceNavigation(login, NavigationEvent.Navigate(TheatreRoute.Settings)).stack,
        )
    }

    @Test
    fun auth_routes_may_only_be_the_closed_login_then_register_suffix() {
        assertFailsWith<IllegalArgumentException> {
            NavigationState(listOf(TheatreRoute.Discovery, TheatreRoute.Login, TheatreRoute.Account))
        }
        assertFailsWith<IllegalArgumentException> {
            NavigationState(listOf(TheatreRoute.Discovery, TheatreRoute.Register))
        }
        assertFailsWith<IllegalArgumentException> {
            NavigationState(listOf(TheatreRoute.Discovery, TheatreRoute.Login, TheatreRoute.Login))
        }
        assertFailsWith<IllegalArgumentException> {
            NavigationState(listOf(TheatreRoute.Discovery, TheatreRoute.Register, TheatreRoute.Login))
        }
        assertTrue(TheatreRoute.Login.isAuth && TheatreRoute.Register.isAuth && !TheatreRoute.Account.isAuth)
    }
}
