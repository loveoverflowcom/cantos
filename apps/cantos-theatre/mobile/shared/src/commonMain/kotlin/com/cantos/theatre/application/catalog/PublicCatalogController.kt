package com.cantos.theatre.application.catalog

import com.cantos.theatre.ports.PublicCatalogPort
import com.cantos.theatre.ports.PublicCatalogRead
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

/**
 * The single owner of the live public catalog for one screen lifetime. Intents and read results
 * are applied sequentially through [reducePublicCatalog]; reads run outside the state assignment
 * and come back as token-tagged events, so a superseded or cancelled read can never overwrite
 * newer state. A work-list read and a detail/unit read may be in flight at the same time; a newer
 * read on the same channel cancels the older one. It never seeds the sandbox inventory, starts
 * playback or touches progress.
 */
class PublicCatalogController(
    private val scope: CoroutineScope,
    private val port: PublicCatalogPort,
    private val onRefresh: () -> Unit = {},
) {
    private val mutableState = MutableStateFlow(PublicCatalogState.Initial)
    val state: StateFlow<PublicCatalogState> = mutableState.asStateFlow()
    private val events = Channel<PublicCatalogEvent>(Channel.UNLIMITED)
    private var worksRead: Job? = null
    private var detailRead: Job? = null
    private val loop: Job = scope.launch {
        for (event in events) {
            if (event == PublicCatalogIntent.RefreshWorks || event == PublicCatalogIntent.RefreshWork ||
                event == PublicCatalogIntent.RetryWorks || event == PublicCatalogIntent.RetryWork || event is PublicCatalogIntent.Query) onRefresh()
            val transition = reducePublicCatalog(mutableState.value, event)
            mutableState.value = transition.state
            transition.effects.forEach(::interpret)
        }
    }

    init {
        loop.invokeOnCompletion {
            events.cancel()
            worksRead?.cancel()
            detailRead?.cancel()
        }
    }

    fun dispatch(intent: PublicCatalogIntent) {
        events.trySend(intent)
    }

    /** Stops the owner and every in-flight read; later intents are ignored. */
    fun close() {
        loop.cancel()
    }

    private fun interpret(effect: PublicCatalogEffect) {
        when (effect) {
            is PublicCatalogEffect.FetchWorks -> {
                worksRead?.cancel()
                worksRead = read({ port.works(effect.cursor, effect.query) }) { WorksResolved(effect.token, it) }
            }
            is PublicCatalogEffect.FetchWork -> {
                detailRead?.cancel()
                detailRead = read({ port.work(effect.id) }) { WorkResolved(effect.token, it) }
            }
            is PublicCatalogEffect.FetchUnits -> {
                detailRead?.cancel()
                detailRead = read({ port.units(effect.work, effect.cursor) }) { UnitsResolved(effect.token, it) }
            }
            PublicCatalogEffect.CancelDetail -> {
                detailRead?.cancel()
                detailRead = null
            }
        }
    }

    private fun <T> read(call: suspend () -> PublicCatalogRead<T>, resolved: (PublicCatalogRead<T>) -> PublicCatalogEvent): Job =
        scope.launch {
            val outcome = try {
                call()
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (_: Exception) {
                // The port contract returns typed values; an escaped fault is contained here.
                PublicCatalogRead.Unavailable(retryable = true)
            }
            events.trySend(resolved(outcome))
        }
}
