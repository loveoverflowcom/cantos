package com.cantos.theatre.app

import com.cantos.theatre.adapters.image.SkiaCoverDecoder
import com.cantos.theatre.resources.Res
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
}
