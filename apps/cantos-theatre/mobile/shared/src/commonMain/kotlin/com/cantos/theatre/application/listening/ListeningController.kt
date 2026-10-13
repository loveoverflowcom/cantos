package com.cantos.theatre.application.listening

import com.cantos.theatre.domain.listening.ListeningEvent
import com.cantos.theatre.domain.listening.ListeningSnapshot
import com.cantos.theatre.domain.listening.ProgressRecord
import com.cantos.theatre.domain.listening.SavedWork
import com.cantos.theatre.domain.listening.reduceListening
import com.cantos.theatre.domain.publication.EpisodeKey
import com.cantos.theatre.ports.ProgressOutcome
import com.cantos.theatre.ports.ProgressPort
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

enum class ListeningFailure { MemoryStoreUnavailable }

data class ListeningState(val snapshot: ListeningSnapshot, val failure: ListeningFailure? = null)

/** Serializes local writes. A failed mock store never becomes an accepted saved point. */
class ListeningController(
    scope: CoroutineScope,
    private val port: ProgressPort,
    private val initialSnapshot: ListeningSnapshot = ListeningSnapshot(),
) {
    private val mutableState = MutableStateFlow(ListeningState(initialSnapshot))
    val state: StateFlow<ListeningState> = mutableState.asStateFlow()
    private val commands = Channel<Command>(Channel.UNLIMITED)
    private var failedEvent: ListeningEvent? = null

    init {
        val loop = scope.launch {
            for (command in commands) {
                when (command) {
                    is Command.Apply -> apply(command.event)
                    Command.Retry -> {
                        val pending = failedEvent
                        if (pending != null) apply(pending) else if (mutableState.value.failure != null) {
                            commands.trySend(Command.Reload)
                        }
                    }
                    is Command.CaptureProgress -> {
                        val snapshot = mutableState.value.snapshot
                        val latest = snapshot.progress.asReversed().firstOrNull { it.episode.work == command.episode.work }
                        val record = if ((command.requireHistory && snapshot.history.none { it.episode == command.episode }) ||
                            (!command.requireHistory && latest?.episode != command.episode)
                        ) {
                            null
                        } else {
                            snapshot.progress.firstOrNull { it.episode == command.episode }
                        }
                        command.accept(record)
                    }
                    Command.Reload -> {
                        try {
                            val snapshot = port.load()
                            currentCoroutineContext().ensureActive()
                            mutableState.value = ListeningState(snapshot, pendingFailure())
                        } catch (cancelled: CancellationException) {
                            throw cancelled
                        } catch (_: Exception) {
                            currentCoroutineContext().ensureActive()
                            mutableState.value = mutableState.value.copy(failure = ListeningFailure.MemoryStoreUnavailable)
                        }
                    }
                }
            }
        }
        loop.invokeOnCompletion { commands.cancel() }
    }

    fun dispatch(event: ListeningEvent) {
        commands.trySend(Command.Apply(event))
    }

    fun reset(snapshot: ListeningSnapshot = initialSnapshot) {
        dispatch(ListeningEvent.Reset(snapshot))
    }

    fun reload() {
        commands.trySend(Command.Reload)
    }

    /** Retry the retained memory-only action against current state, never an old whole snapshot. */
    fun retry() {
        commands.trySend(Command.Retry)
    }

    /** Read accepted progress after preceding writes, rechecking a captured history row. */
    fun captureProgress(
        episode: EpisodeKey,
        requireHistory: Boolean = false,
        accept: (ProgressRecord?) -> Unit,
    ) {
        commands.trySend(Command.CaptureProgress(episode, requireHistory, accept))
    }

    private suspend fun apply(request: ListeningEvent) {
        val event = if (request is ListeningEvent.ToggleSaved) {
            ListeningEvent.SetSaved(request.work, SavedWork(request.work) !in mutableState.value.snapshot.savedWorks)
        } else {
            request
        }
        if (event is ListeningEvent.Reset) failedEvent = null
        val next = reduceListening(mutableState.value.snapshot, event)
        if (next == mutableState.value.snapshot && mutableState.value.failure == null) return
        val outcome = try {
            port.store(next)
        } catch (cancelled: CancellationException) {
            throw cancelled
        } catch (_: Exception) {
            ProgressOutcome.Unavailable
        }
        currentCoroutineContext().ensureActive()
        mutableState.value = when (outcome) {
            ProgressOutcome.Stored -> {
                if (event.supersedes(failedEvent)) failedEvent = null
                ListeningState(next, pendingFailure())
            }
            ProgressOutcome.Unavailable -> {
                if (event !is ListeningEvent.Reset && (event.isUserAction() || failedEvent?.isUserAction() != true)) {
                    failedEvent = event
                }
                mutableState.value.copy(failure = ListeningFailure.MemoryStoreUnavailable)
            }
        }
    }

    private fun pendingFailure(): ListeningFailure? =
        if (failedEvent == null) null else ListeningFailure.MemoryStoreUnavailable

    private fun ListeningEvent.isUserAction(): Boolean = when (this) {
        is ListeningEvent.SetSaved, is ListeningEvent.ToggleSaved, is ListeningEvent.AddBookmark,
        ListeningEvent.ClearHistory -> true
        is ListeningEvent.RecordProgress, is ListeningEvent.RecordStarted, is ListeningEvent.Reset -> false
    }

    private fun ListeningEvent.supersedes(previous: ListeningEvent?): Boolean = when (this) {
        is ListeningEvent.SetSaved -> previous is ListeningEvent.SetSaved && work == previous.work
        is ListeningEvent.ToggleSaved -> false // Normalized to desired state before applying.
        is ListeningEvent.AddBookmark -> previous is ListeningEvent.AddBookmark && bookmark == previous.bookmark
        // A single retained failure is not an outbox. Never replay an older automatic point
        // or start behind a newer accepted one, including a different work's later start.
        is ListeningEvent.RecordProgress, is ListeningEvent.RecordStarted ->
            previous is ListeningEvent.RecordProgress || previous is ListeningEvent.RecordStarted
        ListeningEvent.ClearHistory -> previous == ListeningEvent.ClearHistory || previous is ListeningEvent.RecordStarted
        is ListeningEvent.Reset -> true
    }

    private sealed interface Command {
        data class Apply(val event: ListeningEvent) : Command
        data class CaptureProgress(
            val episode: EpisodeKey,
            val requireHistory: Boolean,
            val accept: (ProgressRecord?) -> Unit,
        ) : Command
        data object Retry : Command
        data object Reload : Command
    }
}
