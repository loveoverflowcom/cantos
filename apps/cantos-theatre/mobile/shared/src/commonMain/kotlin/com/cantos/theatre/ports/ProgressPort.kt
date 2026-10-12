package com.cantos.theatre.ports

import com.cantos.theatre.domain.listening.ListeningSnapshot

sealed interface ProgressOutcome {
    data object Stored : ProgressOutcome
    data object Unavailable : ProgressOutcome
}

/** A local memory boundary in this demo; no durable or server acknowledgement is implied. */
interface ProgressPort {
    suspend fun load(): ListeningSnapshot
    suspend fun store(snapshot: ListeningSnapshot): ProgressOutcome
}
