package com.cantos.theatre.application.catalog

import com.cantos.theatre.domain.catalog.CatalogViolation
import com.cantos.theatre.domain.catalog.CatalogCheck
import com.cantos.theatre.domain.catalog.PublicCatalogQuery
import com.cantos.theatre.domain.catalog.PublicCatalogSamples.cursor
import com.cantos.theatre.domain.catalog.PublicCatalogSamples.unit
import com.cantos.theatre.domain.catalog.PublicCatalogSamples.unitsPage
import com.cantos.theatre.domain.catalog.PublicCatalogSamples.work
import com.cantos.theatre.domain.catalog.PublicCatalogSamples.worksPage
import com.cantos.theatre.domain.catalog.PublicWorkKind
import com.cantos.theatre.ports.PublicCatalogRead
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs

class PublicCatalogSearchTest {
    @Test
    fun a_new_server_query_retains_display_but_drops_the_old_cursor_and_late_result() {
        val initial = reducePublicCatalog(PublicCatalogState.Initial, PublicCatalogIntent.Start).state
        val more = reducePublicCatalog(initial, WorksResolved(1, PublicCatalogRead.Ready(worksPage(listOf(work(1)), cursor(1))))).state
        val pending = reducePublicCatalog(more, PublicCatalogIntent.LoadMoreWorks).state
        val query = assertIs<CatalogCheck.Valid<PublicCatalogQuery>>(PublicCatalogQuery.of("Ánh đèn", PublicWorkKind.Podcast)).value
        val search = reducePublicCatalog(pending, PublicCatalogIntent.Query(query))
        assertEquals(listOf(work(1)), search.state.works.items)
        assertEquals(query, search.state.query)
        assertEquals(PublicCatalogQuery.All, search.state.appliedQuery)
        assertEquals(listOf(PublicCatalogEffect.FetchWorks(3, null, query)), search.effects)
        val stale = reducePublicCatalog(search.state, WorksResolved(2, PublicCatalogRead.Ready(worksPage(listOf(work(2)), null))))
        assertEquals(search.state, stale.state)
        val result = reducePublicCatalog(stale.state, WorksResolved(3, PublicCatalogRead.Ready(worksPage(listOf(work(9)), cursor(9))))).state
        assertEquals(listOf(work(9)), result.works.items)
        assertEquals(query, result.appliedQuery)
        assertEquals(listOf(PublicCatalogEffect.FetchWorks(4, cursor(9), query)),
            reducePublicCatalog(result, PublicCatalogIntent.LoadMoreWorks).effects)
        assertEquals(listOf(PublicCatalogEffect.FetchWorks(4, null, query)),
            reducePublicCatalog(result, PublicCatalogIntent.RefreshWorks).effects)
    }

    @Test
    fun failed_replacement_keeps_applied_snapshot_and_retry_uses_new_query_without_old_cursor() {
        val started = reducePublicCatalog(PublicCatalogState.Initial, PublicCatalogIntent.Start).state
        val ready = reducePublicCatalog(started, WorksResolved(1, PublicCatalogRead.Ready(worksPage(listOf(work(1)), cursor(1))))).state
        val query = assertIs<CatalogCheck.Valid<PublicCatalogQuery>>(PublicCatalogQuery.of("new")).value
        val pending = reducePublicCatalog(ready, PublicCatalogIntent.Query(query)).state
        val failed = reducePublicCatalog(pending, WorksResolved(2, PublicCatalogRead.Transport)).state
        assertEquals(listOf(work(1)), failed.works.items)
        assertEquals(PublicCatalogQuery.All, failed.appliedQuery)
        assertEquals(WalkPhase.Failed(CatalogFailure.Transport), failed.works.phase)
        val retry = reducePublicCatalog(failed, PublicCatalogIntent.RetryWorks)
        assertEquals(listOf(PublicCatalogEffect.FetchWorks(3, null, query)), retry.effects)
        val empty = reducePublicCatalog(retry.state, WorksResolved(3, PublicCatalogRead.Ready(worksPage(emptyList(), null)))).state
        assertEquals(emptyList(), empty.works.items)
        assertEquals(query, empty.appliedQuery)
        assertEquals(true, empty.works.isEmptyResult)
    }

    @Test
    fun account_change_clears_retained_rows_and_fences_the_old_replacement() {
        val started = reducePublicCatalog(PublicCatalogState.Initial, PublicCatalogIntent.Start).state
        val ready = reducePublicCatalog(started, WorksResolved(1, PublicCatalogRead.Ready(worksPage(listOf(work(1)), cursor(1))))).state
        val pending = reducePublicCatalog(ready, PublicCatalogIntent.RefreshWorks).state
        val changed = reducePublicCatalog(pending, PublicCatalogIntent.AccountChanged)
        assertEquals(emptyList(), changed.state.works.items)
        assertEquals(null, changed.state.appliedQuery)
        assertEquals(changed.state, reducePublicCatalog(changed.state, WorksResolved(2, PublicCatalogRead.Ready(worksPage(listOf(work(9)), null)))).state)
    }

    @Test
    fun detail_refresh_rechecks_authoritative_work_and_restarts_units_without_old_releases() {
        val story = work(1)
        val opened = reducePublicCatalog(PublicCatalogState.Initial, PublicCatalogIntent.OpenWork(story)).state
        val detail = reducePublicCatalog(opened, WorkResolved(1, PublicCatalogRead.Ready(story))).state
        val units = reducePublicCatalog(detail, UnitsResolved(2, PublicCatalogRead.Ready(unitsPage(story, listOf(unit(1)))))).state
        val refresh = reducePublicCatalog(units, PublicCatalogIntent.RefreshWork)
        assertEquals(listOf(PublicCatalogEffect.FetchWork(3, story.id)), refresh.effects)
        assertEquals(null, refresh.state.detail?.units)
        val renewed = reducePublicCatalog(refresh.state, WorkResolved(3, PublicCatalogRead.Ready(story)))
        assertEquals(emptyList(), renewed.state.detail?.units?.items)
        assertEquals(listOf(PublicCatalogEffect.FetchUnits(4, story, null)), renewed.effects)
        val withdrawn = reducePublicCatalog(refresh.state, WorkResolved(3, PublicCatalogRead.NotFound)).state
        assertEquals(DetailPhase.Failed(CatalogFailure.NotFound), withdrawn.detail?.phase)
        assertEquals(null, withdrawn.detail?.units)
    }

    @Test
    fun query_factory_applies_the_contract_byte_bound_and_rejects_control_characters() {
        assertIs<CatalogCheck.Valid<PublicCatalogQuery>>(PublicCatalogQuery.of("á".repeat(256)))
        assertEquals(CatalogCheck.Invalid(CatalogViolation.InvalidSearch), PublicCatalogQuery.of("á".repeat(257)))
        assertEquals(CatalogCheck.Invalid(CatalogViolation.InvalidSearch), PublicCatalogQuery.of("a\nb"))
        assertEquals(PublicCatalogQuery.All, assertIs<CatalogCheck.Valid<PublicCatalogQuery>>(PublicCatalogQuery.of("  ")).value)
    }
}
