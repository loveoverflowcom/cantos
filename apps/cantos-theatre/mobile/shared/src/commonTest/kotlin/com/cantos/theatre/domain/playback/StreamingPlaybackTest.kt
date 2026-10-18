package com.cantos.theatre.domain.playback

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertNull

/** Expectations come from native listening semantics, independently of engine timing. */
class StreamingPlaybackTest {
    private val work = "10000000-0000-0000-0000-000000000001"
    private val unit = "20000000-0000-0000-0000-000000000002"
    private val publication = "30000000-0000-0000-0000-000000000003"
    private val rendition = "40000000-0000-0000-0000-000000000004"
    private val digest = "a".repeat(64)
    private val audio = requireNotNull(AudioIdentity.of(work, unit, publication, rendition, digest, 60_000))
    private val source = PlaybackSource.streaming(audio)

    @Test
    fun immutable_audio_identity_refuses_invalid_ids_checksums_and_duration() {
        assertEquals(listOf(work, unit, publication, rendition, digest, 60_000L),
            listOf(audio.workId, audio.unitId, audio.publicationId, audio.renditionId, audio.sha256, audio.durationMs))
        listOf("", "Ánh đèn cuối sân khấu", "https://media.invalid/audio?secret=token",
            "A0000000-0000-0000-0000-000000000001", work.dropLast(1)).forEach { invalid ->
                assertNull(AudioIdentity.of(invalid, unit, publication, rendition, digest, 60_000))
                assertNull(AudioIdentity.of(work, invalid, publication, rendition, digest, 60_000))
                assertNull(AudioIdentity.of(work, unit, invalid, rendition, digest, 60_000))
                assertNull(AudioIdentity.of(work, unit, publication, invalid, digest, 60_000))
            }
        listOf("", "a".repeat(63), "g".repeat(64), "A".repeat(64), "a".repeat(65)).forEach {
            assertNull(AudioIdentity.of(work, unit, publication, rendition, it, 60_000))
        }
        listOf(-1L, 0L, 3_600_001L).forEach {
            assertNull(AudioIdentity.of(work, unit, publication, rendition, digest, it))
        }
        assertNotNull(AudioIdentity.of(work, unit, publication, rendition, digest, 1))
        assertNotNull(AudioIdentity.of(work, unit, publication, rendition, digest, 3_600_000))
    }

    @Test
    fun streaming_projects_playing_only_after_the_current_engine_start_acknowledgement() {
        val selected = reducePlayback(PlaybackState(), PlaybackEvent.SourceSelected(source, 1200)).nextState
        val requested = reducePlayback(selected, PlaybackEvent.PlayRequested)
        val expected = PlaybackState(2, PlaybackSession(source, 1200, 1000,
            PlaybackIntent.Play, PlaybackEngine.Preparing, PlaybackInterruption.None))
        assertEquals(PlaybackTransition(expected, listOf(PlaybackEffect.StartEngine(command(2, 1200)))), requested)
        assertEquals(PlaybackStatus.Loading, requested.nextState.status)
        val ready = expected.copy(session = expected.session!!.copy(engine = PlaybackEngine.Ready))
        assertEquals(PlaybackTransition(ready, emptyList()), reducePlayback(expected, PlaybackEvent.EngineReady(identity(2))))
        assertEquals(PlaybackStatus.Loading, ready.status)
        assertEquals(PlaybackTransition(ready, emptyList()), reducePlayback(ready, PlaybackEvent.EngineStarted(identity(1))))
        val playing = ready.copy(session = ready.session!!.copy(engine = PlaybackEngine.Running))
        assertEquals(PlaybackTransition(playing, emptyList()), reducePlayback(ready, PlaybackEvent.EngineStarted(identity(2))))
        assertEquals(PlaybackStatus.Playing, playing.status)
    }

    @Test
    fun requested_seek_keeps_confirmed_position_until_a_current_engine_position_arrives() {
        val running = running()
        val sought = reducePlayback(running, PlaybackEvent.SeekRequested(18_000))
        val pending = running.copy(generation = 3, session = running.session!!.copy(
            pendingPositionMs = 18_000, engine = PlaybackEngine.Preparing))
        assertEquals(PlaybackTransition(pending, listOf(PlaybackEffect.SeekEngine(command(3, 18_000)))), sought)
        assertEquals(1200L, sought.nextState.session?.positionMs)
        assertEquals(PlaybackTransition(pending, emptyList()),
            reducePlayback(pending, PlaybackEvent.EnginePosition(identity(2), 18_000)))
        val acknowledged = pending.copy(session = pending.session!!.copy(positionMs = 17_980, pendingPositionMs = null))
        assertEquals(PlaybackTransition(acknowledged, emptyList()),
            reducePlayback(pending, PlaybackEvent.EnginePosition(identity(3), 17_980)))
    }

    @Test
    fun cold_restore_offers_the_pinned_point_and_never_emits_start() {
        val snapshot = PlaybackSnapshot(source, 42_000, 1250)
        val restored = PlaybackState(1, PlaybackSession(source, 42_000, 1250,
            PlaybackIntent.Pause(PlaybackPauseCause.ColdRestore), PlaybackEngine.Preparing, PlaybackInterruption.None))
        val command = command(1, 42_000, 1250)
        assertEquals(PlaybackTransition(restored, listOf(
            PlaybackEffect.PrepareEngine(command), PlaybackEffect.OfferResume(source, 42_000))),
            reducePlayback(PlaybackState(), PlaybackEvent.RestoreRequested(snapshot)))
        assertEquals(PlaybackStatus.Paused, restored.status)
        assertEquals(PlaybackPauseCause.ColdRestore, restored.pauseCause)
    }

    @Test
    fun clearing_session_stops_the_engine_and_rejects_every_late_observation() {
        val running = running()
        val cleared = PlaybackState(3)
        assertEquals(PlaybackTransition(cleared, listOf(PlaybackEffect.PauseEngine(command(3, 1200).copy(clearSession = true)))),
            reducePlayback(running, PlaybackEvent.SessionCleared))
        listOf(PlaybackEvent.EngineReady(identity(2)), PlaybackEvent.EngineStarted(identity(2)),
            PlaybackEvent.EnginePosition(identity(2), 44_000), PlaybackEvent.EnginePaused(identity(2)),
            PlaybackEvent.EngineBuffering(identity(2)), PlaybackEvent.EngineEnded(identity(2)),
            PlaybackEvent.EngineFailed(identity(2), PlaybackFailure.Network)).forEach {
            assertEquals(PlaybackTransition(cleared, emptyList()), reducePlayback(cleared, it))
        }
        assertEquals(PlaybackStatus.Idle, cleared.status)
        val exhausted = running.copy(generation = MAX_PLAYBACK_GENERATION)
        val exhaustedClear = PlaybackState(MAX_PLAYBACK_GENERATION)
        assertEquals(PlaybackTransition(exhaustedClear,
            listOf(PlaybackEffect.PauseEngine(command(MAX_PLAYBACK_GENERATION, 1200).copy(clearSession = true)))),
            reducePlayback(exhausted, PlaybackEvent.SessionCleared))
        assertEquals(PlaybackTransition(exhaustedClear, emptyList()),
            reducePlayback(exhaustedClear, PlaybackEvent.EngineStarted(identity(MAX_PLAYBACK_GENERATION))))
    }

    @Test
    fun failure_is_distinct_from_pause_and_retry_requires_a_new_play_generation() {
        val running = running()
        val failed = running.copy(session = running.session!!.copy(engine = PlaybackEngine.Paused, failure = PlaybackFailure.Decode))
        assertEquals(PlaybackTransition(failed, emptyList()),
            reducePlayback(running, PlaybackEvent.EngineFailed(identity(2), PlaybackFailure.Decode)))
        assertEquals(PlaybackStatus.Failed, failed.status)
        val retry = failed.copy(generation = 3,
            session = failed.session!!.copy(engine = PlaybackEngine.Preparing, failure = null))
        assertEquals(PlaybackTransition(retry, listOf(PlaybackEffect.StartEngine(command(3, 1200)))),
            reducePlayback(failed, PlaybackEvent.PlayRequested))
    }

    private fun running() = PlaybackState(2, PlaybackSession(source, 1200, 1000,
        PlaybackIntent.Play, PlaybackEngine.Running, PlaybackInterruption.None))
    private fun identity(generation: Long) = requireNotNull(PlaybackIdentity.of(work, 0, generation))
    private fun command(generation: Long, position: Long, speed: Int = 1000) =
        PlaybackEngineCommand(identity(generation), source, position, speed)
}
