package com.cantos.ui

import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Outline
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp
import com.cantos.ui.generated.CantosTokens
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class AuthVariantContractTest {
    @Test
    fun capsule_follows_intrinsic_height_and_pressed_feedback_keeps_a_bounded_corner() {
        fun radius(fraction: Float, width: Float = 300f, height: Float = 60f): Float {
            val outline = AuthActionShape(fraction, 16.dp)
                .createOutline(Size(width, height), LayoutDirection.Ltr, Density(1f)) as Outline.Rounded
            return outline.roundRect.topLeftCornerRadius.x
        }
        assertEquals(30f, radius(0f))
        assertEquals(16f, radius(1f))
        assertEquals(23f, radius(0.5f))
        assertEquals(60f, radius(0f, height = 120f))
        assertEquals(16f, radius(1f, height = 120f))
        assertEquals(10f, radius(1f, width = 20f))
        assertEquals(30f, radius(-1f))
        assertEquals(16f, radius(2f))
    }

    @Test
    fun scoped_auth_dimensions_do_not_replace_regular_control_or_border_roles() {
        listOf("compact", "comfortable").forEach { density ->
            assertEquals(60.0, CantosTokens.number("size.authPrimary", density))
            assertEquals(56.0, CantosTokens.number("size.authSecondary", density))
            assertEquals(56.0, CantosTokens.number("size.authField", density))
            assertEquals(48.0, CantosTokens.number("size.backControl", density))
        }
        assertEquals(44.0, CantosTokens.number("size.control", "compact"))
        assertEquals(48.0, CantosTokens.number("size.control", "comfortable"))
        assertEquals(8.0, CantosTokens.number("shape.control"))
        assertEquals(16.0, CantosTokens.number("shape.container"))
        assertEquals(1.0, CantosTokens.number("size.border"))
        assertEquals(1.5, CantosTokens.number("size.authOutline"))
        assertEquals(220.0, CantosTokens.number("motion.shape.duration"))
        assertEquals(0.0, CantosTokens.number("motion.shape.duration", reducedMotion = true))
        assertTrue(ButtonVariant.AuthPrimary.isAuth())
        assertTrue(ButtonVariant.AuthSecondary.isAuth())
        assertTrue(ButtonVariant.AuthOutlined.isAuth())
        assertEquals(false, ButtonVariant.Primary.isAuth())
    }
}
