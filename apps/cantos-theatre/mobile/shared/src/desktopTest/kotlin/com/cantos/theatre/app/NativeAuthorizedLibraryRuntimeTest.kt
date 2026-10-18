package com.cantos.theatre.app

import com.cantos.theatre.adapters.http.ApiOrigin
import com.cantos.theatre.adapters.http.NativeAuthorizedHttpPort
import com.cantos.theatre.adapters.library.LibraryCodec
import com.cantos.theatre.adapters.library.NativeLibraryClient
import com.cantos.theatre.adapters.library.desktopLibraryStore
import com.cantos.theatre.adapters.library.nativeLibraryController
import com.cantos.theatre.adapters.library.wire.ContinuePage
import com.cantos.theatre.adapters.library.wire.HistoryPage
import com.cantos.theatre.adapters.library.wire.HistoryStart
import com.cantos.theatre.adapters.library.wire.HistoryState
import com.cantos.theatre.adapters.library.wire.MutationState
import com.cantos.theatre.adapters.library.wire.Receipt
import com.cantos.theatre.adapters.library.wire.SavedPage
import com.cantos.theatre.adapters.library.wire.SavedRecord
import com.cantos.theatre.adapters.library.wire.SavedWrite
import com.cantos.theatre.application.identity.NativeAccountController
import com.cantos.theatre.domain.identity.LoginSecret
import com.cantos.theatre.domain.identity.NativeClientConfiguration
import com.cantos.theatre.domain.identity.NativeGrant
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

    private class Fixture(scope: CoroutineScope) {
        val actor = "actor-a"
        val family = "00000000-0000-4000-8000-000000000001"
        private val configuration = requireNotNull(NativeClientConfiguration.create("https://api.invalid",
            "https://identity.invalid/oauth2/openid/cantos-theatre-native"))
        val initialToken = "ca1_n_" + "a".repeat(64)
        private var grant = requireNotNull(NativeGrant.create(initialToken, "cr1_" + "a".repeat(64),
            actor, family, 220, 700, 1000, 100, now = 100))
        var now = 100L
        var sessionChecks = 0
        var rotations = 0
        val account = NativeAccountController(scope, configuration, object : NativeIdentityApi {
            override suspend fun begin(state: String, nonce: String, challenge: String): NativeApiResult<NativeFlow> =
                NativeApiResult.Refused(NativeApiFailure.Unavailable)
            override suspend fun exchange(flow: String, state: String, code: String, verifier: String): NativeApiResult<NativeGrant> =
                NativeApiResult.Refused(NativeApiFailure.Unavailable)
            override suspend fun refresh(refreshToken: String, session: String): NativeApiResult<NativeGrant> {
                assertEquals(grant.refreshToken, refreshToken)
                assertEquals(family, session)
                rotations += 1
                grant = requireNotNull(NativeGrant.create("ca1_n_" + "b".repeat(64), "cr1_" + "b".repeat(64),
                    actor, family, now + 120, minOf(now + 600, 1000), 1000, 100, now, previous = grant))
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
        }, object : IdentityCrypto {
            override fun secret(): LoginSecret? = null
            override fun challenge(verifier: LoginSecret): String? = null
        }, object : SystemBrowser {
            override fun open(url: String) = false
            override fun cancel() = Unit
        }, object : NativeCredentialJournal {
            private var record: NativeCredentialRecord? = NativeCredentialRecord.Active(grant)
            override fun read(): NativeJournalRead = record?.let(NativeJournalRead::Found) ?: NativeJournalRead.Missing
            override fun write(record: NativeCredentialRecord): CredentialFailure? { this.record = record; return null }
            override fun clear(): CredentialFailure? { record = null; return null }
        }, object : LogoutBarrier {
            private var pending = false
            override fun read() = if (pending) LogoutBarrierRead.Pending else LogoutBarrierRead.Clear
            override fun mark(): CredentialFailure? { pending = true; return null }
            override fun clear(): CredentialFailure? { pending = false; return null }
        }) { now }
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
                        LibraryCodec.json.encodeToString(ContinuePage(emptyList(), null))
                    request.method == "GET" && route.startsWith("/library/saved?") ->
                        LibraryCodec.json.encodeToString(SavedPage(emptyList(), null))
                    request.method == "GET" && route.startsWith("/library/history?") ->
                        LibraryCodec.json.encodeToString(HistoryPage(HistoryState(1, 0), emptyList(), null))
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
                val response = NativeHttpResponse(200, "application/json", body, null, family)
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
        fun close() { runtime.close(); folder.deleteRecursively() }
    }

    @Test
    fun slowLibraryReadAcrossRotationCannotQuarantineTheReplacementOrResetThePlayingEngine() = runTest {
        val f = Fixture(backgroundScope)
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
            assertEquals("authority_changed", f.library.state.value.failure)
            assertEquals(1, f.sessionChecks, "a stale local lease is not a server denial of the new authority")
            assertEquals(player.identity, f.runtime.listening.playback.state.value.identity)
            assertEquals(PlaybackStatus.Playing, f.runtime.listening.playback.state.value.status)
            assertEquals(authority.intent, f.account.verifiedSession.value?.intent)
            f.library.refresh(); runCurrent()
            assertEquals(null, f.library.state.value.failure)
            assertEquals(player.identity, f.runtime.listening.playback.state.value.identity)
        } finally { f.close() }
    }

    @Test
    fun lateCommittedMutationAcrossRotationReconcilesTheSameOperationReceiptWithoutReplayingIt() = runTest {
        val f = Fixture(backgroundScope)
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
}
