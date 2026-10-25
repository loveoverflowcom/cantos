package com.cantos.ui

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import com.cantos.ui.generated.CantosTokens
import com.cantos.ui.preview.PreviewFonts
import com.cantos.ui.preview.PreviewApp
import com.cantos.ui.preview.ViewportPreset
import org.junit.Test
import kotlin.math.roundToInt
import kotlin.test.assertEquals

/** Capture page heights in dp. Each is tall enough to show the specimen detail at that width and text scale. */
private object CaptureHeights {
    const val PhonePage = 1800
    const val SmallLargeTextPage = 5200
    const val ExpandedPage = 1000
}

/**
 * Actual JVM Compose root captures; never mobile/device or OS-window screenshot evidence. Each PNG
 * has a JSON sidecar naming its scenario, viewport (dp), pixel size, theme, text scale and digests.
 */
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

    @Test
    fun a_proposed_theatre_specimen_is_captured_at_the_typical_phone_constraint() {
        render("cmp-jvm-design-v3-light-390-text1-mini-player", UiPreferences(), ViewportPreset.Typical, CaptureHeights.PhonePage,
            section = 2, specimen = "mini-player")
    }

    @Test
    fun a_web_only_specimen_is_captured_at_the_small_constraint_with_double_text_in_dark() {
        render("cmp-jvm-design-v3-dark-320-text2-studio-workspace", UiPreferences(theme = Theme.Dark, textScale = 2f),
            ViewportPreset.Small, CaptureHeights.SmallLargeTextPage, section = 2, specimen = "studio-workspace")
    }

    @Test
    fun an_existing_specimen_is_captured_in_the_two_pane_layout_at_the_expanded_constraint() {
        render("cmp-jvm-design-v3-light-840-text1-button", UiPreferences(), ViewportPreset.Expanded, CaptureHeights.ExpandedPage,
            section = 2, specimen = "button")
    }

    @Test
    fun system_theme_follows_the_injected_host_fact_in_a_typical_phone_capture() {
        render("cmp-jvm-components-system-dark-390-text1", UiPreferences(), ViewportPreset.Typical, CaptureHeights.PhonePage, section = 0,
            mode = ThemeMode.System, systemIsDark = true)
    }

    private fun capture(name: String, preferences: UiPreferences, wide: Boolean, tokens: Boolean) {
        val medium = CantosTokens.number("breakpoint.medium").toFloat()
        val expanded = CantosTokens.number("breakpoint.expanded").toFloat()
        val width = if (wide) expanded + medium / 2 else medium / 2 + CantosTokens.number("space.lg").toFloat() * 3
        render(name, preferences, width.roundToInt(), expanded.roundToInt(), section = if (tokens) 1 else 0)
    }

    private fun render(
        name: String,
        preferences: UiPreferences,
        preset: ViewportPreset,
        heightDp: Int,
        section: Int,
        mode: ThemeMode = if (preferences.theme == Theme.Dark) ThemeMode.Dark else ThemeMode.Light,
        systemIsDark: Boolean = false,
        specimen: String? = null,
    ) = render(name, preferences, preset.widthDp.roundToInt(), heightDp, section, mode, systemIsDark, specimen)

    @OptIn(ExperimentalTestApi::class)
    private fun render(
        name: String,
        preferences: UiPreferences,
        widthDp: Int,
        heightDp: Int,
        section: Int,
        mode: ThemeMode = if (preferences.theme == Theme.Dark) ThemeMode.Dark else ThemeMode.Light,
        systemIsDark: Boolean = false,
        specimen: String? = null,
    ) {
        var hostDensity = 0f
        var hostFontScale = 0f
        // Size the renderer itself: an oversized root inside the default 1024x768 host is clipped.
        runDesktopComposeUiTest(width = widthDp, height = heightDp) {
            setContent {
                val host = LocalDensity.current
                hostDensity = host.density
                hostFontScale = host.fontScale
                Box(Modifier.requiredSize(widthDp.dp, heightDp.dp).testTag("capture-root")) {
                    PreviewApp(uiFontFamily = PreviewFonts.ui, editorialFontFamily = PreviewFonts.editorial,
                        initialPreferences = preferences, initialSection = section,
                        systemIsDark = systemIsDark, initialThemeMode = mode, initialSpecimenId = specimen)
                }
            }
            waitForIdle()
            val bitmap = onNodeWithTag("capture-root").captureToImage()
            assertEquals(widthDp, bitmap.width, "$name must retain the complete requested viewport width")
            assertEquals(heightDp, bitmap.height, "$name must retain the complete requested viewport height")
            writeCapture(
                CaptureScenario(
                    name = name,
                    section = listOf("components", "named-tokens", "design-v3")[section],
                    component = null,
                    state = null,
                    specimen = specimen,
                    themeMode = mode,
                    systemIsDark = systemIsDark,
                    preferences = preferences.copy(theme = mode.resolve(systemIsDark)),
                    viewportWidthDp = widthDp,
                    viewportHeightDp = heightDp,
                    hostDensity = hostDensity,
                    hostFontScale = hostFontScale,
                    fontSource = BundledPreviewFontSource,
                ),
                bitmap,
            )
        }
    }
}
