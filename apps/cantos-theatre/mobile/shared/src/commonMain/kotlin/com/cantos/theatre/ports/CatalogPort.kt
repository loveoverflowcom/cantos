package com.cantos.theatre.ports

import com.cantos.theatre.domain.publication.CatalogInventory

sealed interface CatalogOutcome {
    data class Ready(val inventory: CatalogInventory) : CatalogOutcome
    data object Empty : CatalogOutcome
    data object Unavailable : CatalogOutcome
}

/** Loads the inventory. Search is a screen projection, never its mutable authority. */
interface CatalogPort {
    suspend fun load(): CatalogOutcome
}
