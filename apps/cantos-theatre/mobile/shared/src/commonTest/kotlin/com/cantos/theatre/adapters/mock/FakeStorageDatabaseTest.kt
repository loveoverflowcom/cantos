package com.cantos.theatre.adapters.mock

import com.cantos.theatre.domain.listening.Bookmark
import com.cantos.theatre.domain.listening.ListeningSnapshot
import com.cantos.theatre.domain.listening.ProgressRecord
import com.cantos.theatre.domain.listening.SavedWork
import com.cantos.theatre.domain.publication.EpisodeKey
import com.cantos.theatre.domain.publication.WorkKey
import com.cantos.theatre.ports.ProgressOutcome
import com.cantos.theatre.ports.ProgressPort
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.async
import kotlinx.coroutines.cancel
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertIs
import kotlin.test.assertTrue

/**
 * Commit-order contract: every operation from every port sharing one database is served in
 * request order, with its latency inside the database's serialization boundary.
 */
@OptIn(ExperimentalCoroutinesApi::class)
class FakeStorageDatabaseTest {
    private val work = WorkKey("son-ha-ky")
    private val episode = EpisodeKey(work, 2)
    private val duration = 1_722_000L
    private val seed = ListeningSnapshot(savedWorks = setOf(SavedWork(work)))
    private val checkpoint10 = ListeningSnapshot(progress = listOf(ProgressRecord(episode, 10_000, duration)))
    private val bookmark12 = ListeningSnapshot(
        bookmarks = listOf(Bookmark(episode, 12_000, duration)),
        progress = listOf(ProgressRecord(episode, 12_000, duration)),
    )

    private fun TestScope.advanceTo(timeMs: Long) {
        advanceTimeBy(timeMs - testScheduler.currentTime)
        runCurrent()
    }

    @Test
    fun decide_write_fences_an_older_epoch_before_consuming_a_fault() {
        val current = FakeStorageState(seed, epoch = 1, faults = WriteFaults.Next(1))
        assertEquals(
            WriteCommit(FakeStorageState(seed, epoch = 1, faults = WriteFaults.Next(1), fencedWrites = 1), ProgressOutcome.Unavailable),
            decideWrite(current, requestEpoch = 0, snapshot = checkpoint10),
        )
    }

    @Test
    fun decide_write_consumes_one_fault_per_reached_write_and_never_changes_the_snapshot() {
        val first = decideWrite(FakeStorageState(seed, faults = WriteFaults.Next(2)), 0, checkpoint10)
        assertEquals(WriteCommit(FakeStorageState(seed, faults = WriteFaults.Next(1), refusedWrites = 1), ProgressOutcome.Unavailable), first)
        val second = decideWrite(first.state, 0, checkpoint10)
        assertEquals(WriteCommit(FakeStorageState(seed, refusedWrites = 2), ProgressOutcome.Unavailable), second)
        assertEquals(
            WriteCommit(FakeStorageState(checkpoint10, revision = 1, refusedWrites = 2), ProgressOutcome.Stored),
            decideWrite(second.state, 0, checkpoint10),
        )
        val full = FakeStorageState(seed, revision = 4, faults = WriteFaults.All)
        assertEquals(
            WriteCommit(FakeStorageState(seed, revision = 4, faults = WriteFaults.All, refusedWrites = 1), ProgressOutcome.Unavailable),
            decideWrite(full, 0, checkpoint10),
        )
    }

    @Test
    fun invalid_fault_counts_and_negative_latency_are_refused_without_changing_storage() = runTest {
        val database = FakeStorageDatabase(seed)
        assertFailsWith<IllegalArgumentException> { WriteFaults.Next(0) }
        assertFailsWith<IllegalArgumentException> { database.failNextWrites(-1) }
        assertFailsWith<IllegalArgumentException> { FakeStorageConfig(readLatencyMs = -1) }
        assertFailsWith<IllegalArgumentException> { FakeStorageConfig(writeLatencyMs = -1) }
        assertFailsWith<IllegalArgumentException> { database.read(latencyMs = -1) }
        assertFailsWith<IllegalArgumentException> { database.write(checkpoint10, latencyMs = -1) }
        assertEquals(FakeStorageState(seed), database.state.value)
    }

    @Test
    fun read_and_write_latency_elapse_on_the_virtual_clock_with_exact_outcomes() = runTest {
        val database = FakeStorageDatabase(seed)
        val port = FakePersistentProgressPort(database, FakeStorageConfig(readLatencyMs = 100, writeLatencyMs = 250))
        val write = async { port.store(checkpoint10) }
        advanceTo(249)
        assertFalse(write.isCompleted)
        assertEquals(FakeStorageState(seed), database.state.value)
        advanceTo(250)
        assertEquals(ProgressOutcome.Stored, write.await())
        assertEquals(FakeStorageState(checkpoint10, revision = 1), database.state.value)

        val read = async { port.load() }
        advanceTo(349)
        assertFalse(read.isCompleted)
        advanceTo(350)
        assertEquals(checkpoint10, read.await())
        assertEquals(FakeStorageState(checkpoint10, revision = 1), database.state.value)
    }

    @Test
    fun two_ports_sharing_one_database_commit_in_request_order_despite_faster_later_latency() = runTest {
        val database = FakeStorageDatabase(seed)
        val slow = FakePersistentProgressPort(database, FakeStorageConfig(writeLatencyMs = 300))
        val fast = FakePersistentProgressPort(database, FakeStorageConfig(writeLatencyMs = 50))
        val checkpoint = async { slow.store(checkpoint10) }
        runCurrent()
        val bookmark = async { fast.store(bookmark12) }
        advanceTo(299)
        assertEquals(FakeStorageState(seed), database.state.value)
        advanceTo(300)
        assertEquals(ProgressOutcome.Stored, checkpoint.await())
        assertFalse(bookmark.isCompleted)
        assertEquals(FakeStorageState(checkpoint10, revision = 1), database.state.value)
        advanceTo(349)
        assertEquals(FakeStorageState(checkpoint10, revision = 1), database.state.value)
        advanceTo(350)
        assertEquals(ProgressOutcome.Stored, bookmark.await())
        assertEquals(FakeStorageState(bookmark12, revision = 2), database.state.value)
    }

    @Test
    fun oracle_detects_the_per_port_mutex_counterexample_where_an_older_write_lands_last() = runTest {
        // Locks per port with latency before the database: the shape this simulator must avoid.
        val database = FakeStorageDatabase(seed)
        val slow = PerPortLockPort(database, latencyMs = 300)
        val fast = PerPortLockPort(database, latencyMs = 50)
        val checkpoint = async { slow.store(checkpoint10) }
        runCurrent()
        val bookmark = async { fast.store(bookmark12) }
        advanceTo(300)
        assertEquals(listOf(ProgressOutcome.Stored, ProgressOutcome.Stored), listOf(checkpoint.await(), bookmark.await()))
        assertEquals(FakeStorageState(checkpoint10, revision = 2), database.state.value)
    }

    @Test
    fun an_injected_failure_goes_to_the_first_requested_write_not_the_first_to_finish() = runTest {
        val database = FakeStorageDatabase(seed)
        database.failNextWrites(1)
        val slow = FakePersistentProgressPort(database, FakeStorageConfig(writeLatencyMs = 300))
        val fast = FakePersistentProgressPort(database, FakeStorageConfig(writeLatencyMs = 50))
        val checkpoint = async { slow.store(checkpoint10) }
        runCurrent()
        val bookmark = async { fast.store(bookmark12) }
        advanceTo(300)
        assertEquals(ProgressOutcome.Unavailable, checkpoint.await())
        assertEquals(FakeStorageState(seed, refusedWrites = 1), database.state.value)
        advanceTo(350)
        assertEquals(ProgressOutcome.Stored, bookmark.await())
        assertEquals(FakeStorageState(bookmark12, revision = 1, refusedWrites = 1), database.state.value)
    }

    @Test
    fun a_refused_newer_write_leaves_the_last_accepted_snapshot_whole() = runTest {
        val database = FakeStorageDatabase(seed)
        val slow = FakePersistentProgressPort(database, FakeStorageConfig(writeLatencyMs = 300))
        val fast = FakePersistentProgressPort(database, FakeStorageConfig(writeLatencyMs = 50))
        val checkpoint = async { slow.store(checkpoint10) }
        runCurrent()
        val bookmark = async { fast.store(bookmark12) }
        advanceTo(320)
        database.failNextWrites(1)
        advanceTo(350)
        assertEquals(listOf(ProgressOutcome.Stored, ProgressOutcome.Unavailable), listOf(checkpoint.await(), bookmark.await()))
        assertEquals(FakeStorageState(checkpoint10, revision = 1, refusedWrites = 1), database.state.value)
    }

    @Test
    fun reset_fences_in_flight_and_queued_writes_and_fenced_writes_consume_no_fault() = runTest {
        val database = FakeStorageDatabase(seed)
        database.failNextWrites(1)
        val port = FakePersistentProgressPort(database, FakeStorageConfig(writeLatencyMs = 100))
        val inFlight = async { port.store(checkpoint10) }
        advanceTo(10)
        val queued = async { port.store(bookmark12) }
        advanceTo(50)
        val replacement = ListeningSnapshot(progress = listOf(ProgressRecord(episode, 1_000, duration)))
        database.reset(replacement)
        assertEquals(FakeStorageState(replacement, epoch = 1, faults = WriteFaults.Next(1)), database.state.value)
        advanceTo(200)
        assertEquals(listOf(ProgressOutcome.Unavailable, ProgressOutcome.Unavailable), listOf(inFlight.await(), queued.await()))
        assertEquals(
            FakeStorageState(replacement, epoch = 1, faults = WriteFaults.Next(1), fencedWrites = 2),
            database.state.value,
        )

        val refused = async { port.store(checkpoint10) }
        advanceTo(300)
        val accepted = async { port.store(bookmark12) }
        advanceTo(400)
        assertEquals(listOf(ProgressOutcome.Unavailable, ProgressOutcome.Stored), listOf(refused.await(), accepted.await()))
        assertEquals(
            FakeStorageState(bookmark12, epoch = 1, revision = 1, refusedWrites = 1, fencedWrites = 2),
            database.state.value,
        )
    }

    @Test
    fun cancellation_while_delaying_or_waiting_propagates_changes_nothing_and_consumes_no_fault() = runTest {
        val database = FakeStorageDatabase(seed)
        database.failNextWrites(1)
        val port = FakePersistentProgressPort(database, FakeStorageConfig(writeLatencyMs = 100))
        val delaying = async { port.store(checkpoint10) }
        runCurrent()
        val waiting = async { port.store(bookmark12) }
        advanceTo(50)
        delaying.cancel()
        waiting.cancel()
        advanceTo(400)
        assertTrue(delaying.isCancelled && waiting.isCancelled)
        assertIs<CancellationException>(delaying.getCompletionExceptionOrNull())
        assertIs<CancellationException>(waiting.getCompletionExceptionOrNull())
        assertEquals(FakeStorageState(seed, faults = WriteFaults.Next(1)), database.state.value)

        val refused = async { port.store(checkpoint10) }
        runCurrent()
        val accepted = async { port.store(bookmark12) }
        advanceTo(600)
        assertEquals(listOf(ProgressOutcome.Unavailable, ProgressOutcome.Stored), listOf(refused.await(), accepted.await()))
        assertEquals(FakeStorageState(bookmark12, revision = 1, refusedWrites = 1), database.state.value)
    }

    @Test
    fun a_cancelled_caller_never_commits_through_the_uncontended_zero_latency_path() = runTest {
        val database = FakeStorageDatabase(seed)
        database.failNextWrites(1)
        val port = FakePersistentProgressPort(database)
        val write = launch {
            currentCoroutineContext().cancel()
            port.store(checkpoint10)
        }
        runCurrent()
        assertTrue(write.isCancelled)
        assertEquals(FakeStorageState(seed, faults = WriteFaults.Next(1)), database.state.value)
    }

    @Test
    fun reads_are_served_in_request_order_and_see_only_whole_committed_snapshots() = runTest {
        val database = FakeStorageDatabase(seed)
        val reader = FakePersistentProgressPort(database, FakeStorageConfig(readLatencyMs = 100))
        val writer = FakePersistentProgressPort(database, FakeStorageConfig(writeLatencyMs = 200))
        val before = async { reader.load() }
        runCurrent()
        val write = async { writer.store(bookmark12) }
        runCurrent()
        val after = async { reader.load() }
        advanceTo(100)
        assertEquals(seed, before.await())
        assertFalse(write.isCompleted || after.isCompleted)
        advanceTo(300)
        assertEquals(ProgressOutcome.Stored, write.await())
        assertFalse(after.isCompleted)
        advanceTo(400)
        assertEquals(bookmark12, after.await())
    }

    @Test
    fun recovering_from_storage_full_accepts_the_next_write() = runTest {
        val database = FakeStorageDatabase(seed)
        val port = FakePersistentProgressPort(database)
        database.failAllWrites()
        assertEquals(ProgressOutcome.Unavailable, port.store(checkpoint10))
        assertEquals(ProgressOutcome.Unavailable, port.store(bookmark12))
        assertEquals(FakeStorageState(seed, faults = WriteFaults.All, refusedWrites = 2), database.state.value)
        database.recoverWrites()
        assertEquals(ProgressOutcome.Stored, port.store(bookmark12))
        assertEquals(FakeStorageState(bookmark12, revision = 1, refusedWrites = 2), database.state.value)
    }

    private class PerPortLockPort(private val database: FakeStorageDatabase, private val latencyMs: Long) : ProgressPort {
        private val lock = Mutex()

        override suspend fun load(): ListeningSnapshot = lock.withLock {
            delay(latencyMs)
            database.read()
        }

        override suspend fun store(snapshot: ListeningSnapshot): ProgressOutcome = lock.withLock {
            delay(latencyMs)
            database.write(snapshot)
        }
    }
}
