package com.cantos.theatre.app

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.asSkiaBitmap
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertHasClickAction
import androidx.compose.ui.test.assertContentDescriptionEquals
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertHeightIsAtLeast
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.assertWidthIsAtLeast
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.click
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.printToString
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import com.cantos.theatre.features.shell.presentation.CantosPresentation
import com.cantos.theatre.presentation.model.ShellCatalogMode
import com.cantos.theatre.presentation.model.ShellCatalogView
import com.cantos.theatre.presentation.model.ShellEpisodeId
import com.cantos.theatre.presentation.model.ShellEvent
import com.cantos.theatre.presentation.model.ShellListPhase
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.theatre.presentation.model.ShellProgress
import com.cantos.theatre.presentation.model.ShellRoute
import com.cantos.theatre.presentation.model.ShellState
import com.cantos.theatre.presentation.model.ShellTheme
import com.cantos.theatre.presentation.model.ShellWork
import com.cantos.theatre.presentation.model.ShellWorkId
import java.io.File
import java.security.MessageDigest
import org.jetbrains.skia.EncodedImageFormat
import org.jetbrains.skia.Image
import org.junit.Test
import org.junit.Assume.assumeTrue
import kotlin.test.assertEquals

/** Controlled real presentation; synthetic catalog facts are confined to the JVM sandbox. */
@OptIn(ExperimentalTestApi::class)
class DiscoveryParityTest {
    private val workId = ShellWorkId("parity-work")
    private val episode = ShellEpisodeId("parity-episode", workId)
    private val state = ShellState(selectedEpisode = episode)
    private val work = ShellWork(workId, "Ngày mai, mình có diễn tiếp không? — Ánh đèn cuối sân khấu",
        "Truyện", "Người dẫn chuyện và một lời hẹn còn ở lại. A story for another evening.", emptyList())

    @Test
    fun discovery_only_claims_continuation_when_progress_exists() {
        for (hasProgress in listOf(false, true)) runDesktopComposeUiTest(width = 390, height = 844) {
            val progress = if (hasProgress) listOf(ShellProgress(episode, 20)) else emptyList()
            setContent { CantosPresentation(state.copy(progress = progress), ShellCatalogView(), {}) }
            if (hasProgress) onNodeWithText("CHUYỆN CÒN DỞ").assertExists()
            else onNodeWithText("CHUYỆN CÒN DỞ").assertDoesNotExist()
        }
    }

    @Test
    fun discovery_search_is_a_full_width_pill_with_one_navigation_intent() = runDesktopComposeUiTest(width = 390, height = 844) {
        val events = mutableListOf<ShellEvent>()
        setContent { CantosPresentation(state, ShellCatalogView(), events::add) }
        onAllNodes(hasSetTextAction()).assertCountEquals(0)
        onNodeWithTag("search").assertContentDescriptionEquals("Tìm trong danh mục")
            .assertHasClickAction().assertHeightIsAtLeast(72.dp).assertWidthIsAtLeast(300.dp)
            .performTouchInput { click(centerLeft) }
        assertEquals(listOf<ShellEvent>(ShellEvent.Navigate(ShellRoute.Search)), events.toList())
    }

    @Test
    fun search_back_collapses_before_navigation_and_preserves_query_and_focus() = runDesktopComposeUiTest(width = 390, height = 844) {
        val events = mutableListOf<ShellEvent>()
        var back: () -> Unit = {}
        setContent {
            CantosPresentation(state.copy(stack = listOf(ShellRoute.Discovery, ShellRoute.Search), query = "  Đèn  "),
                ShellCatalogView(), events::add, backHandler = { _, action -> back = action })
        }
        onNodeWithTag("search-dock").assertExists()
        onNodeWithTag("search-input").assertIsFocused()
        runOnIdle { back() }
        onNodeWithTag("search-dock").assertDoesNotExist()
        onNodeWithTag("search-input").assertIsFocused()
        assertEquals(emptyList<ShellEvent>(), events.toList())
        runOnIdle { back() }
        assertEquals(listOf<ShellEvent>(ShellEvent.Back), events.toList())
    }

    @Test
    fun search_loading_and_failure_do_not_announce_a_completed_zero_count() {
        for (mode in listOf(ShellCatalogMode.Loading, ShellCatalogMode.Error)) runDesktopComposeUiTest(width = 390, height = 844) {
            setContent { CantosPresentation(state.copy(stack = listOf(ShellRoute.Search), catalogMode = mode), ShellCatalogView(), {}) }
            onNodeWithText("0 tác phẩm").assertDoesNotExist()
            onNodeWithTag("catalog-empty-artwork").assertDoesNotExist()
        }
    }

    @Test
    fun completed_empty_has_no_retry_but_failed_read_has_a_real_retry() {
        for (mode in listOf(ShellCatalogMode.Empty, ShellCatalogMode.Error, ShellCatalogMode.Loading)) {
            runDesktopComposeUiTest(width = 390, height = 844) {
                val events = mutableListOf<ShellEvent>()
                setContent { CantosPresentation(state.copy(catalogMode = mode), ShellCatalogView(), events::add) }
                onNodeWithTag("catalog-empty-retry").assertDoesNotExist()
                if (mode == ShellCatalogMode.Empty) {
                    onNodeWithTag("retry").assertDoesNotExist()
                    onNodeWithTag("catalog-empty-artwork").assertExists()
                } else {
                    onNodeWithTag("catalog-empty-artwork").assertDoesNotExist()
                    if (mode == ShellCatalogMode.Error) {
                        onNodeWithTag("retry").performScrollTo().performClick()
                        assertEquals(listOf<ShellEvent>(ShellEvent.Retry), events.toList())
                    } else onNodeWithTag("retry").assertDoesNotExist()
                }
            }
        }
    }

    @Test
    fun retained_and_partial_rows_keep_their_footer_and_never_become_empty() {
        for (phase in listOf(ShellListPhase.More, ShellListPhase.Loading, ShellListPhase.Failed)) {
            runDesktopComposeUiTest(width = 390, height = 844) {
                setContent { CantosPresentation(state, ShellCatalogView(listOf(work), worksPhase = phase), {}) }
                onNodeWithTag("work-$workId").assertExists()
                onNodeWithTag("catalog-empty-artwork").assertDoesNotExist()
                val footer = when (phase) {
                    ShellListPhase.More -> "list-load-more"
                    ShellListPhase.Loading -> "list-loading"
                    ShellListPhase.Failed -> "list-failed"
                    ShellListPhase.Complete -> error("not a retained/partial fixture")
                }
                onNodeWithTag(footer).assertExists()
            }
        }
    }

    @Test
    fun controlled_state_reference_captures() {
        val destination = System.getenv("CANTOS_DISCOVERY_PARITY_CAPTURE_DIR")
        assumeTrue("Opt-in capture output directory is required", destination != null)
        val cases = listOf(
            Triple("empty", state.copy(catalogMode = ShellCatalogMode.Empty), ShellCatalogView()),
            Triple("no-results", state.copy(stack = listOf(ShellRoute.Search), query = "no matching story"), ShellCatalogView()),
            Triple("loaded", state, ShellCatalogView(listOf(work))),
            Triple("partial", state, ShellCatalogView(listOf(work), worksPhase = ShellListPhase.More)),
            Triple("loading", state.copy(catalogMode = ShellCatalogMode.Loading), ShellCatalogView()),
            Triple("failed", state.copy(catalogMode = ShellCatalogMode.Error), ShellCatalogView()),
            Triple("retained", state, ShellCatalogView(listOf(work), worksPhase = ShellListPhase.Failed)),
        )
        for (width in listOf(320, 390)) for (theme in ShellTheme.entries) for (locale in ShellLocale.entries) {
            for (scale in listOf(1f, 2f)) for ((name, scenario, catalog) in cases) {
                runDesktopComposeUiTest(width = width, height = 844) {
                    val shown = scenario.copy(theme = theme, locale = locale)
                    setContent {
                        Box(Modifier.requiredSize(width.dp, 844.dp).testTag("parity-root")) {
                            CantosPresentation(shown, catalog, {}, textScale = scale)
                        }
                    }
                    waitForIdle()
                    val root = onNodeWithTag("parity-root")
                    val bitmap = root.captureToImage()
                    val bytes = requireNotNull(Image.makeFromBitmap(bitmap.asSkiaBitmap()).encodeToData(EncodedImageFormat.PNG)).bytes
                    val hash = MessageDigest.getInstance("SHA-256").digest(bytes).joinToString("") { "%02x".format(it) }
                    val out = File(requireNotNull(destination)).apply { mkdirs() }
                    val id = "$name-$width-$theme-$locale-text$scale"
                    File(out, "$id.png").writeBytes(bytes)
                    File(out, "$id.semantics.txt").writeText(root.printToString(maxDepth = 100))
                    File(out, "$id.json").writeText("""{"renderer":"jvm-desktop-compose-test-root","source":"${System.getenv("CANTOS_SOURCE_REVISION") ?: "unrecorded"}","fixture":"$name","widthDp":$width,"heightDp":844,"theme":"$theme","locale":"$locale","textScale":$scale,"motion":"Reduced","pngSha256":"$hash","nativeAcceptance":false}""")
                }
            }
        }
    }
}
