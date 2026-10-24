package com.cantos.theatre.domain.identity

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertSame
import kotlin.test.assertTrue

class EmailVerificationTest {
    private val handler = ProductionVerifyEmailHandler
    private val op = "0123456789abcdef0123456789abcdef"
    private val proof = "fedcba9876543210fedcba9876543210"
    private val continuation = "$handler?operation=$op&proof=$proof"
    private val password = "Ánh đèn cuối sân khấu"

    /** Percent-encodes everything outside the unreserved set, as Firebase does for continueUrl. */
    private fun encode(value: String): String = value.encodeToByteArray().joinToString("") { byte ->
        val char = (byte.toInt() and 0xff).toChar()
        if (char.isLetterOrDigit() && char.code < 0x80 || char in "-_.~") char.toString()
        else "%" + (byte.toInt() and 0xff).toString(16).uppercase().padStart(2, '0')
    }

    private fun link(
        continueUrl: String = continuation, code: String = "synthetic-Action_code.1", extra: String = "",
        mode: String = "verifyEmail", base: String = handler,
    ) = "$base?mode=$mode&oobCode=$code&apiKey=synthetic-key&continueUrl=${encode(continueUrl)}&lang=vi$extra"

    private fun captured(): EmailVerificationState =
        EmailVerificationState.Initial.capture(assertNotNull(VerificationLink.parse(link(), handler)))

    private fun ready(): EmailVerificationState = captured().editPassword(password).editRepeat(password).confirm(true)

    @Test fun the_mailed_firebase_link_parses_against_the_pinned_handler_only() {
        val parsed = assertNotNull(VerificationLink.parse(link(), handler))
        assertEquals(op, parsed.operationId)
        assertEquals(proof, parsed.proofId)
        assertEquals("synthetic-Action_code.1", parsed.oobCode)
        assertEquals("VerificationLink([redacted])", parsed.toString())
        // Locators in either order, and a `+`-free percent-encoded continuation, are the same link.
        assertNotNull(VerificationLink.parse(link("$handler?proof=$proof&operation=$op"), handler))
        assertNotNull(VerificationLink.parse("$handler?continueUrl=${encode(continuation)}&mode=verifyEmail&oobCode=c", handler))
    }

    @Test fun every_deviation_from_the_web_rules_refuses_the_link() {
        val refused = listOf(
            link(extra = "&oobCode=other"),
            link(extra = "&mode=verifyEmail"),
            link(extra = "&unknown=1"),
            link(extra = "#fragment"),
            link(mode = "resetPassword"),
            link(mode = "VerifyEmail"),
            link(code = ""),
            link(code = "a".repeat(4097)),
            link(code = "has%20space"),
            link(code = "slash/char"),
            link(continueUrl = continuation.replace("cantos.cc", "other.example")),
            link(continueUrl = continuation.replace("https://", "http://")),
            link(continueUrl = continuation.replace("/verify-email", "/")),
            link(continueUrl = continuation.replace("/verify-email", "/verify-email/x")),
            link(continueUrl = "https://user@cantos.cc/verify-email?operation=$op&proof=$proof"),
            link(continueUrl = "$continuation&secret=${"ab".repeat(32)}"),
            link(continueUrl = "$continuation&proof=$proof"),
            link(continueUrl = "$continuation#fragment"),
            link(continueUrl = "$handler?operation=$op"),
            link(continueUrl = continuation.replace(op, op.uppercase())),
            link(continueUrl = continuation.replace(op, op.dropLast(1))),
            link(continueUrl = continuation.replace(proof, proof + "0")),
            // The link itself must be the pinned handler, whatever an explicit intent delivers.
            link(base = "https://cantos.cc/"),
            link(base = "https://other.example/verify-email"),
            link(base = "http://cantos.cc/verify-email"),
            link(base = "https://cantos.cc/verify-email/"),
            link(base = "https://www.cantos.cc/verify-email"),
            "$handler?mode=verifyEmail&oobCode=c&continueUrl=%E0%A4",
            "$handler?mode=verifyEmail&oobCode=c&continueUrl=%zz",
            "$handler?mode=verifyEmail&oobCode=c&continueUrl=%2",
            handler,
            link() + "&x".repeat(VerificationLink.MAX_LINK_CHARS),
        )
        for (value in refused) assertNull(VerificationLink.parse(value, handler), value.take(200))
        assertNull(VerificationLink.parse(link(base = "http://127.0.0.1:8082/verify-email"), "http://127.0.0.1:8082/verify-email"))
    }

    @Test fun only_the_pinned_handler_is_claimed_and_anything_else_is_not_this_apps_link() {
        for (value in listOf(handler, link(), "$handler#x", "$handler?garbage")) assertTrue(VerificationLink.claims(value, handler), value)
        for (value in listOf("https://cantos.cc/", "https://cantos.cc/verify-emails", "https://cantos.cc/verify-email/x",
            "https://other.example/verify-email?mode=verifyEmail", "http://cantos.cc/verify-email")) {
            assertFalse(VerificationLink.claims(value, handler), value)
        }
    }

    @Test fun password_hints_follow_the_documented_policy_in_scalars_without_trimming() {
        for ((value, issue) in listOf("" to PasswordIssue.TooShort, "1234567" to PasswordIssue.TooShort,
            "12345678" to null, "Việt Nam" to null, "        " to null, "a".repeat(129) to PasswordIssue.TooLong)) {
            assertEquals(issue, passwordIssue(value, value), value)
        }
        // 128 four-byte emoji are 256 UTF-16 units but 128 scalars and exactly 512 bytes: valid.
        val emoji = "😀".repeat(128)
        assertEquals(null, passwordIssue(emoji, emoji))
        assertEquals(PasswordIssue.TooShort, passwordIssue("😀".repeat(7), "😀".repeat(7)))
        assertEquals(PasswordIssue.Mismatch, passwordIssue(password, "$password "))
    }

    @Test fun opening_a_link_never_builds_a_request_until_the_owner_confirms_a_valid_password() {
        val opened = captured()
        assertEquals(VerificationPhase.AwaitingConfirmation, opened.phase)
        assertNull(opened.beginComplete())
        assertNull(opened.beginRetry(Long.MAX_VALUE))
        val typed = opened.editPassword(password).editRepeat(password)
        assertFalse(typed.canConfirm)
        assertNull(typed.beginComplete())
        val confirmed = typed.confirm(true)
        assertTrue(confirmed.canConfirm)
        // Any edit withdraws the confirmation.
        val edited = confirmed.editRepeat(password.dropLast(1))
        assertFalse(edited.confirmed || edited.canConfirm)
        assertEquals(PasswordIssue.Mismatch, edited.passwordIssue)
        assertNull(edited.editRepeat(password).beginComplete())
        // Without a link nothing can be typed or confirmed.
        assertSame(EmailVerificationState.Initial, EmailVerificationState.Initial.editPassword(password))
    }

    @Test fun the_request_is_built_once_and_the_editable_inputs_are_cleared_before_io() {
        val (completing, issued) = assertNotNull(ready().beginComplete())
        assertEquals(VerificationPhase.Completing, completing.phase)
        assertEquals(Triple("", "", false), Triple(completing.password, completing.repeat, completing.confirmed))
        assertEquals(op, issued.request.operationId)
        assertEquals(proof, issued.request.proofId)
        assertEquals("synthetic-Action_code.1", issued.request.firebaseOobCode)
        assertEquals(password, issued.request.password)
        assertSame(issued.request, completing.retainedRequest)
        // A second press during flight builds nothing and cannot edit the password.
        assertNull(completing.beginComplete())
        assertSame(completing, completing.editPassword("another password"))
        // A new link while one is pending is refused.
        assertSame(completing, completing.capture(assertNotNull(VerificationLink.parse(link(), handler))))
        assertSame(completing, completing.linkRefused())
    }

    @Test fun lost_or_unavailable_answers_retry_the_identical_request_only() {
        for (failure in listOf(CompletionFailure.Unavailable, CompletionFailure.UnknownOutcome, CompletionFailure.RateLimited(null))) {
            val (completing, first) = assertNotNull(ready().beginComplete())
            val retry = completing.finish(first.issued, CompletionResult.Failed(failure), 1_000)
            assertEquals(VerificationPhase.CompleteRetry(failure), retry.phase)
            assertSame(retry, retry.editPassword("silently different"))
            assertNull(retry.beginComplete())
            val (again, second) = assertNotNull(retry.beginRetry(1_000))
            assertTrue(second.issued > first.issued)
            assertSame(first.request, second.request, "the identical request")
            // The first attempt's late answer cannot settle the second.
            assertSame(again, again.finish(first.issued, CompletionResult.Done(CompletionStatus.Completed), 0))
            assertNull(again.beginRetry(1_000))
            val done = again.finish(second.issued, CompletionResult.Done(CompletionStatus.Replayed), 0)
            assertEquals(VerificationPhase.Completed(CompletionStatus.Replayed), done.phase)
            assertNull(done.retainedRequest)
        }
    }

    @Test fun retry_after_is_honoured_and_reported_in_whole_seconds() {
        val (completing, issued) = assertNotNull(ready().beginComplete())
        val held = completing.finish(issued.issued, CompletionResult.Failed(CompletionFailure.RateLimited(30)), 10_000)
        assertEquals(30, held.retryWaitSeconds(10_000))
        assertEquals(1, held.retryWaitSeconds(39_001))
        assertNull(held.beginRetry(39_999))
        assertNull(held.retryWaitSeconds(40_000))
        assertNotNull(held.beginRetry(40_000))
        // An absurd Retry-After saturates instead of wrapping to an earlier instant.
        val huge = completing.finish(issued.issued, CompletionResult.Failed(CompletionFailure.RateLimited(Long.MAX_VALUE)), 10_000)
        assertNull(huge.beginRetry(Long.MAX_VALUE - 1))
    }

    @Test fun success_drops_every_secret_and_confers_no_session() {
        for (status in CompletionStatus.entries) {
            val (completing, issued) = assertNotNull(ready().beginComplete())
            val done = completing.finish(issued.issued, CompletionResult.Done(status), 0)
            assertEquals(VerificationPhase.Completed(status), done.phase)
            assertNull(done.retainedRequest)
            assertNull(done.beginComplete())
            assertNull(done.beginRetry(Long.MAX_VALUE))
            assertSame(done, done.editAfterRefusal())
            assertSame(done, done.finish(issued.issued, CompletionResult.Failed(CompletionFailure.Refused), 0))
        }
    }

    @Test fun definite_refusals_drop_the_request_and_only_a_400_keeps_the_link() {
        for (failure in listOf(CompletionFailure.Refused, CompletionFailure.Forbidden, CompletionFailure.NotConfigured)) {
            val (completing, issued) = assertNotNull(ready().beginComplete())
            val refused = completing.finish(issued.issued, CompletionResult.Failed(failure), 0)
            assertEquals(VerificationPhase.CompleteRefused(failure), refused.phase)
            assertNull(refused.retainedRequest)
            assertSame(refused, refused.editAfterRefusal())
            assertNull(refused.beginRetry(Long.MAX_VALUE))
        }
        val (completing, issued) = assertNotNull(ready().beginComplete())
        val invalid = completing.finish(issued.issued, CompletionResult.Failed(CompletionFailure.Invalid), 0)
        assertNull(invalid.retainedRequest)
        val again = invalid.editAfterRefusal()
        assertEquals(VerificationPhase.AwaitingConfirmation, again.phase)
        val (_, next) = assertNotNull(again.editPassword("một mật khẩu khác").editRepeat("một mật khẩu khác").confirm(true).beginComplete())
        assertEquals("một mật khẩu khác", next.request.password)
        assertEquals(issued.request.firebaseOobCode, next.request.firebaseOobCode, "the intact link is reused")
    }

    @Test fun leaving_or_a_new_link_fences_late_answers_and_forgets_everything() {
        val (completing, issued) = assertNotNull(ready().beginComplete())
        val left = completing.clear()
        assertEquals(VerificationPhase.Idle, left.phase)
        assertNull(left.retainedRequest)
        assertSame(left, left.finish(issued.issued, CompletionResult.Done(CompletionStatus.Completed), 0))
        val reopened = left.capture(assertNotNull(VerificationLink.parse(link(), handler)))
        assertSame(reopened, reopened.finish(issued.issued, CompletionResult.Done(CompletionStatus.Completed), 0))
        assertEquals(VerificationPhase.AwaitingConfirmation, reopened.phase)
        // A new link replaces a typed password and a retained request alike.
        val typed = ready()
        val replaced = typed.capture(assertNotNull(VerificationLink.parse(link(code = "second"), handler)))
        assertEquals(Triple("", "", false), Triple(replaced.password, replaced.repeat, replaced.confirmed))
        assertTrue(replaced.epoch > typed.epoch)
        val unreadable = typed.linkRefused()
        assertEquals(VerificationPhase.LinkRefused, unreadable.phase)
        assertNull(unreadable.beginComplete())
    }

    @Test fun no_derived_string_contains_the_link_password_or_code() {
        val (completing, issued) = assertNotNull(ready().beginComplete())
        for (text in listOf(ready().toString(), completing.toString(), issued.toString(), issued.request.toString())) {
            for (hidden in listOf(op, proof, "synthetic-Action_code.1", password)) assertFalse(hidden in text, text)
        }
    }
}
