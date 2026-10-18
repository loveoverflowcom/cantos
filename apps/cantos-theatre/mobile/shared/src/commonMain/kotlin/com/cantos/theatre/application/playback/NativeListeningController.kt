package com.cantos.theatre.application.playback

import com.cantos.theatre.domain.playback.AudioIdentity
import com.cantos.theatre.domain.playback.PlaybackEvent
import com.cantos.theatre.domain.playback.PlaybackSnapshot
import com.cantos.theatre.domain.playback.PlaybackSource
import com.cantos.theatre.domain.playback.PlaybackState
import com.cantos.theatre.ports.PlaybackAccessFailure
import com.cantos.theatre.ports.PlaybackAccessPort
import com.cantos.theatre.ports.PlaybackAccessResult
import com.cantos.theatre.ports.StreamingMediaEngine
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

data class NativeSelection(val title: String, val workTitle: String)

/** One production playback owner; screens and OS controls attach to its [playback]. */
class NativeListeningController(
    private val scope: CoroutineScope,
    native: StreamingMediaEngine,
    private val access: PlaybackAccessPort,
    nowMs: () -> Long,
    onCommitted: (PlaybackState, PlaybackState) -> Unit = { _, _ -> },
) {
    val playback = PlaybackController(scope, AuthorizingMediaEngine(scope, native, access, nowMs), onCommitted)
    private val mutableSelection = MutableStateFlow<NativeSelection?>(null)
    val selection = mutableSelection.asStateFlow()
    private val mutableFailure = MutableStateFlow<PlaybackAccessFailure?>(null)
    val failure = mutableFailure.asStateFlow()
    private var selectionRequest: Job? = null
    private var selectionGeneration = 0L
    private var closed = false

    fun open(workId: String, unitId: String, publicationId: String, title: String, workTitle: String) {
        if (closed) return
        selectionRequest?.cancel()
        playback.dispatch(PlaybackEvent.PauseRequested)
        val generation = ++selectionGeneration
        mutableFailure.value = null
        selectionRequest = scope.launch {
            when (val result = access.manifest(publicationId)) {
                is PlaybackAccessResult.Refused -> if (!closed && generation == selectionGeneration) mutableFailure.value = result.reason
                is PlaybackAccessResult.Accepted -> {
                    if (closed || generation != selectionGeneration) return@launch
                    val audio = result.value
                    if (audio.workId != workId || audio.unitId != unitId) {
                        mutableFailure.value = PlaybackAccessFailure.IdentityChanged
                        return@launch
                    }
                    mutableSelection.value = NativeSelection(title, workTitle)
                    playback.dispatch(PlaybackEvent.SourceSelected(PlaybackSource.streaming(audio, title, workTitle), 0))
                    playback.dispatch(PlaybackEvent.PlayRequested)
                }
            }
        }
    }

    /** Restoring never resolves a URL, activates audio focus or sends Play. */
    fun restore(audio: AudioIdentity, positionMs: Long, speedPermille: Int = 1000) {
        if (closed) return
        selectionGeneration += 1
        selectionRequest?.cancel()
        mutableFailure.value = null
        mutableSelection.value = NativeSelection("", "")
        playback.dispatch(PlaybackEvent.RestoreRequested(PlaybackSnapshot(PlaybackSource.streaming(audio), positionMs, speedPermille)))
    }

    fun cancelSelection() {
        selectionGeneration += 1
        selectionRequest?.cancel()
        playback.dispatch(PlaybackEvent.PauseRequested)
    }

    fun close() {
        if (closed) return
        closed = true
        selectionGeneration += 1
        selectionRequest?.cancel()
        playback.close()
    }
}
