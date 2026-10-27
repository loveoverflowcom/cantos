package com.cantos.theatre.app

import com.cantos.theatre.adapters.http.ApiOrigin
import com.cantos.theatre.adapters.http.JvmCatalogTransport
import com.cantos.theatre.adapters.http.PublicCatalogHttpAdapter
import com.cantos.theatre.domain.catalog.CatalogCursor
import com.cantos.theatre.domain.catalog.PublicUnit
import com.cantos.theatre.domain.catalog.PublicWork
import com.cantos.theatre.domain.catalog.unitKind
import com.cantos.theatre.ports.PublicCatalogRead
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import kotlin.test.fail
import kotlinx.coroutines.runBlocking
import org.junit.Assume
import org.junit.Test

/**
 * Opt-in live check against a running API (`CANTOS_THEATRE_LIVE_ORIGIN`, e.g.
 * `http://127.0.0.1:8090`). Skipped by default so `theatre-demo verify` stays hermetic. It uses
 * the production path: ApiOrigin + JvmCatalogTransport + PublicCatalogHttpAdapter, whose
 * domain mapping already refuses non-canonical ids, in-page duplicates and kind mismatches.
 */
class PublicCatalogLiveOriginTest {
    private val canonical = Regex("^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$")

    private fun <T> PublicCatalogRead<T>.ready(what: String): T = when (this) {
        is PublicCatalogRead.Ready -> value
        else -> fail("$what: $this")
    }

    @Test
    fun walks_every_work_and_every_unit_page_of_the_live_origin() = runBlocking {
        val raw = System.getenv("CANTOS_THEATRE_LIVE_ORIGIN")
        Assume.assumeTrue("CANTOS_THEATRE_LIVE_ORIGIN is not set", !raw.isNullOrEmpty())
        val port = PublicCatalogHttpAdapter(ApiOrigin.parse(raw!!), JvmCatalogTransport())

        val works = mutableListOf<PublicWork>()
        var cursor: CatalogCursor? = null
        var pages = 0
        do {
            val page = port.works(cursor).ready("works page ${pages + 1}")
            works += page.items
            cursor = page.next
            pages++
            assertTrue(pages <= 1_000, "work walk did not terminate")
        } while (cursor != null)
        assertEquals(works.size, works.map { it.id }.toSet().size, "duplicate work ids across pages")

        var totalUnits = 0
        val allUnitIds = mutableSetOf<String>()
        for (work in works) {
            assertTrue(canonical.matches(work.id.value), "non-canonical work id ${work.id.value}")
            val detail = port.work(work.id).ready("detail ${work.id.value}")
            assertEquals(work.id, detail.id)
            val units = mutableListOf<PublicUnit>()
            var unitCursor: CatalogCursor? = null
            var unitPages = 0
            do {
                val page = port.units(detail, unitCursor).ready("units ${work.id.value} page ${unitPages + 1}")
                assertEquals(work.id, page.work)
                units += page.items
                unitCursor = page.next
                unitPages++
                assertTrue(unitPages <= 1_000, "unit walk did not terminate for ${work.id.value}")
            } while (unitCursor != null)
            units.forEach { unit ->
                assertTrue(canonical.matches(unit.id.value), "non-canonical unit id ${unit.id.value}")
                unit.publicationId?.let { publication ->
                    assertTrue(canonical.matches(publication.value), "non-canonical publication id ${publication.value}")
                    assertTrue((unit.durationMs ?: 0) > 0)
                } ?: assertEquals(null, unit.durationMs)
                assertEquals(detail.kind.unitKind, unit.kind, "unit ${unit.id.value} kind for ${detail.kind}")
            }
            assertEquals(units.size, units.map { it.id }.toSet().size, "duplicate unit ids across pages of ${work.id.value}")
            units.forEach { assertTrue(allUnitIds.add(it.id.value), "unit ${it.id.value} listed under two works") }
            totalUnits += units.size
            println("work ${work.id.value} ${work.kind.name.lowercase()} units=${units.size} " +
                "first=${units.firstOrNull()?.id?.value ?: "-"} last=${units.lastOrNull()?.id?.value ?: "-"}")
        }
        println("total works=${works.size} work_pages=$pages units=$totalUnits")
    }
}
