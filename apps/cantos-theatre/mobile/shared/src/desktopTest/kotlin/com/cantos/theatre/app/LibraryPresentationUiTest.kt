package com.cantos.theatre.app

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import com.cantos.theatre.features.shell.presentation.CantosPresentation
import com.cantos.theatre.presentation.model.CopyKey
import com.cantos.theatre.presentation.model.ShellCatalogView
import com.cantos.theatre.presentation.model.ShellCopy
import com.cantos.theatre.presentation.model.ShellEpisode
import com.cantos.theatre.presentation.model.ShellEpisodeId
import com.cantos.theatre.presentation.model.ShellEvent
import com.cantos.theatre.presentation.model.ShellLibraryPhase
import com.cantos.theatre.presentation.model.ShellLibrarySection
import com.cantos.theatre.presentation.model.ShellLibraryView
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.theatre.presentation.model.ShellProgress
import com.cantos.theatre.presentation.model.ShellResume
import com.cantos.theatre.presentation.model.ShellRoute
import com.cantos.theatre.presentation.model.ShellRowTitles
import com.cantos.theatre.presentation.model.ShellWork
import com.cantos.theatre.presentation.model.ShellWorkId
import com.cantos.theatre.presentation.model.sampleState
import org.junit.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

/** The production Library screen over supplied projections: what a listener sees and which event each control sends. */
@OptIn(ExperimentalTestApi::class)
class LibraryPresentationUiTest {
    private val copy = ShellCopy(ShellLocale.Vietnamese)
    private val outsideWork = ShellWorkId("ngoai-danh-muc")
    private val outsideEpisode = ShellEpisodeId("tap-ngoai-danh-muc", outsideWork)
    private val replacedWork = ShellWorkId("ban-thu-cu")
    private val replacedEpisode = ShellEpisodeId("tap-ban-thu-cu", replacedWork)
    private val bareWork = ShellWorkId("chua-co-mo-ta")

    private fun library(
        phases: ShellLibraryView = ShellLibraryView(),
        progress: List<ShellProgress> = emptyList(),
        history: List<ShellProgress> = emptyList(),
        saved: Set<ShellWorkId> = emptySet(),
        works: List<ShellWork> = emptyList(),
    ) = sampleState(stack = listOf(ShellRoute.Discovery, ShellRoute.Library), savedWorks = saved, progress = progress, history = history) to
        ShellCatalogView(referenced = works, library = phases)

    private fun render(state: Pair<com.cantos.theatre.presentation.model.ShellState, ShellCatalogView>, events: MutableList<ShellEvent>, check: androidx.compose.ui.test.ComposeUiTest.() -> Unit) =
        runDesktopComposeUiTest(width = 390, height = 2400) {
            setContent { Box(Modifier.requiredSize(390.dp, 2400.dp)) { CantosPresentation(state.first, state.second, { events += it }) } }
            check()
        }

    @Test
    fun rows_show_the_apis_titles_duration_and_a_reason_when_the_exact_release_cannot_be_resumed() {
        val named = ShellProgress(outsideEpisode, 40, 40_000, ShellResume.Available, ShellRowTitles("Chuyện ngoài danh mục", "Tập chưa từng được tải"), 125)
        val replaced = ShellProgress(replacedEpisode, 30, 30_000, ShellResume.ReleaseReplaced, ShellRowTitles("Chuyện bản thu cũ", "Tập bản thu cũ"), 90)
        val refresh = ShellProgress(ShellEpisodeId("tap-cho-lam-moi", ShellWorkId("cho-lam-moi")), 10, 10_000, ShellResume.NeedsRefresh, ShellRowTitles("Chuyện chờ làm mới", "Tập chờ làm mới"), 60)
        val bare = ShellWork(bareWork, "Tác phẩm", "", "", emptyList(), unavailable = true)
        val saved = ShellWork(outsideWork, "Chuyện ngoài danh mục", "Truyện", "", emptyList())
        val events = mutableListOf<ShellEvent>()
        // progress is oldest first, so the newest row is last
        render(library(progress = listOf(refresh, replaced, named), saved = setOf(outsideWork, bareWork), works = listOf(saved, bare),
            history = listOf(named)), events) {
            onNodeWithTag("resume-${outsideEpisode}").performScrollTo().assertIsDisplayed()
            onNodeWithTag("resume-${outsideEpisode}").assert(hasText("Tập chưa từng được tải", substring = true))
            onNodeWithTag("resume-${outsideEpisode}").assert(hasText("0:40 / 2:05", substring = true))
            onNodeWithTag("resume-${outsideEpisode}").assert(hasText("Chuyện ngoài danh mục", substring = true))
            assertEquals(0, onAllNodesWithTag("resume-${outsideEpisode}-reason", useUnmergedTree = true).fetchSemanticsNodes().size, "an exact release needs no reason")

            onNodeWithTag("resume-${replacedEpisode}-reason", useUnmergedTree = true).performScrollTo().assert(hasText(copy.text(CopyKey.ReleaseUnavailable)))
            onNodeWithTag("resume-${replacedEpisode}").assert(hasText("0:30 / 1:30", substring = true))
            onNodeWithTag("resume-tap-cho-lam-moi-reason", useUnmergedTree = true).performScrollTo().assert(hasText(copy.text(CopyKey.ResumeNeedsRefresh)))

            onNodeWithTag("saved-undescribed-${bareWork}").performScrollTo().assertIsDisplayed()
            onNodeWithText(copy.text(CopyKey.SavedUndescribed)).assertIsDisplayed()
            onNodeWithTag("work-${outsideWork}").performScrollTo().assert(hasText("Chuyện ngoài danh mục", substring = true))
            assertTrue(events.isEmpty(), "rendering sends nothing and starts no playback")

            onNodeWithTag("resume-${replacedEpisode}").performScrollTo().performClick()
            assertEquals<List<ShellEvent>>(listOf(ShellEvent.ResumeEpisode(replacedEpisode)), events, "the row only asks to resume; the owner decides")
        }
    }

    @Test
    fun an_unread_empty_failed_and_loading_section_look_different_and_each_has_its_own_recovery() {
        val events = mutableListOf<ShellEvent>()
        // Loading with nothing yet: no "empty" claim.
        render(library(ShellLibraryView(ShellLibraryPhase.Loading, ShellLibraryPhase.Loading, ShellLibraryPhase.Loading)), events) {
            onNodeWithTag("library-continue-loading").performScrollTo().assertIsDisplayed()
            onNodeWithTag("library-saved-loading").assertExists()
            onNodeWithTag("library-history-loading").assertExists()
            assertEquals(0, onAllNodesWithTag("library-continue-more").fetchSemanticsNodes().size)
            onNodeWithText(copy.text(CopyKey.NoContinue)).assertDoesNotExist()
            onNodeWithText(copy.text(CopyKey.NoSaved)).assertDoesNotExist()
            onNodeWithText(copy.text(CopyKey.NoHistory)).assertDoesNotExist()
        }
        // Empty after a successful read.
        render(library(), events) {
            onNodeWithText(copy.text(CopyKey.NoContinue)).performScrollTo().assertIsDisplayed()
            onNodeWithText(copy.text(CopyKey.NoSaved)).assertExists()
            onNodeWithText(copy.text(CopyKey.NoHistory)).assertExists()
            assertEquals(0, onAllNodesWithTag("library-continue-loading").fetchSemanticsNodes().size)
        }
        // A failed first read is an error with its section's retry, not an empty list.
        render(library(ShellLibraryView(ShellLibraryPhase.Failed, ShellLibraryPhase.Complete, ShellLibraryPhase.Failed)), events) {
            onNodeWithTag("library-continue-failed").performScrollTo().assertIsDisplayed()
            onNodeWithText(copy.text(CopyKey.NoContinue)).assertDoesNotExist()
            onNodeWithTag("library-continue-retry").performClick()
            onNodeWithTag("library-history-retry").performScrollTo().performClick()
        }
        assertEquals<List<ShellEvent>>(listOf(ShellEvent.RetryLibrary(ShellLibrarySection.Continue), ShellEvent.RetryLibrary(ShellLibrarySection.History)), events)
    }

    @Test
    fun a_section_with_more_pages_loads_more_failed_pages_keep_rows_and_retry_and_other_sections_stay_quiet() {
        val first = ShellProgress(outsideEpisode, 40, 40_000, ShellResume.Available, ShellRowTitles("Chuyện ngoài danh mục", "Tập chưa từng được tải"), 125)
        val saved = ShellWork(outsideWork, "Chuyện ngoài danh mục", "Truyện", "", emptyList())
        val events = mutableListOf<ShellEvent>()
        render(library(ShellLibraryView(ShellLibraryPhase.More, ShellLibraryPhase.MoreFailed, ShellLibraryPhase.Complete),
            progress = listOf(first), saved = setOf(outsideWork), works = listOf(saved)), events) {
            onNodeWithTag("library-continue-more").performScrollTo().performClick()
            onNodeWithTag("library-saved-failed").performScrollTo().assertIsDisplayed()
            onNodeWithText(copy.text(CopyKey.NextPageFailed)).assertIsDisplayed()
            onNodeWithTag("work-${outsideWork}").assertIsDisplayed()
            onNodeWithTag("library-saved-retry").performClick()
            assertEquals(0, onAllNodesWithTag("library-history-more").fetchSemanticsNodes().size)
            assertEquals(0, onAllNodesWithTag("library-history-failed").fetchSemanticsNodes().size)
        }
        assertEquals<List<ShellEvent>>(listOf(ShellEvent.LoadMoreLibrary(ShellLibrarySection.Continue), ShellEvent.RetryLibrary(ShellLibrarySection.Saved)), events)
    }

    @Test
    fun the_full_history_list_pages_with_its_own_footer() {
        val point = ShellProgress(outsideEpisode, 0, 0, ShellResume.Available, ShellRowTitles("Chuyện ngoài danh mục", "Tập chưa từng được tải"), 125)
        val events = mutableListOf<ShellEvent>()
        val state = sampleState(stack = listOf(ShellRoute.Discovery, ShellRoute.History), history = listOf(point))
        runDesktopComposeUiTest(width = 390, height = 2400) {
            setContent { Box(Modifier.requiredSize(390.dp, 2400.dp)) {
                CantosPresentation(state, ShellCatalogView(library = ShellLibraryView(history = ShellLibraryPhase.More)), { events += it })
            } }
            onNodeWithTag("history-${outsideEpisode}").performScrollTo().assert(hasText("Tập chưa từng được tải", substring = true))
            onNodeWithTag("history-more").performScrollTo().performClick()
        }
        assertEquals<List<ShellEvent>>(listOf(ShellEvent.LoadMoreLibrary(ShellLibrarySection.History)), events)
    }
}
