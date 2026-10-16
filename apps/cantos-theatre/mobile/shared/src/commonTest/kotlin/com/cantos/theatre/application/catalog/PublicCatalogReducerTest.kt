package com.cantos.theatre.application.catalog

import com.cantos.theatre.domain.catalog.PublicCatalogSamples.cursor
import com.cantos.theatre.domain.catalog.PublicCatalogSamples.unit
import com.cantos.theatre.domain.catalog.PublicCatalogSamples.unitsPage
import com.cantos.theatre.domain.catalog.PublicCatalogSamples.work
import com.cantos.theatre.domain.catalog.PublicCatalogSamples.workId
import com.cantos.theatre.domain.catalog.PublicCatalogSamples.worksPage
import com.cantos.theatre.domain.catalog.PublicUnitPage
import com.cantos.theatre.domain.catalog.PublicWork
import com.cantos.theatre.ports.CatalogProtocolFault
import com.cantos.theatre.ports.PublicCatalogRead
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs
import kotlin.test.assertNull
import kotlin.test.assertSame
import kotlin.test.assertTrue

class PublicCatalogReducerTest {
    private var state = PublicCatalogState.Initial
    private val effects = mutableListOf<PublicCatalogEffect>()

    private fun send(event: PublicCatalogEvent): List<PublicCatalogEffect> {
        val transition = reducePublicCatalog(state, event)
        state = transition.state
        effects += transition.effects
        return transition.effects
    }

    private fun lastWorksToken(): Long = (effects.last { it is PublicCatalogEffect.FetchWorks } as PublicCatalogEffect.FetchWorks).token
    private fun lastUnits(): PublicCatalogEffect.FetchUnits = effects.last { it is PublicCatalogEffect.FetchUnits } as PublicCatalogEffect.FetchUnits

    @Test
    fun walk_loads_pages_keeps_rows_on_failure_and_retries_the_same_cursor() {
        assertEquals(listOf(PublicCatalogEffect.FetchWorks(1, null)), send(PublicCatalogIntent.Start))
        assertEquals(WalkPhase.Loading, state.works.phase)
        assertEquals(emptyList(), send(PublicCatalogIntent.Start), "Start is idempotent")
        send(WorksResolved(1, PublicCatalogRead.Ready(worksPage(listOf(work(1), work(2)), cursor(1)))))
        assertEquals(listOf(work(1), work(2)), state.works.items)
        assertEquals(WalkPhase.More, state.works.phase)

        assertEquals(listOf(PublicCatalogEffect.FetchWorks(2, cursor(1))), send(PublicCatalogIntent.LoadMoreWorks))
        assertEquals(emptyList(), send(PublicCatalogIntent.LoadMoreWorks), "no second request while loading")
        send(WorksResolved(2, PublicCatalogRead.Transport))
        assertEquals(WalkPhase.Failed(CatalogFailure.Transport), state.works.phase)
        assertEquals(listOf(work(1), work(2)), state.works.items, "a failed page keeps loaded rows")

        // A read never consumes the cursor: retry resends the exact same one.
        assertEquals(listOf(PublicCatalogEffect.FetchWorks(3, cursor(1))), send(PublicCatalogIntent.RetryWorks))
        send(WorksResolved(3, PublicCatalogRead.Ready(worksPage(listOf(work(3)), null))))
        assertEquals(listOf(work(1), work(2), work(3)), state.works.items)
        assertEquals(WalkPhase.Complete, state.works.phase)
        assertEquals(emptyList(), send(PublicCatalogIntent.LoadMoreWorks), "the end has no next page")
        assertTrue(!state.works.isEmptyResult)
    }

    @Test
    fun only_a_terminal_empty_page_is_an_empty_result() {
        send(PublicCatalogIntent.Start)
        assertTrue(!state.works.isEmptyResult, "loading is not empty")
        // An empty intermediate page is followed automatically, never shown as the end.
        assertEquals(listOf(PublicCatalogEffect.FetchWorks(2, cursor(7))),
            send(WorksResolved(1, PublicCatalogRead.Ready(worksPage(emptyList(), cursor(7))))))
        assertEquals(WalkPhase.Loading, state.works.phase)
        assertTrue(!state.works.isEmptyResult)
        send(WorksResolved(2, PublicCatalogRead.Ready(worksPage(emptyList(), null))))
        assertTrue(state.works.isEmptyResult)
    }

    @Test
    fun automatic_following_of_pages_without_new_rows_is_bounded() {
        send(PublicCatalogIntent.Start)
        repeat(MaxAutoFollows) { index ->
            val follow = send(WorksResolved(lastWorksToken(), PublicCatalogRead.Ready(worksPage(emptyList(), cursor(index + 1)))))
            assertEquals(listOf(PublicCatalogEffect.FetchWorks(lastWorksToken(), cursor(index + 1))), follow)
        }
        assertEquals(emptyList(), send(WorksResolved(lastWorksToken(), PublicCatalogRead.Ready(worksPage(emptyList(), cursor(99))))))
        assertEquals(WalkPhase.More, state.works.phase, "the user may continue; it is not the end")
        assertEquals(listOf(PublicCatalogEffect.FetchWorks(lastWorksToken() + 1, cursor(99))), send(PublicCatalogIntent.LoadMoreWorks))
    }

    @Test
    fun invalid_cursor_restarts_from_page_one_and_deduplicates_by_id() {
        send(PublicCatalogIntent.Start)
        send(WorksResolved(1, PublicCatalogRead.Ready(worksPage(listOf(work(1), work(2)), cursor(1)))))
        send(PublicCatalogIntent.LoadMoreWorks)
        // Drop the cursor and restart from page one, keeping the rows on screen.
        assertEquals(listOf(PublicCatalogEffect.FetchWorks(3, null)), send(WorksResolved(2, PublicCatalogRead.InvalidCursor)))
        assertEquals(listOf(work(1), work(2)), state.works.items)
        val renamed = work(1, title = "Ánh đèn cuối sân khấu — bản mới")
        // Page one adds no new id, so the walk continues to the fresh page-two cursor.
        assertEquals(listOf(PublicCatalogEffect.FetchWorks(4, cursor(5))),
            send(WorksResolved(3, PublicCatalogRead.Ready(worksPage(listOf(renamed, work(2)), cursor(5))))))
        send(WorksResolved(4, PublicCatalogRead.Ready(worksPage(listOf(work(2), work(3)), null))))
        assertEquals(listOf(renamed, work(2), work(3)), state.works.items, "replaced in place, no duplicate rows")
        assertEquals(WalkPhase.Complete, state.works.phase)
    }

    @Test
    fun a_restarted_walk_keeps_server_order_for_ids_it_had_not_shown() {
        send(PublicCatalogIntent.Start)
        send(WorksResolved(1, PublicCatalogRead.Ready(worksPage(listOf(work(1), work(2)), cursor(1)))))
        send(PublicCatalogIntent.LoadMoreWorks)
        send(WorksResolved(2, PublicCatalogRead.Ready(worksPage(listOf(work(3)), cursor(2)))))
        send(PublicCatalogIntent.LoadMoreWorks)
        send(WorksResolved(3, PublicCatalogRead.InvalidCursor))
        // Work 9 was first published during the walk, so the server now lists it first.
        send(WorksResolved(4, PublicCatalogRead.Ready(worksPage(listOf(work(9), work(1), work(2)), cursor(5)))))
        assertEquals(listOf(work(9), work(1), work(2), work(3)), state.works.items)
        send(PublicCatalogIntent.LoadMoreWorks)
        // A continuation inserts after the last row it placed: 8 sits between 2 and 3.
        send(WorksResolved(5, PublicCatalogRead.Ready(worksPage(listOf(work(8), work(3)), null))))
        assertEquals(listOf(work(9), work(1), work(2), work(8), work(3)), state.works.items)
        assertEquals(WalkPhase.Complete, state.works.phase)
    }

    @Test
    fun repeated_invalid_cursor_in_one_action_stops_and_retry_restarts() {
        send(PublicCatalogIntent.Start)
        send(WorksResolved(1, PublicCatalogRead.Ready(worksPage(listOf(work(1)), cursor(1)))))
        send(PublicCatalogIntent.LoadMoreWorks)
        send(WorksResolved(2, PublicCatalogRead.InvalidCursor))
        send(WorksResolved(3, PublicCatalogRead.Ready(worksPage(listOf(work(1)), cursor(2)))))
        assertEquals(emptyList(), send(WorksResolved(4, PublicCatalogRead.InvalidCursor)))
        assertEquals(WalkPhase.Failed(CatalogFailure.CursorRejected), state.works.phase)
        assertEquals(listOf(work(1)), state.works.items)
        assertEquals(listOf(PublicCatalogEffect.FetchWorks(5, null)), send(PublicCatalogIntent.RetryWorks))
    }

    @Test
    fun invalid_cursor_without_a_cursor_is_a_protocol_failure() {
        send(PublicCatalogIntent.Start)
        assertEquals(emptyList(), send(WorksResolved(1, PublicCatalogRead.InvalidCursor)))
        assertEquals(WalkPhase.Failed(CatalogFailure.Protocol), state.works.phase)
    }

    @Test
    fun superseded_and_unknown_results_are_dropped_without_changing_state() {
        send(PublicCatalogIntent.Start)
        send(PublicCatalogIntent.RefreshWorks)
        val before = state
        assertEquals(emptyList(), send(WorksResolved(1, PublicCatalogRead.Ready(worksPage(listOf(work(9)), null)))))
        assertSame(before, state)
        send(WorkResolved(42, PublicCatalogRead.NotFound))
        send(UnitsResolved(43, PublicCatalogRead.Transport))
        assertSame(before, state)
        send(WorksResolved(2, PublicCatalogRead.Ready(worksPage(listOf(work(1)), null))))
        assertEquals(listOf(work(1)), state.works.items)
    }

    @Test
    fun refresh_replaces_rows_on_success_and_keeps_them_on_failure() {
        send(PublicCatalogIntent.Start)
        send(WorksResolved(1, PublicCatalogRead.Ready(worksPage(listOf(work(1), work(2)), null))))
        send(PublicCatalogIntent.RefreshWorks)
        assertEquals(listOf(work(1), work(2)), state.works.items, "stale rows stay visible while refreshing")
        send(WorksResolved(2, PublicCatalogRead.Unavailable(retryable = true)))
        assertEquals(WalkPhase.Failed(CatalogFailure.Unavailable(true)), state.works.phase)
        assertEquals(listOf(work(1), work(2)), state.works.items)
        assertEquals(listOf(PublicCatalogEffect.FetchWorks(3, null)), send(PublicCatalogIntent.RetryWorks))
        send(WorksResolved(3, PublicCatalogRead.Ready(worksPage(listOf(work(2)), null))))
        assertEquals(listOf(work(2)), state.works.items, "a fresh walk drops rows no longer listed")
    }

    private fun openReady(work: PublicWork) {
        send(PublicCatalogIntent.OpenWork(work))
        val fetch = effects.last() as PublicCatalogEffect.FetchWork
        assertEquals(work.id, fetch.id)
        send(WorkResolved(fetch.token, PublicCatalogRead.Ready(work)))
    }

    @Test
    fun detail_loads_then_walks_units_of_that_exact_work() {
        val story = work(1)
        send(PublicCatalogIntent.OpenWork(story))
        val detail = state.detail!!
        assertEquals(DetailPhase.Loading, detail.phase)
        assertEquals(story, detail.summary)
        send(WorkResolved(1, PublicCatalogRead.Ready(story)))
        assertEquals(DetailPhase.Ready(story), state.detail!!.phase)
        assertEquals(PublicCatalogEffect.FetchUnits(2, story, null), lastUnits())

        send(UnitsResolved(2, PublicCatalogRead.Ready(unitsPage(story, listOf(unit(1), unit(2)), cursor(3)))))
        assertEquals(listOf(unit(1), unit(2)), state.detail!!.units!!.items)
        assertEquals(listOf(PublicCatalogEffect.FetchUnits(3, story, cursor(3))), send(PublicCatalogIntent.LoadMoreUnits))
        send(UnitsResolved(3, PublicCatalogRead.Unavailable(retryable = true)))
        assertEquals(listOf(unit(1), unit(2)), state.detail!!.units!!.items)
        assertEquals(listOf(PublicCatalogEffect.FetchUnits(4, story, cursor(3))), send(PublicCatalogIntent.RetryUnits))
        // A changed publication for a known unit is a new release that replaces its row.
        val republished = unit(2, publication = 20)
        send(UnitsResolved(4, PublicCatalogRead.Ready(unitsPage(story, listOf(republished, unit(3)), null))))
        assertEquals(listOf(unit(1), republished, unit(3)), state.detail!!.units!!.items)
        assertEquals(WalkPhase.Complete, state.detail!!.units!!.phase)
    }

    @Test
    fun unit_invalid_cursor_restarts_that_work_from_page_one() {
        val story = work(1)
        openReady(story)
        send(UnitsResolved(lastUnits().token, PublicCatalogRead.Ready(unitsPage(story, listOf(unit(1)), cursor(1)))))
        send(PublicCatalogIntent.LoadMoreUnits)
        send(UnitsResolved(lastUnits().token, PublicCatalogRead.InvalidCursor))
        assertEquals(null, lastUnits().cursor)
        assertEquals(story, lastUnits().work)
    }

    @Test
    fun back_and_reopen_fence_late_detail_and_unit_results() {
        val first = work(1)
        val second = work(2)
        send(PublicCatalogIntent.OpenWork(first))
        val firstToken = (effects.last() as PublicCatalogEffect.FetchWork).token
        assertEquals(listOf(PublicCatalogEffect.CancelDetail), send(PublicCatalogIntent.Back))
        assertNull(state.detail)
        val listRoute = state
        send(WorkResolved(firstToken, PublicCatalogRead.Ready(first)))
        assertSame(listRoute, state, "a late detail cannot reopen a closed route")

        send(PublicCatalogIntent.OpenWork(first))
        val reopened = (effects.last() as PublicCatalogEffect.FetchWork).token
        val switched = send(PublicCatalogIntent.OpenWork(second))
        assertEquals(PublicCatalogEffect.CancelDetail, switched.first())
        send(WorkResolved(reopened, PublicCatalogRead.Ready(first)))
        assertEquals(second.id, state.detail!!.id)
        assertEquals(DetailPhase.Loading, state.detail!!.phase)
        assertEquals(emptyList(), send(PublicCatalogIntent.Back).filterNot { it == PublicCatalogEffect.CancelDetail })
        assertEquals(emptyList(), send(PublicCatalogIntent.Back), "Back on the list is a no-op")
    }

    @Test
    fun detail_not_found_and_failures_are_explicit_and_retryable() {
        send(PublicCatalogIntent.OpenWork(work(1)))
        send(WorkResolved(1, PublicCatalogRead.NotFound))
        assertEquals(DetailPhase.Failed(CatalogFailure.NotFound), state.detail!!.phase)
        assertNull(state.detail!!.units)
        assertEquals(listOf(PublicCatalogEffect.FetchWork(2, workId(1))), send(PublicCatalogIntent.RetryWork))
        send(WorkResolved(2, PublicCatalogRead.Protocol(CatalogProtocolFault.Invalid)))
        assertEquals(DetailPhase.Failed(CatalogFailure.Protocol), state.detail!!.phase)
        assertEquals(emptyList(), send(PublicCatalogIntent.LoadMoreUnits))
    }

    @Test
    fun a_unit_page_for_another_work_is_refused_even_from_a_foreign_port() {
        val story = work(1)
        openReady(story)
        val foreign: PublicUnitPage = unitsPage(work(2), listOf(unit(1)))
        send(UnitsResolved(lastUnits().token, PublicCatalogRead.Ready(foreign)))
        assertEquals(WalkPhase.Failed(CatalogFailure.Protocol), state.detail!!.units!!.phase)
        assertTrue(state.detail!!.units!!.items.isEmpty())
    }

    @Test
    fun units_not_found_after_detail_keeps_the_detail_and_reports_the_list() {
        val story = work(1)
        openReady(story)
        send(UnitsResolved(lastUnits().token, PublicCatalogRead.NotFound))
        assertIs<DetailPhase.Ready>(state.detail!!.phase)
        assertEquals(WalkPhase.Failed(CatalogFailure.NotFound), state.detail!!.units!!.phase)
    }
}
