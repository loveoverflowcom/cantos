package com.cantos.theatre.adapters.mock

import com.cantos.theatre.domain.playback.PlaybackEngineCommand
import com.cantos.theatre.domain.playback.PlaybackEvent
import com.cantos.theatre.domain.playback.PlaybackIdentity
import com.cantos.theatre.domain.playback.PlaybackObservation
import com.cantos.theatre.ports.MediaEngine
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.receiveAsFlow

enum class MockCommandKind { Prepare, Start, Pause, Seek, Speed }

data class MockAcceptedCommand(val kind: MockCommandKind, val command: PlaybackEngineCommand)

/** Deterministic command acceptance and explicit fake acknowledgements. It never creates audio. */
class MockMediaEngine(private val acknowledgeAutomatically: Boolean = true) : MediaEngine {
    private val observations = Channel<PlaybackObservation>(Channel.UNLIMITED)
    private val accepted = mutableListOf<MockAcceptedCommand>()
    private var current: PlaybackEngineCommand? = null
    private var closed = false
    override val events: Flow<PlaybackObservation> = observations.receiveAsFlow()

    val acceptedCommands: List<MockAcceptedCommand> get() = accepted.toList()
    val desiredCommand: PlaybackEngineCommand? get() = current

    private fun accept(kind: MockCommandKind, command: PlaybackEngineCommand): Boolean {
        check(!closed) { "Mock engine is closed" }
        accepted += MockAcceptedCommand(kind, command)
        val previous = current
        if (previous != null && command.identity.generation < previous.identity.generation) return false
        current = command
        return true
    }

    override suspend fun prepare(command: PlaybackEngineCommand) {
        if (accept(MockCommandKind.Prepare, command) && acknowledgeAutomatically) {
            emit(PlaybackEvent.EngineReady(command.identity))
        }
    }

    override suspend fun start(command: PlaybackEngineCommand) {
        if (accept(MockCommandKind.Start, command) && acknowledgeAutomatically) {
            emit(PlaybackEvent.EngineStarted(command.identity))
        }
    }

    override suspend fun pause(command: PlaybackEngineCommand) {
        if (accept(MockCommandKind.Pause, command) && acknowledgeAutomatically) {
            emit(PlaybackEvent.EnginePaused(command.identity))
        }
    }

    override suspend fun seek(command: PlaybackEngineCommand) {
        if (accept(MockCommandKind.Seek, command) && acknowledgeAutomatically) {
            emit(PlaybackEvent.EnginePosition(command.identity, command.positionMs))
        }
    }

    override suspend fun setSpeed(command: PlaybackEngineCommand) {
        accept(MockCommandKind.Speed, command)
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
