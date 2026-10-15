package com.cantos.theatre.app

import com.cantos.theatre.demo.DemoWorkIds
import com.cantos.theatre.demo.DemoEpisodeIds
import com.cantos.theatre.adapters.mock.sandbox.database.MockStorageDatabase
import com.cantos.theatre.adapters.mock.sandbox.database.MockStorageState
import com.cantos.theatre.adapters.mock.sandbox.sessions.MockStorageSession
import com.cantos.theatre.adapters.mock.sandbox.sessions.seed
import com.cantos.theatre.application.listening.ListeningState
import com.cantos.theatre.presentation.model.ShellBookmark
import com.cantos.theatre.presentation.model.ShellEpisodeId
import com.cantos.theatre.presentation.model.ShellLibraryTab
import com.cantos.theatre.presentation.model.ShellProgress
import com.cantos.theatre.presentation.model.ShellTimer
import com.cantos.theatre.presentation.model.ShellWorkId
import com.cantos.theatre.domain.listening.Bookmark
import com.cantos.theatre.domain.listening.ListeningSnapshot
import com.cantos.theatre.domain.listening.ProgressRecord
import com.cantos.theatre.domain.listening.SavedWork
import com.cantos.theatre.domain.playback.PlaybackEvent
import com.cantos.theatre.domain.playback.PlaybackStatus
import com.cantos.theatre.domain.publication.EpisodeKey
import com.cantos.theatre.domain.publication.WorkKey
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runTest
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

/** Composition-root wiring over the mock database; JVM mock evidence, not process death. */
@OptIn(ExperimentalCoroutinesApi::class)
class MockStorageSessionTest {
    private val sonHa = WorkKey("son-ha-ky")
    private val sonHa03 = EpisodeKey(sonHa, 2)
    private val sonHa03Ms = 1_722_000L

    @Test
    fun every_preset_seeds_the_database_with_the_controllers_exact_start_snapshot() = runTest {
        val denKhuya01 = EpisodeKey(WorkKey("den-khuya"), 0)
        val points = listOf(ProgressRecord(denKhuya01, 91_000, 1_260_000), ProgressRecord(sonHa03, 768_000, sonHa03Ms))
        val expected = mapOf(
            MockStorageSession.Demo to ListeningSnapshot(
                savedWorks = setOf(SavedWork(sonHa)),
                progress = listOf(ProgressRecord(sonHa03, 768_000, sonHa03Ms)),
            ),
            MockStorageSession.Fresh to ListeningSnapshot(),
            MockStorageSession.Restored to ListeningSnapshot(
                savedWorks = setOf(SavedWork(sonHa), SavedWork(denKhuya01.work)),
                bookmarks = listOf(Bookmark(sonHa03, 600_000, sonHa03Ms)),
                progress = points,
                history = points.reversed(),
            ),
        )
        MockStorageSession.entries.forEach { session ->
            val mount = mountMockStorageSession(this, session)
            try {
                advanceUntilIdle()
                val snapshot = expected.getValue(session)
                assertEquals(ListeningState(snapshot), mount.app.listening.state.value)
                assertEquals(MockStorageState(snapshot), mount.database.state.value)
            } finally {
                mount.app.close()
            }
        }
    }

    @Test
    fun the_default_mock_mount_is_the_demo_preset_and_stores_through_its_database() = runTest {
        var database: MockStorageDatabase? = null
        val default = mockAppContainer(this) { start -> MockStorageDatabase(start).also { database = it } }
        val demo = mountMockStorageSession(this, MockStorageSession.Demo)
        try {
            advanceUntilIdle()
            assertEquals(demo.app.listening.state.value, default.listening.state.value)
            assertEquals(demo.app.navigation.value, default.navigation.value)
            assertEquals(demo.app.playback.state.value.session?.snapshot(), default.playback.state.value.session?.snapshot())
            default.setSaved(WorkKey("den-khuya"), saved = true)
            advanceUntilIdle()
            val stored = checkNotNull(database).state.value
            assertEquals(1L, stored.revision)
            assertEquals(default.listening.state.value.snapshot, stored.snapshot)
            assertEquals(0L, demo.database.state.value.revision)
        } finally {
            default.close()
            demo.app.close()
        }
    }

    @Test
    fun a_recreated_app_on_the_same_database_restores_saved_progress_history_and_bookmark_after_reload() = runTest {
        val first = mountMockStorageSession(this, MockStorageSession.Fresh)
        try {
            advanceUntilIdle()
            first.app.setSaved(sonHa, saved = true)
            assertTrue(first.app.selectEpisode(sonHa03))
            advanceUntilIdle()
            first.app.playback.dispatch(PlaybackEvent.SeekRequested(30_000))
            advanceUntilIdle()
            first.app.bookmark()
            first.app.playback.dispatch(PlaybackEvent.PauseRequested)
            advanceUntilIdle()
        } finally {
            first.app.close()
        }
        val point = ProgressRecord(sonHa03, 30_000, sonHa03Ms)
        val stored = ListeningSnapshot(
            savedWorks = setOf(SavedWork(sonHa)),
            bookmarks = listOf(Bookmark(sonHa03, 30_000, sonHa03Ms)),
            progress = listOf(point),
            history = listOf(point),
        )
        assertEquals(stored, first.database.state.value.snapshot)

        val second = mockAppContainer(this, MockStorageSession.Fresh.seed()) { first.database }
        try {
            advanceUntilIdle()
            assertEquals(ListeningState(ListeningSnapshot()), second.listening.state.value)
            assertFalse(second.resumeEpisode(sonHa03))
            second.listening.reload()
            advanceUntilIdle()
            assertEquals(ListeningState(stored), second.listening.state.value)
            val view = projectDemoState(
                second.navigation.value, second.catalog.state.value, second.listening.state.value,
                second.playback.state.value, second.preferences.state.value, "", ShellLibraryTab.Continuing, ShellTimer.Off,
            )
            assertEquals(setOf(DemoWorkIds.SonHa), view.savedWorks)
            assertEquals(listOf(ShellProgress(DemoEpisodeIds.SonHa03, 30)), view.progress)
            assertEquals(listOf(ShellProgress(DemoEpisodeIds.SonHa03, 30)), view.history)
            assertEquals(listOf(ShellBookmark(DemoEpisodeIds.SonHa03, 30)), view.bookmarks)
            assertTrue(second.resumeEpisode(sonHa03))
            advanceUntilIdle()
            assertEquals(30_000L, second.playback.state.value.session?.positionMs)
            assertEquals(PlaybackStatus.Paused, second.playback.state.value.status)
            assertEquals(TheatreRoute.Player, second.navigation.value.route)
        } finally {
            second.close()
        }

        val fresh = mockAppContainer(this, MockStorageSession.Fresh.seed()) { MockStorageDatabase() }
        try {
            fresh.listening.reload()
            advanceUntilIdle()
            assertEquals(ListeningState(ListeningSnapshot()), fresh.listening.state.value)
            assertFalse(fresh.resumeEpisode(sonHa03))
        } finally {
            fresh.close()
        }
    }
}
