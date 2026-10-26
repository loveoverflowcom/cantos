package com.cantos.ui

import androidx.compose.foundation.layout.requiredSize
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asSkiaBitmap
import androidx.compose.ui.graphics.toPixelMap
import androidx.compose.ui.focus.FocusManager
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.unit.dp
import java.io.File
import kotlin.math.abs
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import org.jetbrains.skia.EncodedImageFormat
import org.jetbrains.skia.Image
import org.junit.Rule
import org.junit.Test

/** Real desktop touch dispatch and rendered frames; not Android/iOS device evidence. */
class AuthActionMotionTest {
    @get:Rule
    val compose = createComposeRule()

    @Test
    fun interrupted_press_restores_capsule_without_dispatch_or_target_movement() {
        verifyMotion(reducedMotion = false)
    }

    @Test
    fun reduced_motion_retains_immediate_press_feedback_and_one_release_intent() {
        verifyMotion(reducedMotion = true)
    }

    private fun verifyMotion(reducedMotion: Boolean) {
        var intents = 0
        lateinit var focusManager: FocusManager
        compose.setContent {
            BundledPreviewCaptureRoot(preferences = UiPreferences(reducedMotion = reducedMotion)) {
                focusManager = LocalFocusManager.current
                Button("Đăng nhập với Cantos ID", { intents += 1 },
                    variant = ButtonVariant.AuthPrimary,
                    controlModifier = Modifier.requiredSize(300.dp, 60.dp).testTag("motion-action"))
            }
        }
        compose.waitForIdle()
        compose.mainClock.autoAdvance = false
        val action = compose.onNodeWithTag("motion-action")
        val bounds = action.fetchSemanticsNode().boundsInRoot
        val prefix = if (reducedMotion) "reduced" else "animated"
        fun frame(name: String): Int {
            // A press focuses the node on desktop, and the inset focus ring is the capsule's own
            // edge in the content colour (white on white here), so it would hide the painted corner
            // this scan measures. The ring's shape follows the corner in FocusPerimeterPixelTest.
            compose.runOnUiThread { focusManager.clearFocus(force = true) }
            compose.mainClock.advanceTimeByFrame()
            val bitmap = action.captureToImage()
            save(bitmap, "cmp-auth-motion-$prefix-$name")
            assertEquals(bounds, action.fetchSemanticsNode().boundsInRoot)
            // A 4 px scanline intersects the capsule/pressed corner well away from text.
            // Inspect the actual filled boundary rather than the implementation's fraction.
            val pixels = bitmap.toPixelMap()
            // Scan outwards from the filled interior to the background. Ripple colour changes
            // remain inside this boundary.
            val background = pixels[0, 0]
            return 1 + ((29 downTo 0).firstOrNull { x ->
                val pixel = pixels[x, 4]
                abs(pixel.red - background.red) + abs(pixel.green - background.green) +
                    abs(pixel.blue - background.blue) + abs(pixel.alpha - background.alpha) <= 0.2f
            } ?: -1)
        }
        fun startPress() {
            action.performTouchInput { down(center) }
            // One frame admits the pressed state; the next starts the animation clock.
            compose.mainClock.advanceTimeByFrame()
            compose.mainClock.advanceTimeByFrame()
        }
        val resting = frame("idle")
        startPress()
        if (!reducedMotion) compose.mainClock.advanceTimeBy(60)
        val middle = frame("mid")
        assertTrue(middle < resting, "The actual painted corner must respond before the midpoint")
        action.performTouchInput { moveTo(Offset(-20f, -20f)); up() }
        compose.mainClock.advanceTimeByFrame()
        compose.mainClock.advanceTimeByFrame()
        compose.mainClock.advanceTimeBy(400)
        assertEquals(resting, frame("cancelled"))
        assertEquals(0, intents, "Cancelling a press must not dispatch the host action")

        startPress()
        compose.mainClock.advanceTimeBy(400)
        val pressed = frame("pressed")
        if (reducedMotion) assertEquals(pressed, middle, "Reduced motion must reach the pressed shape immediately")
        else assertTrue(pressed < middle, "A mid-animation frame must differ from its settled endpoint")
        action.performTouchInput { up() }
        compose.mainClock.advanceTimeByFrame()
        compose.mainClock.advanceTimeByFrame()
        compose.mainClock.advanceTimeBy(400)
        assertEquals(resting, frame("released"))
        assertEquals(1, intents, "Only the completed release may dispatch")
    }

    private fun save(bitmap: ImageBitmap, name: String) {
        val output = File(System.getenv("CANTOS_UI_PREVIEW_CAPTURE_DIR") ?: "build/reports/cantos-preview")
        check(output.isDirectory || output.mkdirs())
        val encoded = requireNotNull(Image.makeFromBitmap(bitmap.asSkiaBitmap()).encodeToData(EncodedImageFormat.PNG))
        File(output, "$name.png").writeBytes(encoded.bytes)
    }
}
