package com.cantos.theatre.presentation.model

/** Presentation facts only: engine acknowledgement and listener intent remain owned by playback. */
enum class WorkListenPhase {
    Ready, Resolving, PauseLoading, PauseBuffering, PausePlaying, Resume, ResolveFailed, TransportFailed,
    ListLoading, ListFailed, NoTarget, Unavailable,
}

/** A source-bound projection, never inferred from the selected unit or transportRequested alone. */
data class WorkListenRelease(val episodeId: ShellEpisodeId, val publicationId: String, val phase: WorkListenPhase)

data class WorkListenProjection(val target: ShellEpisode?, val phase: WorkListenPhase) {
    val label: CopyKey get() = when (phase) {
        WorkListenPhase.Resolving, WorkListenPhase.ListLoading -> CopyKey.WorkListenLoading
        WorkListenPhase.PauseLoading, WorkListenPhase.PauseBuffering, WorkListenPhase.PausePlaying -> CopyKey.Pause
        WorkListenPhase.Resume -> CopyKey.WorkListenResume
        WorkListenPhase.ResolveFailed, WorkListenPhase.TransportFailed -> CopyKey.Retry
        else -> CopyKey.WorkListen
    }

    val status: CopyKey? get() = when (phase) {
        WorkListenPhase.Resolving, WorkListenPhase.PauseLoading -> CopyKey.PlaybackLoading
        WorkListenPhase.PauseBuffering -> CopyKey.PlaybackBuffering
        WorkListenPhase.PausePlaying -> CopyKey.PlaybackPlaying
        WorkListenPhase.Resume -> CopyKey.PlaybackPaused
        WorkListenPhase.ResolveFailed, WorkListenPhase.TransportFailed -> CopyKey.PlaybackFailed
        WorkListenPhase.ListLoading -> CopyKey.Loading
        WorkListenPhase.ListFailed -> CopyKey.WorkListenListFailed
        WorkListenPhase.NoTarget -> CopyKey.WorkListenNoTarget
        WorkListenPhase.Unavailable -> CopyKey.WorkListenUnavailable
        WorkListenPhase.Ready -> null
    }

    val icon: String get() = when (phase) {
        WorkListenPhase.PauseLoading, WorkListenPhase.PauseBuffering, WorkListenPhase.PausePlaying -> "pause"
        WorkListenPhase.ResolveFailed, WorkListenPhase.TransportFailed -> "refresh"
        else -> "play_arrow"
    }

    fun event(): ShellEvent? = when (phase) {
        WorkListenPhase.Ready -> target?.let { ShellEvent.OpenEpisode(it.id) }
        WorkListenPhase.ResolveFailed -> ShellEvent.RetryListening
        WorkListenPhase.PauseLoading, WorkListenPhase.PauseBuffering, WorkListenPhase.PausePlaying,
        WorkListenPhase.Resume, WorkListenPhase.TransportFailed -> ShellEvent.ToggleSimulation
        WorkListenPhase.Resolving, WorkListenPhase.ListLoading, WorkListenPhase.ListFailed,
        WorkListenPhase.NoTarget, WorkListenPhase.Unavailable -> null
    }
}

/** Preserve server ordering. Unloaded/failed pages never become an empty catalogue. */
fun workListenProjection(work: ShellWork, release: WorkListenRelease?): WorkListenProjection {
    val target = work.episodes.firstOrNull { it.durationSeconds > 0 }
    if (target == null) return WorkListenProjection(null, when (work.unitsPhase) {
        ShellListPhase.Loading -> WorkListenPhase.ListLoading
        ShellListPhase.Failed -> WorkListenPhase.ListFailed
        ShellListPhase.More -> WorkListenPhase.NoTarget
        ShellListPhase.Complete -> WorkListenPhase.Unavailable
    })
    val same = release?.takeIf { target.publicationId != null && it.episodeId == target.id && it.publicationId == target.publicationId }
    return WorkListenProjection(target, same?.phase ?: WorkListenPhase.Ready)
}
