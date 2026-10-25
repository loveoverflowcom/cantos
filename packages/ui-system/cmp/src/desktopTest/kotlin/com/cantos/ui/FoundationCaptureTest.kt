package com.cantos.ui

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.material3.Text
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.asSkiaBitmap
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import com.cantos.ui.generated.CantosTokens
import com.cantos.ui.preview.ComponentExample
import java.io.File
import org.jetbrains.skia.EncodedImageFormat
import org.jetbrains.skia.Image
import org.junit.Test
import kotlin.math.roundToInt
import kotlin.test.assertEquals

/** State-specific captures of the same preview specimens; no OS-window/device claim. */
class FoundationCaptureTest {
    @Test
    fun action_registry_and_pending_glyphs_are_captured_in_both_themes() {
        Theme.entries.forEach { theme ->
            capture(PreviewExample.Icon, "decorative", UiPreferences(theme = theme), narrow = true)
            capture(PreviewExample.Button, "loading", UiPreferences(theme = theme), narrow = true)
            capture(PreviewExample.IconButton, "loading", UiPreferences(theme = theme), narrow = true)
        }
    }

    @Test
    fun composed_action_light_and_dark_specimens_are_captured() {
        Theme.entries.forEach { theme ->
            capture(PreviewExample.Button, "composed-content", UiPreferences(theme = theme))
        }
    }

    @Test
    fun rich_text_and_number_fields_are_captured_with_focus_and_large_vietnamese_text() {
        capture(PreviewExample.TextField, "selection", UiPreferences(textScale = 2f), narrow = true, focus = true)
        capture(PreviewExample.TextArea, "selection", UiPreferences(theme = Theme.Dark), focus = true)
        capture(PreviewExample.NumberField, "selection", UiPreferences(theme = Theme.Dark), focus = true)
    }

    @Test
    fun nested_theme_and_blank_error_specimens_are_captured() {
        capture(PreviewExample.UiRoot, "nested-theme", UiPreferences(textScale = 2f))
        capture(PreviewExample.TextField, "blank-error", UiPreferences(theme = Theme.Dark, density = Density.Comfortable))
    }

    @OptIn(ExperimentalTestApi::class)
    private fun capture(
        example: PreviewExample,
        state: String,
        preferences: UiPreferences,
        narrow: Boolean = false,
        focus: Boolean = false,
    ) {
        val medium = CantosTokens.number("breakpoint.medium").toFloat()
        val expanded = CantosTokens.number("breakpoint.expanded").toFloat()
        val width = if (narrow) medium / 2 + CantosTokens.number("space.lg").toFloat() * 3 else expanded + medium / 2
        val name = "cmp-foundation-${example.id}-$state-${preferences.theme.tokenName}-${preferences.density.tokenName}-text${preferences.textScale}-${width.roundToInt()}"
        runDesktopComposeUiTest(width = width.roundToInt(), height = expanded.roundToInt()) {
            setContent {
                Box(Modifier.requiredSize(width.dp, expanded.dp).testTag("specimen-root")) {
                    BundledPreviewCaptureRoot(preferences = preferences) {
                        Column(Modifier.padding(tokenDp("space.lg")),
                            verticalArrangement = Arrangement.spacedBy(tokenDp("space.md"))) {
                            Text("${example.id} · $state · Ỗ, Ữ")
                            ComponentExample(example, state, {}, Modifier)
                        }
                    }
                }
            }
            if (focus) onNode(hasSetTextAction()).performClick()
            waitForIdle()
            val bitmap = onNodeWithTag("specimen-root").captureToImage()
            assertEquals(width.roundToInt(), bitmap.width)
            assertEquals(expanded.roundToInt(), bitmap.height)
            val output = File(System.getenv("CANTOS_UI_PREVIEW_CAPTURE_DIR") ?: "build/reports/cantos-preview")
            check(output.isDirectory || output.mkdirs()) { "Could not create screenshot output directory" }
            val bytes = requireNotNull(Image.makeFromBitmap(bitmap.asSkiaBitmap()).encodeToData(EncodedImageFormat.PNG)).bytes
            File(output, "$name.png").writeBytes(bytes)
        }
    }
}
