package com.cantos.theatre.app

import com.cantos.theatre.adapters.mock.FakePersistentProgressPort
import com.cantos.theatre.adapters.mock.FakeStorageDatabase
import com.cantos.theatre.adapters.mock.FakeStorageState
import com.cantos.theatre.application.listening.ListeningState
import com.cantos.theatre.demo.DemoBookmark
import com.cantos.theatre.demo.DemoEpisodeId
import com.cantos.theatre.demo.DemoLibraryTab
import com.cantos.theatre.demo.DemoProgress
import com.cantos.theatre.demo.DemoTimer
import com.cantos.theatre.demo.DemoWorkId
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

/** Composition-root wiring over the fake database; JVM mock evidence, not process death. */
@OptIn(ExperimentalCoroutinesApi::class)
class FakeStorageSessionTest {
    private val sonHa = WorkKey("son-ha-ky")
    private val sonHa03 = EpisodeKey(sonHa, 2)
    private val sonHa03Ms = 1_722_000L

    @Test
    fun fresh_and_restored_sessions_seed_the_database_with_the_controllers_exact_start_snapshot() = runTest {
        val denKhuya01 = EpisodeKey(WorkKey("den-khuya"), 0)
        val points = listOf(ProgressRecord(denKhuya01, 91_000, 1_260_000), ProgressRecord(sonHa03, 768_000, sonHa03Ms))
        val expected = mapOf(
            FakeStorageSession.Fresh to ListeningSnapshot(),
            FakeStorageSession.Restored to ListeningSnapshot(
                savedWorks = setOf(SavedWork(sonHa), SavedWork(denKhuya01.work)),
                bookmarks = listOf(Bookmark(sonHa03, 600_000, sonHa03Ms)),
                progress = points,
                history = points.reversed(),
            ),
        )
        FakeStorageSession.entries.forEach { session ->
            val mount = mountFakeStorageSession(this, session)
            try {
                advanceUntilIdle()
                val snapshot = expected.getValue(session)
                assertEquals(ListeningState(snapshot), mount.app.listening.state.value)
                assertEquals(FakeStorageState(snapshot), mount.database.state.value)
            } finally {
                mount.app.close()
            }
        }
    }

    @Test
    fun a_recreated_app_on_the_same_database_restores_saved_progress_history_and_bookmark_after_reload() = runTest {
        val first = mountFakeStorageSession(this, FakeStorageSession.Fresh)
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

        val second = AppContainer.mock(this, FakeStorageSession.Fresh.seed()) { FakePersistentProgressPort(first.database) }
        try {
            advanceUntilIdle()
            assertEquals(ListeningState(ListeningSnapshot()), second.listening.state.value)
            assertFalse(second.resumeEpisode(sonHa03))
            second.listening.reload()
            advanceUntilIdle()
            assertEquals(ListeningState(stored), second.listening.state.value)
            val view = projectDemoState(
                second.navigation.value, second.catalog.state.value, second.listening.state.value,
                second.playback.state.value, second.preferences.state.value, "", DemoLibraryTab.Continuing, DemoTimer.Off,
            )
            assertEquals(setOf(DemoWorkId.SonHa), view.savedWorks)
            assertEquals(listOf(DemoProgress(DemoEpisodeId.SonHa03, 30)), view.progress)
            assertEquals(listOf(DemoProgress(DemoEpisodeId.SonHa03, 30)), view.history)
            assertEquals(listOf(DemoBookmark(DemoEpisodeId.SonHa03, 30)), view.bookmarks)
            assertTrue(second.resumeEpisode(sonHa03))
            advanceUntilIdle()
            assertEquals(30_000L, second.playback.state.value.session?.positionMs)
            assertEquals(PlaybackStatus.Paused, second.playback.state.value.status)
            assertEquals(TheatreRoute.Player, second.navigation.value.route)
        } finally {
            second.close()
        }

        val fresh = AppContainer.mock(this, FakeStorageSession.Fresh.seed()) { FakePersistentProgressPort(FakeStorageDatabase()) }
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
