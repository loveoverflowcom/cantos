package com.cantos.ui

import com.cantos.ui.generated.CantosTokens
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue

/** The generated remap table and the pure parts of the Theatre expression; no composition here. */
class ExpressionContractTest {
    @Test
    fun the_enum_and_the_generated_inventory_name_exactly_the_same_expressions() {
        assertEquals(CantosExpression.entries.map { it.tokenName }, CantosTokens.expressionNames)
        assertEquals("standard", CantosExpression.Standard.tokenName)
    }

    @Test
    fun standard_resolves_every_role_to_itself_and_theatre_remaps_only_declared_roles() {
        CantosTokens.names.forEach { name -> assertEquals(name, CantosTokens.role(name)) }
        assertEquals("color.theatrePrimary", CantosTokens.role("color.primary", "theatre"))
        assertEquals("color.theatreSurfaceContainer", CantosTokens.role("color.surfaceContainer", "theatre"))
        assertEquals("type.theatreHeading.weight", CantosTokens.role("type.headline.weight", "theatre"))
        assertEquals("type.theatreControl.weight", CantosTokens.role("type.label.weight", "theatre"))
        // Player, status, outline and geometry roles are not part of the expression.
        listOf("color.playerSurface", "color.error", "color.outline", "size.control", "shape.reading").forEach {
            assertEquals(it, CantosTokens.role(it, "theatre"), it)
        }
        assertFailsWith<IllegalArgumentException> { CantosTokens.role("color.primary", "studio") }
    }

    @Test
    fun a_remap_never_changes_a_roles_kind_and_its_target_exists_in_both_themes() {
        val roles = CantosTokens.entries.associateBy { it.name }
        CantosTokens.names.forEach { name ->
            val target = CantosTokens.role(name, "theatre")
            if (target == name) return@forEach
            assertEquals(roles.getValue(name).kind, roles.getValue(target).kind, "$name -> $target")
            if (name.startsWith("color.")) {
                CantosTokens.color(target, "light")
                CantosTokens.color(target, "dark")
            }
        }
    }

    @Test
    fun the_owner_selected_hero_pair_and_heading_weights_are_the_canonical_values() {
        assertEquals("#1d0b28", CantosTokens.color("color.theatreOnHero", "light"))
        assertEquals("#c6a0f5", CantosTokens.color("color.theatreHero", "light"))
        assertEquals(700.0, CantosTokens.number("type.theatreHeading.weight"))
        assertEquals(700.0, CantosTokens.number("type.theatreControl.weight"))
        assertEquals(48.0, CantosTokens.number("size.avatarControl"))
        assertEquals(44.0, CantosTokens.number("size.brandMark"))
        assertEquals(52.0, CantosTokens.number("size.searchAction"))
        // Studio keeps its own values: nothing in the standard palette moved.
        assertEquals("#675080", CantosTokens.color("color.primary", "light"))
        assertEquals(500.0, CantosTokens.number("type.headline.weight"))
    }

    @Test
    fun the_theatre_chip_radius_moves_from_its_resting_radius_to_a_capsule_without_overshoot() {
        val side = 48f
        assertEquals(12f, chipRadius(side, 12f, 0f))
        assertEquals(24f, chipRadius(side, 12f, 1f))
        assertEquals(18f, chipRadius(side, 12f, 0.5f))
        assertEquals(12f, chipRadius(side, 12f, -3f), "an overshooting fraction is clamped")
        assertEquals(24f, chipRadius(side, 12f, 7f))
        // A chip shorter than twice its resting radius never exceeds its own capsule.
        assertEquals(10f, chipRadius(20f, 12f, 0f))
        assertTrue(chipRadius(96f, 12f, 1f) > chipRadius(48f, 12f, 1f), "a wrapped two-line chip keeps a capsule of its own height")
    }
}
