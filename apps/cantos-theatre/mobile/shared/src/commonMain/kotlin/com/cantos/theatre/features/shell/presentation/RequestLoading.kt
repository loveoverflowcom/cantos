package com.cantos.theatre.features.shell.presentation

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.layout.boundsInWindow
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalWindowInfo
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import com.cantos.ui.AccessibleName
import com.cantos.ui.CantosShapes
import com.cantos.ui.EdgeProgress
import com.cantos.ui.SurfacePulse
import com.cantos.ui.tokenDp

internal val LocalLoadingViewport = staticCompositionLocalOf { Rect.Zero }

/** Actual layout clipping and window focus facts; an unknown/unfocused host paints no travelling edge. */
@Composable
internal fun RequestEdge(active: Boolean, label: String, tag: String, modifier: Modifier = Modifier) {
    val viewport = LocalLoadingViewport.current
    val foreground = LocalWindowInfo.current.isWindowFocused
    var bounds by remember { mutableStateOf(Rect.Zero) }
    val visible = !bounds.isEmpty && !viewport.isEmpty && bounds.overlaps(viewport)
    Box(modifier.testTag(tag).onGloballyPositioned { bounds = it.boundsInWindow() }.semantics {
        if (active) { contentDescription = label; liveRegion = LiveRegionMode.Polite }
    }) {
        EdgeProgress(active, visible, foreground, AccessibleName.of(label), Modifier.clearAndSetSemantics { })
    }
}

/** App composition with the existing 72dp artwork slot and typography-owned line boxes. */
@Composable
internal fun PendingRows(label: String, tag: String, count: Int = 3) {
    val line = with(LocalDensity.current) { MaterialTheme.typography.bodyLarge.lineHeight.toDp() }
    Column(Modifier.fillMaxWidth().testTag(tag).semantics { liveRegion = LiveRegionMode.Polite },
        verticalArrangement = Arrangement.spacedBy(tokenDp("space.md"))) {
        Text(label)
        repeat(count) {
            Row(Modifier.fillMaxWidth().clearAndSetSemantics { }, horizontalArrangement = Arrangement.spacedBy(tokenDp("space.inset"))) {
                SurfacePulse(Modifier.size(tokenDp("size.control") + tokenDp("space.xl")), shape = CantosShapes.thumbnail)
                Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
                    SurfacePulse(Modifier.fillMaxWidth().height(line))
                    SurfacePulse(Modifier.fillMaxWidth().height(line))
                    Box(Modifier.fillMaxWidth(0.65f)) { SurfacePulse(Modifier.fillMaxWidth().height(line)) }
                }
            }
        }
    }
}
