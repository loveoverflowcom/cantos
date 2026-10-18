package com.cantos.theatre.ports

import com.cantos.theatre.domain.playback.PlaybackEngineCommand
import com.cantos.theatre.domain.playback.PlaybackObservation
import kotlinx.coroutines.flow.Flow

/** Commands return before readiness. Observations carry the source/generation fence. */
interface MediaEngine {
    val events: Flow<PlaybackObservation>
    suspend fun prepare(command: PlaybackEngineCommand)
    suspend fun start(command: PlaybackEngineCommand)
    suspend fun pause(command: PlaybackEngineCommand)
    suspend fun seek(command: PlaybackEngineCommand)
    suspend fun setSpeed(command: PlaybackEngineCommand)
    fun close()
}

/** Ephemeral transport credential, never a checkpoint or a durable identity. */
class PlaybackLocation(val url: String) {
    override fun toString(): String = "PlaybackLocation([redacted])"
}

/** Native decoder. Only the authorizing adapter supplies the disposable media location. */
interface StreamingMediaEngine : MediaEngine {
    suspend fun load(command: PlaybackEngineCommand, location: PlaybackLocation)
}
