package com.cantos.theatre.adapters.playback

import com.cantos.theatre.domain.playback.PlaybackEngineCommand
import com.cantos.theatre.domain.playback.PlaybackEvent
import com.cantos.theatre.domain.playback.PlaybackFailure
import com.cantos.theatre.domain.playback.PlaybackObservation
import com.cantos.theatre.ports.PlaybackLocation
import com.cantos.theatre.ports.StreamingMediaEngine
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.withContext

/** The Swift AVPlayer edge uses plain calls; it never implements suspend or exports Flow. */
enum class IosMediaOperation { Prepare, Start, Pause, Seek, Speed }
enum class IosMediaFact { Ready, Started, Paused, Buffering, Position, Ended, Network, Unavailable, Expired, Decode }
enum class IosMediaControl { Play, Pause, Seek, Speed, InterruptionBegan, InterruptionResume, InterruptionPause }

interface IosMediaListener {
    fun observed(token: Long, generation: Long, fact: IosMediaFact, positionMs: Long)
    fun control(control: IosMediaControl, value: Long)
}

/** All methods and callbacks run on the main queue, with one driver for the app process. */
interface IosMediaDriver {
    fun attach(listener: IosMediaListener)
    fun load(token: Long, command: PlaybackEngineCommand, location: PlaybackLocation)
    fun update(token: Long, command: PlaybackEngineCommand, operation: IosMediaOperation)
    fun close()
}

/**
 * Platform-independent command/observation vocabulary stays on the Kotlin side. The item token
 * rejects callbacks from a replaced AVPlayerItem even when access refresh reuses the same
 * playback generation. OS controls enter the same serialized controller as in-app controls;
 * only the reducer decides interruption resumption.
 */
class IosMediaEngine(
    private val driver: IosMediaDriver,
    private val onControl: (PlaybackEvent) -> Unit,
) : StreamingMediaEngine, IosMediaListener {
    private val observations = Channel<PlaybackObservation>(Channel.UNLIMITED)
    override val events = observations.receiveAsFlow()
    private var current: PlaybackEngineCommand? = null
    private var token = 0L
    private var closed = false

    init { driver.attach(this) }

    override suspend fun load(command: PlaybackEngineCommand, location: PlaybackLocation) = withContext(Dispatchers.Main.immediate) {
        if (closed || (current?.identity?.generation ?: -1) > command.identity.generation) return@withContext
        current = command
        check(token < Long.MAX_VALUE) { "Native media item token exhausted" }
        token += 1
        driver.load(token, command, location)
    }

    override suspend fun prepare(command: PlaybackEngineCommand) = update(command, IosMediaOperation.Prepare)
    override suspend fun start(command: PlaybackEngineCommand) = update(command, IosMediaOperation.Start)
    override suspend fun pause(command: PlaybackEngineCommand) = withContext(Dispatchers.Main.immediate) {
        if (closed) return@withContext
        if (command.clearSession) {
            driver.update(token, command, IosMediaOperation.Pause)
            current = null
            return@withContext
        }
        val previous = current ?: return@withContext
        if (command.identity.generation < previous.identity.generation) return@withContext
        // Superseding selection must stop old audio before resolving the new grant. Preserve
        // its old identity so a stop acknowledgement cannot forge readiness of the new source.
        if (previous.source != command.source) {
            driver.update(token, previous, IosMediaOperation.Pause)
            current = null
            return@withContext
        }
        current = command
        driver.update(token, command, IosMediaOperation.Pause)
    }
    override suspend fun seek(command: PlaybackEngineCommand) = update(command, IosMediaOperation.Seek)
    override suspend fun setSpeed(command: PlaybackEngineCommand) = update(command, IosMediaOperation.Speed)

    private suspend fun update(command: PlaybackEngineCommand, operation: IosMediaOperation) = withContext(Dispatchers.Main.immediate) {
        val previous = current ?: return@withContext
        if (closed || command.source != previous.source || command.identity.generation < previous.identity.generation) return@withContext
        current = command
        driver.update(token, command, operation)
    }

    override fun observed(token: Long, generation: Long, fact: IosMediaFact, positionMs: Long) {
        val command = current ?: return
        if (closed || token != this.token || generation != command.identity.generation) return
        val identity = command.identity
        val event = when (fact) {
            IosMediaFact.Ready -> PlaybackEvent.EngineReady(identity)
            IosMediaFact.Started -> PlaybackEvent.EngineStarted(identity)
            IosMediaFact.Paused -> PlaybackEvent.EnginePaused(identity)
            IosMediaFact.Buffering -> PlaybackEvent.EngineBuffering(identity)
            IosMediaFact.Position -> PlaybackEvent.EnginePosition(identity, positionMs.coerceIn(0, command.source.durationMs))
            IosMediaFact.Ended -> PlaybackEvent.EngineEnded(identity)
            IosMediaFact.Network -> PlaybackEvent.EngineFailed(identity, PlaybackFailure.Network)
            IosMediaFact.Unavailable -> PlaybackEvent.EngineFailed(identity, PlaybackFailure.Unavailable)
            IosMediaFact.Expired -> PlaybackEvent.EngineFailed(identity, PlaybackFailure.Expired)
            IosMediaFact.Decode -> PlaybackEvent.EngineFailed(identity, PlaybackFailure.Decode)
        }
        observations.trySend(event)
    }

    override fun control(control: IosMediaControl, value: Long) {
        if (closed || current == null) return
        onControl(when (control) {
            IosMediaControl.Play -> PlaybackEvent.PlayRequested
            IosMediaControl.Pause -> PlaybackEvent.PauseRequested
            IosMediaControl.Seek -> PlaybackEvent.SeekRequested(value)
            IosMediaControl.Speed -> PlaybackEvent.SpeedRequested(value.toInt())
            IosMediaControl.InterruptionBegan -> PlaybackEvent.InterruptionBegan
            IosMediaControl.InterruptionResume -> PlaybackEvent.InterruptionEnded(resumeAllowed = true)
            IosMediaControl.InterruptionPause -> PlaybackEvent.InterruptionEnded(resumeAllowed = false)
        })
    }

    override fun close() {
        if (closed) return
        closed = true
        current = null
        driver.close()
        observations.close()
    }
}
