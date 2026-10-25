package com.cantos.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.style.LineHeightStyle
import androidx.compose.ui.unit.dp
import com.cantos.ui.preview.PreviewFonts
import org.junit.Test
import kotlin.test.assertEquals

/**
 * The same Vietnamese type specimen, rendered by the real theme roles with the platform fallback
 * families and with the shared Noto bundle. The reference is rendered from the design's own font
 * bytes by `artifacts/ui-evidence/issue143/ref/type-specimen-light-390x520.html`; the pair is compared
 * with `cargo xtask visual-compare`. JVM test-root renders, never device or design-approval evidence.
 */
class TypeSpecimenCaptureTest {
    @Test
    fun the_type_specimen_with_platform_fallback_families_is_captured() {
        capture("cmp-jvm-type-specimen-light-390x520-fallback", noto = false)
    }

    /**
     * Diagnostic only: the same Noto render with the CSS line-box model (half-leading above and below,
     * nothing trimmed) applied in this fixture, to test the cause of the vertical drift against the
     * reference. It changes no shipped theme.
     */
    @Test
    fun the_noto_type_specimen_with_a_css_style_line_box_is_captured_as_a_diagnostic() {
        capture("cmp-jvm-type-specimen-light-390x520-noto-cssbox", noto = true, cssLineBox = true)
    }

    @Test
    fun the_type_specimen_with_the_shared_noto_bundle_is_captured() {
        capture("cmp-jvm-type-specimen-light-390x520-noto", noto = true)
    }

    @OptIn(ExperimentalTestApi::class)
    private fun capture(name: String, noto: Boolean, cssLineBox: Boolean = false) {
        val preferences = UiPreferences()
        var hostDensity = 0f
        var hostFontScale = 0f
        runDesktopComposeUiTest(width = WidthDp, height = HeightDp) {
            setContent {
                val host = LocalDensity.current
                hostDensity = host.density
                hostFontScale = host.fontScale
                CantosTheme(preferences, if (noto) PreviewFonts.ui else null, if (noto) PreviewFonts.editorial else null) {
                    Box(Modifier.requiredSize(WidthDp.dp, HeightDp.dp).testTag("capture-root")) {
                        RootContent(Modifier) { TypeSpecimen(cssLineBox) }
                    }
                }
            }
            waitForIdle()
            val bitmap = onNodeWithTag("capture-root").captureToImage()
            assertEquals(WidthDp, bitmap.width)
            assertEquals(HeightDp, bitmap.height)
            writeCapture(
                CaptureScenario(
                    name = name,
                    section = "type-specimen",
                    component = null,
                    state = when {
                        cssLineBox -> "noto-bundle-css-line-box-diagnostic"
                        noto -> "noto-bundle"
                        else -> "platform-fallback"
                    },
                    themeMode = ThemeMode.Light,
                    systemIsDark = false,
                    preferences = preferences,
                    viewportWidthDp = WidthDp,
                    viewportHeightDp = HeightDp,
                    hostDensity = hostDensity,
                    hostFontScale = hostFontScale,
                    fontSource = if (noto) BundledPreviewFontSource else "reviewed platform SansSerif/Serif fallback",
                ),
                bitmap,
            )
        }
    }

    private companion object {
        const val WidthDp = 390
        const val HeightDp = 520
    }
}

/** One block per theme type role, in the order the reference page lists them. */
@Composable
private fun TypeSpecimen(cssLineBox: Boolean) {
    // CSS centers the leading above and below every line and trims none; Compose's default trims the
    // first line's top and last line's bottom. Diagnostic variant only.
    val css = LineHeightStyle(LineHeightStyle.Alignment.Center, LineHeightStyle.Trim.None)
    fun role(style: TextStyle) = if (cssLineBox) style.copy(lineHeightStyle = css) else style
    val type = MaterialTheme.typography
    Column(Modifier.padding(tokenDp("space.lg")), verticalArrangement = Arrangement.spacedBy(tokenDp("space.md"))) {
        Text("Ánh đèn cuối sân khấu", style = role(type.headlineMedium))
        Text("Người dẫn chuyện", style = role(type.titleMedium))
        Text(
            "Một câu chuyện kể về người giữ ngọn đèn bên bờ sông, nơi tiếng chuông vang qua màn sương mỗi sớm mai.",
            style = role(type.bodyMedium),
        )
        Text("Nghe tiếp · Lưu vào tủ sách", style = role(type.labelLarge))
        Text("Tập 02 · 24:10 · Đã tải về", style = role(type.bodySmall), color = tokenColor("color.onSurfaceVariant"))
    }
}
