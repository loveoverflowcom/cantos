package com.cantos.ui.preview

import com.cantos.ui.preview.generated.DesignInventory
import com.cantos.ui.preview.generated.DesignSpecimen

/**
 * Diagnostic discovery over the design inventory. Design status, ownership scope and registration are
 * separate facts: this model reads only the first two, and the shared registry still owns the rest.
 */
data class InventoryFilter(
    val query: String = "",
    val designStatus: String? = null,
    val scope: String? = null,
    /** True keeps only specimens whose design lists CMP among its platforms. */
    val cmpOnly: Boolean = false,
)

/** Search matches id, name, family, scope, owner and consumer; filters compose and never mutate the inventory. */
fun filterInventory(specimens: List<DesignSpecimen>, filter: InventoryFilter): List<DesignSpecimen> {
    val normalized = filter.query.trim().lowercase()
    return specimens.filter { specimen ->
        (filter.designStatus == null || specimen.designStatus == filter.designStatus) &&
            (filter.scope == null || specimen.scope == filter.scope) &&
            (!filter.cmpOnly || "cmp" in specimen.platforms) &&
            (normalized.isEmpty() || specimen.id.contains(normalized) || specimen.name.lowercase().contains(normalized) ||
                specimen.family.contains(normalized) || specimen.scope.contains(normalized) ||
                specimen.owner.lowercase().contains(normalized) || specimen.consumer.lowercase().contains(normalized))
    }
}

/** One displayed selection; filtering does not overwrite the preference. */
fun resolveInventorySelection(specimens: List<DesignSpecimen>, preferredId: String?): DesignSpecimen? =
    specimens.firstOrNull { it.id == preferredId } ?: specimens.firstOrNull()

data class InventoryCounts(
    val total: Int,
    val existing: Int,
    val proposed: Int,
    val cmpApplicable: Int,
    val byScope: Map<String, Int>,
)

fun inventoryCounts(specimens: List<DesignSpecimen> = DesignInventory.specimens): InventoryCounts = InventoryCounts(
    total = specimens.size,
    existing = specimens.count { it.designStatus == "existing" },
    proposed = specimens.count { it.designStatus == "proposed" },
    cmpApplicable = specimens.count { "cmp" in it.platforms },
    byScope = specimens.groupingBy { it.scope }.eachCount(),
)

/** Ownership scopes in the order a reader asks about them; none of them is a registration or qualification word. */
val InventoryScopeOrder: List<String> = listOf("shared-primitive", "shared-composition", "app-owned", "platform-specific")
