package com.cantos.theatre.adapters.image

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull

class BundledAvifBoundsTest {
    @Test
    fun maximum_pixels_downsample_without_stretching_and_small_art_is_not_upscaled() {
        assertEquals(1536 to 1536, bundledDecodeDimensions(4096, 4096, 1536))
        assertEquals(768 to 1536, bundledDecodeDimensions(2048, 4096, 1536))
        assertEquals(1536 to 768, bundledDecodeDimensions(4096, 2048, 1536))
        assertEquals(1374 to 1145, bundledDecodeDimensions(1374, 1145, 1536))
        assertEquals(2 to 3, bundledDecodeDimensions(2, 3, 1536))
    }

    @Test
    fun invalid_overflow_and_over_budget_facts_are_refused_before_pixel_allocation() {
        for ((width, height) in listOf(0 to 1, 1 to 0, -1 to 1, 4097 to 1, Int.MAX_VALUE to Int.MAX_VALUE)) {
            assertNull(bundledDecodeDimensions(width, height, 1536))
        }
        assertNull(bundledDecodeDimensions(2, 3, 0))
        assertNull(bundledDecodeDimensions(2, 3, 4097))
    }
}
