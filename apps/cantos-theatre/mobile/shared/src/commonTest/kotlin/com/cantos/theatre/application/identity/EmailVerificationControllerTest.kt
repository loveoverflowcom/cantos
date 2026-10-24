package com.cantos.theatre.application.identity

import com.cantos.theatre.domain.identity.CompletionFailure
import com.cantos.theatre.domain.identity.CompletionStatus
import com.cantos.theatre.domain.identity.ProductionVerifyEmailHandler
import com.cantos.theatre.domain.identity.SignupCompletionRequest
import com.cantos.theatre.domain.identity.VerificationPhase
import com.cantos.theatre.ports.IdentityV1Failure
import com.cantos.theatre.ports.IdentityV1Result
import com.cantos.theatre.ports.SignupCompletionApi
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.withContext
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertSame

@OptIn(kotlinx.coroutines.ExperimentalCoroutinesApi::class)
class EmailVerificationControllerTest {
    private val handler = ProductionVerifyEmailHandler
    private val op = "0123456789abcdef0123456789abcdef"
    private val proof = "fedcba9876543210fedcba9876543210"
    private val link = "$handler?mode=verifyEmail&oobCode=synthetic&apiKey=k" +
        "&continueUrl=https%3A%2F%2Fcantos.cc%2Fverify-email%3Foperation%3D$op%26proof%3D$proof&lang=vi"
    private val password = "Người dẫn chuyện"

    private class MockCompletion : SignupCompletionApi {
        val requests = mutableListOf<SignupCompletionRequest>()
        var held: CompletableDeferred<IdentityV1Result<CompletionStatus>>? = null
        var result: IdentityV1Result<CompletionStatus> = IdentityV1Result.Accepted(CompletionStatus.Completed)
        override suspend fun completeSignup(request: SignupCompletionRequest): IdentityV1Result<CompletionStatus> {
            requests += request
            // An answer that arrives regardless of the caller's cancellation, like a late network reply.
            return held?.let { withContext(NonCancellable) { it.await() } } ?: result
        }
    }

    private var clock = 0L
    private fun owner(scope: CoroutineScope, api: MockCompletion) = EmailVerificationController(scope, api, handler) { clock }
    private fun EmailVerificationController.typeAndConfirm() {
        editPassword(password); editRepeat(password); confirm(true)
    }

    @Test fun only_the_pinned_handler_is_claimed_and_a_link_alone_sends_nothing() = runTest {
        val api = MockCompletion()
        val owner = owner(this, api)
        assertEquals(LinkReceipt.NotClaimed, owner.receive("https://other.example/verify-email?mode=verifyEmail"))
        assertEquals(VerificationPhase.Idle, owner.state.value.phase)
        assertEquals(LinkReceipt.Shown, owner.receive("$handler?mode=verifyEmail"))
        assertEquals(VerificationPhase.LinkRefused, owner.state.value.phase)
        assertEquals(LinkReceipt.Shown, owner.receive(link))
        owner.complete(); owner.retry(); runCurrent()
        assertEquals(VerificationPhase.AwaitingConfirmation, owner.state.value.phase)
        assertEquals(0, api.requests.size)
    }

    @Test fun confirmation_sends_one_request_and_a_new_link_while_pending_is_dropped() = runTest {
        val api = MockCompletion()
        val held = CompletableDeferred<IdentityV1Result<CompletionStatus>>().also { api.held = it }
        val owner = owner(this, api)
        owner.receive(link); owner.typeAndConfirm()
        owner.complete(); owner.complete(); runCurrent()
        assertEquals(1, api.requests.size)
        assertEquals(password, api.requests.single().password)
        assertEquals(LinkReceipt.Busy, owner.receive(link))
        assertEquals(VerificationPhase.Completing, owner.state.value.phase)
        held.complete(IdentityV1Result.Accepted(CompletionStatus.Replayed)); runCurrent()
        assertEquals(VerificationPhase.Completed(CompletionStatus.Replayed), owner.state.value.phase)
    }

    @Test fun an_unknown_outcome_repeats_the_identical_request_after_the_wait() = runTest {
        val api = MockCompletion().apply { result = IdentityV1Result.Refused(IdentityV1Failure.RateLimited(5)) }
        val owner = owner(this, api)
        owner.receive(link); owner.typeAndConfirm(); owner.complete(); runCurrent()
        assertEquals(VerificationPhase.CompleteRetry(CompletionFailure.RateLimited(5)), owner.state.value.phase)
        assertEquals(5, owner.retryWaitSeconds())
        owner.retry(); runCurrent()
        assertEquals(1, api.requests.size, "no dense retry inside the 429 hold")
        clock = 5_000
        api.result = IdentityV1Result.Refused(IdentityV1Failure.Transport)
        owner.retry(); runCurrent()
        assertEquals(VerificationPhase.CompleteRetry(CompletionFailure.UnknownOutcome), owner.state.value.phase)
        api.result = IdentityV1Result.Accepted(CompletionStatus.Replayed)
        owner.retry(); runCurrent()
        assertEquals(3, api.requests.size)
        assertSame(api.requests[0], api.requests[2], "the identical request, never a new one")
        assertEquals(VerificationPhase.Completed(CompletionStatus.Replayed), owner.state.value.phase)
    }

    @Test fun leaving_fences_a_late_answer_and_forgets_the_link() = runTest {
        val api = MockCompletion()
        val held = CompletableDeferred<IdentityV1Result<CompletionStatus>>().also { api.held = it }
        val owner = owner(this, api)
        owner.receive(link); owner.typeAndConfirm(); owner.complete(); runCurrent()
        owner.leave()
        held.complete(IdentityV1Result.Accepted(CompletionStatus.Completed)); runCurrent()
        assertEquals(VerificationPhase.Idle, owner.state.value.phase)
        owner.editPassword(password); owner.complete(); runCurrent()
        assertEquals(1, api.requests.size)
    }

    @Test fun api_failures_map_to_the_completion_contract() = runTest {
        val expected = listOf(
            IdentityV1Failure.Protocol to VerificationPhase.CompleteRefused(CompletionFailure.Invalid),
            IdentityV1Failure.Unauthorized to VerificationPhase.CompleteRefused(CompletionFailure.Refused),
            IdentityV1Failure.Denied to VerificationPhase.CompleteRefused(CompletionFailure.Forbidden),
            IdentityV1Failure.NotConfigured to VerificationPhase.CompleteRefused(CompletionFailure.NotConfigured),
            IdentityV1Failure.RateLimited(null) to VerificationPhase.CompleteRetry(CompletionFailure.RateLimited(null)),
            IdentityV1Failure.ServiceUnavailable to VerificationPhase.CompleteRetry(CompletionFailure.Unavailable),
            IdentityV1Failure.Transport to VerificationPhase.CompleteRetry(CompletionFailure.UnknownOutcome),
            IdentityV1Failure.UnknownOutcome to VerificationPhase.CompleteRetry(CompletionFailure.UnknownOutcome),
            IdentityV1Failure.SessionChanged to VerificationPhase.CompleteRetry(CompletionFailure.UnknownOutcome),
        )
        for ((failure, phase) in expected) {
            val api = MockCompletion().apply { result = IdentityV1Result.Refused(failure) }
            val owner = owner(this, api)
            owner.receive(link); owner.typeAndConfirm(); owner.complete(); runCurrent()
            assertEquals(phase, owner.state.value.phase, failure.toString())
        }
    }
}
