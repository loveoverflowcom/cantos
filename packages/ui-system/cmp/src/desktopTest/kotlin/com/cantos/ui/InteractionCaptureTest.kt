package com.cantos.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.material3.Text
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.asSkiaBitmap
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import com.cantos.ui.preview.ComponentExample
import java.io.File
import org.jetbrains.skia.EncodedImageFormat
import org.jetbrains.skia.Image
import org.junit.Test
import kotlin.test.assertEquals

/** Same diagnostic specimens on a real JVM root; no mobile or OS-window claim. */
class InteractionCaptureTest {
    @Test
    fun full_label_choices_are_captured_at_both_densities_themes_and_narrow_large_text() {
        capture(PreviewExample.Checkbox, "label-activation", UiPreferences(theme = Theme.Dark, density = Density.Comfortable, textScale = 2f), narrow = true)
        capture(PreviewExample.Radio, "label-activation", UiPreferences())
        capture(PreviewExample.Switch, "error", UiPreferences(), narrow = true)
    }

    @Test
    fun select_range_and_overflow_contracts_are_captured() {
        capture(PreviewExample.Select, "missing-value", UiPreferences(theme = Theme.Dark, density = Density.Comfortable), narrow = true)
        capture(PreviewExample.Select, "empty-options", UiPreferences(), narrow = true)
        capture(PreviewExample.Select, "reordered-options", UiPreferences())
        capture(PreviewExample.RangeField, "stepped", UiPreferences(theme = Theme.Dark))
        capture(PreviewExample.NavTabs, "overflow", UiPreferences(textScale = 2f), narrow = true)
    }

    @Test
    fun quiet_layout_and_recovery_contracts_are_captured() {
        capture(PreviewExample.Stack, "horizontal", UiPreferences(theme = Theme.Dark, density = Density.Comfortable, textScale = 2f), narrow = true)
        capture(PreviewExample.Surface, "reading", UiPreferences())
        capture(PreviewExample.Grid, "three-columns", UiPreferences())
        capture(PreviewExample.Container, "inset-none", UiPreferences(), narrow = true)
        capture(PreviewExample.ErrorState, "recovery-slot", UiPreferences(theme = Theme.Dark), narrow = true)
    }

    @Test
    @OptIn(ExperimentalTestApi::class)
    fun range_regression_and_loading_announcement_are_captured() {
        capture(PreviewExample.LoadingState, "announcement", UiPreferences(textScale = 2f), narrow = true)
        runDesktopComposeUiTest(width = 372, height = 840) {
            setContent {
                var value by androidx.compose.runtime.remember { mutableStateOf(-50f) }
                Box(Modifier.requiredSize(372.dp, 840.dp).testTag("regression-root")) {
                    UiRoot(preferences = UiPreferences(theme = Theme.Dark, density = Density.Comfortable, textScale = 2f)) {
                        Column(Modifier.padding(tokenDp("space.lg"))) {
                            RangeField("Mức tín hiệu thử", value, { value = it }, range = -100f..0f, steps = 3,
                                valueText = "$value trên khoảng −100 đến 0 · Ỗ, Ữ")
                        }
                    }
                }
            }
            waitForIdle()
            val bitmap = onNodeWithTag("regression-root").captureToImage()
            assertEquals(372, bitmap.width)
            assertEquals(840, bitmap.height)
            val output = File(System.getenv("CANTOS_UI_PREVIEW_CAPTURE_DIR") ?: "build/reports/cantos-preview")
            check(output.isDirectory || output.mkdirs())
            File(output, "cmp-regression-negative-range-dark-comfortable-text2.0-372.png").writeBytes(
                requireNotNull(Image.makeFromBitmap(bitmap.asSkiaBitmap()).encodeToData(EncodedImageFormat.PNG)).bytes)
        }
    }

    @OptIn(ExperimentalTestApi::class)
    private fun capture(example: PreviewExample, state: String, preferences: UiPreferences, narrow: Boolean = false) {
        val width = if (narrow) 372 else 1140
        val name = "cmp-interaction-${example.id}-$state-${preferences.theme.tokenName}-${preferences.density.tokenName}-text${preferences.textScale}-$width"
        runDesktopComposeUiTest(width = width, height = 840) {
            setContent {
                Box(Modifier.requiredSize(width.dp, 840.dp).testTag("interaction-root")) {
                    UiRoot(preferences = preferences) {
                        Column(Modifier.padding(tokenDp("space.lg")), verticalArrangement = Arrangement.spacedBy(tokenDp("space.md"))) {
                            Text("${example.id} · $state · Ỗ, Ữ")
                            ComponentExample(example, state, {}, Modifier)
                        }
                    }
                }
            }
            waitForIdle()
            val bitmap = onNodeWithTag("interaction-root").captureToImage()
            assertEquals(width, bitmap.width)
            assertEquals(840, bitmap.height)
            val output = File(System.getenv("CANTOS_UI_PREVIEW_CAPTURE_DIR") ?: "build/reports/cantos-preview")
            check(output.isDirectory || output.mkdirs())
            val bytes = requireNotNull(Image.makeFromBitmap(bitmap.asSkiaBitmap()).encodeToData(EncodedImageFormat.PNG)).bytes
            File(output, "$name.png").writeBytes(bytes)
        }
    }
}
