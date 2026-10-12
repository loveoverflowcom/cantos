package com.cantos.theatre.ports

import com.cantos.theatre.domain.playback.PlaybackEngineCommand
import com.cantos.theatre.domain.playback.PlaybackObservation
import kotlinx.coroutines.flow.Flow

/**
 * The mock engine seam. Commands accept a complete request and return without waiting for
 * readiness; observations arrive separately and carry the source/generation fence.
 * A future native adapter must implement this boundary rather than expose Media3/AVFoundation
 * or HTTP/publication DTOs to the core. No such native adapter is implemented by this demo.
 */
interface MediaEngine {
    val events: Flow<PlaybackObservation>
    suspend fun prepare(command: PlaybackEngineCommand)
    suspend fun start(command: PlaybackEngineCommand)
    suspend fun pause(command: PlaybackEngineCommand)
    suspend fun seek(command: PlaybackEngineCommand)
    suspend fun setSpeed(command: PlaybackEngineCommand)
    fun close()
}
