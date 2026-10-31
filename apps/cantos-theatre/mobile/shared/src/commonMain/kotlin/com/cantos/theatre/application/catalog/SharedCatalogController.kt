package com.cantos.theatre.application.catalog

import com.cantos.theatre.domain.catalog.CatalogCursor
import com.cantos.theatre.domain.catalog.PublicUnit
import com.cantos.theatre.domain.catalog.PublicWork
import com.cantos.theatre.domain.catalog.PublicWorkId
import com.cantos.theatre.ports.SharedCatalogFailure
import com.cantos.theatre.ports.SharedCatalogPort
import com.cantos.theatre.ports.SharedCatalogRead
import com.cantos.theatre.ports.SharedSessionKey
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

enum class SharedCatalogPhase { Idle, SignInRequired, Loading, Ready, Denied, Restart, Unavailable, Protocol }

/** Account/route RAM only. Never copied into KnownCatalog, Library or a checkpoint. */
@ConsistentCopyVisibility
data class SharedCatalogState internal constructor(
    val session: SharedSessionKey? = null,
    val locator: PublicWorkId? = null,
    val phase: SharedCatalogPhase = SharedCatalogPhase.Idle,
    val works: List<PublicWork> = emptyList(),
    val work: PublicWork? = null,
    val units: List<PublicUnit> = emptyList(),
    val cursor: CatalogCursor? = null,
) {
    override fun toString(): String = "SharedCatalogState([redacted])"
}

/** One current visible read, fenced even when a platform callback ignores cancellation. */
class SharedCatalogController(
    private val scope: CoroutineScope,
    private val port: SharedCatalogPort,
    private val currentSession: () -> SharedSessionKey?,
) {
    private val mutableState = MutableStateFlow(SharedCatalogState())
    val state = mutableState.asStateFlow()
    private var request: Job? = null
    private var generation = 0L
    private var closed = false
    private var visible = false
    private var locator: PublicWorkId? = null

    /** Synchronous admission check also covers the turn before the account collector runs. */
    fun visibleState(): SharedCatalogState = state.value.let { value ->
        if (value.session == currentSession()) value
        else SharedCatalogState(locator = locator, phase = if (currentSession() == null) SharedCatalogPhase.SignInRequired else SharedCatalogPhase.Idle)
    }

    fun show(id: PublicWorkId? = null) {
        if (closed) return
        val same = visible && locator == id && mutableState.value.session == currentSession()
        visible = true
        locator = id
        if (!same || mutableState.value.phase == SharedCatalogPhase.Idle) refresh()
    }

    fun sessionChanged() {
        if (closed || mutableState.value.session == currentSession()) return
        invalidate()
        mutableState.value = SharedCatalogState(locator = locator,
            phase = if (currentSession() == null) SharedCatalogPhase.SignInRequired else SharedCatalogPhase.Idle)
        if (visible) refresh()
    }

    fun refresh() {
        if (closed || !visible) return
        invalidate()
        readPage(append = false)
    }

    fun more() {
        if (closed || !visible || state.value.phase != SharedCatalogPhase.Ready || state.value.cursor == null) return
        readPage(append = true)
    }

    private fun readPage(append: Boolean) {
        val session = currentSession()
        val id = locator
        if (session == null) {
            mutableState.value = SharedCatalogState(locator = id, phase = SharedCatalogPhase.SignInRequired)
            return
        }
        val token = ++generation
        val previous = state.value.takeIf { append && it.session == session && it.locator == id }
        mutableState.value = (previous ?: SharedCatalogState(session, id)).copy(phase = SharedCatalogPhase.Loading)
        request = scope.launch {
            if (id == null) {
                when (val result = port.works(session, previous?.cursor)) {
                    is SharedCatalogRead.Refused -> refuse(token, session, result.reason)
                    is SharedCatalogRead.Ready -> if (owns(token, session)) {
                        val rows = previous?.works.orEmpty() + result.value.items
                        if (rows.size > MAX_ROWS) {
                            refuse(token, session, SharedCatalogFailure.Unavailable)
                        } else if (rows.map { it.id }.distinct().size != rows.size ||
                            (result.value.next != null && result.value.next == previous?.cursor)) {
                            refuse(token, session, SharedCatalogFailure.Protocol)
                        } else mutableState.value = SharedCatalogState(session, phase = SharedCatalogPhase.Ready,
                            works = rows.toList(), cursor = result.value.next)
                    }
                }
            } else {
                val work = previous?.work ?: when (val result = port.work(session, id)) {
                    is SharedCatalogRead.Refused -> { refuse(token, session, result.reason); return@launch }
                    is SharedCatalogRead.Ready -> result.value
                }
                if (!owns(token, session)) return@launch
                if (work.id != id) { refuse(token, session, SharedCatalogFailure.Protocol); return@launch }
                when (val result = port.units(session, work, previous?.cursor)) {
                    is SharedCatalogRead.Refused -> refuse(token, session, result.reason)
                    is SharedCatalogRead.Ready -> if (owns(token, session)) {
                        val rows = previous?.units.orEmpty() + result.value.items
                        if (rows.size > MAX_ROWS) {
                            refuse(token, session, SharedCatalogFailure.Unavailable)
                        } else if (result.value.work != id || rows.map { it.id }.distinct().size != rows.size ||
                            rows.zipWithNext().any { (a, b) -> a.position >= b.position } ||
                            (result.value.next != null && result.value.next == previous?.cursor)) {
                            refuse(token, session, SharedCatalogFailure.Protocol)
                        } else mutableState.value = SharedCatalogState(session, id, SharedCatalogPhase.Ready,
                            work = work, units = rows.toList(), cursor = result.value.next)
                    }
                }
            }
        }
    }

    private fun owns(token: Long, session: SharedSessionKey): Boolean =
        !closed && visible && token == generation && currentSession() == session

    private fun refuse(token: Long, session: SharedSessionKey, reason: SharedCatalogFailure) {
        if (!owns(token, session)) return
        // A denial/expired cursor discards every private row, including an earlier page.
        val phase = when (reason) {
            SharedCatalogFailure.SignInRequired -> SharedCatalogPhase.SignInRequired
            SharedCatalogFailure.Denied -> SharedCatalogPhase.Denied
            SharedCatalogFailure.Restart -> SharedCatalogPhase.Restart
            SharedCatalogFailure.Unavailable -> SharedCatalogPhase.Unavailable
            SharedCatalogFailure.Protocol -> SharedCatalogPhase.Protocol
        }
        mutableState.value = SharedCatalogState(session, locator, phase)
    }

    private fun invalidate() { generation += 1; request?.cancel(); request = null }

    fun leave() {
        invalidate()
        visible = false
        locator = null
        mutableState.value = SharedCatalogState()
    }

    fun close() { leave(); closed = true }

    companion object {
        /** Local RAM ceiling, never a server eligibility/count claim. Reaching it asks for a new walk. */
        const val MAX_ROWS = 1000
    }
}
