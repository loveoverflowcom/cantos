package com.cantos.theatre.app

import com.cantos.theatre.application.catalog.DetailPhase
import com.cantos.theatre.application.catalog.PublicCatalogState
import com.cantos.theatre.application.catalog.CatalogFailure
import com.cantos.theatre.application.catalog.WalkPhase
import com.cantos.theatre.domain.catalog.PublicUnit
import com.cantos.theatre.domain.catalog.PublicWork
import com.cantos.theatre.domain.catalog.PublicWorkId

/** A released unit together with the work it was read under. */
class KnownUnit(val workId: PublicWorkId, val unit: PublicUnit)

/**
 * The works and units this process has actually been shown by the API, so a Library row, the
 * mini-player or a Saved shelf can name them after the list or detail that listed them has moved
 * on. It is an immutable, bounded snapshot of server values: it never invents a title, and the
 * oldest entries are dropped first. A unit read again replaces the earlier one (a new release).
 */
class KnownCatalog private constructor(
    private val works: Map<String, PublicWork>,
    private val units: Map<String, KnownUnit>,
) {
    constructor() : this(emptyMap(), emptyMap())

    fun work(id: String): PublicWork? = works[id]
    fun unit(id: String): KnownUnit? = units[id]

    /** Every unit of [workId] seen so far, in server display order. */
    fun unitsOf(workId: String): List<PublicUnit> = units.values.filter { it.workId.value == workId }
        .map { it.unit }.sortedWith(compareBy<PublicUnit> { it.position }.thenBy { it.id.value })

    /** Returns this instance when [state] shows nothing new, so an unchanged catalog emits nothing. */
    fun merge(state: PublicCatalogState): KnownCatalog {
        val listFailure = (state.works.phase as? WalkPhase.Failed)?.failure
        if (listFailure == CatalogFailure.CursorRejected || listFailure is CatalogFailure.Access) return KnownCatalog()
        val detailFailure = (state.detail?.phase as? DetailPhase.Failed)?.failure
        val hiddenWork = state.detail?.id?.value?.takeIf {
            detailFailure == CatalogFailure.CursorRejected || detailFailure == CatalogFailure.NotFound || detailFailure is CatalogFailure.Access
        }
        val baseWorks = if (hiddenWork == null) works else works.filterKeys { it != hiddenWork }
        // A fresh detail walk replaces its known sequence, including while it is loading.
        // Retaining chapters absent from the newly admitted page would revive hidden queue rows.
        val currentWork = state.detail?.id?.value
        val baseUnits = if (currentWork == null) units else units.filterValues { it.workId.value != currentWork }
        val seenWorks = LinkedHashMap<String, PublicWork>()
        val seenUnits = LinkedHashMap<String, KnownUnit>()
        state.works.items.filterNot { it.id.value == hiddenWork }.forEach { seenWorks[it.id.value] = it }
        state.detail?.let { detail ->
            (detail.phase as? DetailPhase.Ready)?.work?.let { work ->
                seenWorks[work.id.value] = work
                detail.units?.items?.forEach { seenUnits[it.id.value] = KnownUnit(work.id, it) }
            }
        }
        val newWorks = seenWorks.filter { (id, work) -> baseWorks[id] != work }
        val newUnits = seenUnits.filter { (id, known) -> baseUnits[id]?.let { it.workId == known.workId && it.unit == known.unit } != true }
        if (newWorks.isEmpty() && newUnits.isEmpty() && baseWorks.size == works.size && baseUnits.size == units.size) return this
        val mergedWorks = LinkedHashMap(baseWorks).apply { newWorks.keys.forEach { remove(it) }; putAll(newWorks) }
        val mergedUnits = LinkedHashMap(baseUnits).apply { newUnits.keys.forEach { remove(it) }; putAll(newUnits) }
        val boundedWorks = mergedWorks.bounded(MaxKnownWorks)
        val boundedUnits = mergedUnits.bounded(MaxKnownUnits)
        if (boundedWorks == works && boundedUnits.size == units.size && boundedUnits.all { (id, known) ->
                units[id]?.let { it.workId == known.workId && it.unit == known.unit } == true
            }) return this
        return KnownCatalog(boundedWorks, boundedUnits)
    }

    private fun <V> LinkedHashMap<String, V>.bounded(limit: Int): Map<String, V> {
        if (size <= limit) return this
        val drop = size - limit
        return entries.drop(drop).associate { it.key to it.value }
    }

    private companion object {
        const val MaxKnownWorks = 1_000
        const val MaxKnownUnits = 5_000
    }
}
