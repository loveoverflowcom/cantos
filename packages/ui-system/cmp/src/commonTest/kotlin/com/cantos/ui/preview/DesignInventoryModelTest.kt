package com.cantos.ui.preview

import com.cantos.ui.PreviewExample
import com.cantos.ui.generated.PreviewCatalog
import com.cantos.ui.preview.generated.DesignInventory
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue

class DesignInventoryModelTest {
    private val existing = DesignInventory.specimens.filter { it.designStatus == "existing" }
    private val proposed = DesignInventory.specimens.filter { it.designStatus == "proposed" }

    @Test
    fun existing_specimens_are_exactly_the_preview_catalog_and_its_native_examples() {
        val ids = existing.map { it.id }.sorted()
        assertEquals(PreviewCatalog.components.map { it.id }.sorted(), ids)
        assertEquals(PreviewExample.entries.map { it.id }.sorted(), ids)
        assertEquals(ids.size, ids.toSet().size)
        existing.forEach { assertEquals(listOf(it.id), it.componentIds, "an existing specimen maps to its own API") }
    }

    @Test
    fun a_proposal_is_never_a_registered_component_or_a_native_example() {
        assertEquals(70, proposed.size)
        assertEquals(102, DesignInventory.specimens.size)
        assertEquals(DesignInventory.specimens.map { it.id }.toSet().size, DesignInventory.specimens.size)
        proposed.forEach { proposal ->
            assertNull(PreviewExample.fromId(proposal.id), "${proposal.id} must have no preview example")
            assertFalse(PreviewCatalog.components.any { it.id == proposal.id }, "${proposal.id} must not be registered")
        }
        // Design status is its own vocabulary: nothing in the inventory claims qualification.
        assertEquals(setOf("existing", "proposed"), DesignInventory.specimens.map { it.designStatus }.toSet())
    }

    @Test
    fun every_reused_component_is_registered_and_every_scope_is_known() {
        val registered = PreviewCatalog.components.map { it.id }.toSet()
        DesignInventory.specimens.forEach { specimen ->
            assertTrue(specimen.componentIds.all { it in registered }, "${specimen.id} reuses an unregistered component")
            assertTrue(specimen.scope in InventoryScopeOrder, "${specimen.id}: ${specimen.scope}")
            assertTrue(specimen.states.isNotEmpty() && specimen.anatomy.isNotEmpty() && specimen.tokenRoles.isNotEmpty(), specimen.id)
        }
    }

    @Test
    fun studio_and_admin_specimens_stay_web_only() {
        val webOnly = DesignInventory.specimens.filter { it.family == "studio" || it.family == "access" }
        assertEquals(29, webOnly.size)
        assertTrue(webOnly.all { it.platforms == listOf("web") })
        assertEquals(73, DesignInventory.specimens.count { "cmp" in it.platforms })
        assertEquals(102 - 29, inventoryCounts().cmpApplicable)
    }

    @Test
    fun each_searchable_field_finds_a_specimen_that_no_other_field_would() {
        val all = DesignInventory.specimens
        // Every query below is found through exactly one field: dropping that field from the search empties it.
        assertEquals(listOf("ui-root"), filterInventory(all, InventoryFilter(query = "ui-root")).map { it.id }, "id (its name is Theme & density)")
        assertEquals(listOf("link"), filterInventory(all, InventoryFilter(query = "links")).map { it.id }, "name (its id is link)")
        val family = filterInventory(all, InventoryFilter(query = "fields"))
        assertEquals(9, family.size, "family")
        assertTrue(family.all { it.family == "fields" })
        assertEquals(57, filterInventory(all, InventoryFilter(query = "app-owned")).size, "scope")
        assertEquals(43, filterInventory(all, InventoryFilter(query = "ui-system")).size, "owner")
        assertEquals(listOf("mini-player"), filterInventory(all, InventoryFilter(query = "persistent listening controls")).map { it.id }, "consumer")
    }

    @Test
    fun filters_compose_and_the_query_ignores_case_and_surrounding_space_without_mutating_the_inventory() {
        val all = DesignInventory.specimens
        assertEquals(102, filterInventory(all, InventoryFilter()).size)
        assertEquals(listOf("mini-player"), filterInventory(all, InventoryFilter(query = " MINI-PLAYER ")).map { it.id })
        assertTrue(filterInventory(all, InventoryFilter(query = "workspace nav")).any { it.id == "studio-workspace" })
        assertTrue(filterInventory(all, InventoryFilter(query = "Thẻ tác phẩm")).any { it.id == "work-cover-card" })
        val admin = filterInventory(all, InventoryFilter(query = "cantos-admin"))
        assertEquals(2, admin.size)
        assertTrue(admin.all { it.owner.startsWith("cantos-admin") })
        assertEquals(32, filterInventory(all, InventoryFilter(designStatus = "existing")).size)
        assertEquals(70, filterInventory(all, InventoryFilter(designStatus = "proposed")).size)
        assertEquals(41, filterInventory(all, InventoryFilter(designStatus = "proposed", cmpOnly = true)).size)
        assertEquals(emptyList(), filterInventory(all, InventoryFilter(query = "mini-player", designStatus = "existing")))
        assertEquals(emptyList(), filterInventory(all, InventoryFilter(query = "không tồn tại")))
        assertEquals(102, all.size)
    }

    @Test
    fun filtering_projects_one_displayed_selection_and_preserves_the_preference() {
        val all = DesignInventory.specimens
        assertEquals("button", resolveInventorySelection(all, "button")?.id)
        val filtered = filterInventory(all, InventoryFilter(query = "mini-player"))
        assertEquals("mini-player", resolveInventorySelection(filtered, "button")?.id)
        assertEquals(null, resolveInventorySelection(emptyList(), "button"))
        assertEquals(all.first().id, resolveInventorySelection(all, null)?.id)
    }

    @Test
    fun counts_separate_registration_from_proposals_and_cover_every_specimen() {
        val counts = inventoryCounts()
        assertEquals(102, counts.total)
        assertEquals(32, counts.existing)
        assertEquals(70, counts.proposed)
        assertEquals(counts.total, counts.byScope.values.sum())
        assertEquals(57, counts.byScope["app-owned"])
        assertEquals(39, counts.byScope["shared-primitive"])
    }
}
