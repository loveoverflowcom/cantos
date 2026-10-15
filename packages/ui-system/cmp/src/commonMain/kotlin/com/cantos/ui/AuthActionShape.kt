package com.cantos.ui

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.geometry.RoundRect
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Outline
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.LayoutDirection

/** Stable CMP fallback, not the native Material Expressive spring API.
 * The capsule radius follows measured content, including wrapped large text.
 */
internal class AuthActionShape(private val pressedFraction: Float, private val pressedRadius: Dp) : Shape {
    override fun createOutline(size: Size, layoutDirection: LayoutDirection, density: Density): Outline {
        val capsule = size.minDimension / 2f
        val pressed = with(density) { pressedRadius.toPx() }.coerceAtMost(capsule)
        val radius = capsule + (pressed - capsule) * pressedFraction.coerceIn(0f, 1f)
        return Outline.Rounded(RoundRect(Rect(0f, 0f, size.width, size.height), CornerRadius(radius)))
    }
}

@Composable
internal fun authActionShape(interactions: MutableInteractionSource): Shape {
    val pressed by interactions.collectIsPressedAsState()
    // The press dispatches on release regardless of this spec; the shape only decorates it.
    val spec = rememberMotionSpec(MotionIntent.Feedback, MotionProperty.Spatial)
    val fraction by animateFloatAsState(if (pressed) 1f else 0f, spec.toAnimationSpec(), label = "auth-pressed-shape")
    val radius = tokenDp("shape.authPressed")
    return remember(fraction, radius) { AuthActionShape(fraction, radius) }
}

internal fun ButtonVariant.isAuth(): Boolean = when (this) {
    ButtonVariant.AuthPrimary, ButtonVariant.AuthSecondary, ButtonVariant.AuthOutlined -> true
    ButtonVariant.Primary, ButtonVariant.Tonal, ButtonVariant.Secondary, ButtonVariant.Text, ButtonVariant.Danger -> false
}
