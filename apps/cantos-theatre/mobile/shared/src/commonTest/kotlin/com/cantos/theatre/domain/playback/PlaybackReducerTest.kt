package com.cantos.theatre.domain.playback

import com.cantos.theatre.domain.publication.CatalogEpisode
import com.cantos.theatre.domain.publication.EpisodeKey
import com.cantos.theatre.domain.publication.WorkKey
import com.cantos.theatre.domain.publication.toPlaybackSource
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNotNull
import kotlin.test.assertNull

class PlaybackReducerTest {
    private val source = requireNotNull(PlaybackSource.of("son-ha-ky", 2, 1_722_000))
    private val otherSource = requireNotNull(PlaybackSource.of("den-khuya", 0, 1_232_000))

    @Test
    fun source_and_callback_factories_refuse_blank_keys_and_out_of_range_values() {
        assertNotNull(PlaybackSource.of("mong-ho-diep", 65_535, MAX_PLAYBACK_GENERATION))
        assertNotNull(PlaybackIdentity.of("mong-ho-diep", 65_535, MAX_PLAYBACK_GENERATION))
        listOf("", " ", "\n").forEach { key ->
            assertNull(PlaybackSource.of(key, 0, 1))
            assertNull(PlaybackIdentity.of(key, 0, 0))
        }
        listOf(-1, 65_536).forEach { index ->
            assertNull(PlaybackSource.of("son-ha-ky", index, 1))
            assertNull(PlaybackIdentity.of("son-ha-ky", index, 0))
        }
        listOf(-1L, 0L, MAX_PLAYBACK_GENERATION + 1).forEach { duration ->
            assertNull(PlaybackSource.of("son-ha-ky", 0, duration))
        }
        listOf(-1L, MAX_PLAYBACK_GENERATION + 1).forEach { generation ->
            assertNull(PlaybackIdentity.of("son-ha-ky", 0, generation))
            assertFailsWith<IllegalArgumentException> { PlaybackState(generation) }
        }
    }

    @Test
    fun opaque_catalog_sources_preserve_callback_source_and_generation_fences() {
        val key = WorkKey("published-work-42")
        val episode = CatalogEpisode(EpisodeKey(key, 4), "Ánh đèn cuối sân khấu", 90_000)
        val selected = assertNotNull(episode.toPlaybackSource())
        assertEquals(PlaybackSource.of(key.value, 4, 90_000), selected)
        val loaded = reducePlayback(PlaybackState(), PlaybackEvent.SourceSelected(selected, 0)).nextState
        val requested = reducePlayback(loaded, PlaybackEvent.PlayRequested).nextState
        val current = assertNotNull(PlaybackIdentity.of(key.value, 4, requested.generation))
        val wrongWork = assertNotNull(PlaybackIdentity.of("another-published-work", 4, requested.generation))
        val wrongEpisode = assertNotNull(PlaybackIdentity.of(key.value, 5, requested.generation))
        val stale = assertNotNull(PlaybackIdentity.of(key.value, 4, loaded.generation))
        listOf(wrongWork, wrongEpisode, stale).forEach { identity ->
            assertEquals(PlaybackTransition(requested, emptyList()), reducePlayback(requested, PlaybackEvent.EngineStarted(identity)))
        }
        val started = reducePlayback(requested, PlaybackEvent.EngineStarted(current))
        assertEquals(PlaybackStatus.Simulating, started.nextState.status)
        assertEquals(selected, started.nextState.session?.source)
        assertEquals(requested.generation, started.nextState.generation)
        assertEquals(emptyList(), started.effects)
    }

    @Test
    fun settled_session_and_command_construction_cannot_forge_position_speed_or_identity() {
        val session = active().session ?: error("test session missing")
        assertFailsWith<IllegalArgumentException> { session.copy(positionMs = -1) }
        assertFailsWith<IllegalArgumentException> { session.copy(positionMs = source.durationMs + 1) }
        assertFailsWith<IllegalArgumentException> { session.copy(speedPermille = 999) }
        assertFailsWith<IllegalArgumentException> {
            PlaybackEngineCommand(identity(otherSource, 1), source, 0, 1000)
        }
        assertFailsWith<IllegalArgumentException> {
            PlaybackEngineCommand(identity(source, 1), source, source.durationMs + 1, 1000)
        }
        assertFailsWith<IllegalArgumentException> {
            PlaybackEngineCommand(identity(source, 1), source, 0, 999)
        }
    }

    @Test
    fun play_changes_intent_but_only_a_matching_started_callback_confirms_simulation() {
        val state = active(intent = PlaybackIntent.Pause(PlaybackPauseCause.User), engine = PlaybackEngine.Ready)
        val preparing = state.copy(
            generation = 2,
            session = requireNotNull(state.session).copy(intent = PlaybackIntent.Play, engine = PlaybackEngine.Preparing),
        )
        assertEquals(
            PlaybackTransition(preparing, listOf(PlaybackEffect.StartEngine(command(source, 2, 768_000, 1000)))),
            reducePlayback(state, PlaybackEvent.PlayRequested),
        )
        assertEquals(PlaybackStatus.Loading, preparing.status)
        assertEquals(PlaybackTransition(preparing, emptyList()), reducePlayback(preparing, PlaybackEvent.PlayRequested))
        val running = preparing.copy(session = requireNotNull(preparing.session).copy(engine = PlaybackEngine.Running))
        assertEquals(
            PlaybackTransition(running, emptyList()),
            reducePlayback(preparing, PlaybackEvent.EngineStarted(identity(source, 2))),
        )
        assertEquals(PlaybackStatus.Simulating, running.status)
    }

    @Test
    fun exhausted_generation_rejects_every_identity_changing_command_without_partial_updates() {
        val state = active(generation = MAX_PLAYBACK_GENERATION)
        listOf(
            state to PlaybackEvent.PauseRequested,
            state to PlaybackEvent.SeekRequested(100),
            state to PlaybackEvent.SkipRequested(15_000),
            state to PlaybackEvent.SourceSelected(otherSource, 0),
            state to PlaybackEvent.RestoreRequested(PlaybackSnapshot(otherSource, 0, 1250)),
            state to PlaybackEvent.InterruptionBegan,
            active(generation = MAX_PLAYBACK_GENERATION, intent = PlaybackIntent.Pause(PlaybackPauseCause.User)) to PlaybackEvent.PlayRequested,
            active(generation = MAX_PLAYBACK_GENERATION, interruption = PlaybackInterruption.Active) to PlaybackEvent.InterruptionEnded(resumeAllowed = true),
        ).forEach { (before, event) ->
            assertEquals(rejected(before, PlaybackRejection.GenerationExhausted), reducePlayback(before, event), "$event")
        }
    }

    @Test
    fun valid_no_ops_at_exhausted_generation_do_not_allocate_an_identity() {
        val state = active(generation = MAX_PLAYBACK_GENERATION)
        listOf(
            PlaybackEvent.PlayRequested,
            PlaybackEvent.SourceSelected(source, 0),
            PlaybackEvent.SeekRequested(768_000),
            PlaybackEvent.SkipRequested(0),
            PlaybackEvent.SpeedRequested(1000),
            PlaybackEvent.InterruptionEnded(resumeAllowed = true),
        ).forEach { event ->
            assertEquals(PlaybackTransition(state, emptyList()), reducePlayback(state, event), "$event")
        }
    }

    @Test
    fun stale_generation_work_or_episode_callbacks_are_equal_state_zero_effect_no_ops() {
        val state = active(generation = 5)
        listOf(identity(source, 4), identity(otherSource, 5), requireNotNull(PlaybackIdentity.of("son-ha-ky", 1, 5))).forEach { stale ->
            listOf(
                PlaybackEvent.EngineReady(stale), PlaybackEvent.EngineStarted(stale), PlaybackEvent.EnginePaused(stale),
                PlaybackEvent.EngineBuffering(stale), PlaybackEvent.EnginePosition(stale, Long.MAX_VALUE), PlaybackEvent.EngineEnded(stale),
            ).forEach { event ->
                assertEquals(PlaybackTransition(state, emptyList()), reducePlayback(state, event), "$event")
            }
        }
    }

    @Test
    fun idle_controls_reject_while_unsolicited_callbacks_and_interruptions_do_nothing() {
        val state = PlaybackState()
        listOf(
            PlaybackEvent.PlayRequested, PlaybackEvent.PauseRequested, PlaybackEvent.SeekRequested(0),
            PlaybackEvent.SkipRequested(15_000), PlaybackEvent.SpeedRequested(1250),
        ).forEach { event -> assertEquals(rejected(state, PlaybackRejection.NoSession), reducePlayback(state, event), "$event") }
        listOf(
            PlaybackEvent.InterruptionBegan, PlaybackEvent.InterruptionEnded(resumeAllowed = true),
            PlaybackEvent.EngineStarted(identity(source, 0)), PlaybackEvent.EnginePosition(identity(source, 0), 0),
        ).forEach { event -> assertEquals(PlaybackTransition(state, emptyList()), reducePlayback(state, event), "$event") }
        assertEquals(rejected(state, PlaybackRejection.UnsupportedSpeed), reducePlayback(state, PlaybackEvent.SpeedRequested(999)))
    }

    @Test
    fun every_supported_speed_has_a_complete_same_generation_effect_and_repeat_is_a_no_op() {
        val state = active()
        listOf(750, 1250, 1500, 2000).forEach { speed ->
            val next = state.copy(session = requireNotNull(state.session).copy(speedPermille = speed))
            assertEquals(
                PlaybackTransition(next, listOf(PlaybackEffect.SetSpeed(command(source, 1, 768_000, speed)))),
                reducePlayback(state, PlaybackEvent.SpeedRequested(speed)),
            )
            assertEquals(PlaybackTransition(next, emptyList()), reducePlayback(next, PlaybackEvent.SpeedRequested(speed)))
        }
        assertEquals(PlaybackTransition(state, emptyList()), reducePlayback(state, PlaybackEvent.SpeedRequested(1000)))
    }

    @Test
    fun invalid_absolute_seek_and_restore_are_transactionally_rejected() {
        val state = active()
        listOf(-1L, source.durationMs + 1, Long.MAX_VALUE).forEach { position ->
            assertEquals(rejected(state, PlaybackRejection.OutOfBounds), reducePlayback(state, PlaybackEvent.SeekRequested(position)))
            assertEquals(
                rejected(state, PlaybackRejection.OutOfBounds),
                reducePlayback(state, PlaybackEvent.RestoreRequested(PlaybackSnapshot(source, position, 1000))),
            )
        }
        assertEquals(
            rejected(state, PlaybackRejection.UnsupportedSpeed),
            reducePlayback(state, PlaybackEvent.RestoreRequested(PlaybackSnapshot(source, 0, 999))),
        )
        assertEquals(
            rejected(state, PlaybackRejection.OutOfBounds),
            reducePlayback(state, PlaybackEvent.EnginePosition(identity(source, 1), source.durationMs + 1)),
        )
    }

    @Test
    fun int32_skips_clamp_using_wide_arithmetic_at_uint32_position_limits() {
        val wideSource = requireNotNull(PlaybackSource.of("mong-ho-diep", 0, MAX_PLAYBACK_GENERATION))
        val state = active(source = wideSource, positionMs = MAX_PLAYBACK_GENERATION - 1)
        val next = state.copy(generation = 2, session = requireNotNull(state.session).copy(positionMs = MAX_PLAYBACK_GENERATION))
        assertEquals(
            PlaybackTransition(next, listOf(PlaybackEffect.SeekEngine(command(wideSource, 2, MAX_PLAYBACK_GENERATION, 1000)))),
            reducePlayback(state, PlaybackEvent.SkipRequested(Int.MAX_VALUE)),
        )
        val nearStart = active(source = wideSource, positionMs = 1)
        val start = nearStart.copy(generation = 2, session = requireNotNull(nearStart.session).copy(positionMs = 0))
        assertEquals(
            PlaybackTransition(start, listOf(PlaybackEffect.SeekEngine(command(wideSource, 2, 0, 1000)))),
            reducePlayback(nearStart, PlaybackEvent.SkipRequested(Int.MIN_VALUE)),
        )
    }

    @Test
    fun source_selection_keeps_active_interruption_and_restore_clears_it_cold_paused() {
        val state = active(interruption = PlaybackInterruption.Active, engine = PlaybackEngine.Paused)
        val selectedSession = PlaybackSession(
            otherSource, 0, 1000, PlaybackIntent.Pause(PlaybackPauseCause.User),
            PlaybackEngine.Preparing, PlaybackInterruption.Active,
        )
        val selected = PlaybackState(2, selectedSession)
        assertEquals(
            PlaybackTransition(selected, listOf(PlaybackEffect.PrepareEngine(command(otherSource, 2, 0, 1000)))),
            reducePlayback(state, PlaybackEvent.SourceSelected(otherSource, 0)),
        )
        assertEquals(rejected(selected, PlaybackRejection.Interrupted), reducePlayback(selected, PlaybackEvent.PlayRequested))
        val restored = PlaybackState(
            2, selectedSession.copy(intent = PlaybackIntent.Pause(PlaybackPauseCause.ColdRestore), interruption = PlaybackInterruption.None, speedPermille = 1250),
        )
        assertEquals(
            PlaybackTransition(
                restored,
                listOf(PlaybackEffect.PrepareEngine(command(otherSource, 2, 0, 1250)), PlaybackEffect.OfferResume(otherSource, 0)),
            ),
            reducePlayback(state, PlaybackEvent.RestoreRequested(PlaybackSnapshot(otherSource, 0, 1250))),
        )
    }

    @Test
    fun allowed_interruption_resume_needs_a_new_engine_started_confirmation() {
        val state = active(generation = 7, interruption = PlaybackInterruption.Active, engine = PlaybackEngine.Paused)
        val loading = state.copy(
            generation = 8,
            session = requireNotNull(state.session).copy(interruption = PlaybackInterruption.None, engine = PlaybackEngine.Preparing),
        )
        assertEquals(
            PlaybackTransition(loading, listOf(PlaybackEffect.StartEngine(command(source, 8, 768_000, 1000)))),
            reducePlayback(state, PlaybackEvent.InterruptionEnded(resumeAllowed = true)),
        )
        assertEquals(PlaybackStatus.Loading, loading.status)
        assertEquals(PlaybackTransition(loading, emptyList()), reducePlayback(loading, PlaybackEvent.EngineStarted(identity(source, 7))))
    }

    @Test
    fun matching_ready_cannot_roll_confirmed_running_back_to_preparation() {
        val state = active()
        assertEquals(PlaybackTransition(state, emptyList()), reducePlayback(state, PlaybackEvent.EngineReady(identity(source, 1))))
    }

    @Test
    fun play_intent_after_manual_seek_to_end_is_duplicate_until_paused_or_observed_ended() {
        listOf(PlaybackEngine.Preparing, PlaybackEngine.Ready, PlaybackEngine.Running,
            PlaybackEngine.Paused, PlaybackEngine.Buffering).forEach { phase ->
            val before = active(engine = phase)
            val seek = reducePlayback(before, PlaybackEvent.SeekRequested(source.durationMs))
            val atEnd = seek.nextState
            assertEquals(source.durationMs, atEnd.session?.positionMs)
            assertEquals(PlaybackIntent.Play, atEnd.session?.intent)
            assertEquals(phase, atEnd.session?.engine)
            assertEquals(
                PlaybackTransition(atEnd, emptyList()),
                reducePlayback(atEnd, PlaybackEvent.PlayRequested),
                "A duplicate play must preserve the $phase request at the end",
            )
            val paused = reducePlayback(atEnd, PlaybackEvent.PauseRequested).nextState
            val restarting = PlaybackState(
                paused.generation + 1L,
                requireNotNull(paused.session).copy(positionMs = 0L, intent = PlaybackIntent.Play, engine = PlaybackEngine.Preparing),
            )
            assertEquals(
                PlaybackTransition(restarting, listOf(PlaybackEffect.StartEngine(command(source, restarting.generation, 0L, 1000)))),
                reducePlayback(paused, PlaybackEvent.PlayRequested),
            )
        }
    }

    @Test
    fun engine_end_records_no_completion_and_only_explicit_play_restarts_from_zero() {
        val state = active()
        val ended = state.copy(
            session = requireNotNull(state.session).copy(
                positionMs = source.durationMs, intent = PlaybackIntent.Pause(PlaybackPauseCause.User), engine = PlaybackEngine.Ended,
            ),
        )
        assertEquals(PlaybackTransition(ended, emptyList()), reducePlayback(state, PlaybackEvent.EngineEnded(identity(source, 1))))
        assertEquals(PlaybackStatus.Ended, ended.status)
        assertNull(ended.pauseCause)
        assertEquals(PlaybackTransition(ended, emptyList()), reducePlayback(ended, PlaybackEvent.EngineStarted(identity(source, 1))))
        val loading = ended.copy(
            generation = 2,
            session = requireNotNull(ended.session).copy(positionMs = 0, intent = PlaybackIntent.Play, engine = PlaybackEngine.Preparing),
        )
        assertEquals(
            PlaybackTransition(loading, listOf(PlaybackEffect.StartEngine(command(source, 2, 0, 1000)))),
            reducePlayback(ended, PlaybackEvent.PlayRequested),
        )
    }

    private fun active(
        generation: Long = 1,
        source: PlaybackSource = this.source,
        positionMs: Long = 768_000,
        intent: PlaybackIntent = PlaybackIntent.Play,
        engine: PlaybackEngine = PlaybackEngine.Running,
        interruption: PlaybackInterruption = PlaybackInterruption.None,
    ): PlaybackState = PlaybackState(generation, PlaybackSession(source, positionMs, 1000, intent, engine, interruption))

    private fun identity(source: PlaybackSource, generation: Long): PlaybackIdentity =
        requireNotNull(PlaybackIdentity.of(source.workId, source.episodeIndex, generation))

    private fun command(source: PlaybackSource, generation: Long, positionMs: Long, speedPermille: Int): PlaybackEngineCommand =
        PlaybackEngineCommand(identity(source, generation), source, positionMs, speedPermille)

    private fun rejected(state: PlaybackState, reason: PlaybackRejection): PlaybackTransition =
        PlaybackTransition(state, listOf(PlaybackEffect.Rejected(reason)))
}
