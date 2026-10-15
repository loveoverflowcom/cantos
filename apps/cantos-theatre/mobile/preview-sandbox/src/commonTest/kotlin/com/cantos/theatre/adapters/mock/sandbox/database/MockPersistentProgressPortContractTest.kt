package com.cantos.theatre.adapters.mock.sandbox.database

import com.cantos.theatre.domain.listening.ListeningSnapshot
import com.cantos.theatre.ports.ProgressPort
import com.cantos.theatre.ports.ProgressPortContract

/** The sandbox mock satisfies the adapter-agnostic contract; memory-only, no durable claim. */
class MockPersistentProgressPortContractTest : ProgressPortContract() {
    override fun openStore(seed: ListeningSnapshot): () -> ProgressPort {
        val database = MockStorageDatabase(seed)
        return { MockPersistentProgressPort(database) }
    }
}

/** Opt-in latency changes timing only, never the contract. */
class DelayedMockPersistentProgressPortContractTest : ProgressPortContract() {
    override fun openStore(seed: ListeningSnapshot): () -> ProgressPort {
        val database = MockStorageDatabase(seed)
        return { MockPersistentProgressPort(database, MockStorageConfig(readLatencyMs = 40, writeLatencyMs = 70)) }
    }
}
