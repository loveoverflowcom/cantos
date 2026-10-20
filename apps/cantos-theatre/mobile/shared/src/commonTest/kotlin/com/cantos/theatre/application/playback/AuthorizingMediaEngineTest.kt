package com.cantos.theatre.application.playback

import com.cantos.theatre.domain.playback.AudioIdentity
import com.cantos.theatre.domain.playback.PlaybackEngineCommand
import com.cantos.theatre.domain.playback.PlaybackEngine
import com.cantos.theatre.domain.playback.PlaybackEvent
import com.cantos.theatre.domain.playback.PlaybackFailure
import com.cantos.theatre.domain.playback.PlaybackIdentity
import com.cantos.theatre.domain.playback.PlaybackIntent
import com.cantos.theatre.domain.playback.PlaybackInterruption
import com.cantos.theatre.domain.playback.PlaybackObservation
import com.cantos.theatre.domain.playback.PlaybackPauseCause
import com.cantos.theatre.domain.playback.PlaybackSession
import com.cantos.theatre.domain.playback.PlaybackSource
import com.cantos.theatre.domain.playback.PlaybackState
import com.cantos.theatre.domain.playback.PlaybackStatus
import com.cantos.theatre.ports.PlaybackAccessFailure
import com.cantos.theatre.ports.PlaybackAccessPort
import com.cantos.theatre.ports.PlaybackAccessResult
import com.cantos.theatre.ports.PlaybackGrant
import com.cantos.theatre.ports.PlaybackLocation
import com.cantos.theatre.ports.StreamingMediaEngine
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.withContext
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue
import kotlin.coroutines.resume
import kotlin.coroutines.suspendCoroutine

@OptIn(kotlinx.coroutines.ExperimentalCoroutinesApi::class)
class AuthorizingMediaEngineTest {
    private val audio = requireNotNull(AudioIdentity.of("10000000-0000-0000-0000-000000000001",
        "20000000-0000-0000-0000-000000000002", "30000000-0000-0000-0000-000000000003",
        "40000000-0000-0000-0000-000000000004", "b".repeat(64), 60_000))
    private val source = PlaybackSource.streaming(audio)
    private val sentinel = "MUST_NOT_PRINT_signed_media_credential"
    private fun grant(expiresAt: Long = 1_000_000) = PlaybackAccessResult.Accepted(
        PlaybackGrant(PlaybackLocation("https://media.invalid/audio.opus?credential=$sentinel"), expiresAt))
    private fun command(generation: Long, position: Long = 1200, speed: Int = 1000) = PlaybackEngineCommand(
        requireNotNull(PlaybackIdentity.of(audio.workId, 0, generation)), source, position, speed)

    @Test
    fun cold_prepare_only_stops_old_audio_without_authorization_load_or_start() = runTest {
        val native = MockStreamingMediaEngine()
        val access = MockPlaybackAccessPort { grant() }
        val engine = AuthorizingMediaEngine(this, native, access) { testScheduler.currentTime }
        try {
            engine.prepare(command(1, 42_000, 1250))
            runCurrent()
            assertEquals(emptyList(), access.authorizations)
            assertEquals(listOf(command(1, 42_000, 1250)), native.pauses)
            assertEquals(emptyList(), native.loads)
            assertEquals(emptyList(), native.starts)
        } finally { engine.close() }
    }

    @Test
    fun pause_fences_an_uncancellable_late_authorization_without_starting() = runTest {
        val gate = CompletableDeferred<PlaybackAccessResult<PlaybackGrant>>()
        val native = MockStreamingMediaEngine()
        val access = MockPlaybackAccessPort { withContext(NonCancellable) { gate.await() } }
        val engine = AuthorizingMediaEngine(this, native, access) { testScheduler.currentTime }
        try {
            engine.start(command(1))
            runCurrent()
            assertEquals(listOf(audio), access.authorizations)
            engine.pause(command(2))
            gate.complete(grant())
            runCurrent()
            assertEquals(listOf(command(2)), native.pauses)
            assertEquals(emptyList(), native.loads)
            assertEquals(emptyList(), native.starts)
        } finally { engine.close(); gate.complete(grant()) }
    }

    @Test
    fun close_fences_an_uncancellable_late_authorization_and_closes_the_native_owner() = runTest {
        val gate = CompletableDeferred<PlaybackAccessResult<PlaybackGrant>>()
        val native = MockStreamingMediaEngine()
        val access = MockPlaybackAccessPort { withContext(NonCancellable) { gate.await() } }
        val engine = AuthorizingMediaEngine(this, native, access) { testScheduler.currentTime }
        try {
            engine.start(command(1))
            runCurrent()
            engine.close()
            gate.complete(grant())
            runCurrent()
            assertTrue(native.closed)
            assertEquals(emptyList(), native.loads)
            assertEquals(emptyList(), native.starts)
        } finally { engine.close(); gate.complete(grant()) }
    }

    @Test
    fun clearing_the_controller_fences_late_authorization_and_keeps_no_session() = runTest {
        val gate = CompletableDeferred<PlaybackAccessResult<PlaybackGrant>>()
        val native = MockStreamingMediaEngine()
        val access = MockPlaybackAccessPort { withContext(NonCancellable) { gate.await() } }
        val engine = AuthorizingMediaEngine(this, native, access) { testScheduler.currentTime }
        val controller = PlaybackController(this, engine)
        try {
            controller.dispatch(PlaybackEvent.SourceSelected(source, 1200))
            controller.dispatch(PlaybackEvent.PlayRequested)
            runCurrent()
            assertEquals(listOf(audio), access.authorizations)
            controller.dispatch(PlaybackEvent.SessionCleared)
            runCurrent()
            gate.complete(grant())
            runCurrent()
            assertEquals(PlaybackState(3), controller.state.value)
            assertEquals(listOf(command(1), command(3).copy(clearSession = true)), native.pauses)
            assertEquals(emptyList(), native.loads)
            assertEquals(emptyList(), native.starts)
        } finally { controller.close(); gate.complete(grant()) }
    }

    @Test
    fun latest_seek_and_speed_supersede_a_slow_old_grant_without_replacing_the_release() = runTest {
        val first = CompletableDeferred<PlaybackAccessResult<PlaybackGrant>>()
        var call = 0
        val native = MockStreamingMediaEngine()
        val access = MockPlaybackAccessPort {
            call += 1
            if (call == 1) withContext(NonCancellable) { first.await() } else grant()
        }
        val engine = AuthorizingMediaEngine(this, native, access) { testScheduler.currentTime }
        try {
            engine.start(command(1))
            runCurrent()
            engine.seek(command(2, 32_000))
            engine.setSpeed(command(2, 32_000, 1500))
            runCurrent()
            first.complete(grant())
            runCurrent()
            assertEquals(listOf(audio, audio), access.authorizations)
            assertEquals(listOf(command(2, 32_000, 1500)), native.loads)
            assertEquals(listOf(command(2, 32_000, 1500)), native.starts)
        } finally { engine.close(); first.complete(grant()) }
    }

    @Test
    fun network_and_expiry_retry_at_most_twice_with_the_same_identity_and_last_engine_position() = runTest {
        val native = MockStreamingMediaEngine()
        val access = MockPlaybackAccessPort { grant() }
        val engine = AuthorizingMediaEngine(this, native, access) { testScheduler.currentTime }
        val observed = mutableListOf<PlaybackObservation>()
        backgroundScope.launch { engine.events.collect { observed += it } }
        try {
            runCurrent()
            engine.start(command(1))
            runCurrent()
            assertEquals(listOf(command(1)), native.starts)
            assertEquals(listOf<PlaybackObservation>(PlaybackEvent.EngineBuffering(command(1).identity)), observed)
            native.events.emit(PlaybackEvent.EnginePosition(command(1).identity, 8750))
            native.events.emit(PlaybackEvent.EngineFailed(command(1).identity, PlaybackFailure.Network))
            runCurrent()
            assertEquals(1, native.loads.size)
            assertEquals(listOf<PlaybackObservation>(PlaybackEvent.EngineBuffering(command(1).identity),
                PlaybackEvent.EnginePosition(command(1).identity, 8750),
                PlaybackEvent.EngineBuffering(command(1).identity)), observed)
            advanceTimeBy(500)
            runCurrent()
            assertEquals(listOf(command(1), command(1, 8750)), native.starts)
            native.events.emit(PlaybackEvent.EnginePosition(command(1).identity, 8800))
            native.events.emit(PlaybackEvent.EngineFailed(command(1).identity, PlaybackFailure.Expired))
            runCurrent()
            advanceTimeBy(1000)
            runCurrent()
            assertEquals(listOf(command(1), command(1, 8750), command(1, 8800)), native.starts)
            native.events.emit(PlaybackEvent.EngineFailed(command(1).identity, PlaybackFailure.Network))
            runCurrent()
            advanceTimeBy(5000)
            runCurrent()
            assertEquals(listOf(audio, audio, audio), access.authorizations)
            assertEquals(3, native.loads.size)
            assertEquals(listOf<PlaybackObservation>(PlaybackEvent.EngineBuffering(command(1).identity),
                PlaybackEvent.EnginePosition(command(1).identity, 8750),
                PlaybackEvent.EngineBuffering(command(1).identity),
                PlaybackEvent.EngineBuffering(command(1).identity),
                PlaybackEvent.EnginePosition(command(1).identity, 8800),
                PlaybackEvent.EngineBuffering(command(1).identity),
                PlaybackEvent.EngineBuffering(command(1).identity),
                PlaybackEvent.EngineFailed(command(1).identity, PlaybackFailure.Network)), observed)
        } finally { engine.close() }
    }

    @Test
    fun refused_network_grants_are_bounded_and_never_reach_the_decoder() = runTest {
        val native = MockStreamingMediaEngine()
        val access = MockPlaybackAccessPort { PlaybackAccessResult.Refused(PlaybackAccessFailure.Network) }
        val engine = AuthorizingMediaEngine(this, native, access) { testScheduler.currentTime }
        val observed = mutableListOf<PlaybackObservation>()
        backgroundScope.launch { engine.events.collect { observed += it } }
        try {
            runCurrent()
            engine.start(command(1))
            runCurrent()
            advanceTimeBy(500)
            runCurrent()
            advanceTimeBy(1000)
            runCurrent()
            advanceTimeBy(5000)
            runCurrent()
            assertEquals(listOf(audio, audio, audio), access.authorizations)
            assertEquals(emptyList(), native.loads)
            assertEquals(emptyList(), native.starts)
            assertEquals(listOf<PlaybackObservation>(PlaybackEvent.EngineFailed(command(1).identity, PlaybackFailure.Network)), observed)
        } finally { engine.close() }
    }

    @Test
    fun already_expired_accepted_grants_exhaust_a_bounded_refresh_without_loading_or_playing() = runTest {
        val native = MockStreamingMediaEngine()
        val access = MockPlaybackAccessPort { grant(expiresAt = -1) }
        val engine = AuthorizingMediaEngine(this, native, access) { testScheduler.currentTime }
        val observed = mutableListOf<PlaybackObservation>()
        backgroundScope.launch { engine.events.collect { observed += it } }
        try {
            runCurrent()
            engine.start(command(1))
            runCurrent()
            assertEquals(emptyList(), native.loads)
            advanceTimeBy(500)
            runCurrent()
            assertEquals(emptyList(), native.loads)
            advanceTimeBy(1000)
            runCurrent()
            advanceTimeBy(5000)
            runCurrent()
            assertEquals(listOf(audio, audio, audio), access.authorizations)
            assertEquals(emptyList(), native.loads)
            assertEquals(emptyList(), native.starts)
            assertEquals(listOf<PlaybackObservation>(PlaybackEvent.EngineFailed(command(1).identity, PlaybackFailure.Expired)), observed)
        } finally { engine.close() }
    }

    @Test
    fun final_absent_or_withdrawn_refresh_stops_the_loaded_decoder_without_selecting_another_release() = runTest {
        var call = 0
        val native = MockStreamingMediaEngine()
        val access = MockPlaybackAccessPort {
            call += 1
            if (call == 1) grant(expiresAt = 16_000) else PlaybackAccessResult.Refused(PlaybackAccessFailure.NotFound)
        }
        val engine = AuthorizingMediaEngine(this, native, access) { testScheduler.currentTime }
        val observed = mutableListOf<PlaybackObservation>()
        backgroundScope.launch { engine.events.collect { observed += it } }
        try {
            runCurrent()
            engine.start(command(1))
            runCurrent()
            native.events.emit(PlaybackEvent.EnginePosition(command(1).identity, 8750))
            runCurrent()
            advanceTimeBy(1000)
            runCurrent()
            assertEquals(listOf(audio, audio), access.authorizations)
            assertEquals(listOf(command(1)), native.loads)
            assertEquals(listOf(command(1)), native.starts)
            assertEquals(listOf(command(1, 8750)), native.pauses)
            assertEquals(listOf<PlaybackObservation>(PlaybackEvent.EngineBuffering(command(1).identity),
                PlaybackEvent.EnginePosition(command(1).identity, 8750),
                PlaybackEvent.EngineFailed(command(1).identity, PlaybackFailure.Unavailable)), observed)
            advanceTimeBy(5000)
            runCurrent()
            assertEquals(2, access.authorizations.size)
        } finally { engine.close() }
    }

    @Test
    fun clear_hides_late_observations_and_the_same_owner_can_play_a_new_session() = runTest {
        val native = MockStreamingMediaEngine()
        val access = MockPlaybackAccessPort { grant() }
        val engine = AuthorizingMediaEngine(this, native, access) { testScheduler.currentTime }
        val observed = mutableListOf<PlaybackObservation>()
        backgroundScope.launch { engine.events.collect { observed += it } }
        try {
            runCurrent()
            engine.start(command(1))
            runCurrent()
            engine.pause(command(2).copy(clearSession = true))
            native.events.emit(PlaybackEvent.EngineStarted(command(2).identity))
            native.events.emit(PlaybackEvent.EnginePosition(command(2).identity, 42_000))
            runCurrent()
            assertEquals(listOf<PlaybackObservation>(PlaybackEvent.EngineBuffering(command(1).identity)), observed)
            assertEquals(listOf(command(2).copy(clearSession = true)), native.pauses)
            assertFalse(native.closed)
            engine.start(command(3, 3000))
            runCurrent()
            assertEquals(listOf(command(1), command(3, 3000)), native.loads)
            assertEquals(listOf(command(1), command(3, 3000)), native.starts)
            assertEquals(listOf(audio, audio), access.authorizations)
            assertEquals(listOf<PlaybackObservation>(PlaybackEvent.EngineBuffering(command(1).identity),
                PlaybackEvent.EngineBuffering(command(3).identity)), observed)
        } finally { engine.close() }
    }

    @Test
    fun continue_restore_fences_an_uncancellable_old_manifest_and_keeps_the_new_source_paused() = runTest {
        val other = requireNotNull(AudioIdentity.of("50000000-0000-0000-0000-000000000005",
            "60000000-0000-0000-0000-000000000006", "70000000-0000-0000-0000-000000000007",
            "80000000-0000-0000-0000-000000000008", "c".repeat(64), 60_000))
        val gate = CompletableDeferred<PlaybackAccessResult<AudioIdentity>>()
        val native = MockStreamingMediaEngine()
        val access = MockPlaybackAccessPort { grant() }
        var manifestReads = 0
        access.onManifest = { publication ->
            manifestReads += 1
            assertEquals(audio.publicationId, publication)
            // A foreign callback can resume even after Job cancellation. Generation fencing,
            // rather than a cooperative cancellation check, must reject this late outcome.
            suspendCoroutine { continuation ->
                gate.invokeOnCompletion { continuation.resume(gate.getCompleted()) }
            }
        }
        val owner = NativeListeningController(this, native, access, { testScheduler.currentTime })
        try {
            owner.open(audio.workId, audio.unitId, audio.publicationId, "Ánh đèn cuối sân khấu", "Một lời hẹn")
            runCurrent()
            assertEquals(1, manifestReads)
            owner.restore(other, 42_000, 1250)
            runCurrent()
            val restored = PlaybackState(1, PlaybackSession(PlaybackSource.streaming(other), 42_000, 1250,
                PlaybackIntent.Pause(PlaybackPauseCause.ColdRestore), PlaybackEngine.Preparing, PlaybackInterruption.None))
            assertEquals(restored, owner.playback.state.value)
            gate.complete(PlaybackAccessResult.Accepted(audio))
            runCurrent()
            assertEquals(restored, owner.playback.state.value)
            assertEquals(PlaybackStatus.Paused, owner.playback.state.value.status)
            assertEquals(NativeSelection("", ""), owner.selection.value)
            assertEquals(emptyList(), access.authorizations)
            assertEquals(emptyList(), native.loads)
            assertEquals(emptyList(), native.starts)
        } finally { owner.close(); gate.complete(PlaybackAccessResult.Accepted(audio)) }
    }

    @Test
    fun restore_preserves_supplied_titles_without_authorization_load_or_start() = runTest {
        val native = MockStreamingMediaEngine()
        val access = MockPlaybackAccessPort { grant() }
        val owner = NativeListeningController(this, native, access, { testScheduler.currentTime })
        try {
            owner.restore(audio, 42_000, 1250, title = "Ánh đèn cuối sân khấu", workTitle = "Một lời hẹn")
            runCurrent()
            val restoredSource = PlaybackSource.streaming(audio, "Ánh đèn cuối sân khấu", "Một lời hẹn")
            val restored = PlaybackState(1, PlaybackSession(restoredSource, 42_000, 1250,
                PlaybackIntent.Pause(PlaybackPauseCause.ColdRestore), PlaybackEngine.Preparing, PlaybackInterruption.None))
            assertEquals(restored, owner.playback.state.value)
            assertEquals(NativeSelection("Ánh đèn cuối sân khấu", "Một lời hẹn"), owner.selection.value)
            assertEquals(emptyList(), access.authorizations)
            assertEquals(emptyList(), native.loads)
            assertEquals(emptyList(), native.starts)
        } finally { owner.close() }
    }

    @Test
    fun stale_native_observations_after_pause_are_not_forwarded() = runTest {
        val native = MockStreamingMediaEngine()
        val access = MockPlaybackAccessPort { grant() }
        val engine = AuthorizingMediaEngine(this, native, access) { testScheduler.currentTime }
        val observed = mutableListOf<PlaybackObservation>()
        backgroundScope.launch { engine.events.collect { observed += it } }
        try {
            runCurrent()
            engine.start(command(1))
            runCurrent()
            engine.pause(command(2))
            native.events.emit(PlaybackEvent.EngineStarted(command(1).identity))
            native.events.emit(PlaybackEvent.EnginePosition(command(1).identity, 40_000))
            native.events.emit(PlaybackEvent.EngineFailed(command(1).identity, PlaybackFailure.Expired))
            native.events.emit(PlaybackEvent.EnginePaused(command(2).identity))
            runCurrent()
            assertEquals(listOf<PlaybackObservation>(PlaybackEvent.EngineBuffering(command(1).identity),
                PlaybackEvent.EnginePaused(command(2).identity)), observed)
            assertEquals(listOf(audio), access.authorizations)
            assertEquals(1, native.starts.size)
        } finally { engine.close() }
    }

    @Test
    fun retry_and_scheduled_reload_stay_buffering_until_the_next_actual_start_acknowledgement() = runTest {
        val native = MockStreamingMediaEngine()
        val access = MockPlaybackAccessPort { grant(expiresAt = 16_000) }
        val engine = AuthorizingMediaEngine(this, native, access) { testScheduler.currentTime }
        val controller = PlaybackController(this, engine)
        val identity = command(2).identity
        try {
            controller.dispatch(PlaybackEvent.SourceSelected(source, 1200))
            controller.dispatch(PlaybackEvent.PlayRequested)
            runCurrent()
            assertEquals(PlaybackStatus.Buffering, controller.state.value.status)
            native.events.emit(PlaybackEvent.EngineStarted(identity))
            runCurrent()
            assertEquals(PlaybackStatus.Playing, controller.state.value.status)
            advanceTimeBy(1000)
            runCurrent()
            assertEquals(2, native.starts.size)
            assertEquals(PlaybackStatus.Buffering, controller.state.value.status)
            native.events.emit(PlaybackEvent.EngineStarted(identity))
            runCurrent()
            assertEquals(PlaybackStatus.Playing, controller.state.value.status)
            native.events.emit(PlaybackEvent.EngineFailed(identity, PlaybackFailure.Network))
            runCurrent()
            assertEquals(2, native.starts.size)
            assertEquals(PlaybackStatus.Buffering, controller.state.value.status)
            advanceTimeBy(500)
            runCurrent()
            assertEquals(3, native.starts.size)
            assertEquals(PlaybackStatus.Buffering, controller.state.value.status)
            native.events.emit(PlaybackEvent.EngineReady(identity))
            runCurrent()
            assertEquals(PlaybackStatus.Buffering, controller.state.value.status)
            native.events.emit(PlaybackEvent.EngineStarted(identity))
            runCurrent()
            assertEquals(PlaybackStatus.Playing, controller.state.value.status)
            assertEquals(listOf(audio, audio, audio), access.authorizations)
        } finally { controller.close() }
    }

    @Test
    fun disposable_grant_and_location_never_print_the_transport_credential() {
        val result = grant()
        assertEquals("PlaybackLocation([redacted])", result.value.location.toString())
        assertEquals("PlaybackGrant([redacted])", result.value.toString())
        listOf(result.toString(), result.value.toString(), result.value.location.toString(), command(1).toString(), audio.toString())
            .forEach { assertFalse(sentinel in it) }
    }

    private class MockStreamingMediaEngine : StreamingMediaEngine {
        override val events = MutableSharedFlow<PlaybackObservation>(extraBufferCapacity = 16)
        val loads = mutableListOf<PlaybackEngineCommand>()
        val starts = mutableListOf<PlaybackEngineCommand>()
        val pauses = mutableListOf<PlaybackEngineCommand>()
        var closed = false
        override suspend fun load(command: PlaybackEngineCommand, location: PlaybackLocation) { loads += command }
        override suspend fun prepare(command: PlaybackEngineCommand) = Unit
        override suspend fun start(command: PlaybackEngineCommand) { starts += command }
        override suspend fun pause(command: PlaybackEngineCommand) { pauses += command }
        override suspend fun seek(command: PlaybackEngineCommand) = Unit
        override suspend fun setSpeed(command: PlaybackEngineCommand) = Unit
        override fun close() { closed = true }
    }

    private class MockPlaybackAccessPort(
        val authorize: suspend (AudioIdentity) -> PlaybackAccessResult<PlaybackGrant>,
    ) : PlaybackAccessPort {
        val authorizations = mutableListOf<AudioIdentity>()
        var onManifest: (suspend (String) -> PlaybackAccessResult<AudioIdentity>)? = null
        override suspend fun manifest(publicationId: String): PlaybackAccessResult<AudioIdentity> =
            onManifest?.invoke(publicationId) ?: PlaybackAccessResult.Refused(PlaybackAccessFailure.Unavailable)
        override suspend fun authorize(identity: AudioIdentity): PlaybackAccessResult<PlaybackGrant> {
            authorizations += identity
            return authorize.invoke(identity)
        }
    }
}
