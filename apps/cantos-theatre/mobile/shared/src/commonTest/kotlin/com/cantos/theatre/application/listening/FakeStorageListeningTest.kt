package com.cantos.theatre.application.listening

import com.cantos.theatre.adapters.mock.FakePersistentProgressPort
import com.cantos.theatre.adapters.mock.FakeStorageConfig
import com.cantos.theatre.adapters.mock.FakeStorageDatabase
import com.cantos.theatre.adapters.mock.FakeStorageState
import com.cantos.theatre.adapters.mock.WriteFaults
import com.cantos.theatre.domain.listening.Bookmark
import com.cantos.theatre.domain.listening.ListeningEvent
import com.cantos.theatre.domain.listening.ListeningSnapshot
import com.cantos.theatre.domain.listening.ProgressRecord
import com.cantos.theatre.domain.listening.SavedWork
import com.cantos.theatre.domain.publication.EpisodeKey
import com.cantos.theatre.domain.publication.WorkKey
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import kotlin.test.Test
import kotlin.test.assertEquals

/** The existing single-writer controller over the shared fake database; no durable claim. */
@OptIn(ExperimentalCoroutinesApi::class)
class FakeStorageListeningTest {
    private val work = WorkKey("son-ha-ky")
    private val episode = EpisodeKey(work, 2)
    private val duration = 1_722_000L

    private fun TestScope.owner(): CoroutineScope =
        CoroutineScope(backgroundScope.coroutineContext + SupervisorJob(backgroundScope.coroutineContext[Job]))

    private fun TestScope.advanceTo(timeMs: Long) {
        advanceTimeBy(timeMs - testScheduler.currentTime)
        runCurrent()
    }

    @Test
    fun a_recreated_controller_on_the_same_database_restores_every_record_only_after_explicit_reload() = runTest {
        val database = FakeStorageDatabase()
        val firstOwner = owner()
        val first = ListeningController(firstOwner, FakePersistentProgressPort(database))
        val started = ProgressRecord(episode, 0, duration)
        val listened = ProgressRecord(episode, 5_000, duration)
        val bookmark = Bookmark(episode, 4_000, duration)
        first.dispatch(ListeningEvent.SetSaved(work, saved = true))
        first.dispatch(ListeningEvent.RecordStarted(started))
        first.dispatch(ListeningEvent.RecordProgress(listened))
        first.dispatch(ListeningEvent.AddBookmark(bookmark))
        runCurrent()
        val expected = ListeningSnapshot(
            savedWorks = setOf(SavedWork(work)),
            bookmarks = listOf(bookmark),
            progress = listOf(listened),
            history = listOf(listened),
        )
        assertEquals(ListeningState(expected), first.state.value)
        assertEquals(FakeStorageState(expected, revision = 4), database.state.value)
        firstOwner.cancel()
        runCurrent()

        val second = ListeningController(owner(), FakePersistentProgressPort(database, FakeStorageConfig(readLatencyMs = 120)))
        runCurrent()
        assertEquals(ListeningState(ListeningSnapshot()), second.state.value)
        val reloadAt = testScheduler.currentTime
        second.reload()
        advanceTo(reloadAt + 119)
        assertEquals(ListeningState(ListeningSnapshot()), second.state.value)
        advanceTo(reloadAt + 120)
        assertEquals(ListeningState(expected), second.state.value)
        assertEquals(FakeStorageState(expected, revision = 4), database.state.value)

        val fresh = ListeningController(owner(), FakePersistentProgressPort(FakeStorageDatabase()))
        fresh.reload()
        runCurrent()
        assertEquals(ListeningState(ListeningSnapshot()), fresh.state.value)
    }

    @Test
    fun two_refused_writes_then_retry_commits_the_retained_intent_without_partial_state() = runTest {
        val initial = ListeningSnapshot(progress = listOf(ProgressRecord(episode, 1_000, duration)))
        val database = FakeStorageDatabase(initial)
        database.failNextWrites(2)
        val controller = ListeningController(backgroundScope, FakePersistentProgressPort(database), initial)
        controller.dispatch(ListeningEvent.SetSaved(work, saved = true))
        runCurrent()
        assertEquals(ListeningState(initial, ListeningFailure.MemoryStoreUnavailable), controller.state.value)
        assertEquals(FakeStorageState(initial, faults = WriteFaults.Next(1), refusedWrites = 1), database.state.value)
        controller.retry()
        runCurrent()
        assertEquals(ListeningState(initial, ListeningFailure.MemoryStoreUnavailable), controller.state.value)
        assertEquals(FakeStorageState(initial, refusedWrites = 2), database.state.value)
        controller.retry()
        runCurrent()
        val saved = ListeningSnapshot(setOf(SavedWork(work)), progress = initial.progress)
        assertEquals(ListeningState(saved), controller.state.value)
        assertEquals(FakeStorageState(saved, revision = 1, refusedWrites = 2), database.state.value)
        controller.retry()
        runCurrent()
        assertEquals(FakeStorageState(saved, revision = 1, refusedWrites = 2), database.state.value)
    }

    @Test
    fun storage_full_keeps_the_last_accepted_snapshot_and_recovery_retries_the_retained_user_action() = runTest {
        val database = FakeStorageDatabase()
        val controller = ListeningController(backgroundScope, FakePersistentProgressPort(database))
        val accepted = ProgressRecord(episode, 5_000, duration)
        controller.dispatch(ListeningEvent.RecordProgress(accepted))
        runCurrent()
        database.failAllWrites()
        val bookmark = Bookmark(episode, 5_000, duration)
        controller.dispatch(ListeningEvent.AddBookmark(bookmark))
        controller.dispatch(ListeningEvent.RecordProgress(ProgressRecord(episode, 6_000, duration)))
        runCurrent()
        val before = ListeningSnapshot(progress = listOf(accepted))
        assertEquals(ListeningState(before, ListeningFailure.MemoryStoreUnavailable), controller.state.value)
        assertEquals(FakeStorageState(before, revision = 1, faults = WriteFaults.All, refusedWrites = 2), database.state.value)
        database.recoverWrites()
        controller.retry()
        runCurrent()
        // Existing controller semantics: a pending user action outranks a refused automatic point.
        val after = ListeningSnapshot(bookmarks = listOf(bookmark), progress = listOf(accepted))
        assertEquals(ListeningState(after), controller.state.value)
        assertEquals(FakeStorageState(after, revision = 2, refusedWrites = 2), database.state.value)
    }

    @Test
    fun storage_reset_fences_the_in_flight_write_and_reload_never_restores_old_records() = runTest {
        val old = ListeningSnapshot(bookmarks = listOf(Bookmark(episode, 9_000, duration)))
        val database = FakeStorageDatabase(old)
        val controller = ListeningController(
            backgroundScope, FakePersistentProgressPort(database, FakeStorageConfig(writeLatencyMs = 100)), old,
        )
        controller.dispatch(ListeningEvent.SetSaved(work, saved = true))
        advanceTo(50)
        val replacement = ListeningSnapshot(progress = listOf(ProgressRecord(episode, 1_000, duration)))
        database.reset(replacement)
        advanceTo(100)
        assertEquals(ListeningState(old, ListeningFailure.MemoryStoreUnavailable), controller.state.value)
        assertEquals(FakeStorageState(replacement, epoch = 1, fencedWrites = 1), database.state.value)
        controller.reload()
        runCurrent()
        assertEquals(ListeningState(replacement, ListeningFailure.MemoryStoreUnavailable), controller.state.value)
        controller.retry()
        advanceTo(200)
        // Retry re-applies the retained intent to current state, never the pre-reset snapshot.
        val retried = ListeningSnapshot(setOf(SavedWork(work)), progress = replacement.progress)
        assertEquals(ListeningState(retried), controller.state.value)
        assertEquals(FakeStorageState(retried, epoch = 1, revision = 1, fencedWrites = 1), database.state.value)
    }

    @Test
    fun rapid_checkpoint_then_bookmark_commit_in_order_and_the_latest_accepted_snapshot_survives() = runTest {
        val database = FakeStorageDatabase()
        val controller = ListeningController(
            backgroundScope, FakePersistentProgressPort(database, FakeStorageConfig(writeLatencyMs = 200)),
        )
        val checkpoint = ProgressRecord(episode, 10_000, duration)
        val later = ProgressRecord(episode, 12_000, duration)
        val bookmark = Bookmark(episode, 12_000, duration)
        controller.dispatch(ListeningEvent.RecordProgress(checkpoint))
        controller.dispatch(ListeningEvent.AddBookmark(bookmark))
        controller.dispatch(ListeningEvent.RecordProgress(later))
        advanceTo(199)
        assertEquals(FakeStorageState(ListeningSnapshot()), database.state.value)
        advanceTo(200)
        assertEquals(FakeStorageState(ListeningSnapshot(progress = listOf(checkpoint)), revision = 1), database.state.value)
        advanceTo(400)
        assertEquals(
            FakeStorageState(ListeningSnapshot(bookmarks = listOf(bookmark), progress = listOf(checkpoint)), revision = 2),
            database.state.value,
        )
        advanceTo(600)
        val latest = ListeningSnapshot(bookmarks = listOf(bookmark), progress = listOf(later))
        assertEquals(ListeningState(latest), controller.state.value)
        assertEquals(FakeStorageState(latest, revision = 3), database.state.value)
    }

    @Test
    fun duplicate_save_and_bookmark_intents_after_restore_write_nothing_new() = runTest {
        val bookmark = Bookmark(episode, 4_000, duration)
        val stored = ListeningSnapshot(setOf(SavedWork(work)), bookmarks = listOf(bookmark))
        val database = FakeStorageDatabase(stored)
        val controller = ListeningController(backgroundScope, FakePersistentProgressPort(database))
        controller.reload()
        repeat(3) {
            controller.dispatch(ListeningEvent.SetSaved(work, saved = true))
            controller.dispatch(ListeningEvent.AddBookmark(bookmark))
        }
        controller.retry()
        runCurrent()
        assertEquals(ListeningState(stored), controller.state.value)
        assertEquals(FakeStorageState(stored), database.state.value)
    }
}
