package com.cantos.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.material3.Text
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asSkiaBitmap
import androidx.compose.ui.graphics.toPixelMap
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.hasContentDescription
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.unit.dp
import java.io.File
import org.jetbrains.skia.EncodedImageFormat
import org.jetbrains.skia.Image
import org.junit.Rule
import org.junit.Test
import kotlin.math.abs
import kotlin.test.assertEquals
import kotlin.test.assertTrue

/** Actual JVM renderer oracle. These tests are not mobile or assistive-technology evidence. */
class LogoRendererTest {
    @get:Rule
    val compose = createComposeRule()

    @Test
    fun decorative_mark_has_no_name_and_standalone_mark_has_one_localized_image_name() {
        compose.setContent {
            CantosTheme(UiPreferences()) {
                Column {
                    Logo(LogoVariant.Symbol)
                    Text("Cantos")
                    Logo(LogoVariant.CantosStudio, accessibleName = AccessibleName.of("Logo của Cantos Studio"))
                }
            }
        }
        compose.onAllNodes(SemanticsMatcher.keyIsDefined(SemanticsProperties.ContentDescription))
            .assertCountEquals(1)
        compose.onNodeWithContentDescription("Logo của Cantos Studio")
            .assert(SemanticsMatcher.expectValue(SemanticsProperties.Role, Role.Image))
        compose.onAllNodes(hasContentDescription("Cantos symbol")).assertCountEquals(0)
    }

    @Test
    fun monochrome_holes_show_two_different_actual_caller_backgrounds() {
        // Deliberately unrelated test colors detect a fake hard-coded paper/dark fill.
        var current by mutableStateOf(Color(0xFF209050))
        compose.setContent {
            Box(Modifier.requiredSize(64.dp).background(current).testTag("mono")) {
                Logo(LogoVariant.Symbol, tone = LogoTone.Mono)
            }
        }
        listOf(Color(0xFF209050), Color(0xFFEC9940)).forEachIndexed { sample, color ->
            compose.runOnIdle { current = color }
            val image = compose.onNodeWithTag("mono").captureToImage()
            val pixels = image.toPixelMap()
            assertColor(color, pixels[image.width * 27 / 64, image.height / 2])
            assertColor(color, pixels[0, 0])
            assertColor(Color(0xFF292331), pixels[image.width * 55 / 64, image.height / 2])
            save(image, "cmp-logo-mono-transparent-background-$sample")
        }
    }

    @Test
    fun official_light_and_dark_ink_are_rendered_without_recoloring() {
        compose.setContent {
            Column {
                Box(Modifier.requiredSize(64.dp).testTag("light")) { Logo(LogoVariant.Symbol, tone = LogoTone.Light) }
                Box(Modifier.requiredSize(64.dp).testTag("dark")) { Logo(LogoVariant.Symbol, tone = LogoTone.Dark) }
            }
        }
        listOf("light" to Color(0xFF675080), "dark" to Color(0xFFD9BDF1)).forEach { (tag, ink) ->
            val image = compose.onNodeWithTag(tag).captureToImage()
            assertColor(ink, image.toPixelMap()[image.width * 55 / 64, image.height / 2])
            save(image, "cmp-logo-symbol-$tag")
        }
    }

    @Test
    fun lockup_fits_square_slot_without_stretching_its_rounded_symbol() {
        val background = Color(0xFF209050)
        compose.setContent {
            Box(Modifier.requiredSize(160.dp).background(background).testTag("square")) {
                Logo(LogoVariant.CantosStudio, modifier = Modifier.requiredSize(160.dp))
            }
        }
        val image = compose.onNodeWithTag("square").captureToImage()
        assertEquals(image.width, image.height)
        val pixels = image.toPixelMap()
        val painted = (0 until pixels.height).flatMap { y ->
            (0 until pixels.width).mapNotNull { x ->
                if (x >= pixels.width * 90 / 283) null
                else if (different(background, pixels[x, y])) Pair(x, y) else null
            }
        }
        val width = painted.maxOf { it.first } - painted.minOf { it.first } + 1
        val height = painted.maxOf { it.second } - painted.minOf { it.second } + 1
        assertTrue(abs(width - height) <= 1, "official square symbol stretched to $width x $height")
        assertColor(background, pixels[image.width / 2, image.height / 10])
        save(image, "cmp-logo-studio-aspect-fit-square160")
    }

    private fun different(expected: Color, actual: Color): Boolean =
        abs(expected.red - actual.red) > 0.01f || abs(expected.green - actual.green) > 0.01f ||
            abs(expected.blue - actual.blue) > 0.01f || abs(expected.alpha - actual.alpha) > 0.01f

    private fun assertColor(expected: Color, actual: Color) {
        assertTrue(!different(expected, actual), "expected $expected, rendered $actual")
    }

    private fun save(bitmap: androidx.compose.ui.graphics.ImageBitmap, name: String) {
        val output = File(System.getenv("CANTOS_UI_PREVIEW_CAPTURE_DIR") ?: "build/reports/cantos-preview")
        check(output.isDirectory || output.mkdirs())
        val encoded = requireNotNull(Image.makeFromBitmap(bitmap.asSkiaBitmap()).encodeToData(EncodedImageFormat.PNG))
        File(output, "$name.png").writeBytes(encoded.bytes)
    }
}
