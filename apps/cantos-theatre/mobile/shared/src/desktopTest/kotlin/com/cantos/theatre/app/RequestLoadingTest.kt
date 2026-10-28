package com.cantos.theatre.app

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asSkiaBitmap
import androidx.compose.ui.graphics.toPixelMap
import androidx.compose.ui.platform.LocalWindowInfo
import androidx.compose.ui.platform.WindowInfo
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import com.cantos.theatre.features.shell.presentation.LocalLoadingViewport
import com.cantos.theatre.features.shell.presentation.PendingRows
import com.cantos.ui.CantosTheme
import com.cantos.ui.UiPreferences
import java.io.File
import kotlin.math.abs
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import org.junit.Test
import org.jetbrains.skia.EncodedImageFormat
import org.jetbrains.skia.Image

/** Production pending rows over injected window facts and a manual JVM clock; no native OS claim. */
@OptIn(ExperimentalTestApi::class)
class RequestLoadingTest {
    private fun distance(first: ImageBitmap, second: ImageBitmap): Double {
        val a = first.toPixelMap()
        val b = second.toPixelMap()
        var total = 0.0
        for (y in 0 until first.height) for (x in 0 until first.width) {
            total += abs(a[x, y].red - b[x, y].red) + abs(a[x, y].alpha - b[x, y].alpha)
        }
        return total
    }

    @Test
    fun pending_rows_stop_when_unobserved_offscreen_or_unfocused_and_resume_without_relayout() =
        runDesktopComposeUiTest(width = 320, height = 480) {
            mainClock.autoAdvance = false
            var viewport by mutableStateOf(Rect.Zero)
            var focused by mutableStateOf(true)
            var mounted by mutableStateOf(true)
            val window = object : WindowInfo {
                override val isWindowFocused: Boolean get() = focused
            }
            setContent {
                CantosTheme(UiPreferences(reducedMotion = false)) {
                    CompositionLocalProvider(LocalLoadingViewport provides viewport, LocalWindowInfo provides window) {
                        Box(Modifier.requiredSize(320.dp, 480.dp).testTag("capture")) {
                            if (mounted) PendingRows("Đang tải", "rows", count = 1)
                        }
                    }
                }
            }
            fun sampleDistance(name: String): Double {
                mainClock.advanceTimeByFrame()
                val first = onNodeWithTag("capture").captureToImage()
                mainClock.advanceTimeBy(352L)
                val later = onNodeWithTag("capture").captureToImage()
                System.getenv("CANTOS_LOADING_REVIEW_CAPTURE_DIR")?.let { destination ->
                    val directory = File(destination).apply { mkdirs() }
                    for ((frame, bitmap) in listOf("first" to first, "later" to later)) {
                        val bytes = requireNotNull(Image.makeFromBitmap(bitmap.asSkiaBitmap()).encodeToData(EncodedImageFormat.PNG)).bytes
                        File(directory, "$name-$frame.png").writeBytes(bytes)
                    }
                }
                return distance(first, later)
            }
            assertEquals(0.0, sampleDistance("unknown"), 1e-6, "unknown viewport must keep motion off")
            viewport = Rect(0f, 0f, 320f, 480f)
            assertTrue(sampleDistance("visible") > 0.0, "a visible foreground request really pulses")
            val bounds = onNodeWithTag("rows").fetchSemanticsNode().boundsInRoot
            focused = false
            assertEquals(0.0, sampleDistance("unfocused"), 1e-6, "backgrounded host must stop")
            focused = true
            viewport = Rect(0f, 1000f, 320f, 1480f)
            assertEquals(0.0, sampleDistance("offscreen"), 1e-6, "offscreen rows must stop")
            viewport = Rect(0f, 0f, 320f, 480f)
            assertTrue(sampleDistance("resumed") > 0.0, "foreground and visibility resume the beat")
            assertEquals(bounds, onNodeWithTag("rows").fetchSemanticsNode().boundsInRoot)
            mounted = false
            mainClock.advanceTimeByFrame()
            onNodeWithTag("rows").assertDoesNotExist()
        }
}
