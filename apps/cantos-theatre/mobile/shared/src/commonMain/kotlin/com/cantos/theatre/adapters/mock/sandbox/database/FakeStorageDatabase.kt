package com.cantos.theatre.adapters.mock.sandbox.database

import com.cantos.theatre.domain.listening.ListeningSnapshot
import com.cantos.theatre.ports.ProgressOutcome
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.delay
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock

/** Storage-level write faults, shared by every port and consumed only at a write's commit point. */
sealed interface WriteFaults {
    data object None : WriteFaults
    data class Next(val count: Int) : WriteFaults {
        init {
            require(count > 0) { "A fail-next-writes fault must refuse at least one write" }
        }
    }
    /** Simulated storage full: every write is refused until recovered. */
    data object All : WriteFaults
}

/**
 * Read-only inspection of one fake database. [epoch] increases on reset and fences writes
 * requested before it; [revision] counts committed writes and never decreases.
 */
@ConsistentCopyVisibility
data class FakeStorageState internal constructor(
    val snapshot: ListeningSnapshot,
    val epoch: Long = 0L,
    val revision: Long = 0L,
    val faults: WriteFaults = WriteFaults.None,
    val refusedWrites: Long = 0L,
    val fencedWrites: Long = 0L,
)

internal data class WriteCommit(val state: FakeStorageState, val outcome: ProgressOutcome)

/** Pure commit decision: fence an older epoch, then consume a fault, then accept the snapshot. */
internal fun decideWrite(state: FakeStorageState, requestEpoch: Long, snapshot: ListeningSnapshot): WriteCommit =
    if (requestEpoch != state.epoch) {
        WriteCommit(state.copy(fencedWrites = state.fencedWrites + 1), ProgressOutcome.Unavailable)
    } else {
        when (val faults = state.faults) {
            WriteFaults.None -> {
                check(state.revision < Long.MAX_VALUE) { "Fake storage revision exhausted" }
                WriteCommit(state.copy(snapshot = snapshot, revision = state.revision + 1), ProgressOutcome.Stored)
            }
            is WriteFaults.Next -> WriteCommit(
                state.copy(
                    faults = if (faults.count == 1) WriteFaults.None else WriteFaults.Next(faults.count - 1),
                    refusedWrites = state.refusedWrites + 1,
                ),
                ProgressOutcome.Unavailable,
            )
            WriteFaults.All -> WriteCommit(state.copy(refusedWrites = state.refusedWrites + 1), ProgressOutcome.Unavailable)
        }
    }

/**
 * A memory-only storage simulator shared by any number of ports in one session. Process death
 * discards it; nothing here is durable, migrated or synchronized.
 *
 * Every read and write, including its latency, runs inside one fair (FIFO) mutex owned by the
 * database, so operations from all ports commit in the order they were requested and a slow
 * older write cannot land after a newer one. Harness controls are atomic state replacements
 * and never wait for the queue: [reset] immediately fences every write requested before it.
 */
class FakeStorageDatabase(seed: ListeningSnapshot = ListeningSnapshot()) {
    private val serial = Mutex()
    private val mutableState = MutableStateFlow(FakeStorageState(seed))
    val state: StateFlow<FakeStorageState> = mutableState.asStateFlow()

    /** Reads the snapshot committed when its turn and latency end. */
    suspend fun read(latencyMs: Long = 0L): ListeningSnapshot {
        require(latencyMs >= 0L) { "Fake storage latency must be nonnegative" }
        return serial.withLock {
            delay(latencyMs)
            currentCoroutineContext().ensureActive()
            mutableState.value.snapshot
        }
    }

    /**
     * Commits [snapshot] whole or not at all. Cancellation before the commit point propagates,
     * changes nothing and consumes no fault; a reset after the request fences it as unavailable.
     */
    suspend fun write(snapshot: ListeningSnapshot, latencyMs: Long = 0L): ProgressOutcome {
        require(latencyMs >= 0L) { "Fake storage latency must be nonnegative" }
        val requestEpoch = mutableState.value.epoch
        return serial.withLock {
            delay(latencyMs)
            // Mutex does not check cancellation on its uncontended path; never write after cancel.
            currentCoroutineContext().ensureActive()
            commit(requestEpoch, snapshot)
        }
    }

    /** Replace the contents and fence in-flight and queued writes from the previous epoch. */
    fun reset(seed: ListeningSnapshot = ListeningSnapshot()) {
        mutableState.update { current ->
            check(current.epoch < Long.MAX_VALUE) { "Fake storage epoch exhausted" }
            current.copy(snapshot = seed, epoch = current.epoch + 1)
        }
    }

    fun failNextWrites(count: Int) {
        val faults = WriteFaults.Next(count)
        mutableState.update { it.copy(faults = faults) }
    }

    fun failAllWrites() {
        mutableState.update { it.copy(faults = WriteFaults.All) }
    }

    fun recoverWrites() {
        mutableState.update { it.copy(faults = WriteFaults.None) }
    }

    private fun commit(requestEpoch: Long, snapshot: ListeningSnapshot): ProgressOutcome {
        while (true) {
            val current = mutableState.value
            val decision = decideWrite(current, requestEpoch, snapshot)
            if (mutableState.compareAndSet(current, decision.state)) return decision.outcome
        }
    }
}
