package com.cantos.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.material3.Text
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
