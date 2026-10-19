package com.cantos.theatre.adapters.preview

import com.cantos.theatre.domain.catalog.CatalogCheck
import com.cantos.theatre.domain.catalog.CatalogCursor
import com.cantos.theatre.domain.catalog.PublicCatalogQuery
import com.cantos.theatre.domain.catalog.PublicUnitPage
import com.cantos.theatre.domain.catalog.PublicWork
import com.cantos.theatre.domain.catalog.PublicWorkId
import com.cantos.theatre.domain.catalog.PublicWorkPage
import com.cantos.theatre.ports.CatalogProtocolFault
import com.cantos.theatre.ports.PublicCatalogPort
import com.cantos.theatre.ports.PublicCatalogRead
import kotlinx.coroutines.awaitCancellation
import kotlinx.coroutines.delay

/** How the sample catalog answers; the workbench selects it from outside the product navigation. */
enum class PreviewCatalogScenario { Ready, Loading, Empty, Error }

/**
 * The anonymous public catalog over [PreviewFixtures]. It is the external boundary the production
 * [com.cantos.theatre.application.catalog.PublicCatalogController] reads: pages, search and the
 * "not found" merge follow the real port contract, and nothing here is reachable from native builds.
 */
class PreviewCatalogPort(
    scenario: PreviewCatalogScenario = PreviewCatalogScenario.Ready,
    /** Items per page; the default is one complete page like the small sample inventory. */
    private val pageSize: Int = 50,
    private val latencyMs: Long = 0,
) : PublicCatalogPort {
    private var selected = scenario
    var worksReads: Int = 0
        private set

    fun setScenario(scenario: PreviewCatalogScenario) { selected = scenario }

    override suspend fun works(cursor: CatalogCursor?, query: PublicCatalogQuery): PublicCatalogRead<PublicWorkPage> {
        worksReads += 1
        pause()?.let { return it }
        val matching = PreviewFixtures.works.map { it.work }.filter { it.matches(query) }
        val offset = if (cursor == null) 0 else offsetOf(cursor) ?: return PublicCatalogRead.InvalidCursor
        return page(matching, offset)
    }

    override suspend fun work(id: PublicWorkId): PublicCatalogRead<PublicWork> {
        pause()?.let { return it }
        return PreviewFixtures.work(id.value)?.let { PublicCatalogRead.Ready(it.work) } ?: PublicCatalogRead.NotFound
    }

    override suspend fun units(work: PublicWork, cursor: CatalogCursor?): PublicCatalogRead<PublicUnitPage> {
        pause()?.let { return it }
        val entry = PreviewFixtures.work(work.id.value) ?: return PublicCatalogRead.NotFound
        val offset = if (cursor == null) 0 else offsetOf(cursor) ?: return PublicCatalogRead.InvalidCursor
        val slice = entry.units.drop(offset).take(pageSize)
        val next = if (offset + slice.size < entry.units.size) cursorAt(offset + slice.size) else null
        return when (val built = PublicUnitPage.of(work, work.id, slice, next)) {
            is CatalogCheck.Valid -> PublicCatalogRead.Ready(built.value)
            is CatalogCheck.Invalid -> PublicCatalogRead.Protocol(CatalogProtocolFault.Invalid)
        }
    }

    /** Scenario gate shared by every read: Loading never answers, Error is a retryable outage. */
    private suspend fun pause(): PublicCatalogRead<Nothing>? {
        if (latencyMs > 0) delay(latencyMs)
        return when (selected) {
            PreviewCatalogScenario.Loading -> awaitCancellation()
            PreviewCatalogScenario.Error -> PublicCatalogRead.Unavailable(retryable = true)
            PreviewCatalogScenario.Ready, PreviewCatalogScenario.Empty -> null
        }
    }

    private fun page(matching: List<PublicWork>, offset: Int): PublicCatalogRead<PublicWorkPage> {
        val visible = if (selected == PreviewCatalogScenario.Empty) emptyList() else matching
        val slice = visible.drop(offset).take(pageSize)
        val next = if (offset + slice.size < visible.size) cursorAt(offset + slice.size) else null
        return when (val built = PublicWorkPage.of(slice, next)) {
            is CatalogCheck.Valid -> PublicCatalogRead.Ready(built.value)
            is CatalogCheck.Invalid -> PublicCatalogRead.Protocol(CatalogProtocolFault.Invalid)
        }
    }

    private fun PublicWork.matches(query: PublicCatalogQuery): Boolean =
        (query.kind == null || query.kind == kind) &&
            (query.text.isEmpty() || title.contains(query.text, ignoreCase = true) ||
                description.contains(query.text, ignoreCase = true))

    private fun cursorAt(offset: Int): CatalogCursor =
        (CatalogCursor.parse("lc1_" + offset.toString(16).padStart(64, '0')) as CatalogCheck.Valid).value

    private fun offsetOf(cursor: CatalogCursor): Int? = cursor.value.removePrefix("lc1_").toIntOrNull(16)
}
