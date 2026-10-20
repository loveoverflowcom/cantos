package com.cantos.theatre.app

import com.cantos.theatre.adapters.library.desktopLibraryStore
import com.cantos.theatre.application.identity.NativeAccountController
import com.cantos.theatre.adapters.library.nativeLibraryController
import com.cantos.theatre.adapters.library.presented
import com.cantos.theatre.adapters.library.wire.PresentedContinuePage
import com.cantos.theatre.adapters.library.LibraryApi
import com.cantos.theatre.adapters.library.LibraryAuthorization
import com.cantos.theatre.adapters.library.LibraryResult
import com.cantos.theatre.adapters.library.wire.ContinuePage
import com.cantos.theatre.adapters.library.wire.HistoryPage
import com.cantos.theatre.adapters.library.wire.HistoryState
import com.cantos.theatre.adapters.library.LibraryCodec
import com.cantos.theatre.adapters.library.wire.MutationState
import com.cantos.theatre.adapters.library.wire.ProgressKind
import com.cantos.theatre.adapters.library.wire.ProgressWrite
import com.cantos.theatre.adapters.library.wire.Receipt
import com.cantos.theatre.adapters.library.wire.SavedPage
import com.cantos.theatre.adapters.library.wire.SavedRecord
import com.cantos.theatre.domain.identity.LoginSecret
import com.cantos.theatre.domain.identity.NativeClientConfiguration
import com.cantos.theatre.domain.identity.NativeGrant
import com.cantos.theatre.domain.playback.AudioIdentity
import com.cantos.theatre.domain.playback.PlaybackEngineCommand
import com.cantos.theatre.domain.playback.PlaybackEvent
import com.cantos.theatre.domain.playback.PlaybackObservation
import com.cantos.theatre.domain.playback.PlaybackStatus
import com.cantos.theatre.ports.*
import java.nio.file.Files
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertNotEquals
import kotlin.test.assertTrue

/** Real process-runtime/account/library owners and SQLite; identity/network/decoder are fakes.
 * Local durable effects are asserted before/after explicit decoder acknowledgements. */
@OptIn(ExperimentalCoroutinesApi::class)
class NativeSessionRuntimeTest {
    private class Decoder : StreamingMediaEngine {
        private val observations = MutableSharedFlow<PlaybackObservation>(extraBufferCapacity = 64)
        override val events = observations.asSharedFlow()
        fun emit(value: PlaybackObservation) { check(observations.tryEmit(value)) }
        override suspend fun load(command: PlaybackEngineCommand, location: PlaybackLocation) = Unit
        override suspend fun prepare(command: PlaybackEngineCommand) = Unit
        override suspend fun start(command: PlaybackEngineCommand) = Unit
        override suspend fun pause(command: PlaybackEngineCommand) = Unit
        override suspend fun seek(command: PlaybackEngineCommand) = Unit
        override suspend fun setSpeed(command: PlaybackEngineCommand) = Unit
        override fun close() = Unit
    }

    private class LibraryApiFixture : LibraryApi {
        var continueReads = 0
        var nextReadFailure: LibraryResult.Failed? = null
        override suspend fun continuePage(auth: LibraryAuthorization, cursor: String?): LibraryResult<PresentedContinuePage> {
            continueReads += 1
            nextReadFailure?.let { nextReadFailure = null; return it }
            return LibraryResult.Value(ContinuePage(emptyList(), null).presented())
        }
        override suspend fun savedPage(auth: LibraryAuthorization, cursor: String?) = LibraryResult.Value(SavedPage(emptyList(), null).presented())
        override suspend fun historyPage(auth: LibraryAuthorization, cursor: String?) = LibraryResult.Value(HistoryPage(HistoryState(0, 0), emptyList(), null).presented())
        override suspend fun saved(auth: LibraryAuthorization, work: String) = LibraryResult.Value(SavedRecord(work, false, 0, 0, 0))
        override suspend fun progress(auth: LibraryAuthorization, publication: String) = LibraryResult.Value(MutationState.Progress(null))
        override suspend fun mutate(auth: LibraryAuthorization, intent: LocalLibraryIntent): LibraryResult<Receipt> = LibraryResult.Failed("unavailable", true, true)
        override suspend fun receipt(auth: LibraryAuthorization, operation: String): LibraryResult<Receipt> = LibraryResult.Failed("not_found", false)
    }

    private class Fixture(scope: CoroutineScope) {
        val actor = "actor-a"
        private val session = "00000000-0000-4000-8000-000000000001"
        private val bearer = "ca1_n_" + "a".repeat(64)
        private var grant = requireNotNull(NativeGrant.create(bearer, "cr1_" + "c".repeat(64), actor,
            "00000000-0000-4000-8000-000000000001", 121, 601, 901, 1, now = 1))
        var identityNow = 1L
        var rotations = 0
        var sessionGate: CompletableDeferred<Unit>? = null
        private var sequence = 10
        private fun uuid() = "90000000-0000-4000-8000-${sequence++.toString().padStart(12, '0')}"
        private val folder = Files.createTempDirectory("cantos-runtime-ack").toFile()
        val store = desktopLibraryStore(folder.resolve("library.sqlite").absolutePath)
        val audio = requireNotNull(AudioIdentity.of("10000000-0000-4000-8000-000000000001",
            "20000000-0000-4000-8000-000000000001", "30000000-0000-4000-8000-000000000001",
            "40000000-0000-4000-8000-000000000001", "b".repeat(64), 100_000))
        val decoder = Decoder()
        val account = NativeAccountController(scope,
            requireNotNull(NativeClientConfiguration.create("https://api.invalid", "https://identity.invalid/oauth2/openid/cantos-theatre-native")),
            object : NativeIdentityApi {
                override suspend fun begin(state: String, nonce: String, challenge: String): NativeApiResult<NativeFlow> = NativeApiResult.Refused(NativeApiFailure.Unavailable)
                override suspend fun exchange(flow: String, state: String, code: String, verifier: String): NativeApiResult<NativeGrant> = NativeApiResult.Refused(NativeApiFailure.Unavailable)
                override suspend fun refresh(refreshToken: String, session: String): NativeApiResult<NativeGrant> {
                    assertEquals(grant.refreshToken, refreshToken)
                    assertEquals(grant.view.session, session)
                    rotations += 1
                    grant = requireNotNull(NativeGrant.create("ca1_n_" + "d".repeat(64), "cr1_" + "e".repeat(64), actor,
                        session, identityNow + 120, minOf(identityNow + 600, 901), 901, 1, identityNow, previous = grant))
                    return NativeApiResult.Accepted(grant)
                }
                override suspend fun revokeFamily(refreshToken: String, session: String) = NativeApiResult.Accepted(Unit)
                override suspend fun cancel(flow: String, state: String, verifier: String) = NativeApiResult.Accepted(Unit)
                override suspend fun session(credential: String): NativeApiResult<NativeSession> {
                    assertEquals(grant.accessToken, credential)
                    sessionGate?.await()
                    return NativeApiResult.Accepted(grant.view)
                }
                override suspend fun revoke(credential: String) = NativeApiResult.Accepted(Unit)
                override suspend fun saved(credential: String, session: String): NativeApiResult<Int> = error("runtime must not use saved page count")
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
                override fun read() = LogoutBarrierRead.Clear
                override fun mark(): CredentialFailure? = null
                override fun clear(): CredentialFailure? = null
            }, now = { identityNow })
        val libraryApi = LibraryApiFixture()
        val library = nativeLibraryController(scope, libraryApi, store, ::uuid,
            onSessionInvalidated = { expected -> account.revalidate(expected) },
            ownsAuthorization = { owner -> account.belongsToCurrent(owner.bearer, owner.session) &&
                account.verifiedSession.value?.session?.actor == owner.account }) { 1_000 }
        val runtime = NativeSessionRuntime(scope, { decoder }, object : PlaybackAccessPort {
            override suspend fun manifest(publicationId: String) = PlaybackAccessResult.Accepted(audio)
            override suspend fun authorize(identity: AudioIdentity) = PlaybackAccessResult.Accepted(PlaybackGrant(PlaybackLocation("https://media.invalid/test-only"), 100_000))
        }, account, library, ::uuid, { 1_000 })
        fun intents() = store.transaction(actor) { intents() }
        fun historyCount() = intents().count { it.path == "/library/history/playback-starts" }
        fun progressCount() = intents().count { it.path.startsWith("/progress/") }
        fun started() { decoder.emit(PlaybackEvent.EngineStarted(requireNotNull(runtime.listening.playback.state.value.identity))) }
        fun close() { runtime.close(); folder.deleteRecursively() }
    }

    @Test
    fun revalidationQuarantinesTheEngineAndPrivateLibraryUntilApiConfirmationThenRestoresPaused() = runTest {
        val f = Fixture(backgroundScope)
        try {
            runCurrent()
            f.runtime.listening.restore(f.audio, 10_000); runCurrent()
            f.library.progressCheckpoint(f.audio, 10_000, force = true); runCurrent()
            val player = f.runtime.listening.playback.state.value
            val operations = f.intents().map { it.operation }
            val confirmation = CompletableDeferred<Unit>().also { f.sessionGate = it }
            f.libraryApi.nextReadFailure = LibraryResult.Failed("session_changed", false)
            f.library.refresh(); runCurrent()
            assertEquals(null, f.account.verifiedSession.value)
            assertEquals(null, f.library.state.value.account)
            assertEquals(null, f.runtime.listening.playback.state.value.session)
            f.library.setSaved(f.audio.workId, true)
            f.library.progressCheckpoint(f.audio, 30_000, force = true)
            assertEquals(operations, f.intents().map { it.operation }, "quarantine cannot authorize local edits")
            confirmation.complete(Unit); runCurrent()
            assertEquals(f.actor, f.library.state.value.account)
            assertNotEquals(player.identity, f.runtime.listening.playback.state.value.identity)
            assertEquals(10_000L, f.runtime.listening.playback.state.value.session?.positionMs)
            assertEquals(PlaybackStatus.Paused, f.runtime.listening.playback.state.value.status)
            assertEquals(operations, f.intents().map { it.operation }, "confirmation cannot enqueue playback or progress")
            assertEquals(0, f.historyCount(), "confirmation never starts the decoder")
        } finally { f.close() }
    }

    @Test
    fun immediateRevalidationClearsThePreviousEngineAndOnlyRestoresAnExistingCheckpointPaused() = runTest {
        for (persistCheckpoint in listOf(false, true)) {
            val f = Fixture(backgroundScope)
            try {
                runCurrent()
                f.runtime.listening.restore(f.audio, 10_000); runCurrent()
                f.runtime.listening.playback.dispatch(PlaybackEvent.PlayRequested); runCurrent()
                f.started(); runCurrent()
                if (persistCheckpoint) f.library.progressCheckpoint(f.audio, 10_000, force = true)
                runCurrent()
                val player = f.runtime.listening.playback.state.value
                val operations = f.intents().map { it.operation }
                val family = requireNotNull(f.account.verifiedSession.value).session.session
                // This fake API returns immediately; correctness cannot depend on a collector
                // observing each transient null between quarantine and confirmation.
                f.account.revalidate(family); runCurrent()
                assertEquals(f.actor, f.library.state.value.account)
                assertNotEquals(player.identity, f.runtime.listening.playback.state.value.identity)
                if (persistCheckpoint) {
                    assertEquals(10_000L, f.runtime.listening.playback.state.value.session?.positionMs)
                    assertEquals(PlaybackStatus.Paused, f.runtime.listening.playback.state.value.status)
                } else {
                    assertEquals(null, f.runtime.listening.playback.state.value.session)
                }
                assertEquals(operations, f.intents().map { it.operation }, "confirmation has no playback side effects")
            } finally { f.close() }
        }
    }

    @Test
    fun sameFamilyRotationPreservesPlayerIdentityPausePositionAndDurableOutbox() = runTest {
        val f = Fixture(backgroundScope)
        try {
            runCurrent()
            f.runtime.listening.restore(f.audio, 10_000); runCurrent()
            f.library.setSaved(f.audio.workId, true); runCurrent()
            val identity = f.runtime.listening.playback.state.value.identity
            val playback = f.runtime.listening.playback.state.value.session
            val operations = f.intents().map { it.operation }
            val reads = f.libraryApi.continueReads
            f.identityNow = 122
            f.account.authorization()
            runCurrent()
            assertEquals(1, f.rotations)
            assertEquals(identity, f.runtime.listening.playback.state.value.identity)
            assertEquals(playback, f.runtime.listening.playback.state.value.session)
            assertEquals(PlaybackStatus.Paused, f.runtime.listening.playback.state.value.status)
            assertEquals(operations, f.intents().map { it.operation })
            assertEquals(reads, f.libraryApi.continueReads, "rotation never leaves/reenters the Library")
            assertEquals(f.actor, f.library.state.value.account)
        } finally { f.close() }
    }

    @Test
    fun logoutFencesSynchronousEngineCallbacksAndClearsTheRuntimeBeforeAnyAccountReuse() = runTest {
        val f = Fixture(backgroundScope)
        try {
            runCurrent()
            f.runtime.listening.restore(f.audio, 10_000); runCurrent()
            f.runtime.listening.playback.dispatch(PlaybackEvent.PlayRequested); runCurrent()
            f.started(); runCurrent()
            val before = f.intents().map { it.operation }
            val identity = requireNotNull(f.runtime.listening.playback.state.value.identity)
            f.account.logout()
            f.library.setSaved(f.audio.workId, true)
            f.library.progressCheckpoint(f.audio, 30_000, force = true)
            assertEquals(before, f.intents().map { it.operation }, "logout fences local writes before the collector runs")
            f.decoder.emit(PlaybackEvent.EnginePosition(identity, 30_000))
            f.runtime.backgrounded()
            runCurrent()
            assertEquals(before, f.intents().map { it.operation })
            assertEquals(null, f.library.state.value.account)
            assertEquals(null, f.runtime.listening.playback.state.value.session)
        } finally { f.close() }
    }

    @Test
    fun sameSourceColdRestoreCannotRecordHistoryOrProgressBeforeNewDecoderStarted() = runTest {
        val f = Fixture(backgroundScope)
        try {
            runCurrent(); assertEquals(f.actor, f.library.state.value.account)
            f.runtime.listening.restore(f.audio, 10_000); runCurrent()
            f.runtime.listening.playback.dispatch(PlaybackEvent.PlayRequested); runCurrent()
            assertTrue(f.intents().isEmpty())
            f.started(); runCurrent()
            f.decoder.emit(PlaybackEvent.EnginePosition(requireNotNull(f.runtime.listening.playback.state.value.identity), 20_000)); runCurrent()
            val histories = f.historyCount(); val checkpoints = f.progressCount()
            assertEquals(1, histories); assertEquals(1, checkpoints)
            val source = f.runtime.listening.playback.state.value.session?.source
            f.runtime.listening.restore(f.audio, 5_000); runCurrent()
            assertEquals(source, f.runtime.listening.playback.state.value.session?.source)
            assertEquals(PlaybackStatus.Paused, f.runtime.listening.playback.state.value.status)
            f.runtime.backgrounded(); runCurrent()
            assertEquals(histories, f.historyCount()); assertEquals(checkpoints, f.progressCount())
            f.runtime.listening.playback.dispatch(PlaybackEvent.PlayRequested); runCurrent()
            f.runtime.backgrounded(); runCurrent()
            assertEquals(histories, f.historyCount()); assertEquals(checkpoints, f.progressCount())
            f.started(); runCurrent()
            assertEquals(histories + 1, f.historyCount())
            f.runtime.backgrounded(); runCurrent()
            assertEquals(checkpoints + 1, f.progressCount())
            val last = assertNotNull(LibraryCodec.decode<ProgressWrite>("ProgressWrite", f.intents().last { it.path.startsWith("/progress/") }.body))
            assertEquals(5_000L, last.position_ms)
        } finally { f.close() }
    }

    @Test
    fun endedRestartRecordsOneExplicitRestartAtZeroOnlyAfterDecoderStarted() = runTest {
        val f = Fixture(backgroundScope)
        try {
            runCurrent(); f.runtime.listening.restore(f.audio, 10_000); runCurrent()
            f.runtime.listening.playback.dispatch(PlaybackEvent.PlayRequested); runCurrent(); f.started(); runCurrent()
            f.decoder.emit(PlaybackEvent.EngineEnded(requireNotNull(f.runtime.listening.playback.state.value.identity))); runCurrent()
            val histories = f.historyCount(); val checkpoints = f.progressCount()
            assertEquals(1, histories); assertEquals(1, checkpoints)
            f.runtime.listening.playback.dispatch(PlaybackEvent.PlayRequested); runCurrent()
            assertEquals(PlaybackStatus.Buffering, f.runtime.listening.playback.state.value.status)
            f.runtime.backgrounded(); runCurrent()
            assertEquals(histories, f.historyCount()); assertEquals(checkpoints, f.progressCount())
            f.started(); runCurrent()
            assertEquals(histories + 1, f.historyCount()); assertEquals(checkpoints + 1, f.progressCount())
            val restart = assertNotNull(LibraryCodec.decode<ProgressWrite>("ProgressWrite", f.intents().last { it.path.startsWith("/progress/") }.body))
            assertEquals(ProgressKind.restart, restart.kind); assertEquals(0L, restart.position_ms)
            f.started(); runCurrent()
            assertEquals(histories + 1, f.historyCount()); assertEquals(checkpoints + 1, f.progressCount())
        } finally { f.close() }
    }
}
