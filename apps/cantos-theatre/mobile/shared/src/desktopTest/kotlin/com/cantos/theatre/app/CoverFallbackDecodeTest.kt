package com.cantos.theatre.app

import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Canvas
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.Paint
import androidx.compose.ui.graphics.RectangleShape
import androidx.compose.ui.graphics.toPixelMap
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import com.cantos.theatre.adapters.image.SkiaCoverDecoder
import com.cantos.theatre.features.shell.presentation.ArtworkFallback
import com.cantos.theatre.features.shell.presentation.LocalCoverFallback
import com.cantos.theatre.features.shell.presentation.rememberCoverFallback
import com.cantos.theatre.resources.Res
import java.util.concurrent.atomic.AtomicInteger
import java.util.concurrent.atomic.AtomicReference
import kotlin.test.assertEquals
import kotlin.test.assertNotSame
import kotlinx.coroutines.test.runTest
import org.junit.Test
import kotlin.test.assertNull

/**
 * Desktop Skia ships no AVIF codec: an AVIF cover or the packaged fallback never decodes here, so
 * the decoder reports `null` (fail closed) and the shell draws its vector celestial backdrop
 * instead. Android decoding of AVIF is a separate platform acceptance (API 31+).
 */
class CoverFallbackDecodeTest {
    @Test
    fun desktop_skia_refuses_avif_bytes_so_the_vector_backdrop_stays_the_fallback() = runTest {
        val bytes = Res.readBytes("files/artwork/cantos_cover_fallback.avif")
        assertNull(SkiaCoverDecoder.decode(bytes, maxEdgePx = 1024))
    }

    @OptIn(ExperimentalTestApi::class)
    @Test
    fun two_slots_and_their_remounts_share_one_background_decode() = runDesktopComposeUiTest(width = 120, height = 64) {
        val image = ImageBitmap(2, 2)
        Canvas(image).drawRect(0f, 0f, 2f, 2f, Paint().apply { color = Color.Magenta })
        val decodes = AtomicInteger()
        val decoderThread = AtomicReference<Thread>()
        var visit by mutableStateOf(0)
        setContent {
            val fallback = rememberCoverFallback {
                decoderThread.set(Thread.currentThread())
                decodes.incrementAndGet()
                image
            }
            CompositionLocalProvider(LocalCoverFallback provides fallback) {
                key(visit) {
                    Row {
                        repeat(2) { slot ->
                            ArtworkFallback(Modifier.size(48.dp).testTag("slot-$slot"), RectangleShape)
                        }
                    }
                }
            }
        }

        fun slotColor(slot: Int): Color {
            val pixels = onNodeWithTag("slot-$slot").captureToImage().toPixelMap()
            return pixels[pixels.width / 2, pixels.height / 2]
        }

        waitUntil(timeoutMillis = 5_000) { slotColor(0) == Color.Magenta }
        assertEquals(Color.Magenta, slotColor(1))
        runOnIdle {
            assertNotSame(Thread.currentThread(), decoderThread.get())
            visit++
        }
        assertEquals(Color.Magenta, slotColor(0))
        assertEquals(Color.Magenta, slotColor(1))
        assertEquals(1, decodes.get())
    }
}
