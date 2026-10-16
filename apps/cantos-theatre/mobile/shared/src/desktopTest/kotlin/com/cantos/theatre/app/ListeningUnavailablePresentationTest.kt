package com.cantos.theatre.app

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.asSkiaBitmap
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertTextEquals
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.printToString
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import com.cantos.theatre.features.launch.presentation.CantosListeningUnavailable
import com.cantos.ui.Theme
import com.cantos.ui.UiPreferences
import java.io.File
import java.util.Locale
import kotlin.test.assertTrue
import org.jetbrains.skia.EncodedImageFormat
import org.jetbrains.skia.Image
import org.junit.Test

/** The production host's explicit state on the JVM renderer; no Android launch is claimed. */
@OptIn(ExperimentalTestApi::class)
class ListeningUnavailablePresentationTest {
    private fun <T> inLocale(language: String, block: () -> T): T {
        val original = Locale.getDefault()
        Locale.setDefault(Locale.forLanguageTag(language))
        return try { block() } finally { Locale.setDefault(original) }
    }

    @Test
    fun explicit_copy_is_localized_without_any_shell_player_or_sample_content() {
        for ((language, title, body) in listOf(
            Triple("vi", "Cantos chưa sẵn sàng", "Phiên bản này chưa phát được truyện hay podcast."),
            Triple("en", "Cantos isn’t ready yet", "This version can’t play stories or podcasts yet."),
        )) inLocale(language) {
            runDesktopComposeUiTest(width = 390, height = 844) {
                setContent { CantosListeningUnavailable(UiPreferences()) }
                onNodeWithTag("listening-unavailable-title").assertTextEquals(title)
                    .assert(SemanticsMatcher.keyIsDefined(SemanticsProperties.Heading))
                onNodeWithTag("listening-unavailable-body").assertTextEquals(body)
                // Not a loading state or an empty shell: nothing implies content or progress.
                for (tag in listOf("local-launch", "header-title", "nav-discovery", "mini-player", "demo-notice", "reset", "retry")) {
                    onNodeWithTag(tag).assertDoesNotExist()
                }
                assertTrue("Diagnostics" !in onNodeWithTag("listening-unavailable").printToString())
            }
        }
    }

    @Test
    fun layout_matrix_keeps_copy_reachable_and_is_captured_on_the_jvm() {
        val output = File(System.getenv("CANTOS_THEATRE_DEMO_CAPTURE_DIR") ?: "build/reports/theatre-demo")
        check(output.isDirectory || output.mkdirs())
        for (language in listOf("vi", "en")) inLocale(language) {
            for (theme in listOf(Theme.Light, Theme.Dark)) {
                for ((width, height, textScale) in listOf(Triple(390, 844, 1f), Triple(320, 568, 1f), Triple(844, 390, 1f), Triple(320, 568, 2f))) {
                    val name = "unavailable-${theme.name.lowercase()}-$language-${width}x$height-text$textScale"
                    runDesktopComposeUiTest(width = width, height = height) {
                        setContent {
                            Box(Modifier.requiredSize(width.dp, height.dp).testTag("capture-root")) {
                                CantosListeningUnavailable(UiPreferences(theme = theme, textScale = textScale))
                            }
                        }
                        val root = onNodeWithTag("capture-root").fetchSemanticsNode().boundsInRoot
                        for (tag in listOf("listening-unavailable-title", "listening-unavailable-body")) {
                            // Large text may scroll vertically, but no copy is clipped sideways.
                            onNodeWithTag(tag).performScrollTo().assertIsDisplayed()
                            val bounds = onNodeWithTag(tag).fetchSemanticsNode().boundsInRoot
                            assertTrue(bounds.left >= root.left && bounds.right <= root.right, "$name clips $tag: $bounds outside $root")
                        }
                        onNodeWithTag("listening-unavailable-title").performScrollTo()
                        val bitmap = onNodeWithTag("capture-root").captureToImage()
                        val data = requireNotNull(Image.makeFromBitmap(bitmap.asSkiaBitmap()).encodeToData(EncodedImageFormat.PNG))
                        File(output, "$name.png").writeBytes(data.bytes)
                        File(output, "$name.semantics.txt").writeText(onNodeWithTag("capture-root").printToString())
                    }
                }
            }
        }
    }
}
