package com.cantos.theatre.adapters.mock.sandbox.database

import com.cantos.theatre.domain.listening.ListeningSnapshot
import com.cantos.theatre.ports.ProgressOutcome
import com.cantos.theatre.ports.ProgressPort

/** Opt-in simulated latency for one port; zero by default. */
data class MockStorageConfig(val readLatencyMs: Long = 0L, val writeLatencyMs: Long = 0L) {
    init {
        require(readLatencyMs >= 0L && writeLatencyMs >= 0L) { "Mock storage latency must be nonnegative" }
    }
}

/**
 * One connection to a shared [MockStorageDatabase]. A recreated port or controller on the same
 * database reads what earlier ones stored; a new database starts with only its seed.
 */
class MockPersistentProgressPort(
    private val database: MockStorageDatabase,
    private val config: MockStorageConfig = MockStorageConfig(),
) : ProgressPort {
    override suspend fun load(): ListeningSnapshot = database.read(config.readLatencyMs)

    override suspend fun store(snapshot: ListeningSnapshot): ProgressOutcome =
        database.write(snapshot, config.writeLatencyMs)
}
