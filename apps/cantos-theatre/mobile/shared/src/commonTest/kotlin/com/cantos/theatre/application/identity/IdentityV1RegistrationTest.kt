package com.cantos.theatre.application.identity

import com.cantos.theatre.domain.identity.NativeGrant
import com.cantos.theatre.domain.identity.NativeSession
import com.cantos.theatre.domain.identity.RegistrationEmail
import com.cantos.theatre.domain.identity.RegistrationFailure
import com.cantos.theatre.domain.identity.RegistrationPhase
import com.cantos.theatre.ports.NativeIdentityV1Api
import com.cantos.theatre.ports.IdentityV1Result
import com.cantos.theatre.ports.IdentityV1Failure
import com.cantos.theatre.ports.IdentitySignupAccepted
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.withContext
import kotlin.test.Test
import kotlin.test.assertEquals

@OptIn(kotlinx.coroutines.ExperimentalCoroutinesApi::class)
class IdentityV1RegistrationTest {
    private class MockApi : NativeIdentityV1Api {
        var starts = 0
        var held: CompletableDeferred<IdentityV1Result<IdentitySignupAccepted>>? = null
        var result: IdentityV1Result<IdentitySignupAccepted> = IdentityV1Result.Accepted(IdentitySignupAccepted(900))
        override suspend fun signup(email: RegistrationEmail): IdentityV1Result<IdentitySignupAccepted> {
            starts++
            return held?.let { withContext(NonCancellable) { it.await() } } ?: result
        }
        override suspend fun login(email: RegistrationEmail, password: String): IdentityV1Result<NativeGrant> = error("signup cannot log in")
        override suspend fun refresh(previous: NativeGrant): IdentityV1Result<NativeGrant> = error("signup cannot refresh")
        override suspend fun revokeFamily(previous: NativeGrant): IdentityV1Result<Unit> = error("signup has no family")
        override suspend fun session(previous: NativeGrant): IdentityV1Result<NativeSession> = error("signup cannot poll")
        override suspend fun revoke(previous: NativeGrant): IdentityV1Result<Unit> = error("signup has no session")
    }
    private fun editing(scope: kotlinx.coroutines.CoroutineScope, api: MockApi) = RegistrationController(scope, api).also {
        it.enter(); it.editEmail("a@example.test")
    }

    @Test fun email_only_signup_validates_and_dispatches_once_without_polling_or_completion() = runTest {
        val api = MockApi(); val owner = RegistrationController(this, api)
        owner.enter(); assertEquals(RegistrationPhase.Editing, owner.state.value.phase)
        owner.editEmail("invalid"); owner.submit(); runCurrent()
        assertEquals(0, api.starts)
        assertEquals(RegistrationPhase.Failed(RegistrationFailure.InvalidEmail), owner.state.value.phase)
        owner.editEmail("a@example.test"); owner.submit(); owner.submit(); runCurrent()
        assertEquals(1, api.starts); assertEquals(RegistrationPhase.Waiting, owner.state.value.phase)
        owner.submit(); owner.enter(); runCurrent()
        assertEquals(1, api.starts); assertEquals(RegistrationPhase.Waiting, owner.state.value.phase)
        owner.close()
    }

    @Test fun back_fences_late_signup_and_unknown_effect_requires_explicit_new_attempt() = runTest {
        val api = MockApi(); val held = CompletableDeferred<IdentityV1Result<IdentitySignupAccepted>>(); api.held = held
        val owner = editing(this, api)
        owner.submit(); runCurrent(); owner.leave()
        held.complete(IdentityV1Result.Accepted(IdentitySignupAccepted(900))); runCurrent()
        assertEquals(RegistrationPhase.Failed(RegistrationFailure.StartUnknownOutcome), owner.state.value.phase)
        assertEquals("a@example.test", owner.state.value.email)
        owner.enter(); owner.submit(); runCurrent(); assertEquals(1, api.starts)
        owner.restart(); assertEquals(RegistrationPhase.Editing, owner.state.value.phase)
        owner.close()
    }

    @Test fun refusals_follow_the_contract_and_never_replay_the_signup_automatically() = runTest {
        val expected = listOf(
            IdentityV1Failure.NotConfigured to RegistrationFailure.Disabled,
            IdentityV1Failure.Protocol to RegistrationFailure.Protocol,
            IdentityV1Failure.Unauthorized to RegistrationFailure.Denied,
            IdentityV1Failure.Denied to RegistrationFailure.Denied,
            // 429 is refused before any effect; the others leave the mail effect unknown.
            IdentityV1Failure.RateLimited(30) to RegistrationFailure.RateLimited,
            IdentityV1Failure.ServiceUnavailable to RegistrationFailure.StartUnknownOutcome,
            IdentityV1Failure.Transport to RegistrationFailure.StartUnknownOutcome,
            IdentityV1Failure.UnknownOutcome to RegistrationFailure.StartUnknownOutcome,
            IdentityV1Failure.SessionChanged to RegistrationFailure.StartUnknownOutcome,
        )
        for ((failure, reason) in expected) {
            val api = MockApi().also { it.result = IdentityV1Result.Refused(failure) }
            val owner = editing(this, api); owner.submit(); runCurrent()
            assertEquals(RegistrationPhase.Failed(reason), owner.state.value.phase, failure.toString())
            owner.submit(); owner.enter(); runCurrent(); assertEquals(1, api.starts)
            owner.close()
        }
    }

    @Test fun cancel_stops_waiting_without_a_request_and_restart_is_the_only_way_to_a_new_attempt() = runTest {
        val api = MockApi(); val owner = editing(this, api)
        owner.submit(); runCurrent(); assertEquals(RegistrationPhase.Waiting, owner.state.value.phase)
        owner.cancel(); assertEquals(RegistrationPhase.Cancelled, owner.state.value.phase)
        owner.submit(); runCurrent(); assertEquals(1, api.starts)
        owner.restart(); owner.submit(); runCurrent(); assertEquals(2, api.starts)
        owner.close()
    }

    @Test fun reset_and_close_forget_the_typed_email() = runTest {
        val api = MockApi(); val owner = editing(this, api)
        owner.reset(); assertEquals("", owner.state.value.email); assertEquals(RegistrationPhase.Idle, owner.state.value.phase)
        owner.enter(); owner.editEmail("b@example.test"); owner.close()
        assertEquals("", owner.state.value.email)
        owner.enter(); owner.submit(); runCurrent(); assertEquals(0, api.starts)
    }
}
