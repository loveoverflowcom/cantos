package com.cantos.ui

import androidx.compose.foundation.layout.Column
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.LocalMinimumInteractiveComponentSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Outline
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.Density as DisplayDensity
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.cantos.ui.generated.CantosTokens
import org.junit.Rule
import org.junit.Test
import kotlin.test.assertEquals

/** Production-theme values sampled from an actual JVM Compose composition, not a copied theme. */
@OptIn(ExperimentalMaterial3Api::class)
class ThemeFoundationTest {
    @get:Rule
    val compose = createComposeRule()

    private val snapshots = mutableMapOf<String, ThemeSnapshot>()
    private val families = mutableMapOf<String, FamilySnapshot>()

    @Test
    fun nested_theme_applies_an_absolute_preview_scale_and_preserves_platform_density() {
        compose.setContent {
            CompositionLocalProvider(LocalDensity provides DisplayDensity(2f, 1.5f)) {
                CantosTheme(UiPreferences(textScale = 1.3f)) {
                    Column {
                        Probe("Ngoài")
                        CantosTheme(UiPreferences(textScale = 1.3f)) { Probe("Cùng mức") }
                        CantosTheme(UiPreferences(textScale = 2f)) {
                            Column {
                                Probe("Hai lần")
                                CantosTheme(UiPreferences(textScale = 1f)) { Probe("Mức nền") }
                            }
                        }
                        Probe("Trở về")
                    }
                }
            }
        }
        compose.runOnIdle {
            assertDensity("Ngoài", 2f, 1.95f)
            assertDensity("Cùng mức", 2f, 1.95f)
            assertDensity("Hai lần", 2f, 3f)
            assertDensity("Mức nền", 2f, 1.5f)
            assertDensity("Trở về", 2f, 1.95f)
        }
    }

    @Test
    fun nested_colors_preferences_and_token_density_restore_for_siblings_and_outer_host() {
        val hostPreferences = UiPreferences(textScale = 0.75f)
        val outerPreferences = UiPreferences(
            theme = Theme.Dark,
            density = Density.Comfortable,
            reducedMotion = true,
            textScale = 1.3f,
        )
        val innerPreferences = UiPreferences(textScale = 2f)
        val hostPrimary = parseTokenColor(CantosTokens.color("color.accent", "light"))
        compose.setContent {
            CompositionLocalProvider(
                LocalDensity provides DisplayDensity(2f, 1.5f),
                LocalUiPreferences provides hostPreferences,
                LocalMinimumInteractiveComponentSize provides 56.dp,
            ) {
                MaterialTheme(colorScheme = lightColorScheme(primary = hostPrimary)) {
                    Column {
                        CantosTheme(outerPreferences) {
                            Column {
                                Probe("Trước")
                                CantosTheme(innerPreferences) { Probe("Bên trong") }
                                Probe("Sau")
                            }
                        }
                        Probe("Host")
                    }
                }
            }
        }
        compose.runOnIdle {
            listOf("Trước", "Sau").forEach { name ->
                val snapshot = snapshots.getValue(name)
                assertEquals(outerPreferences, snapshot.preferences)
                assertEquals(parseTokenColor(CantosTokens.color("color.primary", "dark")), snapshot.primary)
                assertEquals(48f, snapshot.controlSize.value)
                assertEquals(48.dp, snapshot.minimumControlSize)
                assertDensity(name, 2f, 1.95f)
            }
            val inner = snapshots.getValue("Bên trong")
            assertEquals(innerPreferences, inner.preferences)
            assertEquals(parseTokenColor(CantosTokens.color("color.primary", "light")), inner.primary)
            assertEquals(48f, inner.controlSize.value)
            assertEquals(48.dp, inner.minimumControlSize)
            assertDensity("Bên trong", 2f, 3f)
            val host = snapshots.getValue("Host")
            assertEquals(hostPreferences, host.preferences)
            assertEquals(hostPrimary, host.primary)
            assertEquals(56.dp, host.minimumControlSize)
            assertDensity("Host", 2f, 1.5f)
        }
    }

    @Test
    fun host_font_scale_and_nested_preferences_recompute_without_compounding() {
        var hostFontScale by mutableStateOf(1.5f)
        var innerPreferences by mutableStateOf(UiPreferences(textScale = 1.3f))
        compose.setContent {
            CompositionLocalProvider(LocalDensity provides DisplayDensity(2f, hostFontScale)) {
                CantosTheme(UiPreferences(textScale = 2f)) {
                    CantosTheme(innerPreferences) { Probe("Thay đổi") }
                }
            }
        }
        compose.runOnIdle {
            assertDensity("Thay đổi", 2f, 1.95f)
            hostFontScale = 2f
        }
        compose.runOnIdle {
            assertDensity("Thay đổi", 2f, 2.6f)
            innerPreferences = innerPreferences.copy(textScale = 1f)
        }
        compose.runOnIdle { assertDensity("Thay đổi", 2f, 2f) }
    }

    @Test
    fun injected_font_families_map_ui_and_editorial_roles_without_changing_density() {
        var body: FontFamily? = null
        var headline: FontFamily? = null
        var editorial: FontFamily? = null
        compose.setContent {
            CantosTheme(UiPreferences(), uiFontFamily = FontFamily.Monospace, editorialFontFamily = FontFamily.Cursive) {
                body = MaterialTheme.typography.bodyMedium.fontFamily
                headline = MaterialTheme.typography.headlineLarge.fontFamily
                editorial = LocalEditorialFontFamily.current
                Probe("Fonts")
            }
        }
        compose.runOnIdle {
            assertEquals(FontFamily.Monospace, body)
            assertEquals(FontFamily.Cursive, headline)
            assertEquals(FontFamily.Cursive, editorial)
            assertDensity("Fonts", 1f, 1f)
        }
    }

    @Test
    fun a_nested_theme_inherits_host_injected_families_and_an_explicit_family_replaces_only_its_own_role() {
        compose.setContent {
            CantosTheme(UiPreferences(), uiFontFamily = FontFamily.Monospace, editorialFontFamily = FontFamily.Cursive) {
                Column {
                    CantosTheme(UiPreferences(theme = Theme.Dark, textScale = 2f)) { FamilyProbe("inherited") }
                    CantosTheme(UiPreferences(), uiFontFamily = FontFamily.SansSerif) { FamilyProbe("own-ui") }
                    CantosTheme(UiPreferences(), editorialFontFamily = FontFamily.SansSerif) { FamilyProbe("own-editorial") }
                    // Two levels below the host: an intermediate theme that names nothing must pass the families on.
                    CantosTheme(UiPreferences(textScale = 1.3f)) { CantosTheme(UiPreferences(density = Density.Comfortable)) { FamilyProbe("two-levels") } }
                }
            }
        }
        compose.runOnIdle {
            assertEquals(FamilySnapshot(FontFamily.Monospace, FontFamily.Cursive, FontFamily.Cursive), families.getValue("inherited"))
            assertEquals(FamilySnapshot(FontFamily.SansSerif, FontFamily.Cursive, FontFamily.Cursive), families.getValue("own-ui"))
            assertEquals(FamilySnapshot(FontFamily.Monospace, FontFamily.SansSerif, FontFamily.SansSerif), families.getValue("own-editorial"))
            assertEquals(FamilySnapshot(FontFamily.Monospace, FontFamily.Cursive, FontFamily.Cursive), families.getValue("two-levels"))
        }
    }

    @Test
    fun a_theme_with_no_host_families_keeps_the_reviewed_platform_fallbacks_even_when_nested() {
        compose.setContent {
            CantosTheme(UiPreferences()) {
                CantosTheme(UiPreferences(textScale = 2f)) { FamilyProbe("fallback") }
            }
        }
        compose.runOnIdle {
            assertEquals(FamilySnapshot(FontFamily.SansSerif, FontFamily.SansSerif, FontFamily.Serif), families.getValue("fallback"))
        }
    }

    @Test
    fun material_shapes_follow_the_shared_five_step_radius_scale() {
        val radii = mutableListOf<Float>()
        compose.setContent {
            CantosTheme(UiPreferences()) {
                val shapes = MaterialTheme.shapes
                val density = LocalDensity.current
                val values = listOf(shapes.extraSmall, shapes.small, shapes.medium, shapes.large, shapes.extraLarge)
                    .map { shape -> cornerRadius(shape, density) }
                SideEffect { radii.addAll(values) }
            }
        }
        compose.runOnIdle { assertEquals(listOf(4f, 8f, 12f, 16f, 20f), radii) }
    }

    @Test
    fun thumbnail_and_full_shapes_preserve_their_roles_at_multiple_sizes_and_densities() {
        val results = mutableListOf<Float>()
        compose.setContent {
            CompositionLocalProvider(LocalDensity provides DisplayDensity(2f, 2f)) {
                CantosTheme(UiPreferences(density = Density.Comfortable)) {
                    val thumbnail = cornerRadius(CantosShapes.thumbnail, LocalDensity.current)
                    val full = CantosShapes.full
                    val radii = listOf(48f, 80f, 200f).map { width ->
                        val outline = full.createOutline(Size(width, 48f), LayoutDirection.Ltr,
                            LocalDensity.current) as Outline.Rounded
                        outline.roundRect.topLeftCornerRadius.x
                    }
                    SideEffect { results.add(thumbnail); results.addAll(radii) }
                }
            }
        }
        compose.runOnIdle { assertEquals(listOf(10f, 24f, 24f, 24f), results) }
    }

    private fun cornerRadius(shape: Shape, density: DisplayDensity): Float {
        val outline = shape.createOutline(Size(200f, 200f), LayoutDirection.Ltr, density) as Outline.Rounded
        return outline.roundRect.topLeftCornerRadius.x / density.density
    }

    @Composable
    private fun Probe(name: String) {
        val density = LocalDensity.current
        val snapshot = ThemeSnapshot(
            density = density.density,
            fontScale = density.fontScale,
            preferences = LocalUiPreferences.current,
            primary = MaterialTheme.colorScheme.primary,
            controlSize = tokenDp("size.control"),
            minimumControlSize = LocalMinimumInteractiveComponentSize.current,
        )
        SideEffect { snapshots[name] = snapshot }
        Text(name)
    }

    @Composable
    private fun FamilyProbe(name: String) {
        val snapshot = FamilySnapshot(
            body = MaterialTheme.typography.bodyMedium.fontFamily,
            headline = MaterialTheme.typography.headlineLarge.fontFamily,
            editorial = LocalEditorialFontFamily.current,
        )
        SideEffect { families[name] = snapshot }
        Text(name)
    }

    private fun assertDensity(name: String, expectedDensity: Float, expectedFontScale: Float) {
        val snapshot = snapshots.getValue(name)
        assertEquals(expectedDensity, snapshot.density)
        assertEquals(expectedFontScale, snapshot.fontScale, absoluteTolerance = 0.0001f)
    }

    private data class FamilySnapshot(val body: FontFamily?, val headline: FontFamily?, val editorial: FontFamily)

    private data class ThemeSnapshot(
        val density: Float,
        val fontScale: Float,
        val preferences: UiPreferences,
        val primary: Color,
        val controlSize: Dp,
        val minimumControlSize: Dp,
    )
}
