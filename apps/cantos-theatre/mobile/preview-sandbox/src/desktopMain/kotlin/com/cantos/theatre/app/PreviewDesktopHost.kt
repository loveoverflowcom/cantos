package com.cantos.theatre.app

import androidx.compose.runtime.Composable
import androidx.compose.runtime.SideEffect
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEvent
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.type

/**
 * The desktop-only Escape key of the offline Desktop Preview: it is the shell's own Back whenever the shell
 * reports that Back is available, and is otherwise not consumed.
 *
 * It is installed on the **window** (`Window(onKeyEvent = keys::onKeyEvent)`), not on a composable. A window
 * with no focused control (a fresh window, or after pointer navigation away from a focused control) holds focus
 * at the scene root and never visits a descendant's key handler, so a handler inside the content silently stops
 * working. The window's `onKeyEvent` runs regardless of focus but only for keys the content did not consume, so a
 * focused control or dialog that handles Escape itself keeps precedence. (`onPreviewKeyEvent` would run first and
 * take the key from them.)
 */
class PreviewBackKeys {
    private var enabled = false
    private var action: () -> Unit = {}

    /** Whether the shell currently reports that Back is available; read-only, for diagnostics and tests. */
    val backAvailable: Boolean get() = enabled

    internal fun update(enabled: Boolean, action: () -> Unit) {
        this.enabled = enabled
        this.action = action
    }

    fun onKeyEvent(event: KeyEvent): Boolean {
        if (event.key != Key.Escape || event.type != KeyEventType.KeyUp || !enabled) return false
        action()
        return true
    }
}

/** The offline Desktop Preview's window body: the shared application, reporting Back availability to [keys]. */
@Composable
fun PreviewDesktopHost(owners: PreviewOwners, keys: PreviewBackKeys, modifier: Modifier = Modifier) {
    PreviewTheatreApp(
        owners,
        modifier,
        backHandler = { enabled, onBack -> SideEffect { keys.update(enabled, onBack) } },
    )
}
