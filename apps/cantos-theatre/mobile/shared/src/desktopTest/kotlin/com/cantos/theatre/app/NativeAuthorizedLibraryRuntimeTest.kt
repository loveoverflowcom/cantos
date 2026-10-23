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
import com.cantos.theatre.application.identity.StubIdentityV1Api
import com.cantos.theatre.application.identity.v1Grant
import com.cantos.theatre.domain.identity.NativeGrant
import com.cantos.theatre.domain.identity.RegistrationEmail
import com.cantos.theatre.domain.identity.RegistrationResult
import com.cantos.theatre.ports.IdentityV1Result
import com.cantos.theatre.domain.playback.AudioIdentity
import com.cantos.theatre.domain.playback.PlaybackEngineCommand
import com.cantos.theatre.domain.playback.PlaybackEvent
import com.cantos.theatre.domain.playback.PlaybackObservation
import com.cantos.theatre.domain.playback.PlaybackStatus
import com.cantos.theatre.ports.CredentialFailure
import com.cantos.theatre.ports.LogoutBarrier
import com.cantos.theatre.ports.LogoutBarrierRead
import com.cantos.theatre.ports.NativeCredentialJournal
import com.cantos.theatre.ports.NativeCredentialRecord
import com.cantos.theatre.ports.NativeHttpPort
import com.cantos.theatre.ports.NativeHttpRequest
import com.cantos.theatre.ports.NativeHttpResponse
import com.cantos.theatre.ports.NativeJournalRead
import com.cantos.theatre.ports.NativeRequestHandle
import com.cantos.theatre.ports.NativeSession
import com.cantos.theatre.ports.PlaybackAccessPort
import com.cantos.theatre.ports.PlaybackAccessResult
import com.cantos.theatre.ports.PlaybackGrant
import com.cantos.theatre.ports.PlaybackLocation
import com.cantos.theatre.ports.StreamingMediaEngine
import java.nio.file.Files
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.test.advanceTimeBy
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

    private class Fixture(scope: CoroutineScope) {
        val actor = "actor-a"
        val family = "00000000-0000-4000-8000-000000000001"
        private fun pair(letter: Char, now: Long, previous: NativeGrant? = null): NativeGrant =
            v1Grant(letter, letter, actor, family, now + 120, minOf(now + 600, 1000), 1000, 100, now, previous)
        private var grant = pair('a', 100)
        val initialToken = grant.accessToken
        var now = 100L
        var sessionGate: CompletableDeferred<Unit>? = null
        var sessionChecks = 0
        var rotations = 0
        private val identity = object : StubIdentityV1Api() {
            override suspend fun login(email: RegistrationEmail, password: String): IdentityV1Result<NativeGrant> = IdentityV1Result.Accepted(grant)
            override suspend fun refresh(previous: NativeGrant): IdentityV1Result<NativeGrant> {
                assertEquals(grant.refreshToken, previous.refreshToken)
                assertEquals(family, previous.view.session)
                rotations += 1
                grant = pair('b', now, grant)
                return IdentityV1Result.Accepted(grant)
            }
            override suspend fun session(previous: NativeGrant): IdentityV1Result<NativeSession> {
                sessionChecks += 1
                sessionGate?.await()
                assertEquals(grant.accessToken, previous.accessToken)
                return IdentityV1Result.Accepted(grant.view)
            }
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
        val account = NativeAccountController(scope, identity, journal, barrier) { now }
        fun loginB() {
            grant = v1Grant('c', 'd', "actor-b", "00000000-0000-4000-8000-000000000002", now + 120, minOf(now + 600, 1000), 1000, 100, now)
            account.login(assertIs<RegistrationResult.Accepted<RegistrationEmail>>(RegistrationEmail.parse("b@example.test")).value, "synthetic-password")
        }
        class Pending(val request: NativeHttpRequest, val response: NativeHttpResponse,
                      private val completion: (NativeHttpResponse) -> Unit) {
            fun complete() = completion(response)
        }
        var nextResponse: NativeHttpResponse? = null
        var unavailable = false
        var localTransportFailure = false
        var holdNextContinue = false
        var holdNextSaved = false
        var heldRead: Pending? = null
        var heldMutation: Pending? = null
        val requests = mutableListOf<NativeHttpRequest>()
        private val receipts = mutableMapOf<String, Receipt>()
        private val origin = ApiOrigin.parse("https://api.invalid")
        private val rawHttp = object : NativeHttpPort {
            override fun send(request: NativeHttpRequest, completion: (NativeHttpResponse) -> Unit): NativeRequestHandle {
                requests += request
                nextResponse?.let {
                    nextResponse = null
                    completion(it)
                    return object : NativeRequestHandle { override fun cancel() = Unit }
                }
                if (unavailable || localTransportFailure) {
                    completion(if (localTransportFailure) NativeHttpResponse(0, null, null, "offline")
                        else NativeHttpResponse(503, "application/json", """{"code":"unavailable","retryable":true}""", null))
                    return object : NativeRequestHandle { override fun cancel() = Unit }
                }
                val route = request.url.removePrefix(origin.probeUrl)
                if (request.method == "GET" && route.startsWith("/operations/") && route.substringAfterLast('/') !in receipts) {
                    completion(NativeHttpResponse(404, "application/json", """{"code":"not_found","retryable":false}""", null, grant.view.session))
                    return object : NativeRequestHandle { override fun cancel() = Unit }
                }
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

    @Test fun preAdmissionOutageKeepsAuthorityPlayerAndDurableLibrary() = runTest {
        val f = Fixture(backgroundScope)
        try {
            runCurrent()
            f.runtime.listening.restore(f.audio, 10_000); runCurrent()
            f.start(); runCurrent(); f.acknowledgeStarted(); runCurrent()
            val player = f.runtime.listening.playback.state.value
            val authority = f.account.verifiedSession.value
            f.sessionGate = CompletableDeferred()
            f.unavailable = true
            f.library.setSaved(f.audio.workId, true); f.library.refresh(); runCurrent()
            assertEquals(authority, f.account.verifiedSession.value)
            assertEquals(player, f.runtime.listening.playback.state.value)
            assertEquals(f.actor, f.library.state.value.account)
            assertEquals(1, f.library.state.value.pending)
            assertEquals(1, f.sessionChecks, "503 is not a session revalidation")
            assertTrue(f.library.state.value.sections.values.all { it.failure?.code == "unavailable" })
        } finally { f.close() }
    }

    @Test fun exhaustedEpisodeCanStartAnotherBoundedEpisodeWithoutChangingTheFrozenIntent() = runTest {
        for (recover in listOf<(NativeSessionRuntime) -> Unit>({ it.foregrounded() }, { it.networkRestored() })) {
            val f = Fixture(backgroundScope)
            try {
                runCurrent()
                f.localTransportFailure = true
                f.library.setSaved(f.audio.workId, true); runCurrent()
                repeat(6) { advanceTimeBy(70_000); runCurrent() }
                val frozen = f.intents().single()
                val exhausted = f.requests.size
                advanceTimeBy(300_000); runCurrent()
                assertEquals(exhausted, f.requests.size, "automatic retries stop at the budget")
                recover(f.runtime); recover(f.runtime); recover(f.runtime); runCurrent()
                assertEquals(exhausted + 1, f.requests.size, "callbacks coalesce while draining")
                assertEquals(frozen.operation, f.intents().single().operation)
                assertEquals(frozen.body, f.intents().single().body)
                f.localTransportFailure = false
                advanceTimeBy(3_000); runCurrent()
                assertEquals(0, f.library.state.value.pending, "the fresh episode has its own bounded retries")
                assertTrue(f.intents().isEmpty())
                val writes = f.requests.filter { it.method == "PUT" }
                assertEquals(listOf(frozen.body), writes.map { it.body }.distinct())
                assertEquals(2, writes.size, "unknown outcome checks the original receipt before replay")
                assertEquals(1, f.sessionChecks)
            } finally { f.close() }
        }
    }

    @Test fun genuine401AndExplicitWrongStampStillQuarantineAuthorityAndPlayer() = runTest {
        for (reply in listOf(NativeHttpResponse(401, null, null, null),
            NativeHttpResponse(200, "application/json", "{}", null, "other-session"))) {
            val f = Fixture(backgroundScope)
            try {
                runCurrent()
                f.runtime.listening.restore(f.audio, 10_000); runCurrent()
                f.start(); runCurrent(); f.acknowledgeStarted(); runCurrent()
                f.sessionGate = CompletableDeferred()
                f.nextResponse = reply
                f.library.refresh(com.cantos.theatre.domain.library.LibrarySection.Continue); runCurrent()
                assertNull(f.account.verifiedSession.value)
                assertNull(f.library.state.value.account)
                assertNull(f.runtime.listening.playback.state.value.session)
                val before = f.requests.size
                f.runtime.foregrounded(); f.runtime.networkRestored(); runCurrent()
                assertEquals(before, f.requests.size, "recovery hints cannot grant authority")
            } finally { f.close() }
        }
    }

    @Test fun recoveryCallbacksShareAnInFlightDrainAndCannotAuthorizeAfterLogout() = runTest {
        val f = Fixture(backgroundScope)
        try {
            runCurrent()
            f.holdNextSaved = true
            f.library.setSaved(f.audio.workId, true); runCurrent()
            val frozen = f.intents().single()
            val held = assertNotNull(f.heldMutation)
            val before = f.requests.size
            repeat(3) { f.runtime.foregrounded(); f.runtime.networkRestored() }
            runCurrent()
            assertEquals(before, f.requests.size, "no concurrent mutation or receipt drain")
            f.account.logout(); runCurrent(); f.loginB(); runCurrent()
            val switched = f.requests.size
            held.complete()
            repeat(3) { f.runtime.foregrounded(); f.runtime.networkRestored() }
            runCurrent()
            assertEquals(switched, f.requests.size)
            assertEquals("actor-b", f.library.state.value.account)
            assertEquals(frozen.body, f.intents().single().body)
            assertTrue(f.bIntents().isEmpty())
        } finally { f.close() }
    }

    @Test fun rotationPreservesPlayingEngineAndDiscardsOldLeaseResponse() = runTest {
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

    @Test fun rotationReconcilesCommittedReceiptWithoutReplayingMutation() = runTest {
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
    @Test fun accountSwitchFencesLateLibraryReadReceiptAndDispatchWithoutReassigningData() = runTest {
        val f = Fixture(backgroundScope)
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
