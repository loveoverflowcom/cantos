package com.cantos.theatre.app

import com.cantos.theatre.domain.catalog.CatalogCheck
import com.cantos.theatre.domain.catalog.PublicWorkId
import com.cantos.theatre.domain.publication.WorkKey
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue

class NavigationTest {
    private val guest = NavigationState()
    private val discovery = NavigationState(listOf(TheatreRoute.Discovery))
    private val work = TheatreRoute.Work(WorkKey("10000000-0000-4000-8000-000000000001"))

    // cantos.presentation.guest-navigation.run.session-root
    @Test fun admitted_roots_search_back_and_reset_follow_current_authority() {
        assertEquals(listOf(TheatreRoute.Landing), guest.stack)
        assertFalse(guest.canGoBack)
        assertEquals(guest, reduceNavigation(guest, NavigationEvent.Back))
        for ((admission, home) in listOf(NavigationAdmission.Guest to guest, NavigationAdmission.SignedIn to discovery)) {
            val search = reduceNavigation(home, NavigationEvent.Navigate(TheatreRoute.Search), admission)
            assertEquals(home.stack + TheatreRoute.Search, search.stack)
            assertEquals(search, reduceNavigation(search, NavigationEvent.Navigate(TheatreRoute.Search), admission))
            val detail = reduceNavigation(search, NavigationEvent.Navigate(work), admission)
            assertEquals(search, reduceNavigation(detail, NavigationEvent.Back, admission))
            assertEquals(home, reduceNavigation(search, NavigationEvent.Back, admission))
            assertEquals(home, reduceNavigation(detail, NavigationEvent.Reset, admission))
        }
        assertEquals(discovery, reduceNavigation(guest, NavigationEvent.ReconcileSession, NavigationAdmission.SignedIn))
        assertEquals(guest, reduceNavigation(discovery, NavigationEvent.ReconcileSession))
    }

    // cantos.presentation.guest-navigation.run.auth-return
    @Test fun private_guest_destination_is_a_typed_return_and_never_a_visible_private_stack() {
        for (target in PrivateDestination.entries) {
            val requested = reduceNavigation(guest, NavigationEvent.Navigate(target.route))
            assertEquals(listOf(TheatreRoute.Landing), requested.stack)
            assertEquals(target, requested.returnTo)
            val login = reduceNavigation(requested, NavigationEvent.OpenLogin)
            assertEquals(listOf(TheatreRoute.Landing, TheatreRoute.Login), login.stack)
            assertEquals(login, reduceNavigation(login, NavigationEvent.AuthCompleted), "a route confers no session")
            val completed = reduceNavigation(login, NavigationEvent.AuthCompleted, NavigationAdmission.SignedIn)
            assertEquals(target.route, completed.route)
            assertNull(completed.returnTo)
            assertTrue(completed.stack.none { it == TheatreRoute.Landing || it.isAuth })
        }
        val detail = reduceNavigation(guest, NavigationEvent.Navigate(work))
        val login = reduceNavigation(detail, NavigationEvent.OpenLogin)
        assertEquals(listOf(TheatreRoute.Discovery, work),
            reduceNavigation(login, NavigationEvent.ReconcileSession, NavigationAdmission.SignedIn).stack)
    }

    // cantos.presentation.guest-navigation.run.unresolved-session
    @Test fun unresolved_private_and_public_destinations_survive_checking_and_failure() {
        for (route in listOf(TheatreRoute.Library, TheatreRoute.Account, work, TheatreRoute.Search)) {
            val intended = reduceNavigation(guest, NavigationEvent.Navigate(route), NavigationAdmission.Unresolved)
            assertEquals(route, intended.route)
            assertEquals(intended, reduceNavigation(intended, NavigationEvent.ReconcileSession, NavigationAdmission.Unresolved))
            assertEquals(TheatreRoute.Landing,
                reduceNavigation(intended, NavigationEvent.Navigate(TheatreRoute.Discovery), NavigationAdmission.Unresolved).route,
                "explicit public recovery never waits for a successful session lookup")
            val admitted = reduceNavigation(intended, NavigationEvent.ReconcileSession, NavigationAdmission.SignedIn)
            assertEquals(route, admitted.route)
            assertTrue(TheatreRoute.Landing !in admitted.stack)
        }
        val intended = reduceNavigation(guest, NavigationEvent.Navigate(TheatreRoute.Library), NavigationAdmission.Unresolved)
        val settled = reduceNavigation(intended, NavigationEvent.ReconcileSession)
        assertEquals(listOf(TheatreRoute.Landing), settled.stack)
        assertEquals(PrivateDestination.Library, settled.returnTo)
    }

    // cantos.presentation.guest-navigation.run.auth-cancel-late
    @Test fun registration_unwinds_once_and_cancel_invalidates_the_private_return() {
        val origin = reduceNavigation(guest, NavigationEvent.Navigate(TheatreRoute.Library))
        val login = reduceNavigation(origin, NavigationEvent.OpenLogin)
        val register = reduceNavigation(login, NavigationEvent.OpenRegister)
        assertEquals(listOf(TheatreRoute.Landing, TheatreRoute.Login, TheatreRoute.Register), register.stack)
        assertEquals(register, reduceNavigation(register, NavigationEvent.OpenRegister))
        assertEquals(login, reduceNavigation(register, NavigationEvent.OpenLogin))
        assertEquals(login, reduceNavigation(register, NavigationEvent.Back))
        val cancelled = reduceNavigation(login, NavigationEvent.Back)
        assertEquals(guest, cancelled)
        assertNull(cancelled.returnTo)
        assertEquals(cancelled, reduceNavigation(cancelled, NavigationEvent.AuthCompleted, NavigationAdmission.SignedIn))
        val reopened = reduceNavigation(cancelled, NavigationEvent.OpenLogin)
        assertEquals(discovery, reduceNavigation(reopened, NavigationEvent.AuthCompleted, NavigationAdmission.SignedIn))
        assertEquals(guest, reduceNavigation(register, NavigationEvent.Reset))
        assertEquals(guest, reduceNavigation(register, NavigationEvent.Navigate(TheatreRoute.Discovery)))
        assertEquals(listOf(TheatreRoute.Landing, TheatreRoute.Settings),
            reduceNavigation(register, NavigationEvent.Navigate(TheatreRoute.Settings)).stack)
    }

    @Test fun auth_from_public_membership_or_work_returns_without_losing_the_source() {
        for (route in listOf(TheatreRoute.MembershipPreview, work)) {
            val origin = reduceNavigation(guest, NavigationEvent.Navigate(route))
            val login = reduceNavigation(origin, NavigationEvent.OpenLogin)
            assertEquals(origin, reduceNavigation(login, NavigationEvent.Back))
            assertEquals(login, reduceNavigation(login, NavigationEvent.OpenLogin))
            assertEquals(login, reduceNavigation(login, NavigationEvent.Navigate(TheatreRoute.Login)))
            val register = reduceNavigation(guest, NavigationEvent.OpenRegister)
            assertEquals(register, reduceNavigation(register, NavigationEvent.Navigate(TheatreRoute.Register)))
            assertEquals(listOf(TheatreRoute.Discovery, route),
                reduceNavigation(login, NavigationEvent.AuthCompleted, NavigationAdmission.SignedIn).stack)
        }
    }

    // cantos.presentation.guest-navigation.run.logout-expiry
    @Test fun ended_session_cannot_restore_private_entries_or_old_auth_returns_by_back() {
        val private = NavigationState(listOf(TheatreRoute.Library, TheatreRoute.History, TheatreRoute.Account))
        val ended = reduceNavigation(private, NavigationEvent.SessionEnded, NavigationAdmission.Unresolved)
        assertEquals(guest, ended)
        assertEquals(ended, reduceNavigation(ended, NavigationEvent.Back))
        assertEquals(ended, reduceNavigation(ended, NavigationEvent.ReconcileSession))
        val publicDetail = NavigationState(private.stack + work)
        assertEquals(listOf(TheatreRoute.Landing, work), reduceNavigation(publicDetail, NavigationEvent.SessionEnded).stack)
        val login = reduceNavigation(reduceNavigation(guest, NavigationEvent.Navigate(TheatreRoute.History)), NavigationEvent.OpenLogin)
        assertEquals(guest, reduceNavigation(login, NavigationEvent.SessionEnded))
    }

    // cantos.presentation.guest-navigation.run.repeated-navigation
    @Test fun repeated_navigation_and_queue_selection_preserve_one_player_route() {
        val detail = reduceNavigation(guest, NavigationEvent.Navigate(work))
        assertEquals(detail, reduceNavigation(detail, NavigationEvent.Navigate(work)))
        val player = reduceNavigation(detail, NavigationEvent.OpenPlayer)
        val queue = reduceNavigation(player, NavigationEvent.Navigate(TheatreRoute.Queue))
        assertEquals(player, reduceNavigation(queue, NavigationEvent.OpenPlayer))
        val settings = reduceNavigation(player, NavigationEvent.Navigate(TheatreRoute.Settings))
        assertEquals(player, reduceNavigation(settings, NavigationEvent.Back))
        assertEquals(detail, reduceNavigation(player, NavigationEvent.Back))
        val history = reduceNavigation(discovery, NavigationEvent.Navigate(TheatreRoute.History), NavigationAdmission.SignedIn)
        assertEquals(listOf(TheatreRoute.Library, TheatreRoute.History), history.stack)
        assertEquals(NavigationState(listOf(TheatreRoute.Library)), reduceNavigation(history, NavigationEvent.Back, NavigationAdmission.SignedIn))
    }

    // cantos.presentation.guest-navigation.run.public-deep-link
    @Test fun shared_locator_and_verification_survive_without_granting_a_session() {
        val id = (PublicWorkId.parse(work.key.value) as CatalogCheck.Valid).value
        val shared = reduceNavigation(guest, NavigationEvent.Navigate(TheatreRoute.SharedWork(id)), NavigationAdmission.Unresolved)
        assertEquals(shared, reduceNavigation(shared, NavigationEvent.ReconcileSession))
        val player = reduceNavigation(reduceNavigation(guest, NavigationEvent.Navigate(work)), NavigationEvent.OpenPlayer)
        val verifying = reduceNavigation(player, NavigationEvent.OpenVerifyEmail)
        assertEquals(player.stack + TheatreRoute.VerifyEmail, verifying.stack)
        assertEquals(verifying, reduceNavigation(verifying, NavigationEvent.OpenVerifyEmail))
        assertEquals(player, reduceNavigation(verifying, NavigationEvent.Back))
        assertEquals(verifying, reduceNavigation(verifying, NavigationEvent.AuthCompleted))
        assertEquals(TheatreRoute.VerifyEmail,
            reduceNavigation(verifying, NavigationEvent.ReconcileSession, NavigationAdmission.SignedIn).route)
        val login = reduceNavigation(reduceNavigation(player, NavigationEvent.OpenLogin), NavigationEvent.OpenRegister)
        assertEquals(verifying, reduceNavigation(login, NavigationEvent.OpenVerifyEmail))
        assertEquals(reduceNavigation(player, NavigationEvent.OpenLogin), reduceNavigation(verifying, NavigationEvent.OpenLogin))
        assertEquals(reduceNavigation(player, NavigationEvent.OpenRegister), reduceNavigation(verifying, NavigationEvent.OpenRegister))
        assertEquals(player, reduceNavigation(verifying, NavigationEvent.OpenPlayer))
        assertEquals(guest, reduceNavigation(verifying, NavigationEvent.Reset))
        assertEquals(player, reduceNavigation(player, NavigationEvent.Navigate(TheatreRoute.VerifyEmail)))
    }

    @Test fun navigation_snapshots_collections_and_rejects_empty_or_invalid_transient_stacks() {
        val input = mutableListOf<TheatreRoute>(TheatreRoute.Landing, TheatreRoute.Player)
        val snapshot = NavigationState(input)
        input.clear()
        (snapshot.stack as MutableList<*>).clear()
        assertEquals(listOf(TheatreRoute.Landing, TheatreRoute.Player), snapshot.stack)
        for (stack in listOf(
            emptyList(), listOf(TheatreRoute.Login), listOf(TheatreRoute.Register), listOf(TheatreRoute.VerifyEmail),
            listOf(TheatreRoute.Landing, TheatreRoute.Login, TheatreRoute.Account),
            listOf(TheatreRoute.Landing, TheatreRoute.Register),
            listOf(TheatreRoute.Landing, TheatreRoute.Login, TheatreRoute.Login),
            listOf(TheatreRoute.Landing, TheatreRoute.Register, TheatreRoute.Login),
            listOf(TheatreRoute.Landing, TheatreRoute.VerifyEmail, TheatreRoute.Settings),
            listOf(TheatreRoute.Landing, TheatreRoute.Login, TheatreRoute.VerifyEmail),
            listOf(TheatreRoute.Landing, TheatreRoute.VerifyEmail, TheatreRoute.Login),
        )) assertFailsWith<IllegalArgumentException>(stack.toString()) { NavigationState(stack) }
        assertTrue(TheatreRoute.Login.isAuth && TheatreRoute.Register.isAuth && !TheatreRoute.Account.isAuth)
        assertTrue(NavigationState(listOf(TheatreRoute.Landing, TheatreRoute.VerifyEmail)).canGoBack)
    }
}
