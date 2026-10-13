package com.cantos.theatre.app

import androidx.compose.ui.graphics.toPixelMap
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.runDesktopComposeUiTest
import com.cantos.theatre.demo.DemoState
import com.cantos.theatre.demo.DemoTheme
import org.junit.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotEquals

/** Checks the mounted surface, so changing its role back to large fails even if theme tests pass. */
@OptIn(ExperimentalTestApi::class)
class MiniPlayerShapeTest {
    @Test
    fun mini_player_uses_twelve_dp_corners_in_both_themes_at_phone_and_tablet_widths() {
        listOf(320, 390, 840).forEach { width ->
            DemoTheme.entries.forEach { theme ->
                runDesktopComposeUiTest(width = width, height = 844) {
                    setContent { CantosDemoApp(initialState = DemoState(theme = theme)) }
                    waitForIdle()
                    val pixels = onNodeWithTag("mini-player").captureToImage().toPixelMap()
                    // At density 1, (9, 1) is inside radius 12 but outside radius 16 or 24.
                    // (1, 1) is outside rounded corners. Sample away from artwork and controls.
                    val surface = pixels[pixels.width / 2, 1]
                    assertEquals(surface, pixels[9, 1], "Mini player at $width / $theme")
                    assertNotEquals(surface, pixels[1, 1], "Must retain rounded corners")
                    assertEquals(surface, pixels[pixels.width - 10, 1])
                }
            }
        }
    }
}
