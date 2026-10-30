package com.cantos.ui

import com.cantos.ui.generated.CantosTokens
import kotlin.math.pow
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

/** Guest CTA scope, shared geometry and the retained legacy outlined variant. */
class GuestCtaPairContractTest {
    @Test
    fun only_auth_actions_join_the_container_pair_and_only_inside_the_profile_scope() {
        val joined = ButtonVariant.entries.filter { it.joinsContainerPair(AuthActionScope.OnPrimaryContainer) }
        assertEquals(setOf(ButtonVariant.AuthPrimary, ButtonVariant.AuthSecondary, ButtonVariant.AuthOutlined), joined.toSet())
        assertTrue(ButtonVariant.entries.none { it.joinsContainerPair(AuthActionScope.Default) })
    }

    @Test
    fun the_pair_text_and_outline_keep_readable_contrast_against_the_card_and_the_fill_in_every_theme() {
        CantosExpression.entries.forEach { expression ->
            listOf("light", "dark").forEach { theme ->
                val card = hex("color.primaryContainer", theme, expression)
                val ink = hex("color.onPrimaryContainer", theme, expression)
                val label = "${expression.tokenName} / $theme"
                // Sign-in: card colour as text/icon on the foreground fill. Outlined: foreground ink on the card.
                assertTrue(contrast(ink, card) >= 4.5, "$label pair text ${contrast(ink, card)}:1")
                // The outline is a non-text boundary of the transparent action.
                assertTrue(contrast(ink, card) >= 3.0, "$label outline ${contrast(ink, card)}:1")
            }
        }
    }

    @Test
    fun the_pair_shares_one_height_and_outline_weight_from_the_existing_auth_tokens() {
        assertEquals(60.0, CantosTokens.number("size.authPrimary"))
        assertEquals(1.5, CantosTokens.number("size.authOutline"))
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
