package com.cantos.theatre.app

import com.cantos.theatre.adapters.mock.MockCatalogPort
import com.cantos.theatre.adapters.mock.MockCommandKind
import com.cantos.theatre.adapters.mock.MockMediaEngine
import com.cantos.theatre.adapters.mock.mockInventory
import com.cantos.theatre.adapters.mock.sandbox.database.FakePersistentProgressPort
import com.cantos.theatre.adapters.mock.sandbox.database.FakeStorageDatabase
import com.cantos.theatre.adapters.mock.toPlaybackSource
import com.cantos.theatre.adapters.mock.toEpisodeKey
import com.cantos.theatre.demo.DemoEvent
import com.cantos.theatre.demo.DemoLibraryTab
import com.cantos.theatre.demo.DemoTimer
import com.cantos.theatre.domain.listening.ListeningSnapshot
import com.cantos.theatre.domain.listening.ProgressRecord
import com.cantos.theatre.domain.listening.SavedWork
import com.cantos.theatre.domain.playback.PlaybackEvent
import com.cantos.theatre.domain.playback.PlaybackSnapshot
import com.cantos.theatre.domain.playback.PlaybackStatus
import com.cantos.theatre.domain.publication.EpisodeKey
import com.cantos.theatre.domain.publication.WorkKey
import com.cantos.theatre.ports.ProgressOutcome
import com.cantos.theatre.ports.ProgressPort
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertSame
import kotlin.test.assertTrue
import kotlin.coroutines.CoroutineContext
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineName
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.Job
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.runCurrent

@OptIn(ExperimentalCoroutinesApi::class)
class AppContainerTest {
    private val key = EpisodeKey(WorkKey("son-ha-ky"), 2)

    private fun TestScope.app(engine: MockMediaEngine): AppContainer {
        val inventory = mockInventory()
        val episode = requireNotNull(inventory.episode(key))
        val point = PlaybackSnapshot(requireNotNull(episode.toPlaybackSource()), 768_000L, 1000)
        val listening = ListeningSnapshot(progress = listOf(ProgressRecord(key, point.positionMs, episode.durationMs)))
        val seed = AppStartState(inventory, listening, point)
        return AppContainer(this, MockCatalogPort(inventory), FakePersistentProgressPort(FakeStorageDatabase(listening)), engine, seed)
    }

    @Test
    fun initial_restore_and_ready_acknowledgement_preserve_explicit_empty_listening_seed() = runTest {
        val inventory = mockInventory()
        val episode = requireNotNull(inventory.episode(key))
        val point = PlaybackSnapshot(requireNotNull(episode.toPlaybackSource()), 768_000L, 1000)
        val initial = ListeningSnapshot()
        val engine = MockMediaEngine()
        val app = AppContainer(this, MockCatalogPort(inventory), FakePersistentProgressPort(FakeStorageDatabase(initial)), engine,
            AppStartState(inventory, initial, point))
        try {
            advanceUntilIdle()
            assertEquals(initial, app.listening.state.value.snapshot)
            assertEquals(PlaybackStatus.Paused, app.playback.state.value.status)
            assertFalse(app.resumeEpisode(key))
            assertFalse(app.resumeHistoryEpisode(key))
            assertFalse(engine.acceptedCommands.any { it.kind == MockCommandKind.Start })
            app.playback.dispatch(PlaybackEvent.PlayRequested)
            advanceUntilIdle()
            assertEquals(listOf(key), app.listening.state.value.snapshot.progress.map { it.episode })
            assertEquals(listOf(key), app.listening.state.value.snapshot.history.map { it.episode })
        } finally {
            app.close()
        }
    }

    @Test
    fun all_navigation_routes_reuse_one_player_and_engine_with_the_same_point() = runTest {
        val engine = MockMediaEngine(acknowledgeAutomatically = false)
        val app = app(engine)
        try {
            advanceUntilIdle()
            val player = app.playback
            assertTrue(app.selectEpisode(key))
            advanceUntilIdle()
            engine.acknowledgeStarted(requireNotNull(player.state.value.identity))
            advanceUntilIdle()
            listOf(TheatreRoute.Discovery, TheatreRoute.Work(key.work), TheatreRoute.Library,
                TheatreRoute.Settings, TheatreRoute.Player, TheatreRoute.Queue).forEach {
                app.navigate(NavigationEvent.Navigate(it))
                app.navigate(NavigationEvent.Back)
            }
            assertSame(player, app.playback)
            assertSame(engine, app.mediaEngine)
            assertEquals(768_000L, player.state.value.session?.positionMs)
            assertEquals(PlaybackStatus.Simulating, player.state.value.status)
            assertEquals(1, engine.acceptedCommands.count { it.kind == MockCommandKind.Prepare })
            assertEquals(1, engine.acceptedCommands.count { it.kind == MockCommandKind.Start })
        } finally {
            app.close()
        }
    }

    @Test
    fun missing_catalogue_keys_are_refused_before_navigation_or_engine_effects() = runTest {
        val engine = MockMediaEngine(acknowledgeAutomatically = false)
        val app = app(engine)
        try {
            advanceUntilIdle()
            val state = app.playback.state.value
            val commands = engine.acceptedCommands
            val navigation = app.navigation.value
            assertFalse(app.selectEpisode(EpisodeKey(WorkKey("unknown"), 0)))
            assertFalse(app.selectEpisode(EpisodeKey(key.work, 99)))
            advanceUntilIdle()
            assertEquals(state, app.playback.state.value)
            assertEquals(commands, engine.acceptedCommands)
            assertEquals(navigation, app.navigation.value)
        } finally {
            app.close()
        }
    }

    @Test
    fun history_needs_a_committed_simulated_start_and_keeps_the_latest_episode_per_work() = runTest {
        val engine = MockMediaEngine(acknowledgeAutomatically = false)
        val app = app(engine)
        try {
            advanceUntilIdle()
            app.navigate(NavigationEvent.Navigate(TheatreRoute.Work(key.work)))
            app.setSaved(key.work, saved = true)
            app.setSaved(key.work, saved = true)
            assertTrue(app.selectEpisode(key))
            advanceUntilIdle()
            assertEquals(setOf(SavedWork(key.work)), app.listening.state.value.snapshot.savedWorks)
            assertTrue(app.listening.state.value.snapshot.history.isEmpty())
            val identity = requireNotNull(app.playback.state.value.identity)
            // Both commit in one event-loop turn, before any StateFlow collector can run.
            app.playback.dispatch(PlaybackEvent.EngineStarted(identity))
            app.playback.dispatch(PlaybackEvent.PauseRequested)
            advanceUntilIdle()
            assertEquals(listOf(key), app.listening.state.value.snapshot.history.map { it.episode })
            assertEquals(PlaybackStatus.Paused, app.playback.state.value.status)

            val other = EpisodeKey(WorkKey("den-khuya"), 0)
            assertTrue(app.selectEpisode(other))
            advanceUntilIdle()
            engine.acknowledgeStarted(requireNotNull(app.playback.state.value.identity))
            advanceUntilIdle()
            val next = EpisodeKey(key.work, 1)
            assertTrue(app.selectEpisode(next))
            advanceUntilIdle()
            assertEquals(listOf(other, key), app.listening.state.value.snapshot.history.map { it.episode })
            engine.acknowledgeStarted(requireNotNull(app.playback.state.value.identity))
            advanceUntilIdle()
            assertEquals(listOf(next, other), app.listening.state.value.snapshot.history.map { it.episode })
            val current = requireNotNull(app.playback.state.value.identity)
            engine.emit(PlaybackEvent.EngineEnded(current))
            advanceUntilIdle()
            val ended = app.listening.state.value.snapshot.history.first()
            assertEquals(next, ended.episode)
            assertEquals(ended.durationMs, ended.positionMs)
            assertEquals(PlaybackStatus.Ended, app.playback.state.value.status)
        } finally {
            app.close()
        }
    }

    @Test
    fun clear_history_preserves_other_owners_and_duplicate_start_cannot_revive_it() = runTest {
        val engine = MockMediaEngine(acknowledgeAutomatically = false)
        val app = app(engine)
        try {
            advanceUntilIdle()
            app.selectEpisode(key)
            advanceUntilIdle()
            engine.acknowledgeStarted(requireNotNull(app.playback.state.value.identity))
            advanceUntilIdle()
            app.setSaved(key.work, saved = true)
            app.bookmark()
            app.navigate(NavigationEvent.Navigate(TheatreRoute.Library))
            advanceUntilIdle()
            val before = app.listening.state.value.snapshot
            val player = app.playback.state.value
            val navigation = app.navigation.value
            val commands = engine.acceptedCommands
            assertEquals(1, before.history.size)
            app.clearHistory()
            app.clearHistory()
            advanceUntilIdle()
            assertEquals(
                ListeningSnapshot(before.savedWorks, before.bookmarks, before.progress),
                app.listening.state.value.snapshot,
            )
            assertFalse(app.resumeHistoryEpisode(key))
            engine.acknowledgeStarted(requireNotNull(player.identity))
            advanceUntilIdle()
            assertTrue(app.listening.state.value.snapshot.history.isEmpty())
            assertEquals(player, app.playback.state.value)
            assertEquals(navigation, app.navigation.value)
            assertEquals(commands, engine.acceptedCommands)
            assertTrue(app.resumeEpisode(key))
            advanceUntilIdle()
            assertEquals(PlaybackStatus.Paused, app.playback.state.value.status)
            assertTrue(app.listening.state.value.snapshot.history.isEmpty())
        } finally {
            app.close()
        }
    }

    @Test
    fun resume_reads_the_same_episode_after_queued_seek_and_opens_paused_without_new_start() = runTest {
        val engine = MockMediaEngine(acknowledgeAutomatically = false)
        val app = app(engine)
        try {
            advanceUntilIdle()
            app.selectEpisode(key)
            advanceUntilIdle()
            engine.acknowledgeStarted(requireNotNull(app.playback.state.value.identity))
            advanceUntilIdle()
            val stale = requireNotNull(app.playback.state.value.identity)
            val starts = engine.acceptedCommands.count { it.kind == MockCommandKind.Start }
            app.navigate(NavigationEvent.Navigate(TheatreRoute.Library))
            app.playback.dispatch(PlaybackEvent.SeekRequested(5_123L))
            assertTrue(app.resumeEpisode(key))
            advanceUntilIdle()
            val history = app.listening.state.value.snapshot.history
            assertEquals(5_123L, app.playback.state.value.session?.positionMs)
            assertEquals(5_123L, app.listening.state.value.snapshot.progress.single().positionMs)
            assertEquals(PlaybackStatus.Paused, app.playback.state.value.status)
            assertEquals(TheatreRoute.Player, app.navigation.value.route)
            assertEquals(starts, engine.acceptedCommands.count { it.kind == MockCommandKind.Start })
            engine.acknowledgeStarted(stale)
            engine.emit(PlaybackEvent.EnginePosition(stale, 30_000L))
            advanceUntilIdle()
            assertEquals(history, app.listening.state.value.snapshot.history)
            assertEquals(5_123L, app.playback.state.value.session?.positionMs)
            assertEquals(PlaybackStatus.Paused, app.playback.state.value.status)
        } finally {
            app.close()
        }
    }

    @Test
    fun absent_or_duration_mismatched_resume_points_preserve_player_navigation_and_commands() = runTest {
        val inventory = mockInventory()
        val episode = requireNotNull(inventory.episode(key))
        val point = PlaybackSnapshot(requireNotNull(episode.toPlaybackSource()), 768_000L, 1000)
        val other = EpisodeKey(WorkKey("den-khuya"), 0)
        val mismatched = ListeningSnapshot(progress = listOf(ProgressRecord(other, 10_000L, 60_000L)))
        val engine = MockMediaEngine(acknowledgeAutomatically = false)
        val app = AppContainer(this, MockCatalogPort(inventory), FakePersistentProgressPort(FakeStorageDatabase(mismatched)), engine,
            AppStartState(inventory, mismatched, point))
        try {
            advanceUntilIdle()
            val before = app.playback.state.value
            val navigation = app.navigation.value
            val commands = engine.acceptedCommands
            assertFalse(app.resumeEpisode(EpisodeKey(WorkKey("unknown"), 0)))
            assertFalse(app.resumeEpisode(EpisodeKey(key.work, 1)))
            assertFalse(app.resumeEpisode(other))
            advanceUntilIdle()
            assertEquals(before, app.playback.state.value)
            assertEquals(navigation, app.navigation.value)
            assertEquals(commands, engine.acceptedCommands)
        } finally {
            app.close()
        }
    }

    @Test
    fun queued_rewind_then_other_source_selection_resumes_the_accepted_rewound_episode() = runTest {
        val engine = MockMediaEngine(acknowledgeAutomatically = false)
        val app = app(engine)
        try {
            advanceUntilIdle()
            app.playback.dispatch(PlaybackEvent.SeekRequested(5_123L))
            assertTrue(app.selectEpisode(EpisodeKey(WorkKey("den-khuya"), 0)))
            assertTrue(app.resumeEpisode(key))
            advanceUntilIdle()
            val source = requireNotNull(app.playback.state.value.session).source
            assertEquals(key.work.value, source.workId)
            assertEquals(key.index, source.episodeIndex)
            assertEquals(5_123L, app.playback.state.value.session?.positionMs)
            assertEquals(5_123L, app.listening.state.value.snapshot.progress.single { it.episode == key }.positionMs)
            assertEquals(PlaybackStatus.Paused, app.playback.state.value.status)
            assertEquals(TheatreRoute.Player, app.navigation.value.route)
            assertEquals(1, engine.acceptedCommands.count { it.kind == MockCommandKind.Start })
        } finally {
            app.close()
        }
    }

    @Test
    fun delayed_progress_resolution_cannot_pause_a_newer_play_intent() = runTest {
        val inventory = mockInventory()
        val episode = requireNotNull(inventory.episode(key))
        val point = PlaybackSnapshot(requireNotNull(episode.toPlaybackSource()), 768_000L, 1000)
        val initial = ListeningSnapshot(progress = listOf(ProgressRecord(key, point.positionMs, episode.durationMs)))
        val gate = CompletableDeferred<ProgressOutcome>()
        val port = object : ProgressPort {
            override suspend fun load(): ListeningSnapshot = initial
            override suspend fun store(snapshot: ListeningSnapshot): ProgressOutcome = gate.await()
        }
        val engine = MockMediaEngine(acknowledgeAutomatically = false)
        val app = AppContainer(this, MockCatalogPort(inventory), port, engine, AppStartState(inventory, initial, point))
        try {
            advanceUntilIdle()
            app.playback.dispatch(PlaybackEvent.SeekRequested(5_123L))
            assertTrue(app.resumeEpisode(key))
            runCurrent()
            app.playback.dispatch(PlaybackEvent.PlayRequested)
            runCurrent()
            val newer = app.playback.state.value
            gate.complete(ProgressOutcome.Stored)
            advanceUntilIdle()
            assertEquals(newer, app.playback.state.value)
            assertEquals(PlaybackStatus.Loading, app.playback.state.value.status)
            assertEquals(5_123L, app.playback.state.value.session?.positionMs)
            assertEquals(TheatreRoute.Discovery, app.navigation.value.route)
            assertEquals(1, engine.acceptedCommands.count { it.kind == MockCommandKind.Start })
        } finally {
            app.close()
        }
    }

    @Test
    fun an_older_continue_row_is_refused_after_a_newer_episode_of_the_same_work_is_selected() = runTest {
        val engine = MockMediaEngine(acknowledgeAutomatically = false)
        val app = app(engine)
        try {
            advanceUntilIdle()
            val next = EpisodeKey(key.work, 1)
            assertTrue(app.selectEpisode(next))
            advanceUntilIdle()
            val player = app.playback.state.value
            val commands = engine.acceptedCommands
            assertFalse(app.resumeEpisode(key))
            advanceUntilIdle()
            assertEquals(player, app.playback.state.value)
            assertEquals(commands, engine.acceptedCommands)
        } finally {
            app.close()
        }
    }

    @Test
    fun refused_bookmark_does_not_navigate_or_seek_another_episode() = runTest {
        val engine = MockMediaEngine(acknowledgeAutomatically = false)
        val app = app(engine)
        try {
            advanceUntilIdle()
            val before = app.playback.state.value
            val commands = engine.acceptedCommands
            val navigation = app.navigation.value
            assertFalse(app.selectBookmark(EpisodeKey(WorkKey("unknown"), 0), 10_000L))
            assertFalse(app.selectBookmark(key, -1L))
            assertFalse(app.selectBookmark(key, 1_722_001L))
            advanceUntilIdle()
            assertEquals(before, app.playback.state.value)
            assertEquals(commands, engine.acceptedCommands)
            assertEquals(navigation, app.navigation.value)
            assertTrue(app.selectBookmark(EpisodeKey(WorkKey("den-khuya"), 0), 30_000L))
            advanceUntilIdle()
            assertEquals("den-khuya", app.playback.state.value.session?.source?.workId)
            assertEquals(30_000L, app.playback.state.value.session?.positionMs)
            assertEquals(TheatreRoute.Player, app.navigation.value.route)
        } finally {
            app.close()
        }
    }

    @Test
    fun bookmark_projection_and_open_intent_preserve_exact_milliseconds() = runTest {
        val engine = MockMediaEngine(acknowledgeAutomatically = false)
        val app = app(engine)
        try {
            advanceUntilIdle()
            val other = EpisodeKey(WorkKey("den-khuya"), 0)
            assertTrue(app.selectEpisode(other))
            advanceUntilIdle()
            engine.emit(PlaybackEvent.EnginePosition(requireNotNull(app.playback.state.value.identity), 30_123L))
            advanceUntilIdle()
            app.bookmark()
            advanceUntilIdle()
            val projection = projectDemoState(
                app.navigation.value, app.catalog.state.value, app.listening.state.value,
                app.playback.state.value, app.preferences.state.value,
                "", DemoLibraryTab.Bookmarks, DemoTimer.Off,
            )
            val intent = DemoEvent.OpenBookmark(projection.bookmarks.single())
            assertEquals(30, intent.bookmark.positionSeconds)
            assertEquals(30_123L, intent.bookmark.positionMs)
            assertEquals(30_123L, projection.positionMs)
            assertTrue(app.selectEpisode(key))
            advanceUntilIdle()
            assertTrue(app.selectBookmark(intent.bookmark.episodeId.toEpisodeKey(), intent.bookmark.positionMs))
            advanceUntilIdle()
            assertEquals(other.work.value, app.playback.state.value.session?.source?.workId)
            assertEquals(30_123L, app.playback.state.value.session?.positionMs)
        } finally {
            app.close()
        }
    }

    @Test
    fun reset_is_cold_and_late_fake_engine_events_cannot_restore_the_old_episode() = runTest {
        val engine = MockMediaEngine(acknowledgeAutomatically = false)
        val app = app(engine)
        try {
            advanceUntilIdle()
            app.selectEpisode(EpisodeKey(WorkKey("den-khuya"), 0))
            advanceUntilIdle()
            val old = requireNotNull(app.playback.state.value.identity)
            val starts = engine.acceptedCommands.count { it.kind == MockCommandKind.Start }
            app.reset()
            engine.acknowledgeStarted(old)
            engine.emit(PlaybackEvent.EnginePosition(old, 30_000L))
            advanceUntilIdle()
            val state = app.playback.state.value
            assertEquals(key.work.value, state.session?.source?.workId)
            assertEquals(768_000L, state.session?.positionMs)
            assertEquals(PlaybackStatus.Paused, state.status)
            assertTrue(state.generation > old.generation)
            assertEquals(starts, engine.acceptedCommands.count { it.kind == MockCommandKind.Start })
            assertEquals(listOf(TheatreRoute.Discovery), app.navigation.value.stack)
            assertEquals(listOf(key), app.listening.state.value.snapshot.progress.map { it.episode })
        } finally {
            app.close()
        }
    }

    @Test
    fun seek_then_bookmark_records_the_current_point_without_a_playback_flag() = runTest {
        val engine = MockMediaEngine(acknowledgeAutomatically = false)
        val app = app(engine)
        try {
            advanceUntilIdle()
            app.playback.dispatch(PlaybackEvent.SkipRequested(15_000))
            app.bookmark()
            app.bookmark()
            advanceUntilIdle()
            val snapshot = app.listening.state.value.snapshot
            assertEquals(1, snapshot.bookmarks.size)
            assertEquals(783_000L, snapshot.bookmarks.single().positionMs)
            assertEquals(783_000L, snapshot.progress.single().positionMs)
            assertEquals(PlaybackStatus.Paused, app.playback.state.value.status)
        } finally {
            app.close()
        }
    }

    @Test
    fun reset_discards_a_bookmark_capture_that_has_not_yet_crossed_to_listening() = runTest {
        val engine = MockMediaEngine(acknowledgeAutomatically = false)
        val app = app(engine)
        try {
            advanceUntilIdle()
            app.bookmark()
            app.reset()
            advanceUntilIdle()
            assertTrue(app.listening.state.value.snapshot.bookmarks.isEmpty())
            assertEquals(768_000L, app.playback.state.value.session?.positionMs)
            app.playback.dispatch(PlaybackEvent.SkipRequested(15_000))
            app.bookmark()
            advanceUntilIdle()
            assertEquals(783_000L, app.listening.state.value.snapshot.bookmarks.single().positionMs)
        } finally {
            app.close()
        }
    }

    @Test
    fun queued_pre_reset_playback_cannot_repopulate_the_reset_listening_snapshot() = runTest {
        val dispatcher = ControlledDispatcher()
        val inventory = mockInventory()
        val episode = requireNotNull(inventory.episode(key))
        val point = PlaybackSnapshot(requireNotNull(episode.toPlaybackSource()), 768_000L, 1000)
        val baseline = ListeningSnapshot(progress = listOf(ProgressRecord(key, point.positionMs, episode.durationMs)))
        val app = AppContainer(
            CoroutineScope(coroutineContext + dispatcher), MockCatalogPort(inventory), FakePersistentProgressPort(FakeStorageDatabase(baseline)),
            MockMediaEngine(acknowledgeAutomatically = false), AppStartState(inventory, baseline, point),
        )
        try {
            dispatcher.drain()
            val other = EpisodeKey(WorkKey("den-khuya"), 0)
            app.selectEpisode(other)
            dispatcher.drain(excluding = "theatre-playback-events")
            assertEquals(key.work.value, app.playback.state.value.session?.source?.workId)
            assertTrue(dispatcher.has("theatre-playback-events"))
            app.reset()
            // Commit queued old selections before reset, while the cross-owner fence is active.
            dispatcher.runNamed("theatre-playback-events")
            dispatcher.drain()
            assertEquals(baseline, app.listening.state.value.snapshot)
            assertEquals(key.work.value, app.playback.state.value.session?.source?.workId)
        } finally {
            app.close()
            dispatcher.drain()
        }
    }

    @Test
    fun disposal_stops_only_the_app_scope_and_refuses_late_callbacks() = runTest {
        val engine = MockMediaEngine(acknowledgeAutomatically = false)
        val app = app(engine)
        advanceUntilIdle()
        val snapshot = app.playback.state.value
        val listening = app.listening.state.value
        app.close()
        app.close()
        assertTrue(requireNotNull(coroutineContext[Job]).isActive)
        assertFalse(app.selectEpisode(key))
        assertFalse(app.resumeEpisode(key))
        assertFalse(app.resumeHistoryEpisode(key))
        app.setSaved(key.work, saved = true)
        app.clearHistory()
        assertFalse(engine.acknowledgeStarted(requireNotNull(snapshot.identity)))
        advanceUntilIdle()
        assertEquals(snapshot, app.playback.state.value)
        assertEquals(listening, app.listening.state.value)
    }

    /** Explicitly orders independent actors; no clock, sleep or scheduler timing assumption. */
    private class ControlledDispatcher : CoroutineDispatcher() {
        private data class Pending(val name: String?, val block: Runnable)
        private val pending = mutableListOf<Pending>()
        override fun dispatch(context: CoroutineContext, block: Runnable) {
            pending += Pending(context[CoroutineName]?.name, block)
        }
        fun has(name: String): Boolean = pending.any { it.name == name }
        fun runNamed(name: String) {
            val index = pending.indexOfFirst { it.name == name }
            check(index >= 0) { "Expected a queued $name task" }
            pending.removeAt(index).block.run()
        }
        fun drain(excluding: String? = null) {
            repeat(1000) {
                val index = pending.indexOfFirst { excluding == null || it.name != excluding }
                if (index < 0) return
                pending.removeAt(index).block.run()
            }
            error("Actor test did not become idle")
        }
    }
}
