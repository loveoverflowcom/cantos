package com.cantos.ui

import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics

/**
 * Controlled edge for retained content. Hosts provide request, viewport visibility and foreground
 * lifecycle facts; success/error/cancel clear [active], and hidden/background hosts clear the
 * corresponding fact. No OS lookup, fetching, debounce, focus or field state lives here.
 *
 * The reserved 2-unit slot remains when inactive. Only the active/visible/foreground Full branch
 * composes an infinite transition, so stopping or disposal cancels its ticker. Reduced is a static
 * quarter-width segment plus the same localized status; frames only invalidate drawing, not layout.
 * Hosts own one primary indicator and opt into announcements once per significant request change.
 */
@Composable
fun EdgeProgress(
    active: Boolean,
    visible: Boolean,
    foreground: Boolean,
    label: AccessibleName,
    modifier: Modifier = Modifier,
    announcement: Announcement = Announcement.Off,
) {
    val showing = active && visible && foreground
    val cycle = loadingCycleMillis(LoadingCycle.Edge, LocalUiPreferences.current.motion)
    val track = tokenColor("color.primaryContainer")
    val segment = tokenColor("color.primary")
    Box(modifier.fillMaxWidth().height(tokenDp("size.loadingEdge")).semantics {
        if (showing) {
            contentDescription = label.text
            when (announcement) {
                Announcement.Off -> Unit
                Announcement.Polite -> liveRegion = LiveRegionMode.Polite
                Announcement.Assertive -> liveRegion = LiveRegionMode.Assertive
            }
        }
    }) {
        if (showing) {
            if (cycle == null) EdgeTrack(track, segment) { 0.375f }
            else TravellingEdge(track, segment, cycle)
        }
    }
}

/** Quarter width is fixed anatomy, not determinate request progress. */
internal const val EdgeSegmentFraction = 0.25f

@Composable
private fun TravellingEdge(track: Color, segment: Color, cycle: Int) {
    val offset = rememberInfiniteTransition(label = "edge-progress").animateFloat(
        initialValue = -EdgeSegmentFraction,
        targetValue = 1f,
        animationSpec = infiniteRepeatable(tween(cycle, easing = LinearEasing)),
        label = "edge-progress-offset",
    )
    EdgeTrack(track, segment) { offset.value }
}

@Composable
private fun EdgeTrack(track: Color, segment: Color, offset: () -> Float) {
    Canvas(Modifier.fillMaxWidth().height(tokenDp("size.loadingEdge")).clipToBounds()) {
        drawRect(track)
        drawRect(segment, Offset(size.width * offset(), 0f), Size(size.width * EdgeSegmentFraction, size.height))
    }
}
