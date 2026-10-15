package com.cantos.theatre.application.playback

import com.cantos.theatre.adapters.mock.MockCommandKind
import com.cantos.theatre.adapters.mock.MockMediaEngine
import com.cantos.theatre.domain.playback.PlaybackEngineCommand
import com.cantos.theatre.domain.playback.PlaybackEvent
import com.cantos.theatre.domain.playback.PlaybackIntent
import com.cantos.theatre.domain.playback.PlaybackSnapshot
import com.cantos.theatre.domain.playback.PlaybackSource
import com.cantos.theatre.domain.playback.PlaybackState
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
    fun committed_callback_observes_a_brief_start_before_pause_and_ignores_stale_observations() = runTest {
        val engine = MockMediaEngine(acknowledgeAutomatically = false)
        val committed = mutableListOf<Pair<PlaybackState, PlaybackState>>()
        val player = PlaybackController(this, engine) { before, after -> committed += before to after }
        try {
            player.dispatch(PlaybackEvent.SourceSelected(source, 768_000L))
            player.dispatch(PlaybackEvent.PlayRequested)
            advanceUntilIdle()
            val identity = requireNotNull(player.state.value.identity)
            player.dispatch(PlaybackEvent.EngineStarted(identity))
            player.dispatch(PlaybackEvent.PauseRequested)
            player.dispatch(PlaybackEvent.EngineStarted(identity))
            advanceUntilIdle()
            assertEquals(
                listOf(PlaybackStatus.Paused, PlaybackStatus.Loading, PlaybackStatus.Simulating, PlaybackStatus.Paused),
                committed.map { it.second.status },
            )
            assertEquals(listOf(1L, 2L, 2L, 3L), committed.map { it.second.generation })
            assertEquals(PlaybackState(), committed.first().first)
            assertEquals(committed.dropLast(1).map { it.second }, committed.drop(1).map { it.first })
            assertEquals(player.state.value, committed.last().second)
            player.close()
            assertFalse(player.dispatch(PlaybackEvent.EngineStarted(identity)))
            assertEquals(4, committed.size)
        } finally {
            player.close()
        }
    }

    @Test
    fun resume_resolves_a_same_source_point_after_queued_controls_and_never_starts() = runTest {
        val engine = MockMediaEngine(acknowledgeAutomatically = false)
        val player = PlaybackController(this, engine)
        val accepted = mutableListOf<PlaybackState>()
        try {
            val stale = PlaybackSnapshot(source, 768_000L, 1000)
            player.dispatch(PlaybackEvent.RestoreRequested(stale))
            player.dispatch(PlaybackEvent.SeekRequested(5_123L))
            player.dispatch(PlaybackEvent.SpeedRequested(1500))
            player.restoreForResume(stale) { accepted += it }
            advanceUntilIdle()
            assertEquals(1, accepted.size)
            assertEquals(PlaybackStatus.Paused, accepted.single().status)
            assertEquals(PlaybackSnapshot(source, 5_123L, 1500), accepted.single().session?.snapshot())
            assertEquals(player.state.value, accepted.single())
            assertFalse(engine.acceptedCommands.any { it.kind == MockCommandKind.Start })
        } finally {
            player.close()
        }
    }

    @Test
    fun a_later_play_intent_fences_pending_resume_even_when_play_is_a_reducer_noop() = runTest {
        val engine = MockMediaEngine(acknowledgeAutomatically = false)
        val player = PlaybackController(this, engine)
        var captured: Pair<PlaybackState, Long>? = null
        val accepted = mutableListOf<PlaybackState>()
        try {
            player.dispatch(PlaybackEvent.SourceSelected(source, 768_000L))
            player.dispatch(PlaybackEvent.PlayRequested)
            advanceUntilIdle()
            engine.acknowledgeStarted(requireNotNull(player.state.value.identity))
            advanceUntilIdle()
            player.captureState { state, revision -> captured = state to revision }
            advanceUntilIdle()
            val (before, revision) = requireNotNull(captured)
            player.dispatch(PlaybackEvent.PlayRequested)
            player.restoreForResume(requireNotNull(before.session).snapshot(), expectedRevision = revision) { accepted += it }
            advanceUntilIdle()
            assertEquals(before, player.state.value)
            assertEquals(PlaybackStatus.Simulating, player.state.value.status)
            assertEquals(emptyList(), accepted)
            assertEquals(1, engine.acceptedCommands.count { it.kind == MockCommandKind.Start })
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
            assertEquals(PlaybackNotice.MediaEngineUnavailable, player.notice.value)
        } finally {
            player.close()
        }
        assertFalse(player.dispatch(PlaybackEvent.PlayRequested))
        assertFalse(base.emit(PlaybackEvent.EngineStarted(requireNotNull(player.state.value.identity))))
    }
}
