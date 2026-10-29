package com.cantos.theatre.app

import androidx.compose.ui.graphics.toPixelMap
import com.cantos.theatre.adapters.image.decodeBundledAvif
import org.junit.Test
import kotlin.math.abs
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue

/**
 * The Server admits one primary image that is a single item or an ImageGrid, with an optional
 * alpha image. These synthetic files (built and cross-checked against libavif by
 * `build_alpha_grid_fixtures.py` in the Server fixtures) prove the pinned desktop codec shows the
 * same pixels, so an admitted cover never decodes to a different picture here.
 */
class BundledAvifAlphaGridTest {
    private fun fixture(name: String): ByteArray =
        assertNotNull(javaClass.getResourceAsStream("/avif/$name")).use { it.readBytes() }

    @Test
    fun single_item_alpha_keeps_size_and_the_alpha_plane() {
        val pixels = assertNotNull(decodeBundledAvif(fixture("pillow-alpha-64x48.avif"))).also {
            assertEquals(64 to 48, it.width to it.height)
        }.toPixelMap()
        assertEquals(0f, pixels[0, 0].alpha)
        assertEquals(69 / 255f, pixels[63, 47].alpha, 1.5f / 255f)
    }

    @Test
    fun grid_tiles_are_assembled_in_order_and_cropped_to_the_declared_output() {
        val pixels = assertNotNull(decodeBundledAvif(fixture("grid-2x2-120x100.avif"))).also {
            assertEquals(120 to 100, it.width to it.height)
        }.toPixelMap()
        // The source image is a horizontal red ramp and a vertical green ramp across all tiles.
        for ((x, y) in listOf(0 to 0, 63 to 0, 64 to 0, 119 to 0, 0 to 99, 64 to 99, 119 to 99, 100 to 70)) {
            val color = pixels[x, y]
            assertTrue(abs(color.red - (x * 255 / 119) / 255f) < 6f / 255f, "red at ($x, $y)")
            assertTrue(abs(color.green - (y * 255 / 99) / 255f) < 6f / 255f, "green at ($x, $y)")
            assertEquals(1f, color.alpha)
        }
    }

    @Test
    fun grid_with_an_alpha_grid_decodes_both_planes() {
        val pixels = assertNotNull(decodeBundledAvif(fixture("grid-alpha-2x2-120x100.avif"))).also {
            assertEquals(120 to 100, it.width to it.height)
        }.toPixelMap()
        assertEquals(0f, pixels[0, 0].alpha)
        assertEquals(1f, pixels[60, 50].alpha, 1.5f / 255f)
        assertEquals(0f, pixels[119, 99].alpha)
    }

    @Test
    fun a_grid_with_a_missing_tile_is_refused() {
        val file = fixture("grid-2x2-120x100.avif")
        assertNull(decodeBundledAvif(file.copyOf(file.size - 100)))
    }
}
