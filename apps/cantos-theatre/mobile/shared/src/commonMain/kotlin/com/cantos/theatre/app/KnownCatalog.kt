package com.cantos.theatre.app

import com.cantos.theatre.application.catalog.DetailPhase
import com.cantos.theatre.application.catalog.PublicCatalogState
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
        val seenWorks = LinkedHashMap<String, PublicWork>()
        val seenUnits = LinkedHashMap<String, KnownUnit>()
        state.works.items.forEach { seenWorks[it.id.value] = it }
        state.detail?.let { detail ->
            (detail.phase as? DetailPhase.Ready)?.work?.let { work ->
                seenWorks[work.id.value] = work
                detail.units?.items?.forEach { seenUnits[it.id.value] = KnownUnit(work.id, it) }
            }
        }
        val newWorks = seenWorks.filter { (id, work) -> works[id] != work }
        val newUnits = seenUnits.filter { (id, known) -> units[id]?.let { it.workId == known.workId && it.unit == known.unit } != true }
        if (newWorks.isEmpty() && newUnits.isEmpty()) return this
        val mergedWorks = LinkedHashMap(works).apply { newWorks.keys.forEach { remove(it) }; putAll(newWorks) }
        val mergedUnits = LinkedHashMap(units).apply { newUnits.keys.forEach { remove(it) }; putAll(newUnits) }
        return KnownCatalog(mergedWorks.bounded(MaxKnownWorks), mergedUnits.bounded(MaxKnownUnits))
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
