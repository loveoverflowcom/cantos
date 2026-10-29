package com.cantos.theatre.app

import com.cantos.theatre.adapters.image.decodeBundledAvif
import com.cantos.theatre.features.shell.presentation.loadLibraryArtwork
import androidx.compose.ui.graphics.toPixelMap
import kotlinx.coroutines.test.runTest
import org.junit.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertNull

class BundledAvifTest {
    @Test
    fun library_avif_decodes_with_its_original_dimensions_and_transparent_margin() = runTest {
        val image = assertNotNull(loadLibraryArtwork(::decodeBundledAvif))
        assertEquals(1374, image.width)
        assertEquals(1145, image.height)
        assertEquals(0f, image.toPixelMap()[0, 0].alpha)
    }

    @Test
    fun corrupt_and_legacy_bytes_do_not_become_bundled_avif() {
        assertNull(decodeBundledAvif(byteArrayOf()))
        assertNull(decodeBundledAvif("not an image".encodeToByteArray()))
        assertNull(decodeBundledAvif(byteArrayOf(0x89.toByte(), 0x50, 0x4e, 0x47, 13, 10, 26, 10)))
    }
}
