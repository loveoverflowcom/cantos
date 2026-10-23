package com.cantos.theatre.app

import androidx.compose.foundation.focusable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.Modifier
import androidx.compose.ui.awt.ComposeWindow
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.unit.dp
import java.awt.Component
import java.awt.Container
import java.awt.GraphicsEnvironment
import java.awt.event.KeyEvent
import javax.swing.SwingUtilities
import org.junit.Assume
import org.junit.Test
import kotlin.test.assertEquals

/**
 * Delivery of Escape to `Window(onKeyEvent = keys::onKeyEvent)`, the route `main()` installs, in a **real AWT
 * `ComposeWindow`** with static content. It proves: with nothing focused the window still receives the key and the
 * shell's Back runs (the case a handler inside the content lost), and a focused control that handles Escape keeps
 * it. It opens a short-lived window, so it is explicit (`CANTOS_PREVIEW_WINDOW_TESTS=1`) and skipped headless.
 * The key is an AWT event sent to the window's Skia component, not an OS keystroke; the application-to-keys hookup
 * is covered in the Compose scene by `DesktopAcceptanceJourneyTest`. This JVM window does not recompose after its
 * first frame, so no application state is driven here.
 */
@OptIn(ExperimentalComposeUiApi::class)
class DesktopWindowEscapeTest {
    private fun all(component: Component, into: MutableList<Component> = mutableListOf()): List<Component> {
        into += component
        if (component is Container) component.components.forEach { all(it, into) }
        return into
    }

    @Test fun escape_reaches_the_window_with_nothing_focused_and_yields_to_a_focused_control_that_handles_it() {
        Assume.assumeTrue("explicit window lane", System.getenv("CANTOS_PREVIEW_WINDOW_TESTS") == "1")
        Assume.assumeFalse(GraphicsEnvironment.isHeadless())
        var back = 0
        val keys = PreviewBackKeys().also { it.update(enabled = true) { back++ } }
        val focus = FocusRequester()
        var handledByControl = 0
        lateinit var window: ComposeWindow
        SwingUtilities.invokeAndWait {
            window = ComposeWindow()
            window.setSize(300, 300)
            window.setContent(onKeyEvent = keys::onKeyEvent) {
                Box(Modifier.size(50.dp).focusRequester(focus).focusable().onKeyEvent {
                    if (it.key == Key.Escape && it.type == KeyEventType.KeyUp) { handledByControl++; true } else false
                })
            }
            window.isVisible = true
        }
        try {
            fun escape() = SwingUtilities.invokeAndWait {
                val target = all(window).last { it.javaClass.name.contains("SkiaLayer") }
                for (id in listOf(KeyEvent.KEY_PRESSED, KeyEvent.KEY_RELEASED)) {
                    target.dispatchEvent(KeyEvent(target, id, System.currentTimeMillis(), 0, KeyEvent.VK_ESCAPE, KeyEvent.CHAR_UNDEFINED))
                }
                Unit
            }
            Thread.sleep(2_000)
            escape(); Thread.sleep(500)
            assertEquals(1, back, "Escape with nothing focused reaches the window handler and runs Back")
            assertEquals(0, handledByControl)
            SwingUtilities.invokeAndWait { focus.requestFocus() }; Thread.sleep(800)
            escape(); Thread.sleep(500)
            assertEquals(1, handledByControl, "a focused control handles Escape itself")
            assertEquals(1, back, "and the window handler then does not also go Back")
        } finally {
            SwingUtilities.invokeAndWait { window.dispose() }
        }
    }
}
