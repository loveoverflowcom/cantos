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
 * login response is held so each test decides when (and whether) it arrives; none of this is
 * evidence of live identity, secure storage or system-browser behavior.
 */
@OptIn(ExperimentalCoroutinesApi::class)
class PreviewAuthFlowTest {
    private val email = (com.cantos.theatre.domain.identity.RegistrationEmail.parse("listener@example.test") as com.cantos.theatre.domain.identity.RegistrationResult.Accepted).value
    private fun TestScope.harness(seed: PreviewSeed = PreviewSession.Guest.seed()) =
        PreviewHarness(backgroundScope, seed).also { it.identity.mode = PreviewLoginMode.Hold }

    @Test
    fun login_opens_only_by_explicit_intent_and_back_cancels_it_without_replaying_credentials() = runTest {
        val h = harness()
        try {
            assertTrue(h.shell.openLogin()); runCurrent()
            assertEquals(TheatreRoute.Login, h.route)
            assertEquals(0, h.identity.loginRequests)
            assertEquals(NativeAccountState.SignedOut, h.account.state.value)

            assertTrue(h.shell.login(email, "preview-only")); runCurrent()
            assertEquals(NativeAccountState.Pending, h.account.state.value)
            assertEquals(1, h.identity.loginRequests)

            h.shell.dispatch(ShellEvent.Back); runCurrent()
            assertEquals(NativeAccountState.SignedOut, h.account.state.value)
            assertEquals(TheatreRoute.Discovery, h.route)

            // The old attempt's response arrives late: no admission, no account, no navigation.
            h.identity.completeLogin(approve = true); runCurrent()
            assertEquals(0, h.identity.admissions)
            assertEquals(NativeAccountState.SignedOut, h.account.state.value)
            assertEquals(TheatreRoute.Discovery, h.route)
            assertEquals(1, h.identity.loginRequests)
        } finally { h.close() }
    }

    @Test
    fun a_double_tap_begins_one_flow_and_never_restarts_a_pending_one() = runTest {
        val h = harness()
        try {
            h.shell.openLogin()
            assertTrue(h.shell.login(email, "preview-only"))
            assertFalse(h.shell.login(email, "preview-only"))
            runCurrent()
            assertFalse(h.shell.login(email, "preview-only"))
            assertEquals(1, h.identity.loginRequests)
        } finally { h.close() }
    }

    @Test
    fun a_delayed_response_keeps_its_original_attempt_when_a_held_login_replaces_it() = runTest {
        listOf(PreviewLoginMode.Approve, PreviewLoginMode.Deny).forEach { mode ->
            val h = harness()
            try {
                h.identity.mode = mode
                h.shell.openLogin()
                assertTrue(h.shell.login(email, "preview-only")); runCurrent()
                advanceTimeBy(20L)
                h.shell.cancelLogin(); runCurrent()
                assertEquals(NativeAccountState.SignedOut, h.account.state.value)

                h.identity.mode = PreviewLoginMode.Hold
                assertTrue(h.shell.login(email, "preview-only")); runCurrent()
                advanceTimeBy(40L); runCurrent()
                assertEquals(2, h.identity.loginRequests)
                assertEquals(0, h.identity.admissions)
                assertEquals(NativeAccountState.Pending, h.account.state.value)
                assertEquals(TheatreRoute.Login, h.route)

                h.identity.completeLogin(approve = true); runCurrent()
                assertEquals(1, h.identity.admissions)
                assertIs<NativeAccountState.SignedIn>(h.account.state.value)
            } finally { h.close() }
        }
    }

    @Test
    fun login_can_only_begin_while_the_login_route_is_open() = runTest {
        val h = harness()
        try {
            assertFalse(h.shell.login(email, "preview-only"))
            h.shell.navigate(NavigationEvent.Navigate(TheatreRoute.Account))
            assertFalse(h.shell.login(email, "preview-only"))
            h.shell.openLogin(); h.shell.openRegister()
            assertFalse(h.shell.login(email, "preview-only"))
            runCurrent()
            assertEquals(0, h.identity.loginRequests)
        } finally { h.close() }
    }

    @Test
    fun credential_refusal_returns_to_the_login_entry_and_waits_for_a_new_explicit_attempt() = runTest {
        val h = harness()
        try {
            h.shell.openLogin(); h.shell.login(email, "preview-only"); runCurrent()
            h.identity.completeLogin(approve = false); runCurrent()
            assertEquals(NativeAccountState.Failed(NativeAccountFailure.Api, com.cantos.theatre.ports.IdentityV1Failure.Unauthorized, credentialAttempt = true), h.account.state.value)
            assertEquals(TheatreRoute.Login, h.route)
            assertEquals(1, h.identity.loginRequests)
            assertEquals(0, h.identity.admissions)

            assertTrue(h.shell.login(email, "preview-only")); runCurrent()
            assertEquals(2, h.identity.loginRequests)
            assertEquals(NativeAccountState.Pending, h.account.state.value)
        } finally { h.close() }
    }

    @Test
    fun a_stale_failure_is_dismissed_when_login_is_opened_again() = runTest {
        val h = harness()
        try {
            h.shell.openLogin(); h.shell.login(email, "preview-only"); runCurrent()
            h.identity.completeLogin(approve = false); runCurrent()
            h.shell.dispatch(ShellEvent.Back); runCurrent()
            assertIs<NativeAccountState.Failed>(h.account.state.value)

            assertTrue(h.shell.openLogin()); runCurrent()
            assertEquals(NativeAccountState.SignedOut, h.account.state.value)
            assertEquals(1, h.identity.loginRequests)
        } finally { h.close() }
    }

    @Test
    fun a_confirmed_session_closes_the_open_login_and_returns_to_its_origin() = runTest {
        val h = harness()
        try {
            h.shell.navigate(NavigationEvent.Navigate(TheatreRoute.Account))
            h.shell.openLogin(); h.shell.login(email, "preview-only"); runCurrent()
            h.identity.completeLogin(approve = true); runCurrent()
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
            h.shell.openLogin(); h.shell.login(email, "preview-only"); runCurrent()
            h.identity.completeLogin(approve = true); runCurrent()
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
            assertEquals(0, h.identity.loginRequests)
            h.shell.dispatch(ShellEvent.Back); assertEquals(TheatreRoute.Login, h.route)
            h.shell.dispatch(ShellEvent.Back); assertEquals(TheatreRoute.Discovery, h.route)
            assertEquals(NativeAccountState.SignedOut, h.account.state.value)
        } finally { h.close() }
    }

    @Test
    fun login_stays_blocked_until_old_credential_cleanup_is_acknowledged() = runTest {
        val h = harness()
        try {
            h.shell.openLogin(); h.shell.login(email, "preview-only"); runCurrent()
            h.identity.completeLogin(approve = true); runCurrent()
            h.identity.clearFails = true
            h.shell.logout(); runCurrent()
            assertEquals(NativeAccountState.CleanupRequired, h.account.state.value)

            // The Login route is open, so only the account state can be what blocks the attempt.
            assertTrue(h.shell.openLogin())
            val begins = h.identity.loginRequests
            assertFalse(h.shell.login(email, "preview-only")); runCurrent()
            assertEquals(begins, h.identity.loginRequests)
            assertEquals(NativeAccountState.CleanupRequired, h.account.state.value)

            h.identity.clearFails = false
            h.shell.logout(); runCurrent()
            assertEquals(NativeAccountState.SignedOut, h.account.state.value)
            assertTrue(h.shell.login(email, "preview-only")); runCurrent()
            assertEquals(begins + 1, h.identity.loginRequests)
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

            h.shell.openLogin(); h.shell.login(email, "preview-only"); runCurrent()
            h.identity.completeLogin(approve = true); runCurrent()
            assertEquals(PreviewAccount.Minh.actor, h.shell.library.value.account)
            assertTrue(h.shell.library.value.saved.none { it.saved }, "the second sample listener has no saved works")
            assertTrue(h.shell.library.value.continueListening.isEmpty())
        } finally { h.close() }
    }
}
