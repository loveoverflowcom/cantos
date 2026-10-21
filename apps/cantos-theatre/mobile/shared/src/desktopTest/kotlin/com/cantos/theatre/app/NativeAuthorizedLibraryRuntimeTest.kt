package com.cantos.theatre.app

import com.cantos.theatre.adapters.http.ApiOrigin
import com.cantos.theatre.adapters.http.NativeAuthorizedHttpPort
import com.cantos.theatre.adapters.library.LibraryCodec
import com.cantos.theatre.adapters.library.NativeLibraryClient
import com.cantos.theatre.adapters.library.desktopLibraryStore
import com.cantos.theatre.adapters.library.nativeLibraryController
import com.cantos.theatre.adapters.library.wire.PresentedContinuePage
import com.cantos.theatre.adapters.library.wire.PresentedHistoryPage
import com.cantos.theatre.adapters.library.wire.HistoryStart
import com.cantos.theatre.adapters.library.wire.HistoryState
import com.cantos.theatre.adapters.library.wire.MutationState
import com.cantos.theatre.adapters.library.wire.Receipt
import com.cantos.theatre.adapters.library.wire.PresentedSavedPage
import com.cantos.theatre.adapters.library.wire.SavedRecord
import com.cantos.theatre.adapters.library.wire.SavedWrite
import com.cantos.theatre.application.identity.NativeAccountController
import com.cantos.theatre.domain.identity.LoginSecret
import com.cantos.theatre.domain.identity.NativeClientConfiguration
import com.cantos.theatre.domain.identity.NativeGrant
import com.cantos.theatre.domain.identity.IdentityV1Validation
import com.cantos.theatre.domain.identity.NativeIdentityV1Configuration
import com.cantos.theatre.domain.identity.NativeIdentityV1ConfigurationResult
import com.cantos.theatre.domain.identity.RegistrationEmail
import com.cantos.theatre.domain.identity.RegistrationResult
import com.cantos.theatre.ports.NativeIdentityV1Api
import com.cantos.theatre.ports.IdentityV1Result
import com.cantos.theatre.ports.IdentityV1Failure
import com.cantos.theatre.ports.IdentitySignupAccepted
import com.cantos.theatre.domain.playback.AudioIdentity
import com.cantos.theatre.domain.playback.PlaybackEngineCommand
import com.cantos.theatre.domain.playback.PlaybackEvent
import com.cantos.theatre.domain.playback.PlaybackObservation
import com.cantos.theatre.domain.playback.PlaybackStatus
import com.cantos.theatre.ports.CredentialFailure
import com.cantos.theatre.ports.IdentityCrypto
import com.cantos.theatre.ports.LogoutBarrier
import com.cantos.theatre.ports.LogoutBarrierRead
import com.cantos.theatre.ports.NativeApiFailure
import com.cantos.theatre.ports.NativeApiResult
import com.cantos.theatre.ports.NativeCredentialJournal
import com.cantos.theatre.ports.NativeCredentialRecord
import com.cantos.theatre.ports.NativeFlow
import com.cantos.theatre.ports.NativeHttpPort
import com.cantos.theatre.ports.NativeHttpRequest
import com.cantos.theatre.ports.NativeHttpResponse
import com.cantos.theatre.ports.NativeIdentityApi
import com.cantos.theatre.ports.NativeJournalRead
import com.cantos.theatre.ports.NativeRequestHandle
import com.cantos.theatre.ports.NativeSession
import com.cantos.theatre.ports.PlaybackAccessPort
import com.cantos.theatre.ports.PlaybackAccessResult
import com.cantos.theatre.ports.PlaybackGrant
import com.cantos.theatre.ports.PlaybackLocation
import com.cantos.theatre.ports.StreamingMediaEngine
import com.cantos.theatre.ports.SystemBrowser
import java.nio.file.Files
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import kotlinx.serialization.encodeToString
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertNotEquals
import kotlin.test.assertNotNull
import kotlin.test.assertTrue

/** Production identity → authorized HTTP → JSON Library → SQLite → process runtime composition.
 * Identity issuer, HTTP server and decoder are controlled doubles; SQLite is disposable and real. */
@OptIn(ExperimentalCoroutinesApi::class)
class NativeAuthorizedLibraryRuntimeTest {
    private class Decoder : StreamingMediaEngine {
        private val observations = MutableSharedFlow<PlaybackObservation>(extraBufferCapacity = 64)
        override val events = observations.asSharedFlow()
        fun started(event: PlaybackEvent.EngineStarted) { check(observations.tryEmit(event)) }
        override suspend fun load(command: PlaybackEngineCommand, location: PlaybackLocation) = Unit
        override suspend fun prepare(command: PlaybackEngineCommand) = Unit
        override suspend fun start(command: PlaybackEngineCommand) = Unit
        override suspend fun pause(command: PlaybackEngineCommand) = Unit
        override suspend fun seek(command: PlaybackEngineCommand) = Unit
        override suspend fun setSpeed(command: PlaybackEngineCommand) = Unit
        override fun close() = Unit
    }

    private class Fixture(scope: CoroutineScope, val identityV1: Boolean = false) {
        val actor = "actor-a"
        val family = "00000000-0000-4000-8000-000000000001"
        private val configuration = requireNotNull(NativeClientConfiguration.create("https://api.invalid",
            "https://identity.invalid/oauth2/openid/cantos-theatre-native"))
        private fun pair(letter: Char, now: Long, previous: NativeGrant? = null): NativeGrant =
            if (identityV1) assertIs<IdentityV1Validation.Accepted<NativeGrant>>(NativeGrant.identityV1(
                "ci1_n_" + letter.toString().repeat(64), "ci1_r_" + letter.toString().repeat(64),
                actor, family, now + 120, minOf(now + 600, 1000), 1000, 100, now, previous)).value
            else requireNotNull(NativeGrant.create("ca1_n_" + letter.toString().repeat(64), "cr1_" + letter.toString().repeat(64),
                actor, family, now + 120, minOf(now + 600, 1000), 1000, 100, now, previous))
        private var grant = pair('a', 100)
        val initialToken = grant.accessToken
        var now = 100L
        var sessionChecks = 0
        var rotations = 0
        private val providerApi = object : NativeIdentityApi {
            override suspend fun begin(state: String, nonce: String, challenge: String): NativeApiResult<NativeFlow> =
                NativeApiResult.Refused(NativeApiFailure.Unavailable)
            override suspend fun exchange(flow: String, state: String, code: String, verifier: String): NativeApiResult<NativeGrant> =
                NativeApiResult.Refused(NativeApiFailure.Unavailable)
            override suspend fun refresh(refreshToken: String, session: String): NativeApiResult<NativeGrant> {
                assertEquals(grant.refreshToken, refreshToken)
                assertEquals(family, session)
                rotations += 1
                grant = pair('b', now, grant)
                return NativeApiResult.Accepted(grant)
            }
            override suspend fun session(credential: String): NativeApiResult<NativeSession> {
                sessionChecks += 1
                assertEquals(grant.accessToken, credential)
                return NativeApiResult.Accepted(grant.view)
            }
            override suspend fun revokeFamily(refreshToken: String, session: String) = NativeApiResult.Accepted(Unit)
            override suspend fun cancel(flow: String, state: String, verifier: String) = NativeApiResult.Accepted(Unit)
            override suspend fun revoke(credential: String) = NativeApiResult.Accepted(Unit)
            override suspend fun saved(credential: String, session: String) = NativeApiResult.Accepted(0)
        }
        private val crypto = object : IdentityCrypto {
            override fun secret(): LoginSecret? = null
            override fun challenge(verifier: LoginSecret): String? = null
        }
        private val browser = object : SystemBrowser {
            override fun open(url: String) = false
            override fun cancel() = Unit
        }
        private val journal = object : NativeCredentialJournal {
            private var record: NativeCredentialRecord? = NativeCredentialRecord.Active(grant)
            override fun read(): NativeJournalRead = record?.let(NativeJournalRead::Found) ?: NativeJournalRead.Missing
            override fun write(record: NativeCredentialRecord): CredentialFailure? { this.record = record; return null }
            override fun clear(): CredentialFailure? { record = null; return null }
        }
        private val barrier = object : LogoutBarrier {
            private var pending = false
            override fun read() = if (pending) LogoutBarrierRead.Pending else LogoutBarrierRead.Clear
            override fun mark(): CredentialFailure? { pending = true; return null }
            override fun clear(): CredentialFailure? { pending = false; return null }
        }
        private val firstParty = object : NativeIdentityV1Api {
            override suspend fun signup(email: RegistrationEmail): IdentityV1Result<IdentitySignupAccepted> = error("Not part of Library lifecycle")
            override suspend fun login(email: RegistrationEmail, password: String): IdentityV1Result<NativeGrant> = IdentityV1Result.Accepted(grant)
            override suspend fun refresh(previous: NativeGrant): IdentityV1Result<NativeGrant> =
                IdentityV1Result.Accepted(assertIs<NativeApiResult.Accepted<NativeGrant>>(providerApi.refresh(previous.refreshToken, previous.view.session)).value)
            override suspend fun session(previous: NativeGrant): IdentityV1Result<NativeSession> =
                IdentityV1Result.Accepted(assertIs<NativeApiResult.Accepted<NativeSession>>(providerApi.session(previous.accessToken)).value)
            override suspend fun revokeFamily(previous: NativeGrant): IdentityV1Result<Unit> = IdentityV1Result.Accepted(Unit)
            override suspend fun revoke(previous: NativeGrant): IdentityV1Result<Unit> = error("Use exact family cleanup")
        }
        val account = if (identityV1) NativeAccountController(scope,
            assertIs<NativeIdentityV1ConfigurationResult.Available>(NativeIdentityV1Configuration.resolve(configuration.apiOrigin)).configuration,
            firstParty, journal, barrier) { now }
        else NativeAccountController(scope, configuration, providerApi, crypto, browser, journal, barrier) { now }
        fun loginB() {
            grant = assertIs<IdentityV1Validation.Accepted<NativeGrant>>(NativeGrant.identityV1(
                "ci1_n_" + "c".repeat(64), "ci1_r_" + "d".repeat(64), "actor-b",
                "00000000-0000-4000-8000-000000000002", now + 120, minOf(now + 600, 1000), 1000, 100, now)).value
            account.login(assertIs<RegistrationResult.Accepted<RegistrationEmail>>(RegistrationEmail.parse("b@example.test")).value, "synthetic-password")
        }
        class Pending(val request: NativeHttpRequest, val response: NativeHttpResponse,
                      private val completion: (NativeHttpResponse) -> Unit) {
            fun complete() = completion(response)
        }
        var holdNextContinue = false
        var holdNextSaved = false
        var heldRead: Pending? = null
        var heldMutation: Pending? = null
        val requests = mutableListOf<NativeHttpRequest>()
        private val receipts = mutableMapOf<String, Receipt>()
        private val origin = ApiOrigin.parse(configuration.apiOrigin)
        private val rawHttp = object : NativeHttpPort {
            override fun send(request: NativeHttpRequest, completion: (NativeHttpResponse) -> Unit): NativeRequestHandle {
                requests += request
                val route = request.url.removePrefix(origin.probeUrl)
                val body = when {
                    request.method == "GET" && route.startsWith("/library/continue?") ->
                        LibraryCodec.json.encodeToString(PresentedContinuePage(emptyList(), null))
                    request.method == "GET" && route.startsWith("/library/saved?") ->
                        LibraryCodec.json.encodeToString(PresentedSavedPage(emptyList(), null))
                    request.method == "GET" && route.startsWith("/library/bookmarks?") ->
                        LibraryCodec.json.encodeToString(com.cantos.theatre.adapters.library.wire.BookmarkPage(emptyList(), null))
                    request.method == "GET" && route.startsWith("/library/history?") ->
                        LibraryCodec.json.encodeToString(PresentedHistoryPage(HistoryState(1, 0), emptyList(), null))
                    request.method == "GET" && route.startsWith("/operations/") ->
                        LibraryCodec.json.encodeToString(requireNotNull(receipts[route.substringAfterLast('/')]))
                    request.method == "POST" && route == "/library/history/playback-starts" -> {
                        val write = requireNotNull(LibraryCodec.decode<HistoryStart>("HistoryStart", request.body ?: ""))
                        val receipt = Receipt(write.operation.operation_id, "accepted", null, MutationState.History(HistoryState(1, 0)))
                        receipts[receipt.operation_id] = receipt
                        LibraryCodec.json.encodeToString(receipt)
                    }
                    request.method == "PUT" && route.startsWith("/library/saved/") -> {
                        val write = requireNotNull(LibraryCodec.decode<SavedWrite>("SavedWrite", request.body ?: ""))
                        val receipt = Receipt(write.operation.operation_id, "accepted", null,
                            MutationState.Saved(SavedRecord(route.substringAfterLast('/'), write.saved, 1, 1, 1000)))
                        receipts[receipt.operation_id] = receipt // Server commit precedes response delivery.
                        LibraryCodec.json.encodeToString(receipt)
                    }
                    else -> error("Unexpected fixture route")
                }
                val response = NativeHttpResponse(200, "application/json", body, null, grant.view.session)
                when {
                    holdNextContinue && request.method == "GET" && route.startsWith("/library/continue?") -> {
                        holdNextContinue = false
                        heldRead = Pending(request, response, completion)
                    }
                    holdNextSaved && request.method == "PUT" && route.startsWith("/library/saved/") -> {
                        holdNextSaved = false
                        heldMutation = Pending(request, response, completion)
                    }
                    else -> completion(response)
                }
                return object : NativeRequestHandle { override fun cancel() = Unit }
            }
        }
        private val folder = Files.createTempDirectory("cantos-authorized-library-runtime").toFile()
        private val store = desktopLibraryStore(folder.resolve("library.sqlite").absolutePath)
        private var sequence = 10
        private fun uuid() = "90000000-0000-4000-8000-${sequence++.toString().padStart(12, '0')}"
        val library = nativeLibraryController(scope,
            NativeLibraryClient(origin, NativeAuthorizedHttpPort(scope, origin, account, rawHttp)), store, ::uuid,
            onSessionInvalidated = { account.revalidate(it) },
            ownsAuthorization = { owner -> account.belongsToCurrent(owner.bearer, owner.session) &&
                account.verifiedSession.value?.session?.actor == owner.account }) { 1000 }
        val audio = requireNotNull(AudioIdentity.of("10000000-0000-4000-8000-000000000001",
            "20000000-0000-4000-8000-000000000001", "30000000-0000-4000-8000-000000000001",
            "40000000-0000-4000-8000-000000000001", "b".repeat(64), 100_000))
        val decoder = Decoder()
        val runtime = NativeSessionRuntime(scope, { decoder }, object : PlaybackAccessPort {
            override suspend fun manifest(publicationId: String) = PlaybackAccessResult.Accepted(audio)
            override suspend fun authorize(identity: AudioIdentity) = PlaybackAccessResult.Accepted(
                PlaybackGrant(PlaybackLocation("https://media.invalid/test-only"), 100_000))
        }, account, library, ::uuid, { 1000 })
        fun start() {
            runtime.listening.playback.dispatch(PlaybackEvent.PlayRequested)
        }
        fun acknowledgeStarted() = decoder.started(PlaybackEvent.EngineStarted(
            requireNotNull(runtime.listening.playback.state.value.identity)))
        fun intents() = store.transaction(actor) { intents() }
        fun bIntents() = store.transaction("actor-b") { intents() }
        fun close() { runtime.close(); folder.deleteRecursively() }
    }

    @Test
    fun slowLibraryReadAcrossRotationCannotQuarantineTheReplacementOrResetThePlayingEngine() = slowRead(false)

    @Test fun identityV1RotationPreservesPlayingEngineAndDiscardsOldLeaseResponse() = slowRead(true)

    private fun slowRead(identityV1: Boolean) = runTest {
        val f = Fixture(backgroundScope, identityV1)
        try {
            runCurrent()
            f.runtime.listening.restore(f.audio, 10_000); runCurrent()
            f.start(); runCurrent(); f.acknowledgeStarted(); runCurrent()
            assertEquals(PlaybackStatus.Playing, f.runtime.listening.playback.state.value.status)
            f.holdNextContinue = true
            f.library.refresh(); runCurrent()
            val pending = assertNotNull(f.heldRead)
            val player = f.runtime.listening.playback.state.value
            val authority = assertNotNull(f.account.verifiedSession.value)
            f.now = 221
            f.account.authorization(f.family); runCurrent()
            assertNotEquals(authority.bearer, f.account.verifiedSession.value?.bearer)
            pending.complete(); runCurrent()
            assertEquals(f.actor, f.library.state.value.account)
            assertEquals("authority_changed", f.library.state.value.section(com.cantos.theatre.domain.library.LibrarySection.Continue).failure?.code)
            assertEquals(1, f.sessionChecks, "a stale local lease is not a server denial of the new authority")
            assertEquals(player.identity, f.runtime.listening.playback.state.value.identity)
            assertEquals(PlaybackStatus.Playing, f.runtime.listening.playback.state.value.status)
            assertEquals(authority.intent, f.account.verifiedSession.value?.intent)
            f.library.refresh(); runCurrent()
            assertEquals(null, f.library.state.value.section(com.cantos.theatre.domain.library.LibrarySection.Continue).failure)
            assertEquals(player.identity, f.runtime.listening.playback.state.value.identity)
        } finally { f.close() }
    }

    @Test
    fun lateCommittedMutationAcrossRotationReconcilesTheSameOperationReceiptWithoutReplayingIt() = lateReceipt(false)

    @Test fun identityV1RotationReconcilesCommittedReceiptWithoutReplayingMutation() = lateReceipt(true)

    private fun lateReceipt(identityV1: Boolean) = runTest {
        val f = Fixture(backgroundScope, identityV1)
        try {
            runCurrent()
            f.runtime.listening.restore(f.audio, 10_000); runCurrent()
            f.start(); runCurrent(); f.acknowledgeStarted(); runCurrent()
            f.holdNextSaved = true
            f.library.setSaved(f.audio.workId, true); runCurrent()
            val pending = assertNotNull(f.heldMutation)
            val frozen = f.intents().single { it.path.startsWith("/library/saved/") }
            assertTrue(frozen.sent)
            val player = f.runtime.listening.playback.state.value
            f.now = 221
            f.account.authorization(f.family); runCurrent()
            pending.complete(); runCurrent()
            val uncertain = f.intents().single { it.operation == frozen.operation }
            assertEquals(frozen.body, uncertain.body)
            assertTrue(uncertain.sent)
            assertEquals(1, f.sessionChecks)
            assertEquals(player.identity, f.runtime.listening.playback.state.value.identity)
            assertEquals(PlaybackStatus.Playing, f.runtime.listening.playback.state.value.status)
            f.library.drain(); runCurrent()
            assertTrue(f.intents().none { it.operation == frozen.operation })
            val writes = f.requests.filter { it.method == "PUT" && it.url.endsWith("/library/saved/${f.audio.workId}") }
            assertEquals(1, writes.size, "discarding a response must not repeat the committed mutation")
            assertEquals(frozen.body, writes.single().body)
            val recovered = f.requests.single { it.method == "GET" && it.url.endsWith("/operations/${frozen.operation}") }
            assertEquals(f.family, recovered.expectedSession)
            assertEquals(f.account.verifiedSession.value?.bearer, recovered.bearer)
            assertEquals(player.identity, f.runtime.listening.playback.state.value.identity)
            assertEquals(PlaybackStatus.Playing, f.runtime.listening.playback.state.value.status)
        } finally { f.close() }
    }
    @Test fun identityV1AccountSwitchFencesLateLibraryReadReceiptAndDispatchWithoutReassigningData() = runTest {
        val f = Fixture(backgroundScope, true)
        try {
            runCurrent()
            f.holdNextContinue = true; f.library.refresh(); runCurrent()
            val oldRead = assertNotNull(f.heldRead)
            f.holdNextSaved = true; f.library.setSaved(f.audio.workId, true); runCurrent()
            val oldMutation = assertNotNull(f.heldMutation)
            val frozen = f.intents().single { it.path.startsWith("/library/saved/") }
            assertTrue(frozen.sent)
            f.account.logout()
            assertNull(f.account.verifiedSession.value)
            runCurrent(); f.loginB(); runCurrent()
            assertEquals("actor-b", f.library.state.value.account)
            val before = f.requests.size
            oldRead.complete(); oldMutation.complete(); runCurrent()
            f.library.drain(); runCurrent()
            assertEquals("actor-b", f.library.state.value.account)
            assertEquals(frozen.body, f.intents().single { it.operation == frozen.operation }.body)
            assertTrue(f.bIntents().none { it.operation == frozen.operation })
            assertFalse(f.requests.drop(before).any { it.url.endsWith("/operations/${frozen.operation}") || it.body == frozen.body })
            assertEquals(1, f.requests.count { it.method == "PUT" && it.body == frozen.body })
        } finally { f.close() }
    }

}
