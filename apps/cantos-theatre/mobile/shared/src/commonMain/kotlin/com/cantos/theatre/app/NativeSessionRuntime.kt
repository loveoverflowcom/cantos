package com.cantos.theatre.app

import com.cantos.theatre.application.catalog.SharedCatalogController
import com.cantos.theatre.application.identity.AccountChange
import com.cantos.theatre.application.identity.NativeAccountController
import com.cantos.theatre.application.identity.NativeVerifiedSession
import com.cantos.theatre.application.library.NativeLibraryController
import com.cantos.theatre.application.playback.NativeListeningController
import com.cantos.theatre.domain.library.ProgressIntent
import com.cantos.theatre.domain.playback.PlaybackEngine
import com.cantos.theatre.domain.playback.PlaybackPauseCause
import com.cantos.theatre.domain.playback.PlaybackEvent
import com.cantos.theatre.domain.playback.PlaybackIntent
import com.cantos.theatre.domain.playback.PlaybackState
import com.cantos.theatre.domain.playback.PlaybackStatus
import com.cantos.theatre.ports.PlaybackAccessPort
import com.cantos.theatre.ports.StreamingMediaEngine
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch

/** Platform composition retains this for the process, independent of screen navigation. */
class NativeSessionRuntime(
    scope: CoroutineScope,
    nativeFactory: ((PlaybackEvent) -> Unit) -> StreamingMediaEngine,
    access: PlaybackAccessPort,
    val account: NativeAccountController?,
    val library: NativeLibraryController?,
    newUuid: () -> String,
    nowMs: () -> Long,
    private val onAccountChanged: (String?) -> Unit = {},
    val shared: SharedCatalogController? = null,
) {
    val listening: NativeListeningController
    private var started = false
    private var restartPending = false
    private var engineSession = newUuid()
    private val accountJob: Job?

    init {
        lateinit var owner: NativeListeningController
        val engine = nativeFactory { owner.playback.dispatch(it) }
        owner = NativeListeningController(scope, engine, access, nowMs) { before, after ->
            val audio = after.session?.source?.audio
            val restored = before.generation != after.generation &&
                after.session?.intent == PlaybackIntent.Pause(PlaybackPauseCause.ColdRestore)
            val restarting = before.session?.engine == PlaybackEngine.Ended &&
                after.session?.engine == PlaybackEngine.Preparing && after.session.intent == PlaybackIntent.Play
            val retrying = before.session?.failure != null && after.session?.failure == null &&
                after.session?.intent == PlaybackIntent.Play
            if (before.session?.source != after.session?.source || restored || restarting || retrying) {
                started = false
                restartPending = restarting
                engineSession = newUuid()
            }
            // Read authority synchronously as well as observing it: logout fences the very next
            // engine callback even before the session collector gets its next turn.
            val actor = account?.verifiedSession?.value?.session?.actor
            if (actor != null && library?.state?.value?.account == actor && audio != null) {
                if (after.status == PlaybackStatus.Playing && !started) {
                    started = true
                    library.engineStarted(audio, engineSession)
                    if (restartPending) {
                        library.progressCheckpoint(audio, 0, ProgressIntent.Restart, force = true)
                        restartPending = false
                    }
                }
                if (started) checkpoint(before, after)
            }
        }
        listening = owner
        accountJob = account?.let { identity -> scope.launch {
            var previous: NativeVerifiedSession? = null
            identity.verifiedSession.collect { verified ->
                shared?.sessionChanged()
                val sameFamily = verified != null && previous?.let {
                    it.intent == verified.intent && it.session.actor == verified.session.actor &&
                        it.session.session == verified.session.session
                } == true
                val keepPublicPlayback = identity.change == AccountChange.Passive ||
                    (verified != null && previous == null && listening.playback.state.value.session != null)
                previous = verified
                if (sameFamily) {
                    // Rotation changes credentials, not the account or playback session. The
                    // authorized transport renews the Library's existing authorization in place.
                    // A temporary Library read fence can reopen its partition without moving
                    // the player. Revalidation changes the admission epoch and takes the reset
                    // path below, including when StateFlow conflates its intermediate null.
                    if (library?.state?.value?.account != verified?.session?.actor) {
                        verified?.let { library?.enter(it.session, it.bearer) }
                    }
                    return@collect
                }
                onAccountChanged(verified?.session?.actor)
                // Playback has a separate public release capability. Passive account quarantine/loss
                // fences Library immediately but cannot pause public audio or replace its position.
                // Explicit logout/switch retains the engine fence, including conflated transitions.
                if (!keepPublicPlayback) {
                    listening.cancelSelection()
                    listening.playback.dispatch(PlaybackEvent.SessionCleared)
                }
                if (verified == null) {
                    library?.leave()
                } else {
                    library?.enter(verified.session, verified.bearer)
                    library?.state?.value?.restoredPaused?.takeUnless { keepPublicPlayback }?.let { saved ->
                        listening.restore(saved.audio, saved.positionMs, saved.speedPermille)
                    }
                }
                if (identity.change != AccountChange.Passive) {
                    started = false
                    restartPending = false
                    engineSession = newUuid()
                }
            }
        } }
        // API validation precedes every account-owned database read or replay.
        account?.restore()
    }

    private fun checkpoint(before: PlaybackState, after: PlaybackState) {
        val session = after.session ?: return
        val audio = session.source.audio ?: return
        val seekAcknowledged = before.session?.pendingPositionMs != null && session.pendingPositionMs == null
        val paused = before.session?.intent == PlaybackIntent.Play && session.intent is PlaybackIntent.Pause
        val speedChanged = before.session?.speedPermille != session.speedPermille
        if (speedChanged) library?.savePlaybackSettings(audio, session.speedPermille)
        if (session.pendingPositionMs == null && (before.session?.positionMs != session.positionMs || paused || speedChanged)) {
            library?.progressCheckpoint(audio, session.positionMs,
                if (seekAcknowledged) ProgressIntent.ExplicitSeek else ProgressIntent.Checkpoint,
                force = seekAcknowledged || paused || speedChanged)
        }
    }

    /** Scheduling hints only: they grant no admission and never touch the player/session. */
    fun foregrounded() { library?.drain() }
    fun networkRestored() { library?.drain() }

    fun backgrounded() {
        val session = listening.playback.state.value.session ?: return
        val audio = session.source.audio ?: return
        if (started && account?.verifiedSession?.value?.session?.actor == library?.state?.value?.account) {
            library?.progressCheckpoint(audio, session.positionMs, force = true)
        }
    }

    fun close() {
        accountJob?.cancel()
        listening.close()
        library?.close()
        shared?.close()
        account?.close()
    }
}
