package com.cantos.theatre.adapters.mock

import com.cantos.theatre.domain.listening.ListeningSnapshot
import com.cantos.theatre.ports.ProgressOutcome
import com.cantos.theatre.ports.ProgressPort

/** One in-memory adapter; process restart discards every value. */
class MemoryProgressPort(initialSnapshot: ListeningSnapshot = ListeningSnapshot()) : ProgressPort {
    private var snapshot = initialSnapshot

    override suspend fun load(): ListeningSnapshot = snapshot

    override suspend fun store(snapshot: ListeningSnapshot): ProgressOutcome {
        this.snapshot = snapshot
        return ProgressOutcome.Stored
    }
}
