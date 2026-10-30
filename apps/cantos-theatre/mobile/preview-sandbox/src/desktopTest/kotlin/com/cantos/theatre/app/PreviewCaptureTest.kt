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
import com.cantos.theatre.presentation.model.ShellCatalogMode
import com.cantos.theatre.presentation.model.ShellLibraryTab
import com.cantos.theatre.presentation.model.ShellEpisodeId
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.theatre.presentation.model.ShellProgress
import com.cantos.theatre.presentation.model.ShellRoute
import com.cantos.theatre.presentation.model.ShellState
import com.cantos.theatre.adapters.preview.PreviewEpisodeIds
import com.cantos.theatre.app.PreviewSeed
import com.cantos.theatre.app.PreviewSession
import com.cantos.theatre.app.PreviewTestRoot
import com.cantos.theatre.adapters.preview.PreviewWorkIds
import com.cantos.theatre.app.previewState
import com.cantos.theatre.app.seed
import com.cantos.theatre.presentation.model.ShellTheme
import com.cantos.theatre.presentation.model.ShellWorkId
import java.io.File
import org.jetbrains.skia.EncodedImageFormat
import org.jetbrains.skia.Image
import org.junit.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

/** The one app over mock external ports. Captured JVM pixels/semantics require separate inspection and establish no native runtime. */
@OptIn(ExperimentalTestApi::class)
class PreviewCaptureTest {
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
            seed = scene(
                stack = listOf(ShellRoute.Discovery, ShellRoute.Player),
                theme = ShellTheme.Dark,
                playing = true,
            ),
        )
    }

    @Test
    fun light_wide_discovery_is_captured_at_840_by_840() {
        capture("jvm-light-discovery-840x840", width = 840, height = 840)
    }

    @Test
    fun narrow_large_text_work_and_scrolled_episodes_are_captured_at_320_by_800() {
        val state = scene(stack = listOf(ShellRoute.Discovery, ShellRoute.Work(PreviewWorkIds.SonHa)))
        capture(
            "jvm-light-work-text2-320x800",
            width = 320,
            height = 800,
            seed = state,
            textScale = 2f,
        )
        capture(
            "jvm-light-work-text2-episodes-320x800",
            width = 320,
            height = 800,
            seed = state,
            textScale = 2f,
            scrollToTag = "episode-${PreviewEpisodeIds.SonHa03.value}",
        )
    }

    @Test
    fun dark_error_is_captured_at_390_by_844() {
        capture(
            "jvm-dark-error-390x844",
            width = 390,
            height = 844,
            seed = scene(theme = ShellTheme.Dark, catalogMode = ShellCatalogMode.Error),
        )
    }

    @Test
    fun light_saved_library_is_captured_at_390_by_844() {
        capture(
            "jvm-light-library-390x844",
            width = 390,
            height = 844,
            seed = scene(
                stack = listOf(ShellRoute.Library),
                saved = setOf(PreviewWorkIds.SonHa, PreviewWorkIds.DenKhua),
                libraryTab = ShellLibraryTab.Saved,
            ),
        )
    }

    @Test
    fun light_contextual_settings_is_captured_at_390_by_844() {
        capture(
            "jvm-light-settings-390x844",
            width = 390,
            height = 844,
            seed = scene(stack = listOf(ShellRoute.Search, ShellRoute.Settings), query = "Sơn Hà"),
        )
    }

    @Test
    fun library_groups_and_history_are_captured_with_played_fixtures() {
        val state = libraryState()
        capture("jvm-light-library-mvp-390x844", width = 390, height = 844, seed = state)
        capture(
            "jvm-light-library-history-390x844", width = 390, height = 844,
            seed = state, scrollToTag = "library-history",
        )
        capture(
            "jvm-light-history-390x844", width = 390, height = 844,
            seed = libraryState(stack = listOf(ShellRoute.Library, ShellRoute.History)),
        )
        capture(
            "jvm-light-history-confirm-390x844", width = 390, height = 844,
            seed = libraryState(stack = listOf(ShellRoute.Library, ShellRoute.History)),
            clickTag = "history-clear", captureTag = "history-dialog",
        )
    }

    @Test
    fun dark_narrow_library_groups_are_captured_at_double_text_scale_in_english() {
        val state = libraryState(theme = ShellTheme.Dark, locale = ShellLocale.English)
        capture(
            "jvm-dark-library-text2-320x800", width = 320, height = 800,
            seed = state, textScale = 2f,
        )
        capture(
            "jvm-dark-library-saved-text2-320x800", width = 320, height = 800,
            seed = state, textScale = 2f, scrollToTag = "library-saved",
        )
        capture(
            "jvm-dark-library-history-text2-320x800", width = 320, height = 800,
            seed = state, textScale = 2f, scrollToTag = "library-history",
        )
    }

    @Test
    fun empty_library_and_account_are_captured() {
        capture(
            "jvm-light-library-empty-390x844", width = 390, height = 844,
            seed = scene(stack = listOf(ShellRoute.Library), withAccount = true),
        )
        capture(
            "jvm-light-account-390x844", width = 390, height = 844,
            seed = scene(stack = listOf(ShellRoute.Library, ShellRoute.Account)),
        )
    }

    @Test
    fun account_space_is_captured_in_dark_english_and_double_text_scale() {
        val stack = listOf(ShellRoute.Library, ShellRoute.Account)
        capture(
            "jvm-dark-account-390x844", width = 390, height = 844,
            seed = scene(stack = stack, theme = ShellTheme.Dark),
        )
        capture(
            "jvm-light-account-en-390x844", width = 390, height = 844,
            seed = scene(stack = stack, locale = ShellLocale.English),
        )
        capture(
            "jvm-light-account-text2-320x800", width = 320, height = 800,
            seed = scene(stack = stack), textScale = 2f,
        )
        capture(
            "jvm-light-account-text2-settings-320x800", width = 320, height = 800,
            seed = scene(stack = stack), textScale = 2f, scrollToTag = "settings",
        )
    }

    @Test
    fun repair_scenarios_cover_both_themes_short_screens_and_large_text_player() {
        ShellTheme.entries.forEach { theme ->
            val label = theme.name.lowercase()
            capture("jvm-$label-player-390x844", 390, 844,
                scene(stack = listOf(ShellRoute.Discovery, ShellRoute.Player), theme = theme))
            capture("jvm-$label-work-390x844", 390, 844,
                scene(stack = listOf(ShellRoute.Discovery, ShellRoute.Work(PreviewWorkIds.SonHa)), theme = theme))
            capture("jvm-$label-settings-390x844", 390, 844,
                scene(stack = listOf(ShellRoute.Discovery, ShellRoute.Settings), theme = theme))
        }
        capture("jvm-light-shell-short-320x568", 320, 568)
        capture("jvm-dark-player-short-320x568", 320, 568,
            scene(stack = listOf(ShellRoute.Discovery, ShellRoute.Player), theme = ShellTheme.Dark),
            scrollToTag = "transport")
        capture("jvm-light-player-text2-320x800", 320, 800,
            scene(stack = listOf(ShellRoute.Discovery, ShellRoute.Player)), textScale = 2f,
            scrollToTag = "transport")
    }

    @Test
    fun preference_dialogs_are_captured_in_context_and_at_double_text_scale() {
        capture("jvm-light-me-theme-dialog-390x844", 390, 844,
            scene(stack = listOf(ShellRoute.Discovery, ShellRoute.Account)),
            clickTag = "setting-Theme", captureTag = "preference-dialog")
        capture("jvm-dark-timer-dialog-text2-320x800", 320, 800,
            scene(stack = listOf(ShellRoute.Discovery, ShellRoute.Settings), theme = ShellTheme.Dark),
            textScale = 2f, clickTag = "setting-Timer", captureTag = "preference-dialog")
    }

    @Test
    fun shape_review_captures_mini_player_across_sizes_and_themes() {
        ShellTheme.entries.forEach { theme ->
            val label = theme.name.lowercase()
            capture("jvm-$label-mini-320x568", 320, 568, scene(theme = theme))
            capture("jvm-$label-mini-390x844", 390, 844, scene(theme = theme))
            capture("jvm-$label-mini-840x840", 840, 840, scene(theme = theme))
            capture("jvm-$label-mini-text2-320x800", 320, 800,
                scene(theme = theme), textScale = 2f)
        }
    }

    @Test
    fun fresh_guest_and_restored_signed_in_sessions_are_captured_at_390_by_844() {
        // Fresh is a guest; Restored is the first sample account with synced remote data. Both are seeds, not disk reopen.
        listOf(PreviewSession.Fresh, PreviewSession.Restored).forEach { session ->
            val label = session.name.lowercase()
            capture("jvm-light-$label-session-390x844", 390, 844, session.seed())
            capture("jvm-light-$label-session-history-390x844", 390, 844, session.seed(), scrollToTag = "library-history")
        }
    }

    private fun libraryState(
        stack: List<ShellRoute> = listOf(ShellRoute.Library),
        theme: ShellTheme = ShellTheme.Light,
        locale: ShellLocale = ShellLocale.Vietnamese,
    ): PreviewSeed {
        val points = listOf(
            ShellProgress(PreviewEpisodeIds.DenKhua01, 91),
            ShellProgress(PreviewEpisodeIds.SonHa03, 768),
        )
        return scene(
            stack = stack,
            theme = theme,
            locale = locale,
            saved = setOf(PreviewWorkIds.SonHa, PreviewWorkIds.DenKhua),
            progress = points,
            history = points.reversed(),
        )
    }

    /**
     * A guest listening to the Sơn Hà Ký chapter 3 at 12:48, paused, unless a signed-in remote
     * Library (saved works, continue entries, history) is described: then the first sample
     * account owns them.
     */
    private fun scene(
        stack: List<ShellRoute> = listOf(ShellRoute.Discovery),
        theme: ShellTheme = ShellTheme.Light,
        locale: ShellLocale = ShellLocale.Vietnamese,
        playing: Boolean = false,
        saved: Set<ShellWorkId> = emptySet(),
        progress: List<ShellProgress> = emptyList(),
        history: List<ShellProgress> = emptyList(),
        libraryTab: ShellLibraryTab = ShellLibraryTab.Continuing,
        query: String = "",
        catalogMode: ShellCatalogMode = ShellCatalogMode.Ready,
        withAccount: Boolean = saved.isNotEmpty() || progress.isNotEmpty() || history.isNotEmpty(),
    ): PreviewSeed = PreviewSeed(
        previewState(
            stack = stack, theme = theme, locale = locale, savedWorks = saved, progress = progress, history = history,
            libraryTab = libraryTab, query = query, catalogMode = catalogMode, transportRequested = playing,
            selectedEpisode = if (withAccount) com.cantos.theatre.app.PreviewNoSelection else PreviewEpisodeIds.SonHa03,
            positionMs = if (withAccount) 0L else 768_000L,
        ),
        signedIn = withAccount,
    )

    private fun capture(
        name: String,
        width: Int,
        height: Int,
        seed: PreviewSeed = scene(),
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
                    PreviewTestRoot(seed, textScale = textScale)
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
                  "entrypoint": "PreviewTheatreApp → CantosTheatreApp → TheatreShell",
                  "signedIn": ${seed.signedIn},
                  "requestedWidthDp": $width,
                  "requestedHeightDp": $height,
                  "capturedWidthPx": ${bitmap.width},
                  "capturedHeightPx": ${bitmap.height},
                  "rendererDensity": $rendererDensity,
                  "rendererBaseFontScale": $rendererFontScale,
                  "requestedAppTextScale": $textScale,
                  "theme": "${seed.state.theme}",
                  "locale": "${seed.state.locale}",
                  "selectedEpisode": "${seed.state.selectedEpisode.value}",
                  "positionSeconds": ${seed.state.positionSeconds},
                  "catalogMode": "${seed.state.catalogMode}",
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
