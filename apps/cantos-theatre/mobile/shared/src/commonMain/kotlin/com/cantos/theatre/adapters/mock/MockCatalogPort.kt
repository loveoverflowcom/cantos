package com.cantos.theatre.adapters.mock

import com.cantos.theatre.domain.publication.CatalogInventory
import com.cantos.theatre.ports.CatalogOutcome
import com.cantos.theatre.ports.CatalogPort
import kotlinx.coroutines.awaitCancellation

enum class CatalogScenario { Ready, Loading, Empty, Error }

/** Deterministic scenarios over caller-supplied synthetic values; no network or fixture globals. */
class MockCatalogPort(
    private val inventory: CatalogInventory,
    scenario: CatalogScenario = CatalogScenario.Ready,
) : CatalogPort {
    private var selectedScenario = scenario

    fun setScenario(scenario: CatalogScenario) {
        selectedScenario = scenario
    }

    override suspend fun load(): CatalogOutcome = when (selectedScenario) {
        CatalogScenario.Ready -> CatalogOutcome.Ready(inventory)
        CatalogScenario.Loading -> awaitCancellation()
        CatalogScenario.Empty -> CatalogOutcome.Empty
        CatalogScenario.Error -> CatalogOutcome.Unavailable
    }
}
