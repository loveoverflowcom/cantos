package com.cantos.theatre.domain.playback

const val MAX_PLAYBACK_GENERATION: Long = 4_294_967_295L

/** Immutable source metadata with the opaque WorkKey invariant and validated units; no media URL or authorization. */
@ConsistentCopyVisibility
data class PlaybackSource private constructor(
    val workId: String,
    val episodeIndex: Int,
    val durationMs: Long,
) {
    companion object {
        fun of(workId: String, episodeIndex: Int, durationMs: Long): PlaybackSource? =
            if (workId.isNotBlank() && episodeIndex in 0..65_535 && durationMs in 1..MAX_PLAYBACK_GENERATION) {
                PlaybackSource(workId, episodeIndex, durationMs)
            } else {
                null
            }
    }
}

/** Both source key and generation fence callbacks, including callbacks from a previous episode. */
@ConsistentCopyVisibility
data class PlaybackIdentity private constructor(
    val workId: String,
    val episodeIndex: Int,
    val generation: Long,
) {
    companion object {
        fun of(workId: String, episodeIndex: Int, generation: Long): PlaybackIdentity? =
            if (workId.isNotBlank() && episodeIndex in 0..65_535 && generation in 0..MAX_PLAYBACK_GENERATION) {
                PlaybackIdentity(workId, episodeIndex, generation)
            } else {
                null
            }

        internal fun fromState(source: PlaybackSource, state: PlaybackState): PlaybackIdentity =
            PlaybackIdentity(source.workId, source.episodeIndex, state.generation)
    }
}

enum class PlaybackPauseCause { User, Interruption, ColdRestore }

sealed interface PlaybackIntent {
    data object Play : PlaybackIntent
    data class Pause(val cause: PlaybackPauseCause) : PlaybackIntent
}

enum class PlaybackEngine { Preparing, Ready, Running, Paused, Buffering, Ended }

enum class PlaybackInterruption { None, Active }

/** Simulating is confirmation from a fake engine; it never asserts real audio output. */
enum class PlaybackStatus { Idle, Loading, Paused, Simulating, Buffering, Ended }

/** Unaccepted restore input: position and speed are validated transactionally by the reducer. */
data class PlaybackSnapshot(
    val source: PlaybackSource,
    val positionMs: Long,
    val speedPermille: Int,
)

data class PlaybackSession(
    val source: PlaybackSource,
    val positionMs: Long,
    val speedPermille: Int,
    val intent: PlaybackIntent,
    val engine: PlaybackEngine,
    val interruption: PlaybackInterruption,
) {
    init {
        require(positionMs in 0..source.durationMs) { "Playback session position is outside its source" }
        require(isSupportedPlaybackSpeed(speedPermille)) { "Playback session speed is unsupported" }
    }

    fun snapshot(): PlaybackSnapshot = PlaybackSnapshot(source, positionMs, speedPermille)
}

data class PlaybackState(
    val generation: Long = 0,
    val session: PlaybackSession? = null,
) {
    init {
        require(generation in 0..MAX_PLAYBACK_GENERATION) { "Playback generation is outside Uint32 bounds" }
    }

    val identity: PlaybackIdentity?
        get() = session?.let { PlaybackIdentity.fromState(it.source, this) }

    fun accepts(identity: PlaybackIdentity): Boolean = this.identity == identity

    val status: PlaybackStatus
        get() {
            val current = session ?: return PlaybackStatus.Idle
            if (current.engine == PlaybackEngine.Ended) return PlaybackStatus.Ended
            if (current.intent is PlaybackIntent.Pause || current.interruption == PlaybackInterruption.Active) {
                return PlaybackStatus.Paused
            }
            return when (current.engine) {
                PlaybackEngine.Preparing, PlaybackEngine.Ready, PlaybackEngine.Paused -> PlaybackStatus.Loading
                PlaybackEngine.Running -> PlaybackStatus.Simulating
                PlaybackEngine.Buffering -> PlaybackStatus.Buffering
                PlaybackEngine.Ended -> PlaybackStatus.Ended
            }
        }

    val pauseCause: PlaybackPauseCause?
        get() {
            if (status != PlaybackStatus.Paused) return null
            return when (val intent = session?.intent) {
                is PlaybackIntent.Pause -> intent.cause
                PlaybackIntent.Play -> PlaybackPauseCause.Interruption
                null -> null
            }
        }
}

fun isSupportedPlaybackSpeed(speedPermille: Int): Boolean = when (speedPermille) {
    750, 1000, 1250, 1500, 2000 -> true
    else -> false
}
