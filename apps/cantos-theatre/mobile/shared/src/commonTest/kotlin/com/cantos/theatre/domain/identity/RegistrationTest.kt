package com.cantos.theatre.domain.identity

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertIs

class RegistrationTest {
    private fun <T> accepted(result: RegistrationResult<T>): T = assertIs<RegistrationResult.Accepted<T>>(result).value
    private val origin = "https://cantos.example"
    private val link = "$origin/?mode=verifyEmail&oobCode=private-code&continueUrl=https%3A%2F%2Fcantos.example%2F%3FfirebaseEnrollment%3D00000000-0000-4000-8000-000000000001"

    @Test fun mailbox_validation_preserves_spelling_and_rejects_invalid_candidates() {
        assertEquals("Linh@example.test", accepted(RegistrationEmail.parse(" Linh@EXAMPLE.test ")).value)
        listOf("", "a", "a@b", "a..b@example.test", ".a@example.test", "a@-example.test", "a@@example.test", "tên@example.test")
            .forEach { assertEquals(RegistrationResult.Refused(RegistrationFailure.InvalidEmail), RegistrationEmail.parse(it), it) }
    }

    @Test fun action_import_requires_the_exact_handler_continuation_and_unique_canonical_keys() {
        val action = accepted(RegistrationAction.parse(link, origin))
        assertEquals("00000000-0000-4000-8000-000000000001", action.operation)
        assertEquals("private-code", action.code)
        listOf(link + "&oobCode=other", link + "&%6dode=verifyEmail", link + "&unknown=1", link + "#fragment",
            link.replace("verifyEmail", "resetPassword"), link.replace("https://cantos.example/", "https://evil.test/"),
            link.replace("%2Fcantos.example", "%2Fevil.test"), link.replace("private-code", "%00"),
            link.replace("000000000001", "000000000001%26extra%3D1"), link.replace("private-code", "%ZZ"))
            .forEach { assertEquals(RegistrationResult.Refused(RegistrationFailure.InvalidAction), RegistrationAction.parse(it, origin)) }
        assertEquals(RegistrationResult.Refused(RegistrationFailure.InvalidAction), RegistrationAction.parse(link.replace("https://", "http://"), "http://cantos.example"))
        assertFalse(action.toString().contains("private-code"))
    }

    @Test fun setup_is_confined_to_the_exact_issuer_reset_path() {
        accepted(RegistrationSetup.parse("listener_1", "https://idm.example/ui/reset?token=private-token", "https://idm.example"))
        accepted(RegistrationSetup.parse("listener_1", "https://idm.example/ui/reset?token=encoded%2B%26%3Dtoken", "https://idm.example"))
        listOf("http://idm.example/ui/reset?token=x", "https://idm.example.evil/ui/reset?token=x", "https://idm.example/ui/reset?token=x&next=evil",
            "https://idm.example/ui/reset?token=", "https://idm.example/ui/reset?token=x#fragment", "https://idm.example/ui/reset?token=%0A")
            .forEach { assertEquals(RegistrationResult.Refused(RegistrationFailure.Protocol), RegistrationSetup.parse("listener_1", it, "https://idm.example")) }
        assertEquals(RegistrationResult.Refused(RegistrationFailure.Protocol), RegistrationSetup.parse("listener_1", "http://idm.example/ui/reset?token=x", "http://idm.example"))
    }

    @Test fun leaving_a_consuming_operation_preserves_email_and_requires_reconciliation() {
        val action = accepted(RegistrationAction.parse(link, origin))
        listOf(RegistrationPhase.Submitting, RegistrationPhase.Verifying(action), RegistrationPhase.Cancelling).forEach { phase ->
            val next = suspendRegistration(RegistrationState("linh@example.test", phase))
            assertEquals("linh@example.test", next.email)
            val reason = if (phase == RegistrationPhase.Submitting) RegistrationFailure.StartUnknownOutcome else RegistrationFailure.UnknownOutcome
            assertEquals(RegistrationPhase.Failed(reason), next.phase)
            RegistrationCommand.entries.forEach { assertFalse(next.allows(it)) }
        }
        assertEquals(RegistrationPhase.Waiting, suspendRegistration(RegistrationState("a@example.test", RegistrationPhase.Confirmable(action))).phase)
    }

    @Test fun pending_and_accepted_states_never_offer_duplicate_mutations_or_claim_a_session() {
        listOf(RegistrationPhase.Submitting, RegistrationPhase.Waiting, RegistrationPhase.CredentialSetup, RegistrationPhase.Cancelled).forEach { phase ->
            RegistrationCommand.entries.forEach { assertFalse(RegistrationState(phase = phase).allows(it)) }
        }
    }
}
