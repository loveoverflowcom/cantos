package com.cantos.theatre.application.listening

import com.cantos.theatre.domain.listening.ListeningEvent
import com.cantos.theatre.domain.listening.ListeningSnapshot
import com.cantos.theatre.domain.listening.reduceListening
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

    init {
        val loop = scope.launch {
            for (command in commands) {
                when (command) {
                    is Command.Apply -> apply(command.event)
                    Command.Reload -> {
                        try {
                            val snapshot = port.load()
                            currentCoroutineContext().ensureActive()
                            mutableState.value = ListeningState(snapshot)
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

    private suspend fun apply(event: ListeningEvent) {
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
            ProgressOutcome.Stored -> ListeningState(next)
            ProgressOutcome.Unavailable -> mutableState.value.copy(failure = ListeningFailure.MemoryStoreUnavailable)
        }
    }

    private sealed interface Command {
        data class Apply(val event: ListeningEvent) : Command
        data object Reload : Command
    }
}
