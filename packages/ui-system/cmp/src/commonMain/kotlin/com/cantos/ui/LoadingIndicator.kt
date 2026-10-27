package com.cantos.ui

import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.animation.core.animateFloat
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.semantics.clearAndSetSemantics

/**
 * The one indeterminate indicator, internal to the package so it is consumed through the registered
 * components (Button, `LoadingState`) and needs no catalog entry of its own: an arc that turns while it is composed and a stationary arc
 * under reduced motion. It is decorative (cleared semantics): the adjacent localized text or the
 * action's pending reason owns the announcement, so it is never a percentage, a check or a
 * disclosure glyph. Compose it only while work is ongoing; leaving composition (complete, error,
 * hidden, scrolled out of a lazy list) ends the animation, and reduced motion never creates one.
 */
@Composable
internal fun LoadingIndicator(modifier: Modifier = Modifier, tint: Color = androidx.compose.material3.LocalContentColor.current) {
    val turnMillis = pendingRingTurnMillis(LocalUiPreferences.current.motion)
    if (turnMillis == null) LoadingArc(modifier, tint, angle = { 0f }) else TurningLoadingArc(modifier, tint, turnMillis)
}

/** The only composable that owns an infinite transition; it exists solely under full motion. */
@Composable
private fun TurningLoadingArc(modifier: Modifier, tint: Color, turnMillis: Int) {
    val spin = rememberInfiniteTransition(label = "loading-indicator").animateFloat(
        initialValue = 0f,
        targetValue = 360f,
        animationSpec = infiniteRepeatable(tween(turnMillis, easing = LinearEasing)),
        label = "loading-indicator-angle",
    )
    // The angle is read in the layer block, so a turn never recomposes the host.
    LoadingArc(modifier, tint, angle = { spin.value })
}

@Composable
private fun LoadingArc(modifier: Modifier, tint: Color, angle: () -> Float) {
    val stroke = tokenDp("size.focusWidth")
    Canvas(modifier.size(tokenDp("size.icon")).graphicsLayer { rotationZ = angle() }.clearAndSetSemantics { }) {
        val width = stroke.toPx()
        drawArc(tint, -90f, 270f, false,
            topLeft = Offset(width / 2, width / 2),
            size = Size(size.width - width, size.height - width),
            style = Stroke(width))
    }
}
