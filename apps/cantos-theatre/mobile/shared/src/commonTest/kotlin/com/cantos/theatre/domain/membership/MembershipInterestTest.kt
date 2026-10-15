package com.cantos.theatre.domain.membership

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertIs
import kotlin.test.assertNotEquals
import kotlin.test.assertNull
import kotlin.test.assertTrue

class MembershipInterestTest {
    @Test
    fun empty_form_accumulates_independent_findings_and_never_creates_a_receipt() {
        val form = MembershipState.fixture().next(MembershipEvent.OpenForm)
        assertEquals(MembershipDraft(), form.draft)
        val invalid = form.reduce(MembershipEvent.Submit)
        val errors = listOf(MembershipFieldError.NameRequired, MembershipFieldError.EmailRequired)
        assertEquals(MembershipResult.SubmitRefused(MembershipSubmitRefusal.InvalidFields(errors)), invalid.result)
        assertEquals(errors, invalid.state.fieldErrors)
        assertEquals(SubmissionStatus.Idle, invalid.state.submission)
        assertNull(invalid.state.receipt)
        assertEquals(MembershipScreen.Form, invalid.state.screen)
    }

    @Test
    fun default_mode_is_unavailable_even_for_a_valid_form() {
        val form = filled(MembershipState.unavailable())
        val actual = form.reduce(MembershipEvent.Submit)
        assertEquals(MembershipResult.SubmitRefused(MembershipSubmitRefusal.Unavailable), actual.result)
        assertEquals(SubmissionStatus.Failed, actual.state.submission)
        assertEquals(form.draft, actual.state.draft)
        assertEquals(MembershipScreen.Form, actual.state.screen)
        assertNull(actual.state.receipt)
        assertEquals(MembershipResult.Ignored, actual.state.reduce(MembershipEvent.ShowAccountRegisteredFixture).result)
    }

    @Test
    fun pending_duplicate_and_edit_are_refused_until_an_explicit_matching_confirmation() {
        val started = filled(MembershipState.fixture()).reduce(MembershipEvent.Submit)
        val attempt = assertIs<MembershipResult.Started>(started.result).attempt
        val pending = started.state
        assertEquals(SubmissionStatus.Pending, pending.submission)
        assertEquals(pending, pending.reduce(MembershipEvent.Submit).state)
        assertEquals(MembershipResult.SubmitRefused(MembershipSubmitRefusal.AlreadyPending), pending.reduce(MembershipEvent.Submit).result)
        assertEquals(MembershipResult.EditRefused(MembershipEditRefusal.Pending), pending.reduce(MembershipEvent.SetEmail("other@example.com")).result)
        assertNull(pending.receipt)
        val received = pending.next(MembershipEvent.Finish(attempt, MembershipFinishOutcome.Received))
        assertEquals(MembershipScreen.Received, received.screen)
        assertEquals(SubmissionStatus.Received, received.submission)
        assertEquals(attempt, received.receipt)
        assertEquals(RegistrationValues("Nguyễn Thị Ánh", "anh@example.com"), RegistrationValues(attempt.payload.name, attempt.payload.email))
        assertEquals(MembershipResult.Ignored, received.reduce(MembershipEvent.Finish(attempt, MembershipFinishOutcome.Received)).result)
    }

    @Test
    fun unknown_retry_retains_intent_payload_but_rejects_the_original_callback() {
        val first = filled(MembershipState.fixture()).reduce(MembershipEvent.Submit)
        val old = assertIs<MembershipResult.Started>(first.result).attempt
        val unknown = first.state.next(MembershipEvent.Finish(old, MembershipFinishOutcome.Unknown))
        assertNull(unknown.receipt)
        assertEquals(MembershipResult.EditRefused(MembershipEditRefusal.OutcomeUnknown), unknown.reduce(MembershipEvent.SetName("Khác")).result)
        val second = unknown.reduce(MembershipEvent.Submit)
        val retry = assertIs<MembershipResult.Started>(second.result).attempt
        assertEquals(old.operationId, retry.operationId)
        assertEquals(old.payload, retry.payload)
        assertNotEquals(old.requestId, retry.requestId)
        assertEquals(second.state, second.state.next(MembershipEvent.Finish(old, MembershipFinishOutcome.Received)))
        assertEquals(MembershipScreen.Received, second.state.next(MembershipEvent.Finish(retry, MembershipFinishOutcome.Received)).screen)
    }

    @Test
    fun known_failure_retry_keeps_intent_while_an_edit_starts_a_new_intent() {
        val first = filled(MembershipState.fixture()).reduce(MembershipEvent.Submit)
        val attempt = assertIs<MembershipResult.Started>(first.result).attempt
        val failed = first.state.next(MembershipEvent.Finish(attempt, MembershipFinishOutcome.Failed))
        val retry = failed.reduce(MembershipEvent.Submit)
        val second = assertIs<MembershipResult.Started>(retry.result).attempt
        assertEquals(attempt.operationId, second.operationId)
        val editable = retry.state.next(MembershipEvent.Finish(second, MembershipFinishOutcome.Failed))
            .next(MembershipEvent.SetName("Nguyễn Minh An"))
        val third = assertIs<MembershipResult.Started>(editable.reduce(MembershipEvent.Submit).result).attempt
        assertNotEquals(second.operationId, third.operationId)
        assertEquals("Nguyễn Minh An", third.payload.name)
    }

    @Test
    fun back_preserves_draft_and_fences_pending_results_even_after_returning_to_form() {
        val first = filled(MembershipState.fixture()).reduce(MembershipEvent.Submit)
        val attempt = assertIs<MembershipResult.Started>(first.result).attempt
        val intro = first.state.next(MembershipEvent.Back)
        assertEquals(MembershipScreen.Intro, intro.screen)
        assertEquals(first.state.draft, intro.draft)
        assertEquals(SubmissionStatus.Unknown, intro.submission)
        val reopened = intro.next(MembershipEvent.OpenForm)
        assertEquals(reopened, reopened.next(MembershipEvent.Finish(attempt, MembershipFinishOutcome.Received)))
        assertNull(reopened.receipt)
    }

    @Test
    fun session_boundary_clears_every_private_projection_and_never_reuses_request_ids() {
        val first = filled(MembershipState.fixture()).reduce(MembershipEvent.Submit)
        val attempt = assertIs<MembershipResult.Started>(first.result).attempt
        val received = first.state.next(MembershipEvent.Finish(attempt, MembershipFinishOutcome.Received))
            .next(MembershipEvent.SetPlansStatus(PlansStatus.Failed)).next(MembershipEvent.OpenPlans)
        val cleared = received.next(MembershipEvent.ClearSession)
        assertEquals(MembershipScreen.Intro, cleared.screen)
        assertEquals(MembershipDraft(), cleared.draft)
        assertEquals(SubmissionStatus.Idle, cleared.submission)
        assertEquals(PlansStatus.Unpublished, cleared.plansStatus)
        assertTrue(cleared.fieldErrors.isEmpty())
        assertNull(cleared.receipt)
        assertEquals(cleared, cleared.next(MembershipEvent.Finish(attempt, MembershipFinishOutcome.Received)))
        val next = filled(cleared).reduce(MembershipEvent.Submit)
        val newAttempt = assertIs<MembershipResult.Started>(next.result).attempt
        assertTrue(newAttempt.requestId > attempt.requestId)
        assertNotEquals(newAttempt.operationId, attempt.operationId)
    }

    @Test
    fun plans_return_to_each_exact_source_and_deep_links_fall_back_to_account() {
        val fixture = MembershipState.fixture()
        val receivedRequest = filled(fixture).reduce(MembershipEvent.Submit)
        val attempt = assertIs<MembershipResult.Started>(receivedRequest.result).attempt
        val received = receivedRequest.state.next(MembershipEvent.Finish(attempt, MembershipFinishOutcome.Received))
        val cases = listOf(
            fixture to PlansOrigin.Intro,
            fixture.next(MembershipEvent.OpenAccount) to PlansOrigin.Account,
            fixture.next(MembershipEvent.ShowAccountRegisteredFixture) to PlansOrigin.AccountRegistered,
            received to PlansOrigin.Received,
        )
        cases.forEach { (before, origin) ->
            val plans = before.next(MembershipEvent.OpenPlans)
            assertEquals(MembershipScreen.Plans(origin), plans.screen)
            PlansStatus.entries.forEach { status ->
                val state = plans.next(MembershipEvent.SetPlansStatus(status))
                assertEquals(before.receipt, state.receipt)
                assertEquals(before.screen, state.next(MembershipEvent.Back).screen)
            }
        }
        val fallback = fixture.next(MembershipEvent.OpenPlansDeepLink)
        assertEquals(MembershipScreen.Plans(PlansOrigin.Account), fallback.screen)
        assertEquals(MembershipScreen.Account, fallback.next(MembershipEvent.Back).screen)
        assertEquals(MembershipResult.Ignored, filled(fixture).reduce(MembershipEvent.OpenPlans).result)
    }

    @Test
    fun bounded_validation_counts_unicode_scalars_and_has_no_invented_email_domain_policy() {
        assertEquals(emptyList(), validateMembershipDraft(MembershipDraft("  Nguyễn Thị Ánh  ", "  ánh@cantos  ")))
        assertEquals(emptyList(), validateMembershipDraft(MembershipDraft("🎭".repeat(120), "a@b")))
        assertEquals(listOf(MembershipFieldError.NameTooLong), validateMembershipDraft(MembershipDraft("🎭".repeat(121), "a@b")))
        assertEquals(listOf(MembershipFieldError.NameInvalid), validateMembershipDraft(MembershipDraft("Ánh\u0000", "a@b")))
        assertEquals(listOf(MembershipFieldError.EmailTooLong), validateMembershipDraft(MembershipDraft("Ánh", "a".repeat(253) + "@b")))
        listOf("a@@b", "@b", "a@", "a b@c", "a\u0000@b").forEach { email ->
            assertEquals(listOf(MembershipFieldError.EmailInvalid), validateMembershipDraft(MembershipDraft("Ánh", email)))
        }
    }

    @Test
    fun private_text_is_redacted_from_diagnostic_values() {
        val first = filled(MembershipState.fixture()).reduce(MembershipEvent.Submit)
        val attempt = assertIs<MembershipResult.Started>(first.result).attempt
        listOf(first.state, first, attempt, attempt.payload, first.state.draft, MembershipEvent.SetEmail("anh@example.com")).forEach { value ->
            assertFalse(value.toString().contains("anh@example.com"))
            assertFalse(value.toString().contains("Nguyễn"))
        }
    }

    private fun filled(initial: MembershipState): MembershipState = initial.next(MembershipEvent.OpenForm)
        .next(MembershipEvent.SetName("  Nguyễn Thị Ánh  ")).next(MembershipEvent.SetEmail("  anh@example.com  "))

    private fun MembershipState.next(event: MembershipEvent): MembershipState = reduce(event).state
    private data class RegistrationValues(val name: String, val email: String)
}
