package com.cantos.theatre.app

import kotlinx.coroutines.test.runTest
import org.junit.Test
import kotlin.test.assertNotNull
import kotlin.test.assertNull

class PreviewArtworkResourceTest {
    @Test
    fun exact_bundled_paintings_decode_and_missing_or_invalid_art_falls_back() = runTest {
        assertNotNull(loadArtwork("drawable/son_ha_moon.webp"))
        assertNotNull(loadArtwork("drawable/den_khuya.webp"))
        assertNull(loadArtwork("drawable/absent.png"))
        assertNull(loadArtwork("files/provenance.json"))
    }

}
