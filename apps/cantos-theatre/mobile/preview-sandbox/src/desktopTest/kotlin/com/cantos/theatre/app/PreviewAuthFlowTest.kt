package com.cantos.theatre.app

import com.cantos.theatre.adapters.preview.PreviewAccount
import com.cantos.theatre.adapters.preview.PreviewLoginMode
import com.cantos.theatre.app.NavigationEvent
import com.cantos.theatre.app.RestoreOutcome
import com.cantos.theatre.app.TheatreRoute
import com.cantos.theatre.application.identity.NativeAccountFailure
import com.cantos.theatre.application.identity.NativeAccountState
import com.cantos.theatre.presentation.model.ShellEvent
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertIs
import kotlin.test.assertTrue
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest

/**
 * The real TheatreShell, account owner and Library over deterministic identity ports. The
 * provider is held so each test decides when (and whether) the browser returns; none of this is
 * evidence of live identity, secure storage or system-browser behavior.
 */
@OptIn(ExperimentalCoroutinesApi::class)
class PreviewAuthFlowTest {
    private fun TestScope.harness(seed: PreviewSeed = PreviewSession.Guest.seed()) =
        PreviewHarness(backgroundScope, seed).also { it.identity.mode = PreviewLoginMode.Hold }

    @Test
    fun login_opens_only_by_explicit_intent_and_back_cancels_it_without_reopening_the_browser() = runTest {
        val h = harness()
        try {
            assertTrue(h.shell.openLogin()); runCurrent()
            assertEquals(TheatreRoute.Login, h.route)
            assertEquals(0, h.identity.browserOpens)
            assertEquals(0, h.identity.begins)
            assertEquals(NativeAccountState.SignedOut, h.account.state.value)

            assertTrue(h.shell.beginLogin()); runCurrent()
            assertEquals(NativeAccountState.Pending, h.account.state.value)
            assertEquals(1, h.identity.browserOpens)

            h.shell.dispatch(ShellEvent.Back); runCurrent()
            assertEquals(NativeAccountState.SignedOut, h.account.state.value)
            assertEquals(TheatreRoute.Discovery, h.route)

            // The old attempt's callback arrives late: no exchange, no account, no navigation.
            h.identity.returnFromBrowser(approve = true); runCurrent()
            assertEquals(0, h.identity.exchanges)
            assertEquals(NativeAccountState.SignedOut, h.account.state.value)
            assertEquals(TheatreRoute.Discovery, h.route)
            assertEquals(1, h.identity.browserOpens)
        } finally { h.close() }
    }

    @Test
    fun a_double_tap_begins_one_flow_and_never_restarts_a_pending_one() = runTest {
        val h = harness()
        try {
            h.shell.openLogin()
            assertTrue(h.shell.beginLogin())
            assertFalse(h.shell.beginLogin())
            runCurrent()
            assertFalse(h.shell.beginLogin())
            assertEquals(1, h.identity.begins)
            assertEquals(1, h.identity.browserOpens)
        } finally { h.close() }
    }

    @Test
    fun a_delayed_response_keeps_its_original_attempt_when_a_held_login_replaces_it() = runTest {
        listOf(PreviewLoginMode.Approve, PreviewLoginMode.Deny).forEach { mode ->
            val h = harness()
            try {
                h.identity.mode = mode
                h.shell.openLogin()
                assertTrue(h.shell.beginLogin()); runCurrent()
                advanceTimeBy(20L)
                h.shell.cancelLogin(); runCurrent()
                assertEquals(NativeAccountState.SignedOut, h.account.state.value)

                h.identity.mode = PreviewLoginMode.Hold
                assertTrue(h.shell.beginLogin()); runCurrent()
                advanceTimeBy(40L); runCurrent()
                assertEquals(2, h.identity.browserOpens)
                assertEquals(0, h.identity.exchanges)
                assertEquals(NativeAccountState.Pending, h.account.state.value)
                assertEquals(TheatreRoute.Login, h.route)

                h.identity.returnFromBrowser(approve = true); runCurrent()
                assertEquals(1, h.identity.exchanges)
                assertIs<NativeAccountState.SignedIn>(h.account.state.value)
            } finally { h.close() }
        }
    }

    @Test
    fun login_can_only_begin_while_the_login_route_is_open() = runTest {
        val h = harness()
        try {
            assertFalse(h.shell.beginLogin())
            h.shell.navigate(NavigationEvent.Navigate(TheatreRoute.Account))
            assertFalse(h.shell.beginLogin())
            h.shell.openLogin(); h.shell.openRegister()
            assertFalse(h.shell.beginLogin())
            runCurrent()
            assertEquals(0, h.identity.begins)
            assertEquals(0, h.identity.browserOpens)
        } finally { h.close() }
    }

    @Test
    fun provider_cancellation_returns_to_the_login_entry_and_waits_for_a_new_explicit_attempt() = runTest {
        val h = harness()
        try {
            h.shell.openLogin(); h.shell.beginLogin(); runCurrent()
            h.identity.returnFromBrowser(approve = false); runCurrent()
            assertEquals(NativeAccountState.Failed(NativeAccountFailure.Login), h.account.state.value)
            assertEquals(TheatreRoute.Login, h.route)
            assertEquals(1, h.identity.browserOpens)
            assertEquals(0, h.identity.exchanges)

            assertTrue(h.shell.beginLogin()); runCurrent()
            assertEquals(2, h.identity.browserOpens)
            assertEquals(NativeAccountState.Pending, h.account.state.value)
        } finally { h.close() }
    }

    @Test
    fun a_stale_failure_is_dismissed_when_login_is_opened_again() = runTest {
        val h = harness()
        try {
            h.shell.openLogin(); h.shell.beginLogin(); runCurrent()
            h.identity.returnFromBrowser(approve = false); runCurrent()
            h.shell.dispatch(ShellEvent.Back); runCurrent()
            assertIs<NativeAccountState.Failed>(h.account.state.value)

            assertTrue(h.shell.openLogin()); runCurrent()
            assertEquals(NativeAccountState.SignedOut, h.account.state.value)
            assertEquals(1, h.identity.begins)
        } finally { h.close() }
    }

    @Test
    fun a_confirmed_session_closes_the_open_login_and_returns_to_its_origin() = runTest {
        val h = harness()
        try {
            h.shell.navigate(NavigationEvent.Navigate(TheatreRoute.Account))
            h.shell.openLogin(); h.shell.beginLogin(); runCurrent()
            h.identity.returnFromBrowser(approve = true); runCurrent()
            assertIs<NativeAccountState.SignedIn>(h.account.state.value)
            assertEquals(listOf(TheatreRoute.Discovery, TheatreRoute.Account), h.stack)
            // A signed-in listener has no Login destination.
            assertFalse(h.shell.openLogin())
            assertEquals(listOf(TheatreRoute.Discovery, TheatreRoute.Account), h.stack)
        } finally { h.close() }
    }

    @Test
    fun a_confirmed_session_resumes_the_membership_origin() = runTest {
        val h = harness()
        try {
            h.shell.navigate(NavigationEvent.Navigate(TheatreRoute.Account))
            h.shell.navigate(NavigationEvent.Navigate(TheatreRoute.MembershipPreview))
            h.shell.openLogin(); h.shell.beginLogin(); runCurrent()
            h.identity.returnFromBrowser(approve = true); runCurrent()
            assertEquals(TheatreRoute.MembershipPreview, h.route)
            assertEquals(3, h.stack.size)
        } finally { h.close() }
    }

    @Test
    fun a_session_established_while_auth_is_closed_does_not_navigate() = runTest {
        val h = harness()
        try {
            h.shell.navigate(NavigationEvent.Navigate(TheatreRoute.Account))
            h.identity.persistSession()
            h.account.restore(); runCurrent()
            assertIs<NativeAccountState.SignedIn>(h.account.state.value)
            assertEquals(listOf(TheatreRoute.Discovery, TheatreRoute.Account), h.stack)
        } finally { h.close() }
    }

    @Test
    fun restore_is_explicit_reports_an_empty_device_and_resumes_a_saved_session() = runTest {
        val h = harness()
        try {
            // Not from Home or Account: restore belongs to the open Login entry.
            assertEquals(RestoreOutcome.Refused, h.shell.restoreSession())
            h.shell.navigate(NavigationEvent.Navigate(TheatreRoute.Account))
            h.shell.openLogin()
            assertEquals(RestoreOutcome.NothingSaved, h.shell.restoreSession()); runCurrent()
            assertEquals(NativeAccountState.SignedOut, h.account.state.value)
            assertEquals(TheatreRoute.Login, h.route)

            h.identity.persistSession()
            assertEquals(RestoreOutcome.Requested, h.shell.restoreSession()); runCurrent()
            assertIs<NativeAccountState.SignedIn>(h.account.state.value)
            assertEquals(listOf(TheatreRoute.Discovery, TheatreRoute.Account), h.stack)
            assertEquals(RestoreOutcome.Refused, h.shell.restoreSession())
        } finally { h.close() }
    }

    @Test
    fun registration_is_reachable_through_login_and_never_calls_identity() = runTest {
        val h = harness()
        try {
            assertTrue(h.shell.openRegister()); runCurrent()
            assertEquals(listOf(TheatreRoute.Discovery, TheatreRoute.Login, TheatreRoute.Register), h.stack)
            assertEquals(0, h.identity.begins)
            assertEquals(0, h.identity.browserOpens)
            h.shell.dispatch(ShellEvent.Back); assertEquals(TheatreRoute.Login, h.route)
            h.shell.dispatch(ShellEvent.Back); assertEquals(TheatreRoute.Discovery, h.route)
            assertEquals(NativeAccountState.SignedOut, h.account.state.value)
        } finally { h.close() }
    }

    @Test
    fun login_stays_blocked_until_old_credential_cleanup_is_acknowledged() = runTest {
        val h = harness()
        try {
            h.shell.openLogin(); h.shell.beginLogin(); runCurrent()
            h.identity.returnFromBrowser(approve = true); runCurrent()
            h.identity.clearFails = true
            h.shell.logout(); runCurrent()
            assertEquals(NativeAccountState.CleanupRequired, h.account.state.value)

            // The Login route is open, so only the account state can be what blocks the attempt.
            assertTrue(h.shell.openLogin())
            val begins = h.identity.begins
            assertFalse(h.shell.beginLogin()); runCurrent()
            assertEquals(begins, h.identity.begins)
            assertEquals(NativeAccountState.CleanupRequired, h.account.state.value)

            h.identity.clearFails = false
            h.shell.logout(); runCurrent()
            assertEquals(NativeAccountState.SignedOut, h.account.state.value)
            assertTrue(h.shell.beginLogin()); runCurrent()
            assertEquals(begins + 1, h.identity.begins)
        } finally { h.close() }
    }

    @Test
    fun account_switch_replaces_the_library_owner_and_never_shows_the_previous_listeners_data() = runTest {
        val h = harness(PreviewSession.Restored.seed())
        try {
            runCurrent()
            assertEquals(PreviewAccount.Linh.actor, h.shell.library.value.account)
            assertTrue(h.shell.library.value.saved.any { it.saved })

            h.shell.logout(); runCurrent()
            assertEquals(NativeAccountState.SignedOut, h.account.state.value)
            assertEquals(null, h.shell.library.value.account)
            assertTrue(h.shell.library.value.saved.isEmpty())

            h.shell.openLogin(); h.shell.beginLogin(); runCurrent()
            h.identity.returnFromBrowser(approve = true); runCurrent()
            assertEquals(PreviewAccount.Minh.actor, h.shell.library.value.account)
            assertTrue(h.shell.library.value.saved.none { it.saved }, "the second sample listener has no saved works")
            assertTrue(h.shell.library.value.continueListening.isEmpty())
        } finally { h.close() }
    }
}
