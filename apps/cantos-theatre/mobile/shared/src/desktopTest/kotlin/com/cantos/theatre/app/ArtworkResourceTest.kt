package com.cantos.theatre.app

import com.cantos.theatre.features.shell.presentation.loadArtwork
import com.cantos.theatre.resources.Res
import java.awt.Font
import java.io.ByteArrayInputStream
import kotlinx.coroutines.test.runTest
import org.junit.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue

class ArtworkResourceTest {
    @Test
    fun exact_bundled_paintings_decode_and_missing_or_invalid_art_falls_back() = runTest {
        assertNotNull(loadArtwork("drawable/son_ha_moon.png"))
        assertNotNull(loadArtwork("drawable/den_khuya.png"))
        assertNull(loadArtwork("drawable/absent.png"))
        assertNull(loadArtwork("font/noto_sans_regular.ttf"))
    }

    @Test
    fun all_bundled_font_weights_cover_vietnamese_including_stacked_and_combining_marks() = runTest {
        val required = (0x1EA0..0x1EF9).toSet() + setOf(0x110, 0x111, 0x1A0, 0x1A1, 0x1AF, 0x1B0,
            0x300, 0x301, 0x303, 0x309, 0x323)
        listOf("noto_sans_regular", "noto_sans_bold", "noto_serif_regular", "noto_serif_bold").forEach { name ->
            val bytes = Res.readBytes("font/$name.ttf")
            val font = Font.createFont(Font.TRUETYPE_FONT, ByteArrayInputStream(bytes))
            assertEquals(emptyList(), required.filterNot { font.canDisplay(it) }, name)
            assertTrue(font.family.startsWith("Noto "), name)
        }
    }
}
