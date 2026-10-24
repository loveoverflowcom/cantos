package com.cantos.theatre.app

import com.cantos.theatre.application.catalog.PublicCatalogController
import com.cantos.theatre.application.catalog.PublicCatalogIntent
import com.cantos.theatre.application.identity.EmailVerificationController
import com.cantos.theatre.application.identity.LinkReceipt
import com.cantos.theatre.application.preferences.PreferencesController
import com.cantos.theatre.domain.catalog.CatalogCursor
import com.cantos.theatre.domain.catalog.PublicCatalogQuery
import com.cantos.theatre.domain.catalog.PublicCatalogSamples
import com.cantos.theatre.domain.catalog.PublicWork
import com.cantos.theatre.domain.catalog.PublicWorkId
import com.cantos.theatre.domain.identity.CompletionStatus
import com.cantos.theatre.domain.identity.ProductionVerifyEmailHandler
import com.cantos.theatre.domain.identity.SignupCompletionRequest
import com.cantos.theatre.domain.identity.VerificationPhase
import com.cantos.theatre.domain.playback.AudioIdentity
import com.cantos.theatre.domain.playback.PlaybackEngineCommand
import com.cantos.theatre.domain.playback.PlaybackObservation
import com.cantos.theatre.ports.IdentityV1Result
import com.cantos.theatre.ports.PlaybackAccessPort
import com.cantos.theatre.ports.PlaybackAccessResult
import com.cantos.theatre.ports.PlaybackGrant
import com.cantos.theatre.ports.PlaybackLocation
import com.cantos.theatre.ports.PublicCatalogPort
import com.cantos.theatre.ports.PublicCatalogRead
import com.cantos.theatre.ports.SignupCompletionApi
import com.cantos.theatre.ports.StreamingMediaEngine
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.emptyFlow
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest

/**
 * The host-facing `receiveLink` seam that Android `VIEW` intents and both iOS SwiftUI deliveries
 * (`onContinueUserActivity` browsing-web and `onOpenURL`) call: the real shell and completion
 * controller over an explicit fake completion port. No platform host, Universal Link or device.
 */
@OptIn(ExperimentalCoroutinesApi::class)
class TheatreShellVerificationLinkTest {
    private val op = "0123456789abcdef0123456789abcdef"
    private val proof = "fedcba9876543210fedcba9876543210"
    private val link = "$ProductionVerifyEmailHandler?mode=verifyEmail&oobCode=synthetic-action&apiKey=k" +
        "&continueUrl=https%3A%2F%2Fcantos.cc%2Fverify-email%3Foperation%3D$op%26proof%3D$proof&lang=vi"
    private val password = "Ánh đèn cuối sân khấu"

    private class Completion : SignupCompletionApi {
        val requests = mutableListOf<SignupCompletionRequest>()
        var gate: CompletableDeferred<IdentityV1Result<CompletionStatus>>? = null
        override suspend fun completeSignup(request: SignupCompletionRequest): IdentityV1Result<CompletionStatus> {
            requests += request
            return gate?.await() ?: IdentityV1Result.Accepted(CompletionStatus.Completed)
        }
    }

    private object Silent : StreamingMediaEngine {
        override val events = emptyFlow<PlaybackObservation>()
        override suspend fun load(command: PlaybackEngineCommand, location: PlaybackLocation) = Unit
        override suspend fun prepare(command: PlaybackEngineCommand) = Unit
        override suspend fun start(command: PlaybackEngineCommand) = Unit
        override suspend fun pause(command: PlaybackEngineCommand) = Unit
        override suspend fun seek(command: PlaybackEngineCommand) = Unit
        override suspend fun setSpeed(command: PlaybackEngineCommand) = Unit
        override fun close() = Unit
    }

    private class Fixture(scope: CoroutineScope) {
        val work: PublicWork = PublicCatalogSamples.work(1, title = "Ánh đèn cuối sân khấu")
        val completion = Completion()
        val verification = EmailVerificationController(scope, completion, ProductionVerifyEmailHandler) { 0 }
        val catalog = PublicCatalogController(scope, object : PublicCatalogPort {
            override suspend fun works(cursor: CatalogCursor?, query: PublicCatalogQuery) =
                PublicCatalogRead.Ready(PublicCatalogSamples.worksPage(listOf(work)))
            override suspend fun work(id: PublicWorkId) = PublicCatalogRead.Ready(work)
            override suspend fun units(work: PublicWork, cursor: CatalogCursor?) =
                PublicCatalogRead.Ready(PublicCatalogSamples.unitsPage(work, emptyList()))
        }).also { it.dispatch(PublicCatalogIntent.Start) }
        val runtime = NativeSessionRuntime(scope, nativeFactory = { Silent }, access = object : PlaybackAccessPort {
            override suspend fun manifest(publicationId: String): PlaybackAccessResult<AudioIdentity> = error("no playback here")
            override suspend fun authorize(identity: AudioIdentity): PlaybackAccessResult<PlaybackGrant> = error("no playback here")
        }, account = null, library = null, newUuid = { "00000000-0000-4000-8000-000000000001" }, nowMs = { 1_000 })
        val shell = TheatreShell(scope, catalog, runtime, PreferencesController(), verification = verification)
    }

    private fun TestScope.fixture() = Fixture(backgroundScope).also { runCurrent() }

    private fun Fixture.typeAndConfirm() {
        verification.editPassword(password)
        verification.editRepeat(password)
        verification.confirm(true)
    }

    @Test
    fun one_tap_delivered_by_both_host_callbacks_opens_one_completion_and_sends_nothing() = runTest {
        val f = fixture()
        assertEquals(LinkReceipt.Shown, f.shell.receiveLink(link))
        assertEquals(LinkReceipt.Shown, f.shell.receiveLink(link))
        runCurrent()
        assertEquals(listOf(TheatreRoute.Discovery, TheatreRoute.VerifyEmail), f.shell.navigation.value.stack)
        assertEquals(VerificationPhase.AwaitingConfirmation, f.verification.state.value.phase)
        assertEquals(0, f.completion.requests.size, "a link alone never reaches the API")
    }

    @Test
    fun an_unowned_url_never_disturbs_a_password_being_typed() = runTest {
        val f = fixture()
        f.shell.receiveLink(link)
        f.typeAndConfirm()
        val before = f.verification.state.value
        // `onOpenURL` forwards any URL the system routes to the app; only the pinned handler is claimed.
        for (url in listOf(
            "https://cantos.cc/",
            "https://cantos.cc/works/1",
            "https://cantos.cc/verify-emails?mode=verifyEmail",
            "https://cantos.cc/verify-email/x?mode=verifyEmail",
            "https://cantos.cc.example/verify-email?mode=verifyEmail",
            "https://www.cantos.cc/verify-email?mode=verifyEmail",
            "http://cantos.cc/verify-email?mode=verifyEmail",
            "cantos://verify-email?mode=verifyEmail",
            "",
        )) {
            assertEquals(LinkReceipt.NotClaimed, f.shell.receiveLink(url), url)
        }
        runCurrent()
        assertTrue(before === f.verification.state.value, "the captured link, password and confirmation are untouched")
        assertTrue(f.verification.state.value.canConfirm)
        assertEquals(listOf(TheatreRoute.Discovery, TheatreRoute.VerifyEmail), f.shell.navigation.value.stack)
        assertEquals(0, f.completion.requests.size)
    }

    @Test
    fun a_link_delivered_while_completing_is_busy_and_leaves_the_pending_request() = runTest {
        val f = fixture()
        val gate = CompletableDeferred<IdentityV1Result<CompletionStatus>>().also { f.completion.gate = it }
        f.shell.receiveLink(link)
        f.typeAndConfirm()
        f.verification.complete()
        runCurrent()
        assertEquals(VerificationPhase.Completing, f.verification.state.value.phase)
        assertEquals(LinkReceipt.Busy, f.shell.receiveLink(link))
        assertEquals(LinkReceipt.Busy, f.shell.receiveLink("$ProductionVerifyEmailHandler?mode=verifyEmail"))
        assertEquals(listOf(TheatreRoute.Discovery, TheatreRoute.VerifyEmail), f.shell.navigation.value.stack)
        gate.complete(IdentityV1Result.Accepted(CompletionStatus.Completed))
        runCurrent()
        assertEquals(VerificationPhase.Completed(CompletionStatus.Completed), f.verification.state.value.phase)
        assertEquals(1, f.completion.requests.size, "the in-flight request was neither cancelled nor repeated")
    }

    @Test
    fun a_closed_shell_claims_nothing_and_opens_nothing() = runTest {
        val f = fixture()
        f.shell.close()
        assertEquals(LinkReceipt.NotClaimed, f.shell.receiveLink(link))
        runCurrent()
        assertEquals(listOf(TheatreRoute.Discovery), f.shell.navigation.value.stack)
        assertEquals(VerificationPhase.Idle, f.verification.state.value.phase)
    }
}
