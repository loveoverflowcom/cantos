package com.cantos.ui

import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.geometry.RoundRect
import androidx.compose.ui.graphics.Outline
import androidx.compose.ui.graphics.Path
import com.cantos.ui.generated.CantosTokens
import kotlin.math.pow
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

/** Pure decisions behind the inset focus perimeter; no composition and no pixels here. */
class FocusPerimeterContractTest {
    @Test
    fun the_stroke_outer_edge_is_the_node_edge_and_never_leaves_the_bounds() {
        val box = insetStrokeBox(width = 200f, height = 48f, stroke = 2f, radius = 8f)
        assertEquals(1f, box.left)
        assertEquals(1f, box.top)
        // Centre line plus half a stroke on each side is exactly the bounds.
        assertEquals(200f, box.left + box.width + 1f)
        assertEquals(48f, box.top + box.height + 1f)
        assertEquals(7f, box.corner)
    }

    @Test
    fun an_inward_ring_sits_that_far_inside_the_edge_with_a_matching_corner() {
        val box = insetStrokeBox(width = 200f, height = 48f, stroke = 2f, radius = 24f, inward = 2f)
        assertEquals(3f, box.left)
        assertEquals(3f, box.top)
        // The outer edge of the stroke is 2 px inside the node on every side.
        assertEquals(200f - 2f, box.left + box.width + 1f)
        assertEquals(48f - 2f, box.top + box.height + 1f)
        assertEquals(21f, box.corner)
    }

    @Test
    fun an_oversized_radius_is_a_stadium_of_the_shortest_side_and_a_square_has_no_corner() {
        assertEquals(23f, insetStrokeBox(200f, 48f, 2f, 999f).corner)
        assertEquals(23f, insetStrokeBox(48f, 200f, 2f, 999f).corner)
        assertEquals(0f, insetStrokeBox(200f, 48f, 2f, 0f).corner)
        // A radius smaller than half the stroke cannot go negative.
        assertEquals(0f, insetStrokeBox(200f, 48f, 2f, 0.5f).corner)
        // A node thinner than the stroke collapses to nothing rather than a negative size.
        val sliver = insetStrokeBox(1f, 1f, 2f, 0f)
        assertEquals(0f, sliver.width)
        assertEquals(0f, sliver.height)
    }

    @Test
    fun the_corner_follows_the_measured_outline_so_an_animated_shape_keeps_the_ring_on_its_edge() {
        fun rounded(radius: Float) = Outline.Rounded(RoundRect(Rect(0f, 0f, 120f, 48f), CornerRadius(radius)))
        assertEquals(24f, outlineCornerRadius(rounded(24f)))
        assertEquals(16f, outlineCornerRadius(rounded(16f)))
        assertEquals(0f, outlineCornerRadius(Outline.Rectangle(Rect(0f, 0f, 120f, 48f))))
        assertEquals(0f, outlineCornerRadius(Outline.Generic(Path())))
        val uneven = Outline.Rounded(RoundRect(Rect(0f, 0f, 120f, 48f), CornerRadius(4f), CornerRadius(12f), CornerRadius(8f), CornerRadius(2f)))
        assertEquals(12f, outlineCornerRadius(uneven))
    }

    @Test
    fun painted_theatre_actions_are_inset_and_the_standard_expression_keeps_the_outset_ring() {
        ButtonVariant.entries.forEach { variant ->
            assertEquals(variant.isAuth(), variant.focusIsInset(CantosExpression.Standard), "$variant standard")
            assertEquals(variant != ButtonVariant.ListenHero, variant.focusIsInset(CantosExpression.Theatre), "$variant theatre")
        }
        assertFalse(ButtonVariant.Primary.focusIsInset(CantosExpression.Standard))
        assertTrue(ButtonVariant.AuthPrimary.focusIsInset(CantosExpression.Standard))
    }

    @Test
    fun the_ring_has_at_least_non_text_contrast_with_the_fill_it_sits_on_in_every_theme_and_expression() {
        // Independent of the production colour code: each row names the fill and the ring role by hand.
        ButtonVariant.entries.forEach { variant ->
            CantosExpression.entries.filter { variant.focusIsInset(it) }.forEach { expression ->
                listOf("light", "dark").forEach { theme ->
                    val fill = when (variant) {
                        ButtonVariant.Primary, ButtonVariant.AuthPrimary, ButtonVariant.ListenHero -> "color.primary"
                        ButtonVariant.Tonal, ButtonVariant.AuthSecondary ->
                            if (expression == CantosExpression.Theatre) "color.surfaceContainerHigh" else "color.primaryContainer"
                        ButtonVariant.Danger -> "color.errorContainer"
                        ButtonVariant.Secondary, ButtonVariant.Text, ButtonVariant.AuthOutlined -> "color.surface"
                    }
                    val ring = if (variant.ringReadsFromContent()) "color.onPrimary" else "color.focus"
                    val ratio = contrast(hex(fill, theme, expression), hex(ring, theme, expression))
                    assertTrue(ratio >= 3.0, "$variant / ${expression.tokenName} / $theme ring $ring on $fill is only $ratio:1")
                }
            }
        }
    }

    @Test
    fun the_selected_theatre_chip_ring_reads_from_its_label_colour_because_the_focus_hue_is_too_close() {
        listOf("light", "dark").forEach { theme ->
            val fill = hex("color.primaryContainer", theme, CantosExpression.Theatre)
            assertTrue(contrast(fill, hex("color.focus", theme, CantosExpression.Theatre)) < 3.0, "premise: focus on the ink container, $theme")
            assertTrue(contrast(fill, hex("color.onPrimaryContainer", theme, CantosExpression.Theatre)) >= 3.0, "label colour on the ink container, $theme")
        }
    }

    @Test
    fun a_primary_fill_is_the_focus_hue_itself_so_the_focus_colour_would_vanish_on_it() {
        // Why Primary and AuthPrimary read from their content colour: this is the premise, not a choice.
        CantosExpression.entries.forEach { expression ->
            listOf("light", "dark").forEach { theme ->
                val ratio = contrast(hex("color.primary", theme, expression), hex("color.focus", theme, expression))
                assertTrue(ratio < 3.0, "${expression.tokenName} / $theme focus on primary is $ratio:1")
            }
        }
        assertEquals(setOf(ButtonVariant.Primary, ButtonVariant.AuthPrimary), ButtonVariant.entries.filter { it.ringReadsFromContent() }.toSet())
    }

    private fun hex(role: String, theme: String, expression: CantosExpression): String =
        CantosTokens.color(CantosTokens.role(role, expression.tokenName), theme)

    private fun luminance(hex: String): Double {
        val channels = (0..2).map { hex.substring(1 + it * 2, 3 + it * 2).toInt(16) / 255.0 }
            .map { if (it <= 0.03928) it / 12.92 else ((it + 0.055) / 1.055).pow(2.4) }
        return 0.2126 * channels[0] + 0.7152 * channels[1] + 0.0722 * channels[2]
    }

    private fun contrast(a: String, b: String): Double {
        val (light, dark) = listOf(luminance(a), luminance(b)).sortedDescending()
        return (light + 0.05) / (dark + 0.05)
    }
}
