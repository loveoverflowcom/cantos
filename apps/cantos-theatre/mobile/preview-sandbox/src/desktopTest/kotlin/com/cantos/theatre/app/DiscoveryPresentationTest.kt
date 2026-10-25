package com.cantos.theatre.app

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.runDesktopComposeUiTest
import com.cantos.theatre.features.shell.presentation.CantosPresentation
import com.cantos.theatre.presentation.model.ShellCatalogMode
import com.cantos.theatre.presentation.model.ShellCatalogView
import com.cantos.theatre.presentation.model.ShellEpisodeId
import com.cantos.theatre.presentation.model.ShellEvent
import com.cantos.theatre.presentation.model.ShellListPhase
import com.cantos.theatre.presentation.model.ShellRoute
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
    fun completed_catalog_empty_offers_retry_and_filtered_no_results_clears_without_navigation() {
        runDesktopComposeUiTest(width = 390, height = 844) {
            val events = mutableListOf<ShellEvent>()
            setContent { CantosPresentation(ShellState(selectedEpisode = noSelection, catalogMode = ShellCatalogMode.Empty), ShellCatalogView(), events::add) }
            onNodeWithText("Chưa có câu chuyện khả dụng").assertExists()
            onNodeWithText("Chưa tìm thấy câu chuyện").assertDoesNotExist()
            onNodeWithTag("catalog-empty-retry").performScrollTo().performClick()
            assertEquals(listOf<ShellEvent>(ShellEvent.Retry), events.toList())
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
            assertEquals(listOf<ShellEvent>(ShellEvent.Search("")), events.toList())
        }
    }

    private fun work(id: String, title: String) = ShellWork(ShellWorkId(id), title, "Truyện", "Một lời hẹn còn ở lại.", emptyList())
}
