package com.cantos.theatre.application.catalog

import com.cantos.theatre.domain.publication.CatalogInventory
import com.cantos.theatre.ports.CatalogOutcome
import com.cantos.theatre.ports.CatalogPort
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

enum class CatalogStatus { Ready, Loading, Empty, Unavailable }

data class CatalogState(
    val generation: Long = 0,
    val status: CatalogStatus = CatalogStatus.Ready,
    val lastKnownInventory: CatalogInventory = CatalogInventory(),
)

/** One sequential result owner, with refresh I/O outside the pure state assignment. */
class CatalogController(
    private val scope: CoroutineScope,
    private val port: CatalogPort,
    private val initialInventory: CatalogInventory = CatalogInventory(),
) {
    private val mutableState = MutableStateFlow(
        CatalogState(
            status = if (initialInventory.works.isEmpty()) CatalogStatus.Empty else CatalogStatus.Ready,
            lastKnownInventory = initialInventory,
        ),
    )
    val state: StateFlow<CatalogState> = mutableState.asStateFlow()
    private val commands = Channel<Command>(Channel.UNLIMITED)
    private var request: Job? = null

    init {
        val loop = scope.launch {
            for (command in commands) {
                when (command) {
                    Command.Refresh -> startRequest()
                    is Command.Reset -> {
                        request?.cancel()
                        val generation = nextGeneration()
                        mutableState.value = CatalogState(
                            generation = generation,
                            status = if (command.inventory.works.isEmpty()) CatalogStatus.Empty else CatalogStatus.Ready,
                            lastKnownInventory = command.inventory,
                        )
                    }
                    is Command.Resolved -> if (command.generation == mutableState.value.generation) {
                        val current = mutableState.value
                        mutableState.value = when (val outcome = command.outcome) {
                            is CatalogOutcome.Ready -> current.copy(
                                status = if (outcome.inventory.works.isEmpty()) CatalogStatus.Empty else CatalogStatus.Ready,
                                lastKnownInventory = outcome.inventory,
                            )
                            CatalogOutcome.Empty -> current.copy(status = CatalogStatus.Empty)
                            CatalogOutcome.Unavailable -> current.copy(status = CatalogStatus.Unavailable)
                        }
                    }
                }
            }
        }
        loop.invokeOnCompletion { commands.cancel() }
    }

    fun refresh() {
        commands.trySend(Command.Refresh)
    }

    fun reset(inventory: CatalogInventory = initialInventory) {
        commands.trySend(Command.Reset(inventory))
    }

    private fun nextGeneration(): Long {
        check(mutableState.value.generation < Long.MAX_VALUE) { "Catalog generation exhausted" }
        return mutableState.value.generation + 1
    }

    private fun startRequest() {
        request?.cancel()
        val generation = nextGeneration()
        mutableState.value = mutableState.value.copy(generation = generation, status = CatalogStatus.Loading)
        request = scope.launch {
            val outcome = try {
                port.load()
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (_: Exception) {
                CatalogOutcome.Unavailable
            }
            commands.trySend(Command.Resolved(generation, outcome))
        }
    }

    private sealed interface Command {
        data object Refresh : Command
        data class Reset(val inventory: CatalogInventory) : Command
        data class Resolved(val generation: Long, val outcome: CatalogOutcome) : Command
    }
}
