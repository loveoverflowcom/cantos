package com.cantos.theatre.app

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.asSkiaBitmap
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.printToString
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import com.cantos.theatre.domain.library.LibraryLoad
import com.cantos.theatre.domain.library.LibrarySection
import com.cantos.theatre.presentation.model.ShellCatalogMode
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.theatre.presentation.model.ShellRoute
import com.cantos.theatre.presentation.model.ShellTheme
import com.cantos.theatre.adapters.preview.PreviewWorkIds
import java.io.File
import java.security.MessageDigest
import org.jetbrains.skia.EncodedImageFormat
import org.jetbrains.skia.Image
import org.junit.Test
import kotlin.test.assertEquals

/** Same production composition/ports as the workbench; fixtures never enter a native host. */
@OptIn(ExperimentalTestApi::class)
class DiscoveryLibraryCaptureTest {
    @Test
    fun discovery_search_and_history_render_named_reference_scenarios() {
        capture("discovery-390-light-vi", PreviewSession.Guest.seed(), 390, 844)
        capture("discovery-1100-light-en", PreviewSession.Guest.seed().withAppearance(ShellTheme.Light, ShellLocale.English), 1100, 900)
        capture("discovery-320-dark-en-text2", PreviewSession.Guest.seed().withAppearance(ShellTheme.Dark, ShellLocale.English), 320, 800, 2f)
        capture("discovery-320-dark-en-text2-card", PreviewSession.Guest.seed().withAppearance(ShellTheme.Dark, ShellLocale.English), 320, 800, 2f,
            scrollTo = "save-card-${PreviewWorkIds.SonHa.value}")
        capture("search-390-dark-vi", PreviewSeed(previewState(stack = listOf(ShellRoute.Discovery, ShellRoute.Search), query = "Đèn", theme = ShellTheme.Dark)), 390, 844)
        capture("search-no-results-320-dark-en-text2", PreviewSeed(previewState(stack = listOf(ShellRoute.Discovery, ShellRoute.Search), query = "no matching story", theme = ShellTheme.Dark, locale = ShellLocale.English)), 320, 800, 2f)
        capture("catalog-empty-390-light-vi", PreviewSeed(previewState(catalogMode = ShellCatalogMode.Empty)), 390, 844)
        capture("history-empty-390-dark-vi", PreviewSeed(previewState(stack = listOf(ShellRoute.Library, ShellRoute.History), theme = ShellTheme.Dark), signedIn = true), 390, 844)
    }

    private fun PreviewSeed.withAppearance(theme: ShellTheme, locale: ShellLocale) = copy(state = state.copy(theme = theme, locale = locale))

    private fun capture(name: String, seed: PreviewSeed, width: Int, height: Int, scale: Float = 1f, scrollTo: String? = null) {
        lateinit var owners: PreviewOwners
        runDesktopComposeUiTest(width = width, height = height) {
            setContent {
                Box(Modifier.requiredSize(width.dp, height.dp).testTag("issue453-root")) {
                    PreviewTestRoot(seed, textScale = scale, onOwners = { owners = it })
                }
            }
            waitUntil(timeoutMillis = 5_000) {
                val state = owners.projection()
                state.catalogMode != ShellCatalogMode.Loading &&
                    (seed.state.route != ShellRoute.Search || owners.catalog.state.value.query.text == seed.state.query.trim()) &&
                    (seed.state.route != ShellRoute.History || LibrarySection.entries.all {
                        owners.shell.library.value.section(it).loaded && owners.shell.library.value.section(it).loading == LibraryLoad.Idle
                    })
            }
            waitForIdle()
            if (scrollTo != null) onNodeWithTag(scrollTo).performScrollTo()
            waitForIdle()
            val root = onNodeWithTag("issue453-root")
            val bitmap = root.captureToImage()
            assertEquals(width, bitmap.width)
            assertEquals(height, bitmap.height)
            val bytes = requireNotNull(Image.makeFromBitmap(bitmap.asSkiaBitmap()).encodeToData(EncodedImageFormat.PNG)).bytes
            val output = File(System.getenv("CANTOS_THEATRE_DEMO_CAPTURE_DIR") ?: "build/reports/theatre-demo")
            check(output.isDirectory || output.mkdirs())
            File(output, "issue453-$name.png").writeBytes(bytes)
            File(output, "issue453-$name.semantics.txt").writeText(root.printToString(maxDepth = 100))
            val hash = MessageDigest.getInstance("SHA-256").digest(bytes).joinToString("") { "%02x".format(it) }
            File(output, "issue453-$name.json").writeText("""{
                "renderer":"jvm-desktop-compose-test-root", "entrypoint":"PreviewTestRoot → CantosTheatreApp",
                "fixture":"$name", "ports":"mock external ports; production shell/controllers; in-memory Library",
                "source":"${System.getenv("CANTOS_SOURCE_REVISION") ?: "unrecorded"}",
                "widthDp":$width, "heightDp":$height, "widthPx":${bitmap.width}, "heightPx":${bitmap.height},
                "theme":"${seed.state.theme}", "locale":"${seed.state.locale}", "textScale":$scale,
                "scrollTo":"${scrollTo ?: "top"}",
                "motion":"Reduced", "pngSha256":"$hash", "nativeAcceptance":false
            }""".trimIndent())
        }
    }
}
