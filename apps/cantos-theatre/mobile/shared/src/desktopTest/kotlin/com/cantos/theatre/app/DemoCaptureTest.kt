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
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.printToString
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import com.cantos.theatre.demo.DemoCatalogMode
import com.cantos.theatre.demo.DemoLibraryTab
import com.cantos.theatre.demo.DemoEpisodeId
import com.cantos.theatre.demo.DemoLocale
import com.cantos.theatre.demo.DemoProgress
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

    @Test
    fun library_groups_and_history_are_captured_with_played_fixtures() {
        val state = libraryState()
        capture("jvm-light-library-mvp-390x844", width = 390, height = 844, state = state)
        capture(
            "jvm-light-library-history-390x844", width = 390, height = 844,
            state = state, scrollToTag = "library-history",
        )
        capture(
            "jvm-light-history-390x844", width = 390, height = 844,
            state = libraryState(stack = listOf(DemoRoute.Library, DemoRoute.History)),
        )
        capture(
            "jvm-light-history-confirm-390x844", width = 390, height = 844,
            state = libraryState(stack = listOf(DemoRoute.Library, DemoRoute.History)),
            clickTag = "history-clear", captureTag = "history-dialog",
        )
    }

    @Test
    fun dark_narrow_library_groups_are_captured_at_double_text_scale_in_english() {
        val state = libraryState(theme = DemoTheme.Dark, locale = DemoLocale.English)
        capture(
            "jvm-dark-library-text2-320x800", width = 320, height = 800,
            state = state, textScale = 2f,
        )
        capture(
            "jvm-dark-library-saved-text2-320x800", width = 320, height = 800,
            state = state, textScale = 2f, scrollToTag = "library-saved",
        )
        capture(
            "jvm-dark-library-history-text2-320x800", width = 320, height = 800,
            state = state, textScale = 2f, scrollToTag = "library-history",
        )
    }

    @Test
    fun empty_library_and_account_are_captured() {
        capture(
            "jvm-light-library-empty-390x844", width = 390, height = 844,
            state = DemoState(stack = listOf(DemoRoute.Library), progress = emptyList(), savedWorks = emptySet()),
        )
        capture(
            "jvm-light-account-390x844", width = 390, height = 844,
            state = DemoState(stack = listOf(DemoRoute.Library, DemoRoute.Account)),
        )
    }

    @Test
    fun repair_scenarios_cover_both_themes_short_screens_and_large_text_player() {
        DemoTheme.entries.forEach { theme ->
            val label = theme.name.lowercase()
            capture("jvm-$label-player-390x844", 390, 844,
                DemoState(stack = listOf(DemoRoute.Discovery, DemoRoute.Player), theme = theme))
            capture("jvm-$label-work-390x844", 390, 844,
                DemoState(stack = listOf(DemoRoute.Discovery, DemoRoute.Work(DemoWorkId.SonHa)), theme = theme))
            capture("jvm-$label-settings-390x844", 390, 844,
                DemoState(stack = listOf(DemoRoute.Discovery, DemoRoute.Settings), theme = theme))
        }
        capture("jvm-light-shell-short-320x568", 320, 568)
        capture("jvm-dark-player-short-320x568", 320, 568,
            DemoState(stack = listOf(DemoRoute.Discovery, DemoRoute.Player), theme = DemoTheme.Dark),
            scrollToTag = "transport")
        capture("jvm-light-player-text2-320x800", 320, 800,
            DemoState(stack = listOf(DemoRoute.Discovery, DemoRoute.Player)), textScale = 2f,
            scrollToTag = "transport")
    }

    @Test
    fun preference_dialogs_are_captured_in_context_and_at_double_text_scale() {
        capture("jvm-light-theme-dialog-390x844", 390, 844,
            DemoState(stack = listOf(DemoRoute.Discovery, DemoRoute.Settings)),
            clickTag = "setting-Theme", captureTag = "preference-dialog")
        capture("jvm-dark-timer-dialog-text2-320x800", 320, 800,
            DemoState(stack = listOf(DemoRoute.Discovery, DemoRoute.Settings), theme = DemoTheme.Dark),
            textScale = 2f, clickTag = "setting-Timer", captureTag = "preference-dialog")
    }

    @Test
    fun shape_review_captures_mini_player_across_sizes_and_themes() {
        DemoTheme.entries.forEach { theme ->
            val label = theme.name.lowercase()
            capture("jvm-$label-mini-320x568", 320, 568, DemoState(theme = theme))
            capture("jvm-$label-mini-390x844", 390, 844, DemoState(theme = theme))
            capture("jvm-$label-mini-840x840", 840, 840, DemoState(theme = theme))
            capture("jvm-$label-mini-text2-320x800", 320, 800,
                DemoState(theme = theme), textScale = 2f)
        }
    }

    private fun libraryState(
        stack: List<DemoRoute> = listOf(DemoRoute.Library),
        theme: DemoTheme = DemoTheme.Light,
        locale: DemoLocale = DemoLocale.Vietnamese,
    ): DemoState {
        val points = listOf(
            DemoProgress(DemoEpisodeId.DenKhua01, 91),
            DemoProgress(DemoEpisodeId.SonHa03, 768),
        )
        return DemoState(
            stack = stack,
            theme = theme,
            locale = locale,
            savedWorks = setOf(DemoWorkId.SonHa, DemoWorkId.DenKhua),
            progress = points,
            history = points.reversed(),
        )
    }

    private fun capture(
        name: String,
        width: Int,
        height: Int,
        state: DemoState = DemoState(),
        textScale: Float = 1f,
        scrollToTag: String? = null,
        clickTag: String? = null,
        captureTag: String = "capture-root",
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
            if (clickTag != null) onNodeWithTag(clickTag).performScrollTo().performClick()
            waitForIdle()
            assertTrue(rendererDensity.isFinite() && rendererDensity > 0f)
            val bitmap = onNodeWithTag(captureTag).captureToImage()
            if (captureTag == "capture-root") {
                assertEquals(width, bitmap.width, "$name must retain the complete explicitly sized renderer width")
                assertEquals(height, bitmap.height, "$name must retain the complete explicitly sized renderer height")
            }
            val output = File(System.getenv("CANTOS_THEATRE_DEMO_CAPTURE_DIR") ?: "build/reports/theatre-demo")
            check(output.isDirectory || output.mkdirs()) { "Could not create demo capture output directory" }
            val bytes = requireNotNull(
                Image.makeFromBitmap(bitmap.asSkiaBitmap()).encodeToData(EncodedImageFormat.PNG),
            ).bytes
            File(output, "$name.png").writeBytes(bytes)
            File(output, "$name.semantics.txt").writeText(onNodeWithTag(captureTag, useUnmergedTree = true).printToString())
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
                  "clickedTag": ${clickTag?.let { "\"$it\"" } ?: "null"},
                  "capturedNode": "$captureTag",
                  "evidence": "screenshot-captured; inspection pending"
                }
                """.trimIndent(),
            )
        }
    }
}
