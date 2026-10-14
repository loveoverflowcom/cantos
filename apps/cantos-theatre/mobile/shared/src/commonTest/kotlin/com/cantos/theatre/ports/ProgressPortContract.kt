package com.cantos.theatre.ports

import com.cantos.theatre.domain.listening.Bookmark
import com.cantos.theatre.domain.listening.ListeningSnapshot
import com.cantos.theatre.domain.listening.ProgressRecord
import com.cantos.theatre.domain.listening.SavedWork
import com.cantos.theatre.domain.publication.EpisodeKey
import com.cantos.theatre.domain.publication.WorkKey
import kotlinx.coroutines.cancel
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.runTest
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

/**
 * Adapter-agnostic [ProgressPort] behavior (ADR 0026). Every adapter binds [openStore] and
 * passes these cases on each target it claims. A fake's harness controls (latency, injected
 * faults, reset fencing) stay in that fake's own tests.
 */
abstract class ProgressPortContract {
    /**
     * Creates a new store holding only [seed]. The returned function opens another connection
     * to that same store; separate calls create isolated stores.
     */
    protected abstract fun openStore(seed: ListeningSnapshot): () -> ProgressPort

    private val sonHa = WorkKey("son-ha-ky")
    // Any nonblank key is valid, so stored text must survive exactly, including Vietnamese.
    private val bell = WorkKey("Người giữ tiếng chuông")
    private val sonHa2 = EpisodeKey(sonHa, 2)
    private val bellLast = EpisodeKey(bell, Int.MAX_VALUE)
    private val duration = 1_722_000L

    // Orders sorted by neither key nor position, opposite between progress and history; a
    // position above 2^53 detects a floating-point encoding.
    private val rich = ListeningSnapshot(
        savedWorks = setOf(SavedWork(bell), SavedWork(sonHa)),
        bookmarks = listOf(
            Bookmark(sonHa2, 12_999, duration),
            Bookmark(EpisodeKey(bell, 0), 0, 1),
            Bookmark(sonHa2, 12_001, duration),
        ),
        progress = listOf(
            ProgressRecord(sonHa2, 12_001, duration),
            ProgressRecord(bellLast, Long.MAX_VALUE - 1, Long.MAX_VALUE),
        ),
        history = listOf(
            ProgressRecord(bellLast, Long.MAX_VALUE - 1, Long.MAX_VALUE),
            ProgressRecord(sonHa2, 12_001, duration),
        ),
    )

    @Test
    fun a_new_store_loads_exactly_its_seed() = runTest {
        assertEquals(ListeningSnapshot(), openStore(ListeningSnapshot())().load())
        assertEquals(rich, openStore(rich)().load())
    }

    @Test
    fun a_stored_snapshot_round_trips_every_record_with_exact_values_and_order() = runTest {
        val connect = openStore(ListeningSnapshot())
        val port = connect()
        assertEquals(ProgressOutcome.Stored, port.store(rich))
        assertEquals(rich, port.load())
        assertEquals(rich, connect().load())
    }

    @Test
    fun a_store_replaces_the_whole_snapshot_and_removed_records_never_return() = runTest {
        val connect = openStore(rich)
        // Unsave, delete bookmarks, clear history and rewind: no merge or maximum position survives.
        val replaced = ListeningSnapshot(
            savedWorks = setOf(SavedWork(sonHa)),
            progress = listOf(ProgressRecord(sonHa2, 1_000, duration)),
        )
        assertEquals(ProgressOutcome.Stored, connect().store(replaced))
        assertEquals(replaced, connect().load())
    }

    @Test
    fun a_reopened_connection_reads_the_last_stored_snapshot_and_writes_through() = runTest {
        val connect = openStore(ListeningSnapshot())
        val first = connect()
        val written = ListeningSnapshot(savedWorks = setOf(SavedWork(sonHa)))
        assertEquals(ProgressOutcome.Stored, first.store(written))
        val second = connect()
        assertEquals(written, second.load())
        assertEquals(ProgressOutcome.Stored, second.store(rich))
        assertEquals(rich, first.load())
        assertEquals(rich, connect().load())
    }

    @Test
    fun separate_stores_never_share_records() = runTest {
        val written = openStore(ListeningSnapshot())
        val untouched = openStore(ListeningSnapshot())
        assertEquals(ProgressOutcome.Stored, written().store(rich))
        assertEquals(ListeningSnapshot(), untouched().load())
        assertEquals(rich, written().load())
    }

    @Test
    fun a_caller_cancelled_before_storing_commits_nothing() = runTest {
        val connect = openStore(rich)
        val port = connect()
        val write = launch {
            currentCoroutineContext().cancel()
            port.store(ListeningSnapshot())
        }
        write.join()
        assertTrue(write.isCancelled)
        assertEquals(rich, connect().load())
    }
}
