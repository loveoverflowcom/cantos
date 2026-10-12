package com.cantos.ui

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.asSkiaBitmap
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import com.cantos.ui.generated.CantosTokens
import com.cantos.ui.preview.PreviewApp
import java.io.File
import org.jetbrains.skia.EncodedImageFormat
import org.jetbrains.skia.Image
import org.junit.Test
import kotlin.math.roundToInt
import kotlin.test.assertEquals

/** Actual JVM Compose root captures; never mobile/device or OS-window screenshot evidence. */
class PreviewCaptureTest {
    @Test
    fun light_wide_component_root_is_rendered_and_captured() {
        capture("cmp-jvm-light-wide-components", UiPreferences(), wide = true, tokens = false)
    }

    @Test
    fun dark_wide_component_root_is_rendered_and_captured() {
        capture("cmp-jvm-dark-wide-components", UiPreferences(theme = Theme.Dark), wide = true, tokens = false)
    }

    @Test
    fun light_narrow_large_text_root_is_rendered_and_captured() {
        capture("cmp-jvm-light-narrow-text2-components", UiPreferences(textScale = 2f), wide = false, tokens = false)
    }

    @Test
    fun dark_wide_named_token_root_is_rendered_and_captured() {
        capture("cmp-jvm-dark-wide-tokens", UiPreferences(theme = Theme.Dark), wide = true, tokens = true)
    }

    @OptIn(ExperimentalTestApi::class)
    private fun capture(name: String, preferences: UiPreferences, wide: Boolean, tokens: Boolean) {
        val medium = CantosTokens.number("breakpoint.medium").toFloat()
        val expanded = CantosTokens.number("breakpoint.expanded").toFloat()
        val width = if (wide) expanded + medium / 2 else medium / 2 + CantosTokens.number("space.lg").toFloat() * 3
        // Size the renderer itself: an oversized root inside the default 1024x768 host is clipped.
        runDesktopComposeUiTest(width = width.roundToInt(), height = expanded.roundToInt()) {
            setContent {
                Box(Modifier.requiredSize(width.dp, expanded.dp).testTag("capture-root")) {
                    PreviewApp(initialPreferences = preferences, initialSection = if (tokens) 1 else 0)
                }
            }
            waitForIdle()
            val bitmap = onNodeWithTag("capture-root").captureToImage()
            assertEquals(width.roundToInt(), bitmap.width, "$name must retain the complete requested viewport width")
            assertEquals(expanded.roundToInt(), bitmap.height, "$name must retain the complete requested viewport height")
            val output = File(System.getenv("CANTOS_UI_PREVIEW_CAPTURE_DIR") ?: "build/reports/cantos-preview")
            check(output.isDirectory || output.mkdirs()) { "Could not create screenshot output directory" }
            val bytes = requireNotNull(Image.makeFromBitmap(bitmap.asSkiaBitmap()).encodeToData(EncodedImageFormat.PNG)).bytes
            File(output, "$name.png").writeBytes(bytes)
        }
    }
}
