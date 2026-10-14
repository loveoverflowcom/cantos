package com.cantos.theatre.adapters.mock

import com.cantos.theatre.domain.listening.ListeningSnapshot
import com.cantos.theatre.domain.listening.ProgressRecord
import com.cantos.theatre.domain.publication.EpisodeKey
import com.cantos.theatre.domain.publication.WorkKey
import com.cantos.theatre.ports.ProgressOutcome
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withContext
import org.junit.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

/** Real JVM threads: counters stay exact under contention. Not a proof of thread safety. */
class FakeStorageContentionTest {
    @Test
    fun real_thread_writes_across_ports_lose_no_commit_and_consume_exactly_the_injected_faults() = runBlocking {
        val database = FakeStorageDatabase()
        database.failNextWrites(100)
        val ports = List(4) { FakePersistentProgressPort(database) }
        val episode = EpisodeKey(WorkKey("son-ha-ky"), 2)
        val written = List(1_000) { index ->
            ListeningSnapshot(progress = listOf(ProgressRecord(episode, index.toLong(), 1_722_000)))
        }
        val outcomes = withContext(Dispatchers.Default) {
            written.mapIndexed { index, snapshot -> async { ports[index % ports.size].store(snapshot) } }.awaitAll()
        }
        assertEquals(900, outcomes.count { it == ProgressOutcome.Stored })
        assertEquals(100, outcomes.count { it == ProgressOutcome.Unavailable })
        val final = database.state.value
        assertEquals(FakeStorageState(final.snapshot, revision = 900, refusedWrites = 100), final)
        assertTrue(final.snapshot in written)
    }
}
