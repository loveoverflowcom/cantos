package com.cantos.theatre.features.shell.presentation

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

    internal fun open(onBack: () -> Unit): () -> Unit {
        val entry = Entry(onBack)
        top = entry
        return { if (top === entry) top = null }
    }

    private class Entry(val onBack: () -> Unit)
}

/** Registers [onBack] as the top overlay's Back action on [registry] for as long as this call is composed. */
@Composable
internal fun BackOverlay(registry: BackOverlayRegistry, onBack: () -> Unit) {
    val latest by rememberUpdatedState(onBack)
    DisposableEffect(registry) {
        val close = registry.open { latest() }
        onDispose(close)
    }
}
