package com.cantos.theatre.adapters.preview

import com.cantos.theatre.domain.playback.AudioIdentity
import com.cantos.theatre.domain.playback.PlaybackEngineCommand
import com.cantos.theatre.domain.playback.PlaybackEvent
import com.cantos.theatre.domain.playback.PlaybackObservation
import com.cantos.theatre.ports.PlaybackAccessFailure
import com.cantos.theatre.ports.PlaybackAccessPort
import com.cantos.theatre.ports.PlaybackAccessResult
import com.cantos.theatre.ports.PlaybackGrant
import com.cantos.theatre.ports.PlaybackLocation
import com.cantos.theatre.ports.StreamingMediaEngine
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.receiveAsFlow

/**
 * Release manifest and short-lived media grant for the sample units. A refusal can be injected
 * from the workbench; the grant location names no real resource and the engine never opens it.
 */
class PreviewPlaybackAccess(
    private val nowMs: () -> Long,
    /** When set, every manifest and grant request is refused with this reason. */
    var refusal: PlaybackAccessFailure? = null,
) : PlaybackAccessPort {
    var manifests: Int = 0
        private set
    var authorizations: Int = 0
        private set

    override suspend fun manifest(publicationId: String): PlaybackAccessResult<AudioIdentity> {
        manifests += 1
        refusal?.let { return PlaybackAccessResult.Refused(it) }
        val (work, unit) = PreviewFixtures.unitByPublication(publicationId)
            ?: return PlaybackAccessResult.Refused(PlaybackAccessFailure.NotFound)
        return PlaybackAccessResult.Accepted(PreviewFixtures.audio(work, unit))
    }

    override suspend fun authorize(identity: AudioIdentity): PlaybackAccessResult<PlaybackGrant> {
        authorizations += 1
        refusal?.let { return PlaybackAccessResult.Refused(it) }
        if (PreviewFixtures.unitByPublication(identity.publicationId) == null) {
            return PlaybackAccessResult.Refused(PlaybackAccessFailure.NotFound)
        }
        return PlaybackAccessResult.Accepted(
            PlaybackGrant(PlaybackLocation("preview://no-audio/${identity.publicationId}"), nowMs() + GrantLifetimeMs),
        )
    }

    private companion object {
        const val GrantLifetimeMs = 15L * 60_000L
    }
}

/**
 * A decoder stand-in: it acknowledges the commands the production engine shell issues, so the
 * shared reducer, projection and controllers run unchanged. It produces no audio, never advances
 * the position by itself and establishes no native playback; every observation is fenced by the
 * command identity exactly like a real adapter's.
 */
class PreviewStreamingEngine : StreamingMediaEngine {
    private val observations = Channel<PlaybackObservation>(Channel.UNLIMITED)
    override val events: Flow<PlaybackObservation> = observations.receiveAsFlow()
    private var closed = false

    /** Commands in arrival order, for deterministic assertions about what the shell asked of the decoder. */
    val commands: List<String> get() = log.toList()
    private val log = mutableListOf<String>()

    override suspend fun load(command: PlaybackEngineCommand, location: PlaybackLocation) { record("load", command) }

    override suspend fun prepare(command: PlaybackEngineCommand) {
        record("prepare", command)
        emit(PlaybackEvent.EngineReady(command.identity))
    }

    override suspend fun start(command: PlaybackEngineCommand) {
        record("start", command)
        // A decoder reports where it resumed (a seek while playing reloads at the new point), then that it runs.
        emit(PlaybackEvent.EnginePosition(command.identity, command.positionMs))
        emit(PlaybackEvent.EngineStarted(command.identity))
    }

    override suspend fun pause(command: PlaybackEngineCommand) {
        record("pause", command)
        emit(PlaybackEvent.EnginePaused(command.identity))
    }

    override suspend fun seek(command: PlaybackEngineCommand) {
        record("seek", command)
        emit(PlaybackEvent.EnginePosition(command.identity, command.positionMs))
    }

    override suspend fun setSpeed(command: PlaybackEngineCommand) { record("speed", command) }

    /** Delayed or stale observations for scenarios; they never overwrite the desired command. */
    fun emit(event: PlaybackObservation): Boolean = !closed && observations.trySend(event).isSuccess

    private fun record(kind: String, command: PlaybackEngineCommand) {
        check(!closed) { "Preview engine is closed" }
        log += "$kind@${command.positionMs}"
    }

    override fun close() {
        if (closed) return
        closed = true
        observations.close()
    }
}
