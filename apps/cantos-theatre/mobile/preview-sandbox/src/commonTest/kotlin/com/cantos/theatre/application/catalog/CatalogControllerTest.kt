package com.cantos.theatre.application.catalog

import com.cantos.theatre.adapters.mock.CatalogScenario
import com.cantos.theatre.adapters.mock.MockCatalogPort
import com.cantos.theatre.domain.publication.CatalogEpisode
import com.cantos.theatre.domain.publication.CatalogInventory
import com.cantos.theatre.domain.publication.CatalogWork
import com.cantos.theatre.domain.publication.EpisodeKey
import com.cantos.theatre.domain.publication.WorkKey
import com.cantos.theatre.ports.CatalogOutcome
import com.cantos.theatre.ports.CatalogPort
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.withContext
import kotlin.test.Test
import kotlin.test.assertEquals

@OptIn(ExperimentalCoroutinesApi::class)
class CatalogControllerTest {
    @Test
    fun loading_empty_and_error_preserve_metadata_until_ready_inventory_arrives() = runTest {
        val initial = inventory("son-ha-ky")
        val port = MockCatalogPort(initial, CatalogScenario.Loading)
        val controller = CatalogController(backgroundScope, port, initial)
        controller.refresh()
        runCurrent()
        assertEquals(CatalogState(1, CatalogStatus.Loading, initial), controller.state.value)

        port.setScenario(CatalogScenario.Error)
        controller.refresh()
        runCurrent()
        assertEquals(CatalogState(2, CatalogStatus.Unavailable, initial), controller.state.value)

        port.setScenario(CatalogScenario.Empty)
        controller.refresh()
        runCurrent()
        assertEquals(CatalogState(3, CatalogStatus.Empty, initial), controller.state.value)

        port.setScenario(CatalogScenario.Ready)
        controller.refresh()
        runCurrent()
        assertEquals(CatalogState(4, CatalogStatus.Ready, initial), controller.state.value)
    }

    @Test
    fun cancelled_request_that_returns_late_cannot_replace_new_inventory() = runTest {
        val initial = inventory("son-ha-ky")
        val replacement = inventory("den-khuya")
        val port = DelayedCatalogPort()
        val controller = CatalogController(backgroundScope, port, initial)
        controller.refresh()
        runCurrent()
        controller.refresh()
        runCurrent()
        assertEquals(2, port.responses.size)

        port.responses[1].complete(CatalogOutcome.Ready(replacement))
        runCurrent()
        val accepted = CatalogState(2, CatalogStatus.Ready, replacement)
        assertEquals(accepted, controller.state.value)
        port.responses[0].complete(CatalogOutcome.Ready(initial))
        runCurrent()
        assertEquals(2, port.completedLoads)
        assertEquals(accepted, controller.state.value)
    }

    @Test
    fun reset_invalidates_pending_request_even_if_adapter_ignores_cancellation() = runTest {
        val initial = inventory("son-ha-ky")
        val port = DelayedCatalogPort()
        val controller = CatalogController(backgroundScope, port, initial)
        controller.refresh()
        runCurrent()
        controller.reset()
        runCurrent()
        val reset = CatalogState(2, CatalogStatus.Ready, initial)
        assertEquals(reset, controller.state.value)
        port.responses.single().complete(CatalogOutcome.Ready(inventory("den-khuya")))
        runCurrent()
        assertEquals(1, port.completedLoads)
        assertEquals(reset, controller.state.value)
    }

    @Test
    fun unexpected_adapter_failure_becomes_typed_unavailability() = runTest {
        val initial = inventory("son-ha-ky")
        val port = object : CatalogPort {
            override suspend fun load(): CatalogOutcome = error("private adapter detail")
        }
        val controller = CatalogController(backgroundScope, port, initial)
        controller.refresh()
        runCurrent()
        assertEquals(CatalogState(1, CatalogStatus.Unavailable, initial), controller.state.value)
    }

    private class DelayedCatalogPort : CatalogPort {
        val responses = mutableListOf<CompletableDeferred<CatalogOutcome>>()
        var completedLoads: Int = 0
            private set

        override suspend fun load(): CatalogOutcome {
            val response = CompletableDeferred<CatalogOutcome>()
            responses.add(response)
            // Deliberately hostile fake: the controller must fence facts independently of cancel.
            val outcome = withContext(NonCancellable) { response.await() }
            completedLoads += 1
            return outcome
        }
    }

    private fun inventory(key: String): CatalogInventory {
        val work = WorkKey(key)
        return CatalogInventory(listOf(CatalogWork(
            key = work,
            title = "Tiêu đề minh họa",
            category = "Kỳ ảo",
            description = "Mô tả nguyên bản cho kiểm thử.",
            episodes = listOf(CatalogEpisode(EpisodeKey(work, 0), "Ánh đèn cuối sân khấu", 60_000)),
        )))
    }
}
