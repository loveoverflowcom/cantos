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
import kotlin.test.assertTrue

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
    @Test fun email_only_signup_validates_and_dispatches_once_without_polling_or_completion() = runTest {
        val api = MockApi(); val owner = RegistrationController(this, api)
        owner.enter(); assertEquals(RegistrationPhase.Editing, owner.state.value.phase)
        assertTrue(owner.state.value.firstParty)
        owner.editEmail("invalid"); owner.submit(); runCurrent()
        assertEquals(0, api.starts)
        assertEquals(RegistrationPhase.Failed(RegistrationFailure.InvalidEmail), owner.state.value.phase)
        owner.editEmail("a@example.test"); owner.submit(); owner.submit(); runCurrent()
        assertEquals(1, api.starts); assertEquals(RegistrationPhase.Waiting, owner.state.value.phase)
        owner.importAction("https://untrusted.invalid/#secret=private"); owner.confirm(); owner.openSetup(); runCurrent()
        assertEquals(RegistrationPhase.Waiting, owner.state.value.phase)
        owner.close()
    }
    @Test fun back_fences_late_signup_and_unknown_effect_requires_explicit_new_attempt() = runTest {
        val api = MockApi(); val held = CompletableDeferred<IdentityV1Result<IdentitySignupAccepted>>(); api.held = held
        val owner = RegistrationController(this, api); owner.enter(); owner.editEmail("a@example.test")
        owner.submit(); runCurrent(); owner.leave()
        held.complete(IdentityV1Result.Accepted(IdentitySignupAccepted(900))); runCurrent()
        assertEquals(RegistrationPhase.Failed(RegistrationFailure.StartUnknownOutcome), owner.state.value.phase)
        assertTrue(owner.state.value.firstParty)
        owner.enter(); owner.submit(); runCurrent(); assertEquals(1, api.starts)
        owner.restart(); assertEquals(RegistrationPhase.Editing, owner.state.value.phase)
        owner.close()
    }
    @Test fun unavailable_signup_never_claims_delivery_or_automatically_replays() = runTest {
        val api = MockApi().also { it.result = IdentityV1Result.Refused(IdentityV1Failure.ServiceUnavailable) }
        val owner = RegistrationController(this, api); owner.enter(); owner.editEmail("a@example.test"); owner.submit(); runCurrent()
        assertEquals(RegistrationPhase.Failed(RegistrationFailure.StartUnknownOutcome), owner.state.value.phase)
        owner.submit(); owner.enter(); runCurrent(); assertEquals(1, api.starts); owner.close()
    }
}
