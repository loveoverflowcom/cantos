package com.cantos.theatre.features.shell.presentation

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue

/**
 * The one transient overlay (a confirmation sheet) that owns Back before the route does. The host
 * root consults [dismiss] ahead of route navigation, so system Back, Escape and a hosted Back button
 * close the top overlay first and leave the route stack untouched. Only the most recently opened
 * overlay is registered; a stale one can never remove its successor.
 */
@Stable
internal class BackOverlayRegistry {
    private var top by mutableStateOf<Entry?>(null)

    /** True while an overlay is open, so the host keeps its Back handler enabled for it. */
    val active: Boolean get() = top != null

    /** Closes the top overlay; returns false when none was open so the route may handle Back. */
    fun dismiss(): Boolean {
        val entry = top ?: return false
        entry.onBack()
        return true
    }

    internal fun open(bounds: () -> Rect? = { null }, onBack: () -> Unit): () -> Unit {
        val entry = Entry(onBack, bounds)
        top = entry
        return { if (top === entry) top = null }
    }

    /** Only a registered bounded overlay opts in; existing confirmation sheets keep their policy. */
    fun dismissOutside(point: Offset) {
        val entry = top ?: return
        if (entry.bounds()?.contains(point) == false) entry.onBack()
    }

    private class Entry(val onBack: () -> Unit, val bounds: () -> Rect?)
}

/** Registers [onBack] as the top overlay's Back action on [registry] for as long as this call is composed. */
@Composable
internal fun BackOverlay(registry: BackOverlayRegistry, onBack: () -> Unit, bounds: Rect? = null) {
    val latest by rememberUpdatedState(onBack)
    val latestBounds by rememberUpdatedState(bounds)
    DisposableEffect(registry) {
        val close = registry.open(bounds = { latestBounds }) { latest() }
        onDispose(close)
    }
}
