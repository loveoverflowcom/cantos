package com.cantos.theatre.application.listening

import com.cantos.theatre.adapters.mock.MemoryProgressPort
import com.cantos.theatre.domain.listening.Bookmark
import com.cantos.theatre.domain.listening.ListeningEvent
import com.cantos.theatre.domain.listening.ListeningSnapshot
import com.cantos.theatre.domain.listening.ProgressRecord
import com.cantos.theatre.domain.listening.SavedWork
import com.cantos.theatre.domain.publication.EpisodeKey
import com.cantos.theatre.domain.publication.WorkKey
import com.cantos.theatre.ports.ProgressOutcome
import com.cantos.theatre.ports.ProgressPort
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.Job
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.withContext
import kotlin.test.Test
import kotlin.test.assertEquals

@OptIn(ExperimentalCoroutinesApi::class)
class ListeningControllerTest {
    private val work = WorkKey("son-ha-ky")
    private val episode = EpisodeKey(work, 0)

    @Test
    fun memory_adapter_and_controller_record_independent_values_and_reset() = runTest {
        val initial = ListeningSnapshot(savedWorks = setOf(SavedWork(work)))
        val port = MemoryProgressPort(initial)
        val controller = ListeningController(backgroundScope, port, initial)
        val bookmark = Bookmark(episode, 20_000, 60_000)
        val progress = ProgressRecord(episode, 5_000, 60_000)
        controller.dispatch(ListeningEvent.AddBookmark(bookmark))
        controller.dispatch(ListeningEvent.RecordProgress(progress))
        runCurrent()
        val expected = ListeningSnapshot(setOf(SavedWork(work)), listOf(bookmark), listOf(progress))
        assertEquals(ListeningState(expected), controller.state.value)
        assertEquals(expected, port.load())
        controller.reset()
        runCurrent()
        assertEquals(ListeningState(initial), controller.state.value)
        assertEquals(initial, port.load())
    }

    @Test
    fun duplicate_bookmark_and_progress_commands_do_not_repeat_store_effects() = runTest {
        val port = ScriptedProgressPort()
        val controller = ListeningController(backgroundScope, port)
        val bookmark = Bookmark(episode, 20_000, 60_000)
        val progress = ProgressRecord(episode, 5_000, 60_000)
        repeat(3) { controller.dispatch(ListeningEvent.AddBookmark(bookmark)) }
        repeat(3) { controller.dispatch(ListeningEvent.RecordProgress(progress)) }
        runCurrent()
        assertEquals(2, port.writes.size)
        assertEquals(ListeningState(ListeningSnapshot(bookmarks = listOf(bookmark), progress = listOf(progress))), controller.state.value)
    }

    @Test
    fun unavailable_store_preserves_accepted_state_and_retry_has_exact_outcome() = runTest {
        val port = ScriptedProgressPort()
        port.outcome = ProgressOutcome.Unavailable
        val initial = ListeningSnapshot()
        val controller = ListeningController(backgroundScope, port, initial)
        val event = ListeningEvent.ToggleSaved(work)
        controller.dispatch(event)
        runCurrent()
        assertEquals(ListeningState(initial, ListeningFailure.MemoryStoreUnavailable), controller.state.value)
        port.outcome = ProgressOutcome.Stored
        controller.dispatch(event)
        runCurrent()
        val accepted = ListeningSnapshot(savedWorks = setOf(SavedWork(work)))
        assertEquals(ListeningState(accepted), controller.state.value)
        assertEquals(listOf(accepted, accepted), port.writes)
    }

    @Test
    fun delayed_store_effects_run_in_order_and_reset_is_not_overwritten() = runTest {
        val port = ScriptedProgressPort()
        val pending = CompletableDeferred<ProgressOutcome>()
        port.pendingStore = pending
        val controller = ListeningController(backgroundScope, port)
        controller.dispatch(ListeningEvent.ToggleSaved(work))
        controller.reset()
        runCurrent()
        assertEquals(1, port.writes.size)
        assertEquals(ListeningState(ListeningSnapshot()), controller.state.value)
        port.pendingStore = null
        pending.complete(ProgressOutcome.Stored)
        runCurrent()
        assertEquals(2, port.writes.size)
        assertEquals(ListeningState(ListeningSnapshot()), controller.state.value)
        assertEquals(ListeningSnapshot(), port.load())
    }

    @Test
    fun cancelled_owner_does_not_publish_late_noncooperative_store_result() = runTest {
        val port = CancellationResistantProgressPort()
        val pending = CompletableDeferred<ProgressOutcome>()
        port.storeGate = pending
        val owner = CoroutineScope(backgroundScope.coroutineContext + SupervisorJob(backgroundScope.coroutineContext[Job]))
        val initial = ListeningSnapshot()
        val controller = ListeningController(owner, port, initial)
        controller.dispatch(ListeningEvent.ToggleSaved(work))
        runCurrent()
        owner.cancel()
        runCurrent()
        pending.complete(ProgressOutcome.Stored)
        runCurrent()
        assertEquals(1, port.completedStores)
        assertEquals(ListeningState(initial), controller.state.value)
    }

    @Test
    fun cancelled_owner_does_not_publish_late_noncooperative_reload_result() = runTest {
        val port = CancellationResistantProgressPort()
        val pending = CompletableDeferred<ListeningSnapshot>()
        port.readGate = pending
        val owner = CoroutineScope(backgroundScope.coroutineContext + SupervisorJob(backgroundScope.coroutineContext[Job]))
        val initial = ListeningSnapshot()
        val controller = ListeningController(owner, port, initial)
        controller.reload()
        runCurrent()
        owner.cancel()
        runCurrent()
        pending.complete(ListeningSnapshot(savedWorks = setOf(SavedWork(work))))
        runCurrent()
        assertEquals(1, port.completedReads)
        assertEquals(ListeningState(initial), controller.state.value)
    }

    private class CancellationResistantProgressPort : ProgressPort {
        var readGate: CompletableDeferred<ListeningSnapshot>? = null
        var storeGate: CompletableDeferred<ProgressOutcome>? = null
        var completedReads: Int = 0
            private set
        var completedStores: Int = 0
            private set

        override suspend fun load(): ListeningSnapshot {
            val gate = readGate
            val snapshot = if (gate == null) ListeningSnapshot() else withContext(NonCancellable) { gate.await() }
            completedReads += 1
            return snapshot
        }

        override suspend fun store(snapshot: ListeningSnapshot): ProgressOutcome {
            val gate = storeGate
            val outcome = if (gate == null) ProgressOutcome.Stored else withContext(NonCancellable) { gate.await() }
            completedStores += 1
            return outcome
        }
    }

    private class ScriptedProgressPort : ProgressPort {
        var outcome: ProgressOutcome = ProgressOutcome.Stored
        var pendingStore: CompletableDeferred<ProgressOutcome>? = null
        val writes = mutableListOf<ListeningSnapshot>()
        private var accepted = ListeningSnapshot()

        override suspend fun load(): ListeningSnapshot = accepted

        override suspend fun store(snapshot: ListeningSnapshot): ProgressOutcome {
            writes.add(snapshot)
            val result = pendingStore?.await() ?: outcome
            if (result == ProgressOutcome.Stored) accepted = snapshot
            return result
        }
    }
}
