package com.cantos.theatre.app

import com.cantos.theatre.adapters.preview.PreviewIdentity
import com.cantos.theatre.adapters.preview.previewLibraryStore
import com.cantos.theatre.application.identity.NativeAccountState
import com.cantos.theatre.application.identity.RegistrationController
import com.cantos.theatre.domain.identity.RegistrationEmail
import com.cantos.theatre.domain.identity.RegistrationFailure
import com.cantos.theatre.domain.identity.RegistrationPhase
import com.cantos.theatre.domain.identity.RegistrationResult
import com.cantos.theatre.ports.IdentitySignupAccepted
import com.cantos.theatre.ports.IdentityV1Result
import com.cantos.theatre.ports.NativeIdentityV1Api
import com.cantos.theatre.presentation.model.ShellEvent
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.withContext
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import org.junit.Test

/** Real shell/registration owners; a non-cooperative remote answer deliberately outlives Back. */
@OptIn(ExperimentalCoroutinesApi::class)
class RegistrationBackFlowTest {
    @Test
    fun header_back_leaves_pending_signup_and_reentry_cannot_apply_or_resend_the_late_answer() = runTest {
        val identity = PreviewIdentity(backgroundScope, { 1 })
        val email = (RegistrationEmail.parse("listener@example.test") as RegistrationResult.Accepted).value
        val accepted = identity.api.signup(email)
        val answer = CompletableDeferred<IdentityV1Result<IdentitySignupAccepted>>()
        var starts = 0
        val api = object : NativeIdentityV1Api by identity.api {
            override suspend fun signup(email: RegistrationEmail): IdentityV1Result<IdentitySignupAccepted> {
                starts += 1
                return withContext(NonCancellable) { answer.await() }
            }
        }
        val registration = RegistrationController(backgroundScope, api)
        val owners = PreviewOwners(backgroundScope, PreviewSession.Guest.seed(), previewLibraryStore(), registration = registration)
        try {
            assertTrue(owners.shell.openRegister())
            registration.editEmail("listener@example.test")
            registration.submit(); registration.submit(); runCurrent()
            assertEquals(RegistrationPhase.Submitting, registration.state.value.phase)
            assertEquals(1, starts)
            owners.shell.dispatch(ShellEvent.Back); runCurrent()
            assertEquals(TheatreRoute.Login, owners.shell.navigation.value.route)
            val expected = RegistrationPhase.Failed(RegistrationFailure.StartUnknownOutcome)
            assertEquals(expected, registration.state.value.phase)
            assertTrue(owners.shell.openRegister())
            answer.complete(accepted); runCurrent()
            assertEquals(expected, registration.state.value.phase)
            assertEquals("listener@example.test", registration.state.value.email)
            registration.submit(); runCurrent()
            assertEquals(1, starts, "Back and reentry never resend or turn a late answer into acceptance")
            assertEquals(TheatreRoute.Register, owners.shell.navigation.value.route)
            assertEquals(NativeAccountState.SignedOut, owners.account.state.value)
            assertEquals(0, owners.identity.loginRequests)
            registration.restart(); runCurrent()
            assertEquals(RegistrationPhase.Editing, registration.state.value.phase)
            assertEquals(1, starts, "recovery returns to editing without a remote effect")
        } finally {
            answer.complete(accepted)
            owners.close()
            identity.account.close()
        }
    }
}
