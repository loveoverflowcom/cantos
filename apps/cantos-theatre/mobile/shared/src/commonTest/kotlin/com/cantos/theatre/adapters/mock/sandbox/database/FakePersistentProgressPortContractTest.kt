package com.cantos.theatre.adapters.mock.sandbox.database

import com.cantos.theatre.domain.listening.ListeningSnapshot
import com.cantos.theatre.ports.ProgressPort
import com.cantos.theatre.ports.ProgressPortContract

/** The sandbox fake satisfies the adapter-agnostic contract; memory-only, no durable claim. */
class FakePersistentProgressPortContractTest : ProgressPortContract() {
    override fun openStore(seed: ListeningSnapshot): () -> ProgressPort {
        val database = FakeStorageDatabase(seed)
        return { FakePersistentProgressPort(database) }
    }
}

/** Opt-in latency changes timing only, never the contract. */
class DelayedFakePersistentProgressPortContractTest : ProgressPortContract() {
    override fun openStore(seed: ListeningSnapshot): () -> ProgressPort {
        val database = FakeStorageDatabase(seed)
        return { FakePersistentProgressPort(database, FakeStorageConfig(readLatencyMs = 40, writeLatencyMs = 70)) }
    }
}
