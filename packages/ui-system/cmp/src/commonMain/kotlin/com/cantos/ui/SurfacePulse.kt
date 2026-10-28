package com.cantos.ui

import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.RectangleShape
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.semantics.clearAndSetSemantics

/**
 * One placeholder fill for an area whose content is still loading: a cover frame, a title bar or a
 * list row. A single monochrome fill whose alpha fades on the shared pulse beat (see
 * [surfacePulseFor]), so a group of fills breathes together. The caller owns geometry: size,
 * aspect ratio and corner [shape] come from [modifier] and [shape], so the placeholder keeps the
 * loaded content's layout. The fill is decorative (cleared semantics): the host names the
 * surrounding region and its state once and never exposes each block as content.
 *
 * Alpha is read in the layer block, so a beat never recomposes the host or relayouts a list. [active] false,
 * leaving composition (complete, error, scrolled out of a lazy list) or a stopped host frame clock
 * ends the animation; reduced motion composes no infinite transition and holds
 * [SurfacePulseRestingAlpha].
 */
@Composable
fun SurfacePulse(
    modifier: Modifier = Modifier,
    active: Boolean = true,
    shape: Shape = RectangleShape,
    color: Color = tokenColor("color.outlineVariant"),
) {
    val spec = surfacePulseFor(LocalUiPreferences.current.motion)
    if (active && spec != null) {
        BeatingFill(modifier, shape, color, spec)
    } else {
        PulseFill(modifier, shape, color) { SurfacePulseRestingAlpha }
    }
}

/** The only composable that owns an infinite transition; it exists solely while a pulse is active under full motion. */
@Composable
private fun BeatingFill(modifier: Modifier, shape: Shape, color: Color, spec: SurfacePulseSpec) {
    val alpha = rememberInfiniteTransition(label = "surface-pulse").animateFloat(
        initialValue = spec.lowAlpha,
        targetValue = spec.highAlpha,
        animationSpec = infiniteRepeatable(
            tween(spec.halfCycleMillis, easing = CubicBezierEasing(spec.easing.x1, spec.easing.y1, spec.easing.x2, spec.easing.y2)),
            RepeatMode.Reverse,
        ),
        label = "surface-pulse-alpha",
    )
    PulseFill(modifier, shape, color) { alpha.value }
}

@Composable
private fun PulseFill(modifier: Modifier, shape: Shape, color: Color, alpha: () -> Float) {
    Box(
        modifier
            .graphicsLayer { this.alpha = alpha(); this.shape = shape; clip = true }
            .background(color)
            .clearAndSetSemantics { },
    )
}
