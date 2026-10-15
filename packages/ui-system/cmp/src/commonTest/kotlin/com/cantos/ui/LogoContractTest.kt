package com.cantos.ui

import com.cantos.ui.generated.CantosLogos
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class LogoContractTest {
    @Test
    fun all_nine_official_variants_have_original_ratio_and_real_vector_shapes() {
        assertEquals(9, CantosLogos.assets.size)
        val expectedInk = mapOf(LogoTone.Light to 0xFF675080L,
            LogoTone.Dark to 0xFFD9BDF1L, LogoTone.Mono to 0xFF292331L)
        LogoVariant.entries.forEach { variant ->
            LogoTone.entries.forEach { tone ->
                val asset = logoAsset(variant, tone)
                val dimensions = if (variant == LogoVariant.Symbol) Pair(64f, 64f) else Pair(283f, 96f)
                assertEquals(dimensions, Pair(asset.width, asset.height))
                assertEquals(expectedInk.getValue(tone), asset.shapes.first().fill)
                assertTrue(asset.shapes.any { it.pathData?.contains("M42 15a20 20") == true })
                assertEquals(2, asset.shapes.count { it.strokeWidth > 0f })
                if (variant != LogoVariant.Symbol) {
                    assertTrue(asset.shapes.count { it.pathData != null && it.fill != null } >= 6)
                    assertEquals(Pair(16f, 16f), Pair(asset.shapes.first().translateX, asset.shapes.first().translateY))
                }
            }
        }
    }

    @Test
    fun mono_strokes_are_actual_transparent_knockouts_not_background_paint() {
        LogoVariant.entries.forEach { variant ->
            val mono = logoAsset(variant, LogoTone.Mono)
            val holes = mono.shapes.filter { it.knockout }
            assertEquals(listOf(5f, 3f), holes.map { it.strokeWidth })
            assertTrue(holes.all { it.roundCap && it.fill == null && it.pathData != null })
            assertTrue(mono.shapes.filter { !it.knockout }.all { it.fill == 0xFF292331L })
            listOf(LogoTone.Light, LogoTone.Dark).forEach { tone ->
                assertFalse(logoAsset(variant, tone).shapes.any { it.knockout })
            }
        }
    }
}
