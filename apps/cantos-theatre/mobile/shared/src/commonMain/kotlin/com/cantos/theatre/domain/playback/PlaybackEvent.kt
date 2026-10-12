package com.cantos.theatre.domain.playback

sealed interface PlaybackEvent {
    data class SourceSelected(val source: PlaybackSource, val positionMs: Long) : PlaybackEvent
    data object PlayRequested : PlaybackEvent
    data object PauseRequested : PlaybackEvent
    data class SeekRequested(val positionMs: Long) : PlaybackEvent
    data class SkipRequested(val deltaMs: Int) : PlaybackEvent
    data class SpeedRequested(val speedPermille: Int) : PlaybackEvent
    data class RestoreRequested(val snapshot: PlaybackSnapshot) : PlaybackEvent
    data object InterruptionBegan : PlaybackEvent
    data class InterruptionEnded(val resumeAllowed: Boolean) : PlaybackEvent
    data class EngineReady(val identity: PlaybackIdentity) : PlaybackObservation
    data class EngineStarted(val identity: PlaybackIdentity) : PlaybackObservation
    data class EnginePaused(val identity: PlaybackIdentity) : PlaybackObservation
    data class EngineBuffering(val identity: PlaybackIdentity) : PlaybackObservation
    data class EnginePosition(val identity: PlaybackIdentity, val positionMs: Long) : PlaybackObservation
    data class EngineEnded(val identity: PlaybackIdentity) : PlaybackObservation
}

/** The engine port may report observed facts, never issue listener intents or select sources. */
sealed interface PlaybackObservation : PlaybackEvent

/** Every command stands alone; no later generation waits for a discarded prepare callback. */
data class PlaybackEngineCommand(
    val identity: PlaybackIdentity,
    val source: PlaybackSource,
    val positionMs: Long,
    val speedPermille: Int,
) {
    init {
        require(identity.workId == source.workId && identity.episodeIndex == source.episodeIndex) {
            "Playback command identity differs from its source"
        }
        require(positionMs in 0..source.durationMs) { "Playback command position is outside its source" }
        require(isSupportedPlaybackSpeed(speedPermille)) { "Playback command speed is unsupported" }
    }
}

enum class PlaybackRejection {
    NoSession, Interrupted, OutOfBounds, UnsupportedSpeed, GenerationExhausted, InvalidSource,
}

sealed interface PlaybackEffect {
    data class PrepareEngine(val command: PlaybackEngineCommand) : PlaybackEffect
    data class StartEngine(val command: PlaybackEngineCommand) : PlaybackEffect
    data class PauseEngine(val command: PlaybackEngineCommand) : PlaybackEffect
    data class SeekEngine(val command: PlaybackEngineCommand) : PlaybackEffect
    data class SetSpeed(val command: PlaybackEngineCommand) : PlaybackEffect
    data class OfferResume(val source: PlaybackSource, val positionMs: Long) : PlaybackEffect
    data class Rejected(val reason: PlaybackRejection) : PlaybackEffect
}

/** The reducer only emits fresh empty/listOf snapshots, with no mutable collection retained. */
data class PlaybackTransition(val nextState: PlaybackState, val effects: List<PlaybackEffect>)
