package com.cantos.theatre.application.playback

import com.cantos.theatre.domain.playback.PlaybackEngineCommand
import com.cantos.theatre.domain.playback.PlaybackEvent
import com.cantos.theatre.domain.playback.PlaybackIdentity
import com.cantos.theatre.domain.playback.PlaybackObservation
import com.cantos.theatre.ports.MediaEngine
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.receiveAsFlow

enum class ScriptedCommandKind { Prepare, Start, Pause, Seek, Speed }

data class ScriptedAcceptedCommand(val kind: ScriptedCommandKind, val command: PlaybackEngineCommand)

/** Test double: deterministic command acceptance and explicit acknowledgements. It never creates audio. */
class ScriptedMediaEngine(private val acknowledgeAutomatically: Boolean = true) : MediaEngine {
    private val observations = Channel<PlaybackObservation>(Channel.UNLIMITED)
    private val accepted = mutableListOf<ScriptedAcceptedCommand>()
    private var current: PlaybackEngineCommand? = null
    private var closed = false
    override val events: Flow<PlaybackObservation> = observations.receiveAsFlow()

    val acceptedCommands: List<ScriptedAcceptedCommand> get() = accepted.toList()
    val desiredCommand: PlaybackEngineCommand? get() = current

    private fun accept(kind: ScriptedCommandKind, command: PlaybackEngineCommand): Boolean {
        check(!closed) { "Mock engine is closed" }
        accepted += ScriptedAcceptedCommand(kind, command)
        val previous = current
        if (previous != null && command.identity.generation < previous.identity.generation) return false
        current = command
        return true
    }

    override suspend fun prepare(command: PlaybackEngineCommand) {
        if (accept(ScriptedCommandKind.Prepare, command) && acknowledgeAutomatically) {
            emit(PlaybackEvent.EngineReady(command.identity))
        }
    }

    override suspend fun start(command: PlaybackEngineCommand) {
        if (accept(ScriptedCommandKind.Start, command) && acknowledgeAutomatically) {
            emit(PlaybackEvent.EngineStarted(command.identity))
        }
    }

    override suspend fun pause(command: PlaybackEngineCommand) {
        if (accept(ScriptedCommandKind.Pause, command) && acknowledgeAutomatically) {
            emit(PlaybackEvent.EnginePaused(command.identity))
        }
    }

    override suspend fun seek(command: PlaybackEngineCommand) {
        if (accept(ScriptedCommandKind.Seek, command) && acknowledgeAutomatically) {
            emit(PlaybackEvent.EnginePosition(command.identity, command.positionMs))
        }
    }

    override suspend fun setSpeed(command: PlaybackEngineCommand) {
        accept(ScriptedCommandKind.Speed, command)
    }

    /** Test/demo observations can be delayed or stale. They never overwrite desired command data. */
    fun emit(event: PlaybackObservation): Boolean = observations.trySend(event).isSuccess

    fun acknowledgeStarted(identity: PlaybackIdentity): Boolean = emit(PlaybackEvent.EngineStarted(identity))

    override fun close() {
        if (closed) return
        closed = true
        observations.close()
    }
}
