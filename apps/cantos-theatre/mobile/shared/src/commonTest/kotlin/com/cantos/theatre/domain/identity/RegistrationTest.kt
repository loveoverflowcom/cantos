package com.cantos.theatre.domain.identity

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertIs

class RegistrationTest {
    private fun <T> accepted(result: RegistrationResult<T>): T = assertIs<RegistrationResult.Accepted<T>>(result).value

    @Test fun mailbox_validation_preserves_spelling_and_rejects_invalid_candidates() {
        assertEquals("Linh@example.test", accepted(RegistrationEmail.parse(" Linh@EXAMPLE.test ")).value)
        listOf("", "a", "a@b", "a..b@example.test", ".a@example.test", "a@-example.test", "a@@example.test", "tên@example.test")
            .forEach { assertEquals(RegistrationResult.Refused(RegistrationFailure.InvalidEmail), RegistrationEmail.parse(it), it) }
        assertFalse(accepted(RegistrationEmail.parse("linh@example.test")).toString().contains("linh"))
    }

    @Test fun leaving_a_submission_preserves_email_and_requires_the_user_to_check_mail_first() {
        val next = suspendRegistration(RegistrationState("linh@example.test", RegistrationPhase.Submitting))
        assertEquals("linh@example.test", next.email)
        assertEquals(RegistrationPhase.Failed(RegistrationFailure.StartUnknownOutcome), next.phase)
        RegistrationCommand.entries.forEach { assertFalse(next.allows(it)) }
        listOf(RegistrationPhase.Editing, RegistrationPhase.Waiting, RegistrationPhase.Cancelled, RegistrationPhase.Idle).forEach { phase ->
            assertEquals(phase, suspendRegistration(RegistrationState("a@example.test", phase)).phase)
        }
    }

    @Test fun only_an_editable_state_offers_a_signup_so_no_duplicate_mutation_or_session_is_claimed() {
        val phases = listOf(RegistrationPhase.Idle, RegistrationPhase.Editing, RegistrationPhase.Submitting, RegistrationPhase.Waiting,
            RegistrationPhase.Cancelled, RegistrationPhase.Failed(RegistrationFailure.StartUnknownOutcome),
            RegistrationPhase.Failed(RegistrationFailure.RateLimited))
        for (phase in phases) {
            assertEquals(phase == RegistrationPhase.Editing, RegistrationState(phase = phase).allows(RegistrationCommand.Submit), phase.toString())
        }
        assertEquals(listOf(RegistrationCommand.Submit), RegistrationCommand.entries, "Signup initiation has no confirm or setup command")
    }
}
