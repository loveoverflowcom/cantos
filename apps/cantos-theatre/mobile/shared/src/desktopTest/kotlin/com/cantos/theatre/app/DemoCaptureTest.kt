package com.cantos.theatre.app

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.asSkiaBitmap
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.printToString
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import com.cantos.theatre.demo.DemoCatalogMode
import com.cantos.theatre.demo.DemoLibraryTab
import com.cantos.theatre.demo.DemoRoute
import com.cantos.theatre.demo.DemoState
import com.cantos.theatre.demo.DemoTheme
import com.cantos.theatre.demo.DemoWorkId
import java.io.File
import org.jetbrains.skia.EncodedImageFormat
import org.jetbrains.skia.Image
import org.junit.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

/** Captured JVM pixels/semantics require separate inspection; they establish no native runtime. */
@OptIn(ExperimentalTestApi::class)
class DemoCaptureTest {
    @Test
    fun light_discovery_is_captured_at_390_by_844() {
        capture("jvm-light-discovery-390x844", width = 390, height = 844)
    }

    @Test
    fun dark_player_is_captured_at_390_by_844() {
        capture(
            "jvm-dark-player-390x844",
            width = 390,
            height = 844,
            state = DemoState(
                stack = listOf(DemoRoute.Discovery, DemoRoute.Player),
                theme = DemoTheme.Dark,
                simulating = true,
            ),
        )
    }

    @Test
    fun light_wide_discovery_is_captured_at_840_by_840() {
        capture("jvm-light-discovery-840x840", width = 840, height = 840)
    }

    @Test
    fun narrow_large_text_work_and_scrolled_episodes_are_captured_at_320_by_800() {
        val state = DemoState(stack = listOf(DemoRoute.Discovery, DemoRoute.Work(DemoWorkId.SonHa)))
        capture(
            "jvm-light-work-text2-320x800",
            width = 320,
            height = 800,
            state = state,
            textScale = 2f,
        )
        capture(
            "jvm-light-work-text2-episodes-320x800",
            width = 320,
            height = 800,
            state = state,
            textScale = 2f,
            scrollToTag = "episode-SonHa03",
        )
    }

    @Test
    fun dark_error_is_captured_at_390_by_844() {
        capture(
            "jvm-dark-error-390x844",
            width = 390,
            height = 844,
            state = DemoState(theme = DemoTheme.Dark, catalogMode = DemoCatalogMode.Error),
        )
    }

    @Test
    fun light_saved_library_is_captured_at_390_by_844() {
        capture(
            "jvm-light-library-390x844",
            width = 390,
            height = 844,
            state = DemoState(
                stack = listOf(DemoRoute.Library),
                savedWorks = setOf(DemoWorkId.SonHa, DemoWorkId.DenKhua),
                libraryTab = DemoLibraryTab.Saved,
            ),
        )
    }

    @Test
    fun light_contextual_settings_is_captured_at_390_by_844() {
        capture(
            "jvm-light-settings-390x844",
            width = 390,
            height = 844,
            state = DemoState(stack = listOf(DemoRoute.Search, DemoRoute.Settings), query = "Sơn Hà"),
        )
    }

    private fun capture(
        name: String,
        width: Int,
        height: Int,
        state: DemoState = DemoState(),
        textScale: Float = 1f,
        scrollToTag: String? = null,
    ) {
        var rendererDensity = Float.NaN
        var rendererFontScale = Float.NaN
        runDesktopComposeUiTest(width = width, height = height) {
            setContent {
                rendererDensity = LocalDensity.current.density
                rendererFontScale = LocalDensity.current.fontScale
                Box(Modifier.requiredSize(width.dp, height.dp).testTag("capture-root")) {
                    CantosDemoApp(initialState = state, textScale = textScale)
                }
            }
            if (scrollToTag != null) onNodeWithTag(scrollToTag).performScrollTo()
            waitForIdle()
            assertTrue(rendererDensity.isFinite() && rendererDensity > 0f)
            val bitmap = onNodeWithTag("capture-root").captureToImage()
            assertEquals(width, bitmap.width, "$name must retain the complete explicitly sized renderer width")
            assertEquals(height, bitmap.height, "$name must retain the complete explicitly sized renderer height")
            val output = File(System.getenv("CANTOS_THEATRE_DEMO_CAPTURE_DIR") ?: "build/reports/theatre-demo")
            check(output.isDirectory || output.mkdirs()) { "Could not create demo capture output directory" }
            val bytes = requireNotNull(
                Image.makeFromBitmap(bitmap.asSkiaBitmap()).encodeToData(EncodedImageFormat.PNG),
            ).bytes
            File(output, "$name.png").writeBytes(bytes)
            File(output, "$name.semantics.txt").writeText(onNodeWithTag("capture-root", useUnmergedTree = true).printToString())
            File(output, "$name.json").writeText(
                """
                {
                  "renderer": "JVM Desktop Compose UI test",
                  "entrypoint": "CantosDemoApp",
                  "requestedWidthDp": $width,
                  "requestedHeightDp": $height,
                  "capturedWidthPx": ${bitmap.width},
                  "capturedHeightPx": ${bitmap.height},
                  "rendererDensity": $rendererDensity,
                  "rendererBaseFontScale": $rendererFontScale,
                  "requestedAppTextScale": $textScale,
                  "theme": "${state.theme}",
                  "locale": "${state.locale}",
                  "selectedEpisode": "${state.selectedEpisode}",
                  "positionSeconds": ${state.positionSeconds},
                  "catalogMode": "${state.catalogMode}",
                  "scrolledToTag": ${scrollToTag?.let { "\"$it\"" } ?: "null"},
                  "evidence": "screenshot-captured; inspection pending"
                }
                """.trimIndent(),
            )
        }
    }
}
