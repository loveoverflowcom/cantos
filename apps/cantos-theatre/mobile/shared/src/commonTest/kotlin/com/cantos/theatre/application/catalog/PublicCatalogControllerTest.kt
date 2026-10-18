package com.cantos.theatre.application.catalog

import com.cantos.theatre.domain.catalog.CatalogCursor
import com.cantos.theatre.domain.catalog.CatalogSearchQuery
import com.cantos.theatre.domain.catalog.PublicCatalogSamples.cursor
import com.cantos.theatre.domain.catalog.PublicCatalogSamples.unit
import com.cantos.theatre.domain.catalog.PublicCatalogSamples.unitsPage
import com.cantos.theatre.domain.catalog.PublicCatalogSamples.work
import com.cantos.theatre.domain.catalog.PublicCatalogSamples.worksPage
import com.cantos.theatre.domain.catalog.PublicUnitPage
import com.cantos.theatre.domain.catalog.PublicWork
import com.cantos.theatre.domain.catalog.PublicWorkId
import com.cantos.theatre.domain.catalog.PublicWorkPage
import com.cantos.theatre.ports.PublicCatalogPort
import com.cantos.theatre.ports.PublicCatalogRead
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.withContext
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

@OptIn(ExperimentalCoroutinesApi::class)
class PublicCatalogControllerTest {
    /** Each read waits for the test to answer it; cancellation is recorded, never answered. */
    private class GatedPort(private val cooperateWithCancellation: Boolean = true) : PublicCatalogPort {
        data class Call(val label: String, val answer: CompletableDeferred<PublicCatalogRead<Any>>, var cancelled: Boolean = false)
        val calls = mutableListOf<Call>()

        @Suppress("UNCHECKED_CAST")
        private suspend fun <T> gate(label: String): PublicCatalogRead<T> {
            val call = Call(label, CompletableDeferred())
            calls += call
            return try {
                val answer = if (cooperateWithCancellation) call.answer.await()
                else withContext(NonCancellable) { call.answer.await() }
                answer as PublicCatalogRead<T>
            } catch (cancelled: CancellationException) {
                call.cancelled = true
                throw cancelled
            }
        }
        override suspend fun works(cursor: CatalogCursor?): PublicCatalogRead<PublicWorkPage> = gate("works:${cursor?.value}")
        override suspend fun search(query: CatalogSearchQuery, cursor: CatalogCursor?): PublicCatalogRead<PublicWorkPage> =
            gate("search:${query.value}:${cursor?.value}")
        override suspend fun work(id: PublicWorkId): PublicCatalogRead<PublicWork> = gate("work:${id.value}")
        override suspend fun units(work: PublicWork, cursor: CatalogCursor?): PublicCatalogRead<PublicUnitPage> =
            gate("units:${work.id.value}:${cursor?.value}")

        fun answer(label: String, outcome: PublicCatalogRead<*>) {
            @Suppress("UNCHECKED_CAST")
            calls.last { it.label == label && !it.answer.isCompleted }.answer.complete(outcome as PublicCatalogRead<Any>)
        }
    }

    private fun TestScope.controller(port: PublicCatalogPort) = PublicCatalogController(backgroundScope, port)

    @Test
    fun search_cancels_superseded_queries_and_late_a_to_b_to_a_answers_are_ignored() = runTest(StandardTestDispatcher()) {
        val port = GatedPort(cooperateWithCancellation = false)
        val controller = controller(port)
        for (query in listOf("A", "B", "A")) {
            controller.dispatch(PublicCatalogIntent.Search(query))
            runCurrent()
        }
        assertEquals(listOf("search:A:null", "search:B:null", "search:A:null"), port.calls.map { it.label })
        port.calls[0].answer.complete(PublicCatalogRead.Ready(worksPage(listOf(work(1)), null)))
        port.calls[1].answer.complete(PublicCatalogRead.Ready(worksPage(listOf(work(2)), null)))
        runCurrent()
        assertEquals(WalkPhase.Loading, controller.state.value.works.phase)
        assertEquals(emptyList(), controller.state.value.works.items)
        port.calls[2].answer.complete(PublicCatalogRead.Ready(worksPage(listOf(work(3)), cursor(1))))
        runCurrent()
        assertEquals(listOf(work(3)), controller.state.value.works.items)
        controller.dispatch(PublicCatalogIntent.LoadMoreWorks)
        runCurrent()
        assertEquals("search:A:${cursor(1).value}", port.calls.last().label)
        port.calls.last().answer.complete(PublicCatalogRead.Transport)
        runCurrent()
        controller.dispatch(PublicCatalogIntent.RetryWorks)
        runCurrent()
        assertEquals("search:A:${cursor(1).value}", port.calls.last().label)
        port.calls.last().answer.complete(PublicCatalogRead.Ready(worksPage(listOf(work(4)), null)))
        runCurrent()
        assertEquals(listOf(work(3), work(4)), controller.state.value.works.items)
        controller.dispatch(PublicCatalogIntent.Search("  "))
        runCurrent()
        assertEquals("works:null", port.calls.last().label)
        port.calls.last().answer.complete(PublicCatalogRead.Ready(worksPage(emptyList(), null)))
        runCurrent()
        assertTrue(controller.state.value.works.isEmptyResult)
        controller.close()
    }

    @Test
    fun invalid_search_cancels_in_flight_transport_without_sending_invalid_text() = runTest(StandardTestDispatcher()) {
        val port = GatedPort()
        val controller = controller(port)
        controller.dispatch(PublicCatalogIntent.Search("a"))
        runCurrent()
        controller.dispatch(PublicCatalogIntent.Search("a".repeat(513)))
        runCurrent()
        assertTrue(port.calls.single().cancelled)
        assertEquals(WalkPhase.Failed(CatalogFailure.InvalidQuery), controller.state.value.works.phase)
        controller.dispatch(PublicCatalogIntent.RetryWorks)
        runCurrent()
        assertEquals(1, port.calls.size)
    }

    @Test
    fun refresh_cancels_the_in_flight_read_and_its_late_answer_never_lands() = runTest(StandardTestDispatcher()) {
        val port = GatedPort()
        val controller = controller(port)
        controller.dispatch(PublicCatalogIntent.Start)
        runCurrent()
        controller.dispatch(PublicCatalogIntent.RefreshWorks)
        runCurrent()
        assertEquals(listOf("works:null", "works:null"), port.calls.map { it.label })
        assertTrue(port.calls[0].cancelled, "the superseded read is cancelled")
        port.calls[0].answer.complete(PublicCatalogRead.Ready(worksPage(listOf(work(9)), null)))
        runCurrent()
        assertEquals(WalkPhase.Loading, controller.state.value.works.phase)
        port.answer("works:null", PublicCatalogRead.Ready(worksPage(listOf(work(1)), cursor(1))))
        runCurrent()
        assertEquals(listOf(work(1)), controller.state.value.works.items)
        assertEquals(WalkPhase.More, controller.state.value.works.phase)
    }

    @Test
    fun list_and_detail_reads_are_independent_and_back_cancels_only_the_detail() = runTest(StandardTestDispatcher()) {
        val port = GatedPort()
        val controller = controller(port)
        val story = work(1)
        controller.dispatch(PublicCatalogIntent.Start)
        runCurrent()
        port.answer("works:null", PublicCatalogRead.Ready(worksPage(listOf(story, work(2)), cursor(1))))
        runCurrent()
        controller.dispatch(PublicCatalogIntent.LoadMoreWorks)
        controller.dispatch(PublicCatalogIntent.OpenWork(story))
        runCurrent()
        port.answer("work:${story.id.value}", PublicCatalogRead.Ready(story))
        runCurrent()
        val unitsLabel = "units:${story.id.value}:null"
        assertEquals(unitsLabel, port.calls.last().label)
        controller.dispatch(PublicCatalogIntent.Back)
        runCurrent()
        assertTrue(port.calls.last().cancelled, "leaving the detail cancels its unit read")
        assertTrue(port.calls.single { it.label == "works:${cursor(1).value}" }.let { !it.cancelled })
        port.answer("works:${cursor(1).value}", PublicCatalogRead.Ready(worksPage(listOf(work(3)), null)))
        runCurrent()
        assertEquals(null, controller.state.value.detail)
        assertEquals(listOf(story, work(2), work(3)), controller.state.value.works.items)
    }

    @Test
    fun units_walk_through_the_controller_and_an_escaped_fault_is_contained() = runTest(StandardTestDispatcher()) {
        val story = work(1)
        var unitCalls = 0
        val port = object : PublicCatalogPort {
            override suspend fun works(cursor: CatalogCursor?) = PublicCatalogRead.Ready(worksPage(listOf(story), null))
            override suspend fun search(query: CatalogSearchQuery, cursor: CatalogCursor?) = works(cursor)
            override suspend fun work(id: PublicWorkId) = PublicCatalogRead.Ready(story)
            override suspend fun units(work: PublicWork, cursor: CatalogCursor?): PublicCatalogRead<PublicUnitPage> {
                unitCalls++
                if (unitCalls == 2) throw IllegalStateException("adapter defect")
                return PublicCatalogRead.Ready(unitsPage(work, if (cursor == null) listOf(unit(1)) else listOf(unit(2)), if (cursor == null) cursor(4) else null))
            }
        }
        val controller = controller(port)
        controller.dispatch(PublicCatalogIntent.OpenWork(story))
        runCurrent()
        assertEquals(listOf(unit(1)), controller.state.value.detail!!.units!!.items)
        controller.dispatch(PublicCatalogIntent.LoadMoreUnits)
        runCurrent()
        assertEquals(WalkPhase.Failed(CatalogFailure.Unavailable(true)), controller.state.value.detail!!.units!!.phase)
        controller.dispatch(PublicCatalogIntent.RetryUnits)
        runCurrent()
        assertEquals(listOf(unit(1), unit(2)), controller.state.value.detail!!.units!!.items)
        assertEquals(WalkPhase.Complete, controller.state.value.detail!!.units!!.phase)
    }

    @Test
    fun close_cancels_reads_and_ignores_later_intents() = runTest(StandardTestDispatcher()) {
        val port = GatedPort()
        val controller = controller(port)
        controller.dispatch(PublicCatalogIntent.Start)
        runCurrent()
        controller.close()
        runCurrent()
        assertTrue(port.calls.single().cancelled)
        val closed = controller.state.value
        controller.dispatch(PublicCatalogIntent.RefreshWorks)
        runCurrent()
        assertEquals(1, port.calls.size)
        assertEquals(closed, controller.state.value)
    }
}
