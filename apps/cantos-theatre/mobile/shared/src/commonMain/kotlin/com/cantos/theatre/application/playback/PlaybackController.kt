package com.cantos.theatre.application.playback

import com.cantos.theatre.domain.playback.PlaybackEffect
import com.cantos.theatre.domain.playback.PlaybackEvent
import com.cantos.theatre.domain.playback.PlaybackIdentity
import com.cantos.theatre.domain.playback.PlaybackIntent
import com.cantos.theatre.domain.playback.PlaybackRejection
import com.cantos.theatre.domain.playback.PlaybackSource
import com.cantos.theatre.domain.playback.PlaybackSnapshot
import com.cantos.theatre.domain.playback.PlaybackState
import com.cantos.theatre.domain.playback.reducePlayback
import com.cantos.theatre.ports.MediaEngine
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.CoroutineName
import kotlinx.coroutines.Job
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

sealed interface PlaybackNotice {
    data class ResumeOffered(val source: PlaybackSource, val positionMs: Long) : PlaybackNotice
    data class Rejected(val reason: PlaybackRejection) : PlaybackNotice
    data object MediaEngineUnavailable : PlaybackNotice
}

/**
 * One player owner for the mounted app. Its event loop is the sole state writer; a separate
 * ordered effect worker accepts engine commands without waiting for engine acknowledgement.
 * Navigation never constructs another controller. There is no I/O in a StateFlow update.
 */
class PlaybackController(
    scope: CoroutineScope,
    private val engine: MediaEngine,
    private val onCommittedState: (PlaybackState, PlaybackState) -> Unit = { _, _ -> },
) {
    private sealed interface Message {
        data class Event(val event: PlaybackEvent) : Message
        data object ToggleIntent : Message
        data class BookmarkPoint(val accept: (PlaybackSnapshot) -> Unit) : Message
        data class CaptureState(val accept: (PlaybackState, Long) -> Unit) : Message
        data class RestoreForReset(val snapshot: PlaybackSnapshot, val accepted: (PlaybackState) -> Unit) : Message
        data class RestoreForResume(
            val snapshot: PlaybackSnapshot,
            val expectedRevision: Long?,
            val accepted: (PlaybackState) -> Unit,
        ) : Message
        data class Notice(val identity: PlaybackIdentity?, val notice: PlaybackNotice) : Message
    }

    private data class Effects(val identity: PlaybackIdentity?, val values: List<PlaybackEffect>)

    private val messages = Channel<Message>(Channel.UNLIMITED)
    private val effects = Channel<Effects>(Channel.UNLIMITED)
    private val mutableState = MutableStateFlow(PlaybackState())
    private val mutableNotice = MutableStateFlow<PlaybackNotice?>(null)
    val state: StateFlow<PlaybackState> = mutableState.asStateFlow()
    val notice: StateFlow<PlaybackNotice?> = mutableNotice.asStateFlow()
    private val jobs: List<Job>
    private var controlRevision = 0L

    init {
        val eventLoop = scope.launch(CoroutineName("theatre-playback-events")) {
            for (message in messages) {
                when (message) {
                    is Message.Event -> {
                        if (message.event.isControl()) advanceControlRevision()
                        apply(message.event)
                    }
                    Message.ToggleIntent -> {
                        advanceControlRevision()
                        apply(
                            if (mutableState.value.session?.intent == PlaybackIntent.Play && mutableState.value.session?.failure == null) {
                                PlaybackEvent.PauseRequested
                            } else {
                                PlaybackEvent.PlayRequested
                            },
                        )
                    }
                    is Message.BookmarkPoint -> mutableState.value.session?.snapshot()?.let(message.accept)
                    is Message.CaptureState -> {
                        advanceControlRevision()
                        message.accept(mutableState.value, controlRevision)
                    }
                    is Message.RestoreForReset -> {
                        advanceControlRevision()
                        val generation = mutableState.value.generation
                        apply(PlaybackEvent.RestoreRequested(message.snapshot))
                        if (mutableState.value.generation > generation) message.accepted(mutableState.value)
                    }
                    is Message.RestoreForResume -> {
                        val current = mutableState.value
                        if (message.expectedRevision == null || controlRevision == message.expectedRevision) {
                            advanceControlRevision()
                            val snapshot = current.session?.takeIf { it.source == message.snapshot.source }?.snapshot()
                                ?: message.snapshot
                            apply(PlaybackEvent.RestoreRequested(snapshot))
                            if (mutableState.value.generation > current.generation) message.accepted(mutableState.value)
                        }
                    }
                    is Message.Notice -> if (message.identity == mutableState.value.identity) {
                        mutableNotice.value = message.notice
                    }
                }
            }
        }
        val engineEvents = scope.launch {
            engine.events.collect { event -> messages.send(Message.Event(event)) }
        }
        val effectLoop = scope.launch {
            for (batch in effects) {
                for (effect in batch.values) {
                    try {
                        when (effect) {
                            is PlaybackEffect.PrepareEngine -> engine.prepare(effect.command)
                            is PlaybackEffect.StartEngine -> engine.start(effect.command)
                            is PlaybackEffect.PauseEngine -> engine.pause(effect.command)
                            is PlaybackEffect.SeekEngine -> engine.seek(effect.command)
                            is PlaybackEffect.SetSpeed -> engine.setSpeed(effect.command)
                            is PlaybackEffect.OfferResume -> messages.send(Message.Notice(
                                batch.identity,
                                PlaybackNotice.ResumeOffered(effect.source, effect.positionMs),
                            ))
                            is PlaybackEffect.Rejected -> messages.send(Message.Notice(
                                batch.identity, PlaybackNotice.Rejected(effect.reason),
                            ))
                        }
                    } catch (cancelled: CancellationException) {
                        throw cancelled
                    } catch (_: Exception) {
                        messages.send(Message.Notice(batch.identity, PlaybackNotice.MediaEngineUnavailable))
                    }
                }
            }
        }
        jobs = listOf(eventLoop, engineEvents, effectLoop)
    }

    fun dispatch(event: PlaybackEvent): Boolean = messages.trySend(Message.Event(event)).isSuccess

    /** Resolve toggles inside the event loop, so rapid presses never read an old projection. */
    fun toggleIntent(): Boolean = messages.trySend(Message.ToggleIntent).isSuccess

    /** Capture after preceding controls; the caller only enqueues the independent bookmark write. */
    fun captureBookmarkPoint(accept: (PlaybackSnapshot) -> Unit): Boolean =
        messages.trySend(Message.BookmarkPoint(accept)).isSuccess

    /** A resume-intent barrier after preceding controls and their committed listening writes. */
    fun captureState(accept: (PlaybackState, Long) -> Unit): Boolean =
        messages.trySend(Message.CaptureState(accept)).isSuccess

    /** A concrete cross-owner reset barrier; rejected restores never publish a new baseline. */
    fun restoreForReset(snapshot: PlaybackSnapshot, accepted: (PlaybackState) -> Unit): Boolean =
        messages.trySend(Message.RestoreForReset(snapshot, accepted)).isSuccess

    /** Resolve a same-episode point after preceding controls, then cold restore without Play. */
    fun restoreForResume(
        snapshot: PlaybackSnapshot,
        expectedRevision: Long? = null,
        accepted: (PlaybackState) -> Unit,
    ): Boolean = messages.trySend(Message.RestoreForResume(snapshot, expectedRevision, accepted)).isSuccess

    private fun advanceControlRevision() {
        check(controlRevision < Long.MAX_VALUE) { "Mock playback control revision exhausted" }
        controlRevision += 1L
    }

    private fun PlaybackEvent.isControl(): Boolean = when (this) {
        is PlaybackEvent.SourceSelected, is PlaybackEvent.RestoreRequested, PlaybackEvent.SessionCleared, PlaybackEvent.PlayRequested,
        PlaybackEvent.PauseRequested, is PlaybackEvent.SeekRequested, is PlaybackEvent.SkipRequested,
        is PlaybackEvent.SpeedRequested, PlaybackEvent.InterruptionBegan, is PlaybackEvent.InterruptionEnded -> true
        is PlaybackEvent.EngineReady, is PlaybackEvent.EngineStarted, is PlaybackEvent.EnginePaused,
        is PlaybackEvent.EngineBuffering, is PlaybackEvent.EnginePosition, is PlaybackEvent.EngineEnded, is PlaybackEvent.EngineFailed -> false
    }

    private fun apply(event: PlaybackEvent) {
        val previous = mutableState.value
        val transition = reducePlayback(previous, event)
        mutableState.value = transition.nextState
        if (transition.nextState != previous) {
            mutableNotice.value = null
            // A StateFlow projection can conflate Started followed immediately by Pause.
            onCommittedState(previous, transition.nextState)
        }
        if (transition.effects.isNotEmpty()) {
            effects.trySend(Effects(transition.nextState.identity, transition.effects))
        }
    }

    fun close() {
        messages.cancel()
        effects.cancel()
        jobs.forEach { it.cancel() }
        engine.close()
    }
}
