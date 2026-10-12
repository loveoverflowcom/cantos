package com.cantos.theatre.application.playback

import com.cantos.theatre.adapters.mock.MockCommandKind
import com.cantos.theatre.adapters.mock.MockMediaEngine
import com.cantos.theatre.domain.playback.PlaybackEngineCommand
import com.cantos.theatre.domain.playback.PlaybackEvent
import com.cantos.theatre.domain.playback.PlaybackIntent
import com.cantos.theatre.domain.playback.PlaybackSnapshot
import com.cantos.theatre.domain.playback.PlaybackSource
import com.cantos.theatre.domain.playback.PlaybackStatus
import com.cantos.theatre.ports.MediaEngine
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertIs
import kotlin.test.assertTrue
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runTest

@OptIn(ExperimentalCoroutinesApi::class)
class PlaybackControllerTest {
    private val source = requireNotNull(PlaybackSource.of("son-ha-ky", 2, 1_722_000L))

    @Test
    fun intent_requires_matching_fake_acknowledgement_and_late_start_cannot_undo_pause() = runTest {
        val engine = MockMediaEngine(acknowledgeAutomatically = false)
        val player = PlaybackController(this, engine)
        try {
            player.dispatch(PlaybackEvent.SourceSelected(source, 768_000L))
            player.dispatch(PlaybackEvent.PlayRequested)
            advanceUntilIdle()
            assertEquals(PlaybackStatus.Loading, player.state.value.status)
            val started = requireNotNull(player.state.value.identity)
            engine.acknowledgeStarted(started)
            advanceUntilIdle()
            assertEquals(PlaybackStatus.Simulating, player.state.value.status)
            player.dispatch(PlaybackEvent.PauseRequested)
            engine.acknowledgeStarted(started)
            advanceUntilIdle()
            assertEquals(PlaybackStatus.Paused, player.state.value.status)
            assertIs<PlaybackIntent.Pause>(player.state.value.session?.intent)
        } finally {
            player.close()
        }
    }

    @Test
    fun rapid_toggles_and_relative_seeks_read_the_event_loop_current_state() = runTest {
        val engine = MockMediaEngine(acknowledgeAutomatically = false)
        val player = PlaybackController(this, engine)
        try {
            player.dispatch(PlaybackEvent.SourceSelected(source, 768_000L))
            player.toggleIntent()
            player.toggleIntent()
            repeat(3) { player.dispatch(PlaybackEvent.SkipRequested(15_000)) }
            advanceUntilIdle()
            assertEquals(PlaybackStatus.Paused, player.state.value.status)
            assertEquals(813_000L, player.state.value.session?.positionMs)
            assertEquals(1, engine.acceptedCommands.count { it.kind == MockCommandKind.Start })
            assertEquals(1, engine.acceptedCommands.count { it.kind == MockCommandKind.Pause })
        } finally {
            player.close()
        }
    }

    @Test
    fun ordered_speed_command_is_not_undone_by_a_deferred_start_acknowledgement() = runTest {
        val engine = MockMediaEngine(acknowledgeAutomatically = false)
        val player = PlaybackController(this, engine)
        try {
            player.dispatch(PlaybackEvent.SourceSelected(source, 0L))
            player.dispatch(PlaybackEvent.PlayRequested)
            player.dispatch(PlaybackEvent.SpeedRequested(1500))
            advanceUntilIdle()
            val commands = engine.acceptedCommands
            assertEquals(listOf(MockCommandKind.Prepare, MockCommandKind.Start, MockCommandKind.Speed), commands.map { it.kind })
            val identity = commands[1].command.identity
            assertEquals(identity, commands[2].command.identity)
            assertEquals(1500, engine.desiredCommand?.speedPermille)
            engine.acknowledgeStarted(identity)
            advanceUntilIdle()
            assertEquals(PlaybackStatus.Simulating, player.state.value.status)
            assertEquals(1500, player.state.value.session?.speedPermille)
            assertEquals(1500, engine.desiredCommand?.speedPermille)
        } finally {
            player.close()
        }
    }

    @Test
    fun cold_restore_offers_resume_without_start_and_a_later_restore_fences_old_callbacks() = runTest {
        val engine = MockMediaEngine(acknowledgeAutomatically = false)
        val player = PlaybackController(this, engine)
        try {
            player.dispatch(PlaybackEvent.RestoreRequested(PlaybackSnapshot(source, 120_000L, 1250)))
            advanceUntilIdle()
            assertEquals(PlaybackStatus.Paused, player.state.value.status)
            assertIs<PlaybackNotice.ResumeOffered>(player.notice.value)
            assertFalse(engine.acceptedCommands.any { it.kind == MockCommandKind.Start })
            val old = requireNotNull(player.state.value.identity)
            player.dispatch(PlaybackEvent.RestoreRequested(PlaybackSnapshot(source, 768_000L, 1000)))
            engine.emit(PlaybackEvent.EnginePosition(old, 130_000L))
            engine.acknowledgeStarted(old)
            advanceUntilIdle()
            assertEquals(768_000L, player.state.value.session?.positionMs)
            assertEquals(PlaybackStatus.Paused, player.state.value.status)
            assertTrue(player.state.value.generation > old.generation)
        } finally {
            player.close()
        }
    }

    @Test
    fun bookmark_capture_observes_accepted_seek_commands_in_order() = runTest {
        val engine = MockMediaEngine(acknowledgeAutomatically = false)
        val player = PlaybackController(this, engine)
        val points = mutableListOf<PlaybackSnapshot>()
        try {
            player.dispatch(PlaybackEvent.SourceSelected(source, 768_000L))
            player.dispatch(PlaybackEvent.SkipRequested(15_000))
            player.captureBookmarkPoint { points += it }
            advanceUntilIdle()
            assertEquals(1, points.size)
            assertEquals(783_000L, points.single().positionMs)
        } finally {
            player.close()
        }
    }

    @Test
    fun failed_fake_command_is_a_typed_notice_and_never_confirms_audio_or_running() = runTest {
        val base = MockMediaEngine()
        val failing = object : MediaEngine by base {
            override suspend fun start(command: PlaybackEngineCommand) {
                error("Synthetic acceptance failure")
            }
        }
        val player = PlaybackController(this, failing)
        try {
            player.dispatch(PlaybackEvent.SourceSelected(source, 0L))
            player.dispatch(PlaybackEvent.PlayRequested)
            advanceUntilIdle()
            assertEquals(PlaybackStatus.Loading, player.state.value.status)
            assertEquals(PlaybackNotice.MockEngineUnavailable, player.notice.value)
        } finally {
            player.close()
        }
        assertFalse(player.dispatch(PlaybackEvent.PlayRequested))
        assertFalse(base.emit(PlaybackEvent.EngineStarted(requireNotNull(player.state.value.identity))))
    }
}
