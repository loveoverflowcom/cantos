package com.cantos.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.padding
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import com.cantos.ui.preview.ComponentExample
import org.junit.Test

/** Scoped paired-admission specimens. Real shared components; JVM test root, never native OS/AT. */
@OptIn(ExperimentalTestApi::class)
class LoadingAdmissionCaptureTest {
    @Test
    fun loading_variants_retain_their_geometry_in_both_themes_at_normal_and_large_text() {
        for (theme in Theme.entries) for (width in listOf(320, 840)) for (reduced in listOf(false, true)) {
            val preferences = UiPreferences(theme = theme, textScale = if (width == 320) 2f else 1f, reducedMotion = reduced)
            runDesktopComposeUiTest(width = width, height = 4000) {
                mainClock.autoAdvance = false
                var density = 1f
                var scale = 1f
                setContent {
                    density = LocalDensity.current.density
                    scale = LocalDensity.current.fontScale
                    BundledPreviewCaptureRoot(preferences) {
                        Column(Modifier.testTag("loading-admission").padding(16.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
                            ComponentExample(PreviewExample.SurfacePulse, "running", {}, Modifier)
                            ComponentExample(PreviewExample.LoadingState, "loading", {}, Modifier)
                            ComponentExample(PreviewExample.Button, "loading", {}, Modifier)
                            ComponentExample(PreviewExample.FilterChip, "loading", {}, Modifier)
                        }
                    }
                }
                mainClock.advanceTimeByFrame()
                writeCapture(CaptureScenario("loading-admission-${theme.tokenName}-$width-${if (reduced) "reduced" else "full"}",
                    "feedback", "surface-pulse/loading-state/button/filter-chip", "loading",
                    themeMode = if (theme == Theme.Dark) ThemeMode.Dark else ThemeMode.Light, systemIsDark = false,
                    preferences = preferences, viewportWidthDp = width, viewportHeightDp = 4000,
                    hostDensity = density, hostFontScale = scale, fontSource = BundledPreviewFontSource,
                    frame = CaptureFrame("loading-v1-588", "first-frame", mainClock.currentTime, "manual-compose-test-clock")),
                    onNodeWithTag("loading-admission").captureToImage())
            }
        }
    }
}
