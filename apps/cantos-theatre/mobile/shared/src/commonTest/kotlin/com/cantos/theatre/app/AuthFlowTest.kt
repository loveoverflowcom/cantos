package com.cantos.theatre.app

import com.cantos.theatre.application.identity.NativeAccountFailure
import com.cantos.theatre.application.identity.NativeAccountState
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertIs
import kotlin.test.assertNull
import kotlin.test.assertTrue

/** Real container, navigation and account owner over deterministic identity ports. */
@OptIn(ExperimentalCoroutinesApi::class)
class AuthFlowTest {
    private val TestScope.harness get() = IdentityHarness(this)
    private val AppContainer.route get() = navigation.value.route
    private val AppContainer.stack get() = navigation.value.stack

    @Test
    fun login_opens_only_by_explicit_intent_and_back_cancels_it_without_reopening_the_browser() = runTest {
        val h = harness
        try {
            assertTrue(h.app.openLogin()); runCurrent()
            assertEquals(TheatreRoute.Login, h.app.route)
            assertTrue(h.browser.opened.isEmpty())
            assertEquals(0, h.api.begins)
            assertEquals(NativeAccountState.SignedOut, h.account.state.value)

            assertTrue(h.app.beginLogin()); runCurrent()
            assertEquals(NativeAccountState.Pending, h.account.state.value)
            assertEquals(1, h.browser.opened.size)

            h.app.back(); runCurrent()
            assertEquals(NativeAccountState.SignedOut, h.account.state.value)
            assertEquals(TheatreRoute.Discovery, h.app.route)

            // The old attempt's callback arrives late: no exchange, no account, no navigation.
            h.account.callback(h.first()); runCurrent()
            assertEquals(0, h.api.exchanges)
            assertEquals(NativeAccountState.SignedOut, h.account.state.value)
            assertEquals(TheatreRoute.Discovery, h.app.route)
            assertEquals(1, h.browser.opened.size)
        } finally { h.close() }
    }

    @Test
    fun a_double_tap_begins_one_flow_and_never_restarts_a_pending_one() = runTest {
        val h = harness
        try {
            h.app.openLogin()
            assertTrue(h.app.beginLogin())
            assertFalse(h.app.beginLogin())
            runCurrent()
            assertFalse(h.app.beginLogin())
            assertEquals(1, h.api.begins)
            assertEquals(1, h.browser.opened.size)
        } finally { h.close() }
    }

    @Test
    fun login_can_only_begin_while_the_login_route_is_open() = runTest {
        val h = harness
        try {
            assertFalse(h.app.beginLogin())
            h.app.navigate(NavigationEvent.Navigate(TheatreRoute.Account))
            assertFalse(h.app.beginLogin())
            h.app.openLogin(); h.app.openRegister()
            assertFalse(h.app.beginLogin())
            runCurrent()
            assertEquals(0, h.api.begins)
            assertTrue(h.browser.opened.isEmpty())
        } finally { h.close() }
    }

    @Test
    fun provider_cancellation_returns_to_the_login_entry_and_waits_for_a_new_explicit_attempt() = runTest {
        val h = harness
        try {
            h.app.openLogin(); h.app.beginLogin(); runCurrent()
            h.account.callback(h.first(hasError = true)); runCurrent()
            assertEquals(NativeAccountState.Failed(NativeAccountFailure.Login), h.account.state.value)
            assertEquals(TheatreRoute.Login, h.app.route)
            assertEquals(1, h.browser.opened.size)
            assertEquals(0, h.api.exchanges)

            assertTrue(h.app.beginLogin()); runCurrent()
            assertEquals(2, h.browser.opened.size)
            assertEquals(NativeAccountState.Pending, h.account.state.value)
        } finally { h.close() }
    }

    @Test
    fun a_stale_failure_is_dismissed_when_login_is_opened_again() = runTest {
        val h = harness
        try {
            h.app.openLogin(); h.app.beginLogin(); runCurrent()
            h.account.callback(h.first(hasError = true)); runCurrent()
            h.app.back(); runCurrent()
            assertIs<NativeAccountState.Failed>(h.account.state.value)

            assertTrue(h.app.openLogin()); runCurrent()
            assertEquals(NativeAccountState.SignedOut, h.account.state.value)
            assertEquals(1, h.api.begins)
        } finally { h.close() }
    }

    @Test
    fun a_confirmed_session_closes_the_open_login_and_returns_to_its_origin() = runTest {
        val h = harness
        try {
            h.app.navigate(NavigationEvent.Navigate(TheatreRoute.Account))
            h.app.openLogin(); h.app.beginLogin(); runCurrent()
            h.account.callback(h.first()); runCurrent()
            assertIs<NativeAccountState.SignedIn>(h.account.state.value)
            assertEquals(listOf(TheatreRoute.Discovery, TheatreRoute.Account), h.app.stack)
            // A signed-in listener has no Login destination.
            assertFalse(h.app.openLogin())
            assertEquals(listOf(TheatreRoute.Discovery, TheatreRoute.Account), h.app.stack)
        } finally { h.close() }
    }

    @Test
    fun a_confirmed_session_resumes_the_membership_origin() = runTest {
        val h = harness
        try {
            h.app.navigate(NavigationEvent.Navigate(TheatreRoute.Account))
            h.app.navigate(NavigationEvent.Navigate(TheatreRoute.MembershipPreview))
            h.app.openLogin(); h.app.beginLogin(); runCurrent()
            h.account.callback(h.first()); runCurrent()
            assertEquals(TheatreRoute.MembershipPreview, h.app.route)
            assertEquals(3, h.app.stack.size)
        } finally { h.close() }
    }

    @Test
    fun a_session_established_while_auth_is_closed_does_not_navigate() = runTest {
        val h = harness
        try {
            h.app.navigate(NavigationEvent.Navigate(TheatreRoute.Account))
            h.store.value = h.token
            h.account.restore(); runCurrent()
            assertIs<NativeAccountState.SignedIn>(h.account.state.value)
            assertEquals(listOf(TheatreRoute.Discovery, TheatreRoute.Account), h.app.stack)
        } finally { h.close() }
    }

    @Test
    fun restore_is_explicit_reports_an_empty_device_and_resumes_a_saved_session() = runTest {
        val h = harness
        try {
            // Not from Home or Account: restore belongs to the open Login entry.
            assertEquals(RestoreOutcome.Refused, h.app.restoreSession())
            h.app.navigate(NavigationEvent.Navigate(TheatreRoute.Account))
            h.app.openLogin()
            assertEquals(RestoreOutcome.NothingSaved, h.app.restoreSession()); runCurrent()
            assertEquals(NativeAccountState.SignedOut, h.account.state.value)
            assertEquals(TheatreRoute.Login, h.app.route)

            h.store.value = h.token
            assertEquals(RestoreOutcome.Requested, h.app.restoreSession()); runCurrent()
            assertIs<NativeAccountState.SignedIn>(h.account.state.value)
            assertEquals(listOf(TheatreRoute.Discovery, TheatreRoute.Account), h.app.stack)
            assertEquals(RestoreOutcome.Refused, h.app.restoreSession())
        } finally { h.close() }
    }

    @Test
    fun registration_is_reachable_through_login_and_never_calls_identity() = runTest {
        val h = harness
        try {
            assertTrue(h.app.openRegister()); runCurrent()
            assertEquals(listOf(TheatreRoute.Discovery, TheatreRoute.Login, TheatreRoute.Register), h.app.stack)
            assertEquals(0, h.api.begins)
            assertTrue(h.browser.opened.isEmpty())
            h.app.back(); assertEquals(TheatreRoute.Login, h.app.route)
            h.app.back(); assertEquals(TheatreRoute.Discovery, h.app.route)
            assertEquals(NativeAccountState.SignedOut, h.account.state.value)
        } finally { h.close() }
    }

    @Test
    fun login_stays_blocked_until_old_credential_cleanup_is_acknowledged() = runTest {
        val h = harness
        try {
            h.app.openLogin(); h.app.beginLogin(); runCurrent()
            h.account.callback(h.first()); runCurrent()
            h.store.clearFails = true
            h.app.logout(); runCurrent()
            assertEquals(NativeAccountState.CleanupRequired, h.account.state.value)

            // The Login route is open, so only the account state can be what blocks the attempt.
            assertTrue(h.app.openLogin())
            val begins = h.api.begins
            assertFalse(h.app.beginLogin()); runCurrent()
            assertEquals(begins, h.api.begins)
            assertEquals(NativeAccountState.CleanupRequired, h.account.state.value)

            h.store.clearFails = false
            h.app.logout(); runCurrent()
            assertEquals(NativeAccountState.SignedOut, h.account.state.value)
            assertTrue(h.app.beginLogin()); runCurrent()
            assertEquals(begins + 1, h.api.begins)
        } finally { h.close() }
    }

    @Test
    fun without_an_identity_capability_the_shell_stays_guest_only() = runTest {
        val app = AppContainer.mock(this)
        try {
            val before = app.navigation.value
            assertNull(app.account)
            assertNull(app.accountState.value)
            assertFalse(app.openLogin())
            assertFalse(app.openRegister())
            assertFalse(app.beginLogin())
            app.logout(); app.cancelLogin()
            assertEquals(before, app.navigation.value)
        } finally { app.close() }
    }
}
