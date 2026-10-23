package com.cantos.theatre.app

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.asSkiaBitmap
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.printToString
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import com.cantos.theatre.app.workbench.TheatreFixture
import com.cantos.theatre.app.workbench.TheatreRecipe
import com.cantos.theatre.domain.library.LibraryLoad
import com.cantos.theatre.domain.library.LibrarySection
import com.cantos.ui.Density
import com.cantos.ui.MotionPreference
import com.cantos.ui.ThemeMode
import com.cantos.ui.preview.workbench.ConstraintFrame
import com.cantos.ui.preview.workbench.PreviewLocale
import com.cantos.ui.preview.workbench.PreviewSettings
import java.io.File
import java.security.MessageDigest
import org.jetbrains.skia.EncodedImageFormat
import org.jetbrains.skia.Image
import org.junit.Test
import kotlin.test.assertEquals

/** Reproducible workbench recipes: production shell/controller/store, mocked external ports only. */
@OptIn(ExperimentalTestApi::class)
class LibraryEmptyCaptureTest {
    @Test
    fun library_phase_recipes_render_in_both_locales_and_themes() {
        val fixtures = listOf(TheatreFixture.LibraryEmpty, TheatreFixture.LibraryPartial, TheatreFixture.LibraryLoading,
            TheatreFixture.LibraryError, TheatreFixture.LibraryStale, TheatreFixture.HistoryEmpty)
        fixtures.forEach { fixture ->
            listOf(ThemeMode.Light, ThemeMode.Dark).forEach { theme ->
                PreviewLocale.entries.forEach { locale -> capture(fixture, PreviewSettings(themeMode = theme, locale = locale, density = Density.Comfortable, reducedMotion = true)) }
            }
        }
    }

    @Test
    fun narrow_long_copy_at_double_text_scale_scrolls_to_the_primary_action_and_last_section() {
        listOf(ThemeMode.Light, ThemeMode.Dark).forEach { theme ->
            PreviewLocale.entries.forEach { locale ->
                val settings = PreviewSettings(themeMode = theme, locale = locale, fontScale = 2f,
                    constraints = ConstraintFrame(320, 800), density = Density.Comfortable, reducedMotion = true)
                capture(TheatreFixture.LibraryEmpty, settings, "library-empty-discover")
                capture(TheatreFixture.LibraryEmpty, settings, "library-history")
                capture(TheatreFixture.HistoryEmpty, settings, "library-empty-discover")
            }
        }
    }

    @Test
    fun completed_empty_discover_navigates_once_through_the_real_shell() {
        lateinit var owners: PreviewOwners
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent { PreviewTestRoot(TheatreRecipe.fixture(TheatreFixture.LibraryEmpty).initialSeed(false), onOwners = { owners = it }) }
            waitUntil(timeoutMillis = 5_000) { onAllNodesWithTag("library-empty-hero").fetchSemanticsNodes().isNotEmpty() }
            onAllNodesWithTag("library-empty-discover").assertCountEquals(1)
            onNodeWithTag("history-all").assertDoesNotExist()
            onNodeWithTag("library-empty-discover").performScrollTo().performClick()
            assertEquals(listOf(TheatreRoute.Discovery), owners.shell.navigation.value.stack)
        }
    }

    private fun capture(fixture: TheatreFixture, settings: PreviewSettings, scrollTo: String? = null) {
        val recipe = TheatreRecipe.fixture(fixture, settings)
        val seed = recipe.initialSeed(false)
        val width = settings.constraints.widthDp
        val height = settings.constraints.heightDp
        val name = "issue417-${fixture.name}-${settings.themeMode}-${settings.locale}-${width}x$height-text${settings.fontScale}-${scrollTo ?: "top"}"
        lateinit var owners: PreviewOwners
        var density = Float.NaN
        var fontScale = Float.NaN
        runDesktopComposeUiTest(width = width, height = height) {
            setContent {
                density = LocalDensity.current.density
                fontScale = LocalDensity.current.fontScale
                Box(Modifier.requiredSize(width.dp, height.dp).testTag("issue417-root")) {
                    PreviewTestRoot(seed, textScale = settings.fontScale, density = settings.density,
                        motionPreference = if (settings.reducedMotion) MotionPreference.Reduced else MotionPreference.Full,
                        onOwners = { owners = it })
                }
            }
            waitUntil(timeoutMillis = 5_000) {
                val library = owners.shell.library.value
                library.account != null && when (seed.libraryRead) {
                    PreviewLibraryRead.Ready -> LibrarySection.entries.all { library.section(it).loaded && library.section(it).loading == LibraryLoad.Idle }
                    PreviewLibraryRead.Loading -> LibrarySection.entries.any { library.section(it).loading == LibraryLoad.Refreshing }
                    PreviewLibraryRead.Failed -> LibrarySection.entries.all { library.section(it).failure != null }
                    PreviewLibraryRead.Stale -> library.section(LibrarySection.Continue).failure != null
                }
            }
            waitForIdle()
            if (scrollTo != null) onNodeWithTag(scrollTo).performScrollTo()
            waitForIdle()
            val root = onNodeWithTag("issue417-root")
            val bitmap = root.captureToImage()
            assertEquals(width, bitmap.width)
            assertEquals(height, bitmap.height)
            val bytes = requireNotNull(Image.makeFromBitmap(bitmap.asSkiaBitmap()).encodeToData(EncodedImageFormat.PNG)).bytes
            val output = File(System.getenv("CANTOS_THEATRE_DEMO_CAPTURE_DIR") ?: "build/reports/theatre-demo")
            check(output.isDirectory || output.mkdirs())
            File(output, "$name.png").writeBytes(bytes)
            File(output, "$name.semantics.txt").writeText(root.printToString())
            val hash = MessageDigest.getInstance("SHA-256").digest(bytes).joinToString("") { "%02x".format(it) }
            File(output, "$name.json").writeText("""{
                "renderer":"jvm-desktop-compose-test-root", "entrypoint":"CantosTheatreApp → TheatreShell",
                "recipe":"${recipe.id}", "recipeVersion":${TheatreRecipe.Version}, "seed":1,
                "source":"${System.getenv("CANTOS_SOURCE_REVISION") ?: "unrecorded"}",
                "widthDp":$width, "heightDp":$height, "widthPx":${bitmap.width}, "heightPx":${bitmap.height},
                "density":$density, "baseFontScale":$fontScale, "appTextScale":${settings.fontScale},
                "theme":"${settings.themeMode}", "locale":"${settings.locale}", "scrollTo":"${scrollTo ?: "top"}",
                "uiDensity":"${settings.density}", "motion":"${if (settings.reducedMotion) "Reduced" else "Full"}", "pngSha256":"$hash", "nativeAcceptance":false
            }""".trimIndent())
        }
    }
}
