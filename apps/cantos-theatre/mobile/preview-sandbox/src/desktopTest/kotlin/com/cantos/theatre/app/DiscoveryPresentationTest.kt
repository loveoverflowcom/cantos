package com.cantos.theatre.app

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertHasClickAction
import androidx.compose.ui.test.assertHeightIsAtLeast
import androidx.compose.ui.test.assertTextEquals
import androidx.compose.ui.test.assertWidthIsAtLeast
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performTextReplacement
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.unit.dp
import com.cantos.theatre.features.shell.presentation.CantosPresentation
import com.cantos.theatre.presentation.model.ShellCatalogMode
import com.cantos.theatre.presentation.model.ShellCatalogView
import com.cantos.theatre.presentation.model.ShellEpisodeId
import com.cantos.theatre.presentation.model.ShellEvent
import com.cantos.theatre.presentation.model.ShellListPhase
import com.cantos.theatre.presentation.model.ShellRoute
import com.cantos.theatre.presentation.model.ShellSearchKind
import com.cantos.theatre.presentation.model.ShellState
import com.cantos.theatre.presentation.model.ShellWork
import com.cantos.theatre.presentation.model.ShellWorkId
import org.junit.Test
import kotlin.test.assertEquals

/** Controlled facts into the production compositor; these tests exercise no catalog adapter. */
@OptIn(ExperimentalTestApi::class)
class DiscoveryPresentationTest {
    private val noSelection = ShellEpisodeId("none", ShellWorkId("none"))

    @Test
    fun discovery_brand_uses_one_decorative_official_mark_and_account_keeps_its_existing_header() {
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent { CantosPresentation(ShellState(selectedEpisode = noSelection), ShellCatalogView(), {}) }
            onNodeWithTag("header-logo", useUnmergedTree = true)
                .assertHeightIsAtLeast(44.dp).assertWidthIsAtLeast(44.dp)
                .assert(SemanticsMatcher.keyNotDefined(SemanticsProperties.ContentDescription))
            onNodeWithTag("theme-toggle").assertDoesNotExist()
            assertEquals(onNodeWithTag("account").fetchSemanticsNode().boundsInRoot.center.y,
                onNodeWithTag("header-logo", useUnmergedTree = true).fetchSemanticsNode().boundsInRoot.center.y)
            onNodeWithTag("header-title").assertExists()
        }
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent { CantosPresentation(ShellState(stack = listOf(ShellRoute.Account), selectedEpisode = noSelection), ShellCatalogView(), {}) }
            onNodeWithTag("header-logo", useUnmergedTree = true).assertDoesNotExist()
            onNodeWithTag("header-title").assertTextEquals("Tài khoản")
        }
    }

    @Test
    fun floating_search_keeps_query_on_change_and_clear_emits_no_navigation_or_submit() = runDesktopComposeUiTest(width = 390, height = 844) {
        var state by mutableStateOf(ShellState(stack = listOf(ShellRoute.Discovery, ShellRoute.Search), selectedEpisode = noSelection, query = "Đèn"))
        val events = mutableListOf<ShellEvent>()
        setContent {
            CantosPresentation(state, ShellCatalogView(), { event ->
                events += event
                if (event is ShellEvent.Search) state = state.copy(query = event.query)
            })
        }
        onNodeWithTag("search-input").performTextReplacement("Sơn Hà")
        assertEquals(listOf<ShellEvent>(ShellEvent.Search("Sơn Hà")), events.toList())
        onNodeWithTag("search-clear").performClick()
        assertEquals(listOf<ShellEvent>(ShellEvent.Search("Sơn Hà"), ShellEvent.Search("")), events.toList())
        assertEquals(ShellRoute.Search, state.route)
    }

    @Test
    fun the_docked_search_holds_one_magnifier_that_submits_without_changing_text_or_route() = runDesktopComposeUiTest(width = 390, height = 844) {
        val events = mutableListOf<ShellEvent>()
        val state = ShellState(stack = listOf(ShellRoute.Discovery, ShellRoute.Search), selectedEpisode = noSelection, query = "Đèn")
        setContent { CantosPresentation(state, ShellCatalogView(), events::add) }
        onNodeWithTag("search-submit").assertHasClickAction()
        onNodeWithContentDescription("Tìm kiếm", useUnmergedTree = true).assertExists()
        onNodeWithTag("search-submit").performClick()
        assertEquals(listOf<ShellEvent>(ShellEvent.SubmitSearch), events.toList())
        onNodeWithTag("search-clear").assertExists()
    }

    @Test
    fun a_work_keeps_its_semantic_identity_when_reordered_across_grid_rows() = runDesktopComposeUiTest(width = 1100, height = 1000) {
        val a = work("a", "Ánh đèn cuối sân khấu")
        val b = work("b", "Ngày mai, mình có diễn tiếp không?")
        val c = work("c", "Người dẫn chuyện")
        var listed by mutableStateOf(listOf(a, b, c))
        setContent {
            CantosPresentation(ShellState(stack = listOf(ShellRoute.Search), selectedEpisode = noSelection),
                ShellCatalogView(works = listed), {})
        }
        val before = onNodeWithTag("work-b").fetchSemanticsNode().id
        runOnIdle { listed = listOf(c, a, b) }
        assertEquals(before, onNodeWithTag("work-b").fetchSemanticsNode().id)
    }

    @Test
    fun an_empty_intermediate_page_keeps_pagination_and_does_not_claim_a_completed_empty_catalog() = runDesktopComposeUiTest(width = 390, height = 844) {
        val events = mutableListOf<ShellEvent>()
        setContent {
            CantosPresentation(ShellState(selectedEpisode = noSelection),
                ShellCatalogView(worksPhase = ShellListPhase.More), events::add)
        }
        onNodeWithTag("catalog-empty-artwork").assertDoesNotExist()
        onNodeWithTag("catalog-empty-retry").assertDoesNotExist()
        onNodeWithTag("list-load-more").performScrollTo().performClick()
        assertEquals(listOf<ShellEvent>(ShellEvent.LoadMore), events.toList())
    }

    @Test
    fun completed_catalog_empty_has_no_recovery_and_filtered_no_results_clears_without_navigation() {
        runDesktopComposeUiTest(width = 390, height = 844) {
            val events = mutableListOf<ShellEvent>()
            setContent { CantosPresentation(ShellState(selectedEpisode = noSelection, catalogMode = ShellCatalogMode.Empty), ShellCatalogView(), events::add) }
            onNodeWithText("Chưa có câu chuyện khả dụng").assertExists()
            onNodeWithText("Chưa tìm thấy câu chuyện").assertDoesNotExist()
            onNodeWithTag("catalog-empty-retry").assertDoesNotExist()
            onNodeWithTag("retry").assertDoesNotExist()
            assertEquals(emptyList<ShellEvent>(), events.toList())
        }
        runDesktopComposeUiTest(width = 390, height = 844) {
            val events = mutableListOf<ShellEvent>()
            setContent {
                CantosPresentation(ShellState(stack = listOf(ShellRoute.Discovery, ShellRoute.Search), selectedEpisode = noSelection,
                    catalogMode = ShellCatalogMode.Empty, query = "không có trong danh mục"), ShellCatalogView(), events::add)
            }
            onNodeWithText("Chưa tìm thấy câu chuyện").assertExists()
            onNodeWithText("Chưa có câu chuyện khả dụng").assertDoesNotExist()
            onNodeWithTag("catalog-clear-search").performScrollTo().performClick()
            assertEquals(listOf<ShellEvent>(ShellEvent.Search(""), ShellEvent.SelectSearchKind(ShellSearchKind.All)), events.toList())
        }
    }

    private fun work(id: String, title: String) = ShellWork(ShellWorkId(id), title, "Truyện", "Một lời hẹn còn ở lại.", emptyList())
}
