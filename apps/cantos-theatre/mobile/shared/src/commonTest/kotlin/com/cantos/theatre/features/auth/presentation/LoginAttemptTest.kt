package com.cantos.theatre.features.auth.presentation

import com.cantos.theatre.application.identity.AccountLoginFailure
import com.cantos.theatre.application.identity.AccountView
import com.cantos.theatre.application.identity.NativeAccountFailure
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class LoginAttemptTest {
    private val refused = AccountView.Failed(NativeAccountFailure.Api, AccountLoginFailure.InvalidCredentials)
    private val limited = AccountView.Failed(NativeAccountFailure.Api, AccountLoginFailure.RateLimited(17))
    private val expired = AccountView.Failed(NativeAccountFailure.Expired)
    private val resting = listOf(AccountView.SignedOut, AccountView.SignedIn, AccountView.CleanupRequired, AccountView.Unavailable)

    private fun fold(start: LoginAttempt, vararg views: AccountView) = views.fold(start) { attempt, view -> attempt.after(view) }

    @Test
    fun an_attempt_owns_only_the_failure_it_settles_as() {
        for (failure in listOf(refused, limited)) {
            val attempt = fold(LoginAttempt.Open, AccountView.Pending, failure)
            assertEquals(LoginAttempt.Settled(failure), attempt)
            assertTrue(attempt.owns(failure))
            assertFalse(attempt.owns(expired), "a different failure is not this attempt's feedback")
        }
    }

    @Test
    fun without_an_attempt_every_failure_stays_account_status() {
        for (failure in listOf(refused, limited, expired)) {
            val attempt = fold(LoginAttempt.None, AccountView.Pending, failure)
            assertEquals(LoginAttempt.None, attempt)
            assertFalse(attempt.owns(failure), "identical failure type, but nobody attempted it on Login")
        }
    }

    @Test
    fun any_other_owner_state_ends_the_scope_so_a_later_failure_is_shown() {
        for (view in resting) {
            assertEquals(LoginAttempt.None, fold(LoginAttempt.Open, AccountView.Pending, view), "$view settles nothing")
            val ended = fold(LoginAttempt.Open, refused, view)
            assertEquals(LoginAttempt.None, ended, "$view replaces the attempt's failure")
            // The same failure value later, after a sign-in or cleanup, is a new account fact.
            assertFalse(ended.after(refused).owns(refused))
        }
        assertFalse(fold(LoginAttempt.Open, refused, AccountView.Pending, refused).owns(refused), "a new attempt is not opened by the owner")
        assertFalse(fold(LoginAttempt.Open, refused, expired).owns(expired))
    }

    @Test
    fun the_settled_failure_survives_recomposition_with_the_same_value() {
        val attempt = fold(LoginAttempt.Open, refused, refused, refused.copy())
        assertTrue(attempt.owns(refused))
    }

    @Test
    fun a_scope_never_owns_a_non_failure_state() {
        val attempt = fold(LoginAttempt.Open, refused)
        for (view in resting + AccountView.Pending) assertFalse(attempt.owns(view))
    }
}
