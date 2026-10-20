package com.cantos.theatre.application.identity

import com.cantos.theatre.domain.identity.RegistrationAction
import com.cantos.theatre.domain.identity.RegistrationEmail
import com.cantos.theatre.domain.identity.RegistrationFailure
import com.cantos.theatre.domain.identity.RegistrationPhase
import com.cantos.theatre.domain.identity.RegistrationSetup
import com.cantos.theatre.ports.RegistrationBrowser
import com.cantos.theatre.ports.RegistrationCapability
import com.cantos.theatre.ports.RegistrationPort
import com.cantos.theatre.domain.identity.RegistrationResult
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.withContext
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs

@OptIn(kotlinx.coroutines.ExperimentalCoroutinesApi::class)
class RegistrationControllerTest {
    private val link = "https://cantos.example/?mode=verifyEmail&oobCode=private-code&continueUrl=https%3A%2F%2Fcantos.example%2F%3FfirebaseEnrollment%3D00000000-0000-4000-8000-000000000001"
    private class MockRegistration : RegistrationPort, RegistrationBrowser {
        var starts = 0
        var confirms = 0
        var cancels = 0
        var opens = 0
        var openResult = true
        var available = true
        var startResult: RegistrationResult<Unit> = RegistrationResult.Accepted(Unit)
        var held: CompletableDeferred<RegistrationResult<RegistrationSetup>>? = null
        override suspend fun configuration() = RegistrationResult.Accepted(if (available) RegistrationCapability.Available("https://cantos.example") else RegistrationCapability.Disabled)
        override suspend fun start(email: RegistrationEmail): RegistrationResult<Unit> { starts += 1; return startResult }
        override suspend fun confirm(action: RegistrationAction): RegistrationResult<RegistrationSetup> {
            confirms += 1
            return held?.let { withContext(NonCancellable) { it.await() } }
                ?: RegistrationSetup.parse("listener_1", "https://idm.example/ui/reset?token=private-setup", "https://idm.example")
        }
        override suspend fun cancel(action: RegistrationAction): RegistrationResult<Unit> { cancels += 1; return RegistrationResult.Accepted(Unit) }
        override fun openSetup(url: String, completion: (Boolean) -> Unit) { opens += 1; completion(openResult) }
    }

    @Test fun accepted_email_and_setup_never_change_account_authority_and_effects_are_one_shot() = runTest {
        val port = MockRegistration()
        val owner = RegistrationController(this, port, port)
        owner.enter(); runCurrent()
        owner.editEmail("Linh@example.test")
        owner.submit(); owner.submit(); runCurrent()
        assertEquals(1, port.starts)
        assertEquals(RegistrationPhase.Waiting, owner.state.value.phase)
        owner.importAction(link)
        assertEquals(0, port.confirms)
        owner.confirm(); owner.confirm(); runCurrent()
        assertEquals(1, port.confirms)
        assertIs<RegistrationPhase.SetupReady>(owner.state.value.phase)
        owner.openSetup(); owner.openSetup(); runCurrent()
        assertEquals(1, port.opens)
        assertEquals(RegistrationPhase.CredentialSetup, owner.state.value.phase)
        owner.close()
    }

    @Test fun back_during_confirmation_fences_late_setup_and_preserves_email_without_replay() = runTest {
        val port = MockRegistration()
        val completion = CompletableDeferred<RegistrationResult<RegistrationSetup>>()
        port.held = completion
        val owner = RegistrationController(this, port, port)
        owner.enter(); runCurrent(); owner.editEmail("a@example.test"); owner.submit(); runCurrent()
        owner.importAction(link); owner.confirm(); runCurrent()
        owner.leave()
        completion.complete(RegistrationSetup.parse("listener_1", "https://idm.example/ui/reset?token=late-secret", "https://idm.example"))
        runCurrent()
        assertEquals("a@example.test", owner.state.value.email)
        assertEquals(RegistrationPhase.Failed(RegistrationFailure.UnknownOutcome), owner.state.value.phase)
        owner.importAction(link); owner.confirm(); owner.openSetup(); runCurrent()
        assertEquals(1, port.confirms)
        assertEquals(0, port.opens)
        owner.close()
    }

    @Test fun unknown_submit_can_check_mail_without_automatically_resending_and_cancel_consumes_only_an_imported_action() = runTest {
        val port = MockRegistration().also { it.startResult = RegistrationResult.Refused(RegistrationFailure.UnknownOutcome) }
        val owner = RegistrationController(this, port, port)
        owner.enter(); runCurrent(); owner.editEmail("a@example.test"); owner.submit(); runCurrent()
        assertEquals(RegistrationPhase.Failed(RegistrationFailure.StartUnknownOutcome), owner.state.value.phase)
        owner.submit(); runCurrent()
        assertEquals(1, port.starts)
        owner.importAction(link); owner.cancel(); runCurrent(); owner.cancel(); runCurrent()
        // The second Cancel cannot produce a second action consumption.
        assertEquals(0, port.confirms)
        assertEquals(0, port.opens)
        assertEquals(1, port.cancels)
        owner.close()
    }

    @Test fun disabled_or_unreachable_capability_is_retryable_and_invalid_input_never_dispatches() = runTest {
        val port = MockRegistration().also { it.available = false }
        val owner = RegistrationController(this, port, port)
        owner.enter(); runCurrent()
        assertEquals(RegistrationPhase.Failed(RegistrationFailure.Disabled), owner.state.value.phase)
        port.available = true; owner.restart(); runCurrent()
        owner.editEmail("invalid"); owner.submit(); runCurrent()
        assertEquals(RegistrationPhase.Failed(RegistrationFailure.InvalidEmail), owner.state.value.phase)
        assertEquals(0, port.starts)
        owner.editEmail("a@example.test"); owner.submit(); runCurrent()
        owner.importAction("https://evil.test/private")
        assertEquals(RegistrationPhase.Failed(RegistrationFailure.InvalidAction), owner.state.value.phase)
        assertEquals(0, port.confirms)
        owner.close()
    }

    @Test fun a_failed_browser_open_cannot_replay_the_one_shot_setup_capability() = runTest {
        val port = MockRegistration().also { it.openResult = false }
        val owner = RegistrationController(this, port, port)
        owner.enter(); runCurrent(); owner.editEmail("a@example.test"); owner.submit(); runCurrent()
        owner.importAction(link); owner.confirm(); runCurrent()
        owner.openSetup(); runCurrent()
        assertEquals(RegistrationPhase.Failed(RegistrationFailure.Browser), owner.state.value.phase)
        owner.openSetup(); runCurrent()
        assertEquals(1, port.opens)
        assertEquals(1, port.confirms)
        owner.close()
    }
}
