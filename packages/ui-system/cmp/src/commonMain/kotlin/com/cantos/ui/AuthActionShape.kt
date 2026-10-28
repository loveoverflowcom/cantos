package com.cantos.ui

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.ProvidedValue
import androidx.compose.runtime.getValue
import androidx.compose.runtime.staticCompositionLocalOf
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
    ButtonVariant.Primary, ButtonVariant.Tonal, ButtonVariant.Secondary, ButtonVariant.Text, ButtonVariant.Danger, ButtonVariant.ListenHero -> false
}

/**
 * Where an auth action sits decides its colour pair. [OnPrimaryContainer] is the profile card (the
 * guest sign-in / create-account pair, design v4.2b): the primary action is filled with the card's
 * own foreground and the outlined one is transparent with the same foreground as ink and outline.
 * [Surface] provides it for that card only; nothing global changes and callbacks, eligibility and
 * the disabled state stay with the button.
 */
internal enum class AuthActionScope { Default, OnPrimaryContainer, Tonal }

internal val LocalAuthActionScope = staticCompositionLocalOf { AuthActionScope.Default }

/** True under [tonalAuthActions]: no action in the subtree draws a stroke, ring or halo. */
internal fun AuthActionScope.isTonal(): Boolean = this == AuthActionScope.Tonal

/**
 * Auth v4.3 Minimal Expressive: provide this around a subtree and every [Button], [BackButton] and
 * [IconButton] in it is borderless at idle, focus, error and disabled. Containers stay tonal; keyboard
 * and accessibility focus is a state-layer fill over the control's own shape ([cantosFocusFill]).
 * Callbacks, eligibility, loading and disabled reasons are the caller's and do not change, and
 * nothing outside the subtree does. Use `CompositionLocalProvider(tonalAuthActions()) { ... }`.
 */
fun tonalAuthActions(): ProvidedValue<*> = LocalAuthActionScope provides AuthActionScope.Tonal

/** Both pair members share one pill height and one outline weight so the stacked pair is equal. */
internal fun ButtonVariant.joinsContainerPair(scope: AuthActionScope): Boolean =
    scope == AuthActionScope.OnPrimaryContainer &&
        (this == ButtonVariant.AuthPrimary || this == ButtonVariant.AuthOutlined)
