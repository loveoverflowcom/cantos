package com.cantos.theatre.application.identity

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse

class AccountViewTest {
    private val actor = "00000000-0000-4000-8000-000000000001"

    @Test
    fun an_absent_identity_capability_is_a_guest_shell_not_a_failure() {
        assertEquals(AccountView.Unavailable, null.toAccountView())
    }

    @Test
    fun every_owner_state_has_one_distinct_presentation_state() {
        assertEquals(AccountView.SignedOut, NativeAccountState.SignedOut.toAccountView())
        assertEquals(AccountView.Pending, NativeAccountState.Pending.toAccountView())
        assertEquals(AccountView.CleanupRequired, NativeAccountState.CleanupRequired.toAccountView())
        assertEquals(AccountView.SignedIn(), NativeAccountState.SignedIn(actor, 200).toAccountView())
        NativeAccountFailure.entries.forEach {
            assertEquals(AccountView.Failed(it), NativeAccountState.Failed(it).toAccountView())
        }
    }

    @Test
    fun signed_in_presentation_carries_no_actor_session_or_expiry() {
        val rendered = NativeAccountState.SignedIn(actor, 200).toAccountView().toString()
        assertFalse(actor in rendered)
        assertFalse("200" in rendered)
    }

    @Test
    fun only_the_guest_resting_states_may_begin_login() {
        assertEquals(
            listOf(true, true, false, false, false, false),
            listOf(
                AccountView.SignedOut, AccountView.Failed(NativeAccountFailure.Api), AccountView.Unavailable,
                AccountView.Pending, AccountView.SignedIn(), AccountView.CleanupRequired,
            ).map { it.canBeginLogin },
        )
    }
}
