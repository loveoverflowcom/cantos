package com.cantos.theatre.application.catalog

import com.cantos.theatre.domain.catalog.PublicCatalogQuery
import com.cantos.theatre.domain.catalog.CatalogCursor
import com.cantos.theatre.domain.catalog.PublicUnit
import com.cantos.theatre.domain.catalog.PublicUnitPage
import com.cantos.theatre.domain.catalog.PublicWork
import com.cantos.theatre.domain.catalog.PublicWorkId
import com.cantos.theatre.domain.catalog.PublicWorkPage
import com.cantos.theatre.ports.CatalogProtocolFault
import com.cantos.theatre.ports.PublicCatalogRead

/*
 * Pure state machine for the live public catalog (Theatre Discovery v1 walk rules, ADR 0035 §4).
 * It is independent of the sandbox `CatalogInventory`: rows are keyed by server UUIDs only.
 *
 * - A cursor is not consumed by a read: a failed page keeps its rows and retries the same cursor.
 * - An empty intermediate page (or one that adds no new id) is not the end; the walk follows
 *   `next_cursor` automatically, at most [MaxAutoFollows] times per user action.
 * - `invalid_cursor` drops the cursor and restarts from page one, merging by id (no duplicate
 *   rows); a repeat within the same action stops with [CatalogFailure.CursorRejected].
 * - The same id with changed data (for a unit, a changed `publication_id`: a new release)
 *   replaces its row in place. No total count is ever inferred from a partial walk.
 * - Every request carries a fresh token; a result whose token is not awaited is stale and dropped.
 */

const val MaxAutoFollows: Int = 5
const val MaxCursorRestarts: Int = 1

sealed interface CatalogFailure {
    data class Access(val fault: com.cantos.theatre.ports.CatalogAccessFault) : CatalogFailure
    data object NotFound : CatalogFailure
    data class Unavailable(val retryable: Boolean) : CatalogFailure
    data object Protocol : CatalogFailure
    data object Transport : CatalogFailure
    /** The server rejected fresh cursors repeatedly during one action (e.g. it restarted). */
    data object CursorRejected : CatalogFailure
}

sealed interface WalkPhase {
    data object NotStarted : WalkPhase
    /** A request for [PageWalk.requested] is in flight. */
    data object Loading : WalkPhase
    /** Idle with a next page available. */
    data object More : WalkPhase
    /** The server answered `next_cursor: null`. */
    data object Complete : WalkPhase
    /** The request for [PageWalk.requested] failed; rows are kept and Retry resends it. */
    data class Failed(val failure: CatalogFailure) : WalkPhase
}

/** One keyset walk. Construction is internal to the reducer; the UI only reads it. */
@ConsistentCopyVisibility
data class PageWalk<T> internal constructor(
    val items: List<T> = emptyList(),
    val phase: WalkPhase = WalkPhase.NotStarted,
    /** Whether any page was accepted; distinguishes a real empty result from not-yet-loaded. */
    val received: Boolean = false,
    internal val next: CatalogCursor? = null,
    internal val requested: CatalogCursor? = null,
    internal val awaiting: Long? = null,
    internal val replaceOnFirstPage: Boolean = false,
    internal val autoFollows: Int = 0,
    internal val restarts: Int = 0,
    /** Where the current walk inserts ids it has not shown yet, so server order is kept. */
    internal val mergeAt: Int = 0,
) {
    val isEmptyResult: Boolean get() = received && phase == WalkPhase.Complete && items.isEmpty()
    val canLoadMore: Boolean get() = phase == WalkPhase.More
}

sealed interface DetailPhase {
    data object Loading : DetailPhase
    data class Ready(val work: PublicWork) : DetailPhase
    data class Failed(val failure: CatalogFailure) : DetailPhase
}

@ConsistentCopyVisibility
data class WorkDetailState internal constructor(
    val id: PublicWorkId,
    /** The list row, shown while the authoritative detail loads; never used to fetch units. */
    val summary: PublicWork?,
    val phase: DetailPhase,
    /** Present once the detail is Ready. */
    val units: PageWalk<PublicUnit>?,
    internal val awaiting: Long?,
)

@ConsistentCopyVisibility
data class PublicCatalogState internal constructor(
    val works: PageWalk<PublicWork> = PageWalk(),
    val query: PublicCatalogQuery = PublicCatalogQuery.All,
    /** Query of the displayed snapshot, committed only with an accepted page. */
    val appliedQuery: PublicCatalogQuery? = null,
    /** `null` is the work list route; otherwise the open work. */
    val detail: WorkDetailState? = null,
    internal val issued: Long = 0,
) {
    companion object {
        val Initial: PublicCatalogState = PublicCatalogState()
    }
}

sealed interface PublicCatalogEvent

/** What the presentation may ask for; results can only come from the controller. */
sealed interface PublicCatalogIntent : PublicCatalogEvent {
    /** Load the first page once; repeated Start is a no-op. */
    data object Start : PublicCatalogIntent
    data object AccountChanged : PublicCatalogIntent
    data class Query(val query: PublicCatalogQuery) : PublicCatalogIntent
    data object RefreshWorks : PublicCatalogIntent
    data object LoadMoreWorks : PublicCatalogIntent
    data object RetryWorks : PublicCatalogIntent
    data class OpenWorkId(val id: PublicWorkId) : PublicCatalogIntent
    data class OpenWork(val work: PublicWork) : PublicCatalogIntent
    data object Back : PublicCatalogIntent
    data object RefreshWork : PublicCatalogIntent
    data object RetryWork : PublicCatalogIntent
    data object LoadMoreUnits : PublicCatalogIntent
    data object RetryUnits : PublicCatalogIntent
}

data class WorksResolved(val token: Long, val outcome: PublicCatalogRead<PublicWorkPage>) : PublicCatalogEvent
data class WorkResolved(val token: Long, val outcome: PublicCatalogRead<PublicWork>) : PublicCatalogEvent
data class UnitsResolved(val token: Long, val outcome: PublicCatalogRead<PublicUnitPage>) : PublicCatalogEvent

sealed interface PublicCatalogEffect {
    data class FetchWorks(val token: Long, val cursor: CatalogCursor?, val query: PublicCatalogQuery = PublicCatalogQuery.All) : PublicCatalogEffect
    data class FetchWork(val token: Long, val id: PublicWorkId) : PublicCatalogEffect
    data class FetchUnits(val token: Long, val work: PublicWork, val cursor: CatalogCursor?) : PublicCatalogEffect
    /** Cancel any in-flight detail or unit read; its result would be stale anyway. */
    data object CancelDetail : PublicCatalogEffect
}

data class PublicCatalogTransition(val state: PublicCatalogState, val effects: List<PublicCatalogEffect> = emptyList())

fun reducePublicCatalog(state: PublicCatalogState, event: PublicCatalogEvent): PublicCatalogTransition = when (event) {
    PublicCatalogIntent.Start ->
        if (state.works.phase == WalkPhase.NotStarted) state.requestWorks(null, replace = false) else PublicCatalogTransition(state)
    PublicCatalogIntent.AccountChanged -> {
        val requested = state.copy(works = PageWalk(), appliedQuery = null, detail = null).requestWorks(null, replace = true)
        requested.copy(effects = listOf(PublicCatalogEffect.CancelDetail) + requested.effects)
    }
    is PublicCatalogIntent.Query ->
        if (event.query == state.query && state.works.phase == WalkPhase.Loading) PublicCatalogTransition(state)
        else state.copy(query = event.query, works = PageWalk(items = state.works.items)).requestWorks(null, replace = true)
    PublicCatalogIntent.RefreshWorks -> state.requestWorks(null, replace = true)
    PublicCatalogIntent.LoadMoreWorks ->
        if (state.works.canLoadMore) state.requestWorks(state.works.next, replace = false)
        else PublicCatalogTransition(state)
    PublicCatalogIntent.RetryWorks -> when (state.works.phase) {
        is WalkPhase.Failed -> state.requestWorks(state.works.requested, replace = state.works.replaceOnFirstPage)
        else -> PublicCatalogTransition(state)
    }
    is PublicCatalogIntent.OpenWork -> {
        val (token, issued) = state.issue()
        PublicCatalogTransition(
            issued.copy(detail = WorkDetailState(event.work.id, event.work, DetailPhase.Loading, units = null, awaiting = token)),
            listOfNotNull(PublicCatalogEffect.CancelDetail.takeIf { state.detail != null }, PublicCatalogEffect.FetchWork(token, event.work.id)),
        )
    }
    is PublicCatalogIntent.OpenWorkId -> {
        val (token, issued) = state.issue()
        PublicCatalogTransition(
            issued.copy(detail = WorkDetailState(event.id, null, DetailPhase.Loading, units = null, awaiting = token)),
            listOfNotNull(PublicCatalogEffect.CancelDetail.takeIf { state.detail != null }, PublicCatalogEffect.FetchWork(token, event.id)),
        )
    }
    PublicCatalogIntent.Back ->
        if (state.detail == null) PublicCatalogTransition(state)
        else PublicCatalogTransition(state.copy(detail = null), listOf(PublicCatalogEffect.CancelDetail))
    PublicCatalogIntent.RefreshWork, PublicCatalogIntent.RetryWork -> {
        val detail = state.detail
        if (detail == null || (event == PublicCatalogIntent.RetryWork && detail.phase !is DetailPhase.Failed)) PublicCatalogTransition(state)
        else {
            val (token, issued) = state.issue()
            PublicCatalogTransition(
                issued.copy(detail = detail.copy(phase = DetailPhase.Loading, units = null, awaiting = token)),
                listOf(PublicCatalogEffect.FetchWork(token, detail.id)),
            )
        }
    }
    PublicCatalogIntent.LoadMoreUnits -> state.withUnits { units ->
        if (units.canLoadMore) Follow.Request(units.next, replace = false) else Follow.None
    }
    PublicCatalogIntent.RetryUnits -> state.withUnits { units ->
        if (units.phase is WalkPhase.Failed) Follow.Request(units.requested, units.replaceOnFirstPage) else Follow.None
    }
    is WorksResolved -> resolveWorks(state, event)
    is WorkResolved -> resolveWork(state, event)
    is UnitsResolved -> resolveUnits(state, event)
}

// --- works ----------------------------------------------------------------------------------

private fun PublicCatalogState.issue(): Pair<Long, PublicCatalogState> {
    check(issued < Long.MAX_VALUE) { "Catalog request tokens exhausted" }
    val token = issued + 1
    return token to copy(issued = token)
}

/** A user action: it resets the automatic follow/restart budgets. */
private fun PublicCatalogState.requestWorks(cursor: CatalogCursor?, replace: Boolean): PublicCatalogTransition {
    val (token, issued) = issue()
    val walk = works.request(token, cursor, replace, resetCounters = true)
    return PublicCatalogTransition(issued.copy(works = walk), listOf(PublicCatalogEffect.FetchWorks(token, cursor, query)))
}

private fun resolveWorks(state: PublicCatalogState, event: WorksResolved): PublicCatalogTransition {
    if (state.works.awaiting != event.token) return PublicCatalogTransition(state)
    // A refusal ends permission to retain this snapshot, including any cover it disclosed.
    if (event.outcome is PublicCatalogRead.AccessFailure) {
        val failure = CatalogFailure.Access(event.outcome.fault)
        return PublicCatalogTransition(state.copy(works = PageWalk(phase = WalkPhase.Failed(failure)), appliedQuery = null))
    }
    val step = state.works.resolve(event.outcome.toWalkOutcome { it.items to it.next }) { it.id }
    val applied = if (event.outcome is PublicCatalogRead.Ready) state.query else state.appliedQuery
    return when (val follow = step.follow) {
        Follow.None -> PublicCatalogTransition(state.copy(works = step.walk, appliedQuery = applied))
        is Follow.Request -> {
            val (token, issued) = state.issue()
            PublicCatalogTransition(
                issued.copy(works = step.walk.request(token, follow.cursor, follow.replace, resetCounters = false), appliedQuery = applied),
                listOf(PublicCatalogEffect.FetchWorks(token, follow.cursor, state.query)),
            )
        }
    }
}

// --- detail and units -----------------------------------------------------------------------

private fun resolveWork(state: PublicCatalogState, event: WorkResolved): PublicCatalogTransition {
    val detail = state.detail
    if (detail == null || detail.awaiting != event.token) return PublicCatalogTransition(state)
    return when (val outcome = event.outcome) {
        is PublicCatalogRead.Ready -> {
            // The HTTP adapter checks this too; a matching token cannot bind a foreign port's Work.
            if (outcome.value.id != detail.id) return state.failDetail(detail, CatalogFailure.Protocol)
            val (token, issued) = state.issue()
            val units = PageWalk<PublicUnit>().request(token, null, replace = false, resetCounters = true)
            PublicCatalogTransition(
                issued.copy(detail = detail.copy(phase = DetailPhase.Ready(outcome.value), units = units, awaiting = null)),
                listOf(PublicCatalogEffect.FetchUnits(token, outcome.value, null)),
            )
        }
        // A cursor is never sent for a detail read.
        is PublicCatalogRead.AccessFailure -> state.failDetail(detail, CatalogFailure.Access(outcome.fault))
        PublicCatalogRead.InvalidCursor -> state.failDetail(detail, CatalogFailure.Protocol)
        PublicCatalogRead.NotFound -> state.failDetail(detail, CatalogFailure.NotFound)
        is PublicCatalogRead.Unavailable -> state.failDetail(detail, CatalogFailure.Unavailable(outcome.retryable))
        is PublicCatalogRead.Protocol -> state.failDetail(detail, CatalogFailure.Protocol)
        PublicCatalogRead.Transport -> state.failDetail(detail, CatalogFailure.Transport)
    }
}

private fun PublicCatalogState.failDetail(detail: WorkDetailState, failure: CatalogFailure) =
    PublicCatalogTransition(copy(detail = detail.copy(phase = DetailPhase.Failed(failure), awaiting = null)))

private fun PublicCatalogState.withUnits(decide: (PageWalk<PublicUnit>) -> Follow): PublicCatalogTransition {
    val detail = detail ?: return PublicCatalogTransition(this)
    val work = (detail.phase as? DetailPhase.Ready)?.work ?: return PublicCatalogTransition(this)
    val units = detail.units ?: return PublicCatalogTransition(this)
    return when (val follow = decide(units)) {
        Follow.None -> PublicCatalogTransition(this)
        is Follow.Request -> {
            val (token, issued) = issue()
            PublicCatalogTransition(
                issued.copy(detail = detail.copy(units = units.request(token, follow.cursor, follow.replace, resetCounters = true))),
                listOf(PublicCatalogEffect.FetchUnits(token, work, follow.cursor)),
            )
        }
    }
}

private fun resolveUnits(state: PublicCatalogState, event: UnitsResolved): PublicCatalogTransition {
    val detail = state.detail ?: return PublicCatalogTransition(state)
    val work = (detail.phase as? DetailPhase.Ready)?.work ?: return PublicCatalogTransition(state)
    val units = detail.units
    if (units == null || units.awaiting != event.token) return PublicCatalogTransition(state)
    // The adapter already bound the page to this work; recheck so a foreign port cannot cross works.
    val outcome = event.outcome.let { read ->
        if (read is PublicCatalogRead.Ready && read.value.work != work.id) {
            PublicCatalogRead.Protocol(CatalogProtocolFault.Invalid)
        } else read
    }
    val step = units.resolve(outcome.toWalkOutcome { it.items to it.next }) { it.id }
    return when (val follow = step.follow) {
        Follow.None -> PublicCatalogTransition(state.copy(detail = detail.copy(units = step.walk)))
        is Follow.Request -> {
            val (token, issued) = state.issue()
            PublicCatalogTransition(
                issued.copy(detail = detail.copy(units = step.walk.request(token, follow.cursor, follow.replace, resetCounters = false))),
                listOf(PublicCatalogEffect.FetchUnits(token, work, follow.cursor)),
            )
        }
    }
}

// --- generic walk ---------------------------------------------------------------------------

private sealed interface Follow {
    data object None : Follow
    data class Request(val cursor: CatalogCursor?, val replace: Boolean) : Follow
}

private class WalkStep<T>(val walk: PageWalk<T>, val follow: Follow)

private sealed interface WalkOutcome<out T> {
    data class Page<T>(val items: List<T>, val next: CatalogCursor?) : WalkOutcome<T>
    data object InvalidCursor : WalkOutcome<Nothing>
    data class Failure(val failure: CatalogFailure) : WalkOutcome<Nothing>
}

private fun <P, T> PublicCatalogRead<P>.toWalkOutcome(page: (P) -> Pair<List<T>, CatalogCursor?>): WalkOutcome<T> = when (this) {
    is PublicCatalogRead.Ready -> page(value).let { (items, next) -> WalkOutcome.Page(items, next) }
    is PublicCatalogRead.AccessFailure -> WalkOutcome.Failure(CatalogFailure.Access(fault))
    PublicCatalogRead.InvalidCursor -> WalkOutcome.InvalidCursor
    PublicCatalogRead.NotFound -> WalkOutcome.Failure(CatalogFailure.NotFound)
    is PublicCatalogRead.Unavailable -> WalkOutcome.Failure(CatalogFailure.Unavailable(retryable))
    is PublicCatalogRead.Protocol -> WalkOutcome.Failure(CatalogFailure.Protocol)
    PublicCatalogRead.Transport -> WalkOutcome.Failure(CatalogFailure.Transport)
}

private fun <T> PageWalk<T>.request(token: Long, cursor: CatalogCursor?, replace: Boolean, resetCounters: Boolean): PageWalk<T> = copy(
    phase = WalkPhase.Loading,
    requested = cursor,
    awaiting = token,
    replaceOnFirstPage = replace,
    autoFollows = if (resetCounters) 0 else autoFollows,
    restarts = if (resetCounters) 0 else restarts,
)

private fun <T, K> PageWalk<T>.resolve(outcome: WalkOutcome<T>, key: (T) -> K): WalkStep<T> = when (outcome) {
    is WalkOutcome.Page -> {
        val base = if (requested == null && replaceOnFirstPage) emptyList() else items
        // Page one of a walk (fresh or restarted) merges from the top; a continuation resumes
        // after the last row the previous page of this walk placed.
        val start = if (requested == null) 0 else mergeAt
        val merge = mergeById(base, outcome.items, key, start)
        val accepted = copy(
            items = merge.items, received = true, next = outcome.next, awaiting = null,
            replaceOnFirstPage = false, mergeAt = merge.mergeAt,
        )
        val added = merge.added
        when {
            outcome.next == null -> WalkStep(accepted.copy(phase = WalkPhase.Complete, autoFollows = 0), Follow.None)
            added == 0 && autoFollows < MaxAutoFollows ->
                WalkStep(accepted.copy(autoFollows = autoFollows + 1), Follow.Request(outcome.next, replace = false))
            else -> WalkStep(accepted.copy(phase = WalkPhase.More, autoFollows = 0), Follow.None)
        }
    }
    WalkOutcome.InvalidCursor -> when {
        // No cursor was sent, so the server cannot have rejected one.
        requested == null -> WalkStep(copy(phase = WalkPhase.Failed(CatalogFailure.Protocol), awaiting = null), Follow.None)
        restarts < MaxCursorRestarts -> WalkStep(
            copy(next = null, awaiting = null, restarts = restarts + 1, autoFollows = 0),
            Follow.Request(null, replace = false),
        )
        else -> WalkStep(
            copy(phase = WalkPhase.Failed(CatalogFailure.CursorRejected), next = null, requested = null, awaiting = null),
            Follow.None,
        )
    }
    is WalkOutcome.Failure -> WalkStep(copy(phase = WalkPhase.Failed(outcome.failure), awaiting = null), Follow.None)
}

private class Merge<T>(val items: List<T>, val added: Int, val mergeAt: Int)

/**
 * Replace known ids in place and insert new ids where the server placed them: after the last
 * row this walk has already placed, starting at [start]. A restarted page one therefore puts a
 * newly visible id before the known rows that follow it; a plain continuation appends.
 */
private fun <T, K> mergeById(base: List<T>, page: List<T>, key: (T) -> K, start: Int): Merge<T> {
    val result = base.toMutableList()
    var at = start.coerceIn(0, result.size)
    var added = 0
    for (item in page) {
        val id = key(item)
        val known = result.indexOfFirst { key(it) == id }
        if (known >= 0) {
            result[known] = item
            at = maxOf(at, known + 1)
        } else {
            result.add(at, item)
            at += 1
            added++
        }
    }
    return Merge(result.toList(), added, at)
}
