package com.cantos.theatre.app

import kotlinx.coroutines.test.runTest
import org.junit.Test
import kotlin.test.assertNotNull
import kotlin.test.assertNull

class PreviewArtworkResourceTest {
    @Test
    fun exact_bundled_paintings_decode_and_missing_or_invalid_art_falls_back() = runTest {
        assertNotNull(loadArtwork("files/artwork/son_ha_moon.webp"))
        assertNotNull(loadArtwork("files/artwork/den_khuya.webp"))
        assertNull(loadArtwork("files/artwork/absent.webp"))
        assertNull(loadArtwork("files/provenance.json"))
    }

}
