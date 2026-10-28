package com.cantos.ui

import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.animation.core.animateFloat
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.unit.Dp

/**
 * The two MicroArc frames (`size.loadingArcCompact` 16, `size.loadingArc` 20 logical units). Compact is
 * the frame in a button or chip; Standard is the section/footer frame. The frame never sets a hit
 * target: the host keeps its own control size, and a glyph slot keeps its footprint through `slot`.
 */
internal enum class LoadingSize(val role: String) {
    Compact("size.loadingArcCompact"),
    Standard("size.loadingArc"),
}

/**
 * The one indeterminate indicator (the MicroArc), internal to the package so it is consumed through
 * the registered components (Button, FilterChip, `LoadingState`) and needs no catalog entry of its
 * own: one fixed 270 degree monochrome arc with round ends and a `size.loadingArcStroke` stroke that
 * turns while it is composed, and a stationary arc under reduced motion. It is decorative (cleared
 * semantics): the adjacent localized text or the action's pending reason owns the announcement, so it
 * is never a percentage, a check or a disclosure glyph. Compose it only while work is ongoing; leaving
 * composition (complete, error, hidden, scrolled out of a lazy list) ends the animation, and reduced
 * motion never creates one.
 *
 * [slot] centers the arc in a box of that size, so a 16 unit arc replaces a 24 unit glyph without the
 * label or the control moving; without it the box is the arc's own frame.
 */
@Composable
internal fun LoadingIndicator(
    modifier: Modifier = Modifier,
    tint: Color = androidx.compose.material3.LocalContentColor.current,
    size: LoadingSize = LoadingSize.Compact,
    slot: Dp? = null,
) {
    val turnMillis = loadingCycleMillis(LoadingCycle.Arc, LocalUiPreferences.current.motion)
    val frame = tokenDp(size.role)
    Box(modifier.size(slot ?: frame).clearAndSetSemantics { }, contentAlignment = Alignment.Center) {
        if (turnMillis == null) LoadingArc(frame, tint, angle = { 0f }) else TurningLoadingArc(frame, tint, turnMillis)
    }
}

/** The only composable that owns an infinite transition; it exists solely under full motion. */
@Composable
private fun TurningLoadingArc(frame: Dp, tint: Color, turnMillis: Int) {
    val spin = rememberInfiniteTransition(label = "loading-indicator").animateFloat(
        initialValue = 0f,
        targetValue = 360f,
        animationSpec = infiniteRepeatable(tween(turnMillis, easing = LinearEasing)),
        label = "loading-indicator-angle",
    )
    // The angle is read in the layer block, so a turn never recomposes the host.
    LoadingArc(frame, tint, angle = { spin.value })
}

@Composable
private fun LoadingArc(frame: Dp, tint: Color, angle: () -> Float) {
    val stroke = tokenDp("size.loadingArcStroke")
    Canvas(Modifier.size(frame).graphicsLayer { rotationZ = angle() }) {
        val width = stroke.toPx()
        drawArc(tint, -90f, LoadingArcSweepDegrees, false,
            topLeft = Offset(width / 2, width / 2),
            size = Size(size.width - width, size.height - width),
            style = Stroke(width, cap = StrokeCap.Round))
    }
}

/** The arc's fixed sweep: twelve o'clock clockwise to nine o'clock. */
internal const val LoadingArcSweepDegrees: Float = 270f
