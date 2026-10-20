package com.cantos.theatre.app

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.ComposeUiTest
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertContentDescriptionContains
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
import com.cantos.theatre.features.shell.presentation.LocalShellCapabilities
import com.cantos.theatre.presentation.model.CopyKey
import com.cantos.theatre.presentation.model.SampleEpisodeIds
import com.cantos.theatre.presentation.model.ShellBookmark
import com.cantos.theatre.presentation.model.ShellBookmarkSync
import com.cantos.theatre.presentation.model.ShellCapabilities
import com.cantos.theatre.presentation.model.ShellCatalogView
import com.cantos.theatre.presentation.model.ShellCopy
import com.cantos.theatre.presentation.model.ShellEpisodeId
import com.cantos.theatre.presentation.model.ShellEvent
import com.cantos.theatre.presentation.model.ShellLibraryPhase
import com.cantos.theatre.presentation.model.ShellLibrarySection
import com.cantos.theatre.presentation.model.ShellLibraryView
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.theatre.presentation.model.ShellResume
import com.cantos.theatre.presentation.model.ShellRoute
import com.cantos.theatre.presentation.model.ShellRowTitles
import com.cantos.theatre.presentation.model.ShellState
import com.cantos.theatre.presentation.model.ShellWorkId
import com.cantos.theatre.presentation.model.sampleState
import com.cantos.theatre.presentation.model.sampleWorks
import org.junit.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

/** The production Library and player bookmark surfaces over supplied projections; each control's own event is asserted. */
@OptIn(ExperimentalTestApi::class)
class BookmarkPresentationUiTest {
    private val vi = ShellCopy(ShellLocale.Vietnamese)
    private val en = ShellCopy(ShellLocale.English)
    private val synced = ShellBookmark(SampleEpisodeIds.SonHa03, 768, id = "b-synced")
    private val pending = ShellBookmark(SampleEpisodeIds.SonHa01, 30, id = "b-pending", sync = ShellBookmarkSync.Pending)
    private val offline = ShellBookmark(SampleEpisodeIds.DenKhua01, 91, id = "b-offline", sync = ShellBookmarkSync.Offline)
    private val failed = ShellBookmark(SampleEpisodeIds.DenKhua02, 5, id = "b-failed", sync = ShellBookmarkSync.Failed)
    private val replaced = ShellBookmark(
        ShellEpisodeId("tap-ban-cu", ShellWorkId("ban-cu")), 40, id = "b-replaced", resume = ShellResume.ReleaseReplaced,
        titles = ShellRowTitles("Chuyện bản thu cũ", "Tập bản thu cũ"), durationSeconds = 90,
    )

    private fun render(
        state: ShellState, catalog: ShellCatalogView = ShellCatalogView(works = sampleWorks), locale: ShellLocale = ShellLocale.Vietnamese,
        bookmarksOffered: Boolean = true, events: MutableList<ShellEvent> = mutableListOf(), check: ComposeUiTest.() -> Unit,
    ) = runDesktopComposeUiTest(width = 390, height = 3000) {
        setContent {
            Box(Modifier.requiredSize(390.dp, 3000.dp)) {
                CompositionLocalProvider(LocalShellCapabilities provides ShellCapabilities(bookmarks = bookmarksOffered)) {
                    CantosPresentation(state.copy(locale = locale), catalog, { events += it })
                }
            }
        }
        check()
    }

    private fun library(vararg rows: ShellBookmark) = sampleState(stack = listOf(ShellRoute.Discovery, ShellRoute.Library), bookmarks = rows.toList())
    private fun ComposeUiTest.count(tag: String) = onAllNodesWithTag(tag, useUnmergedTree = true).fetchSemanticsNodes().size

    @Test
    fun the_library_lists_each_bookmark_with_only_the_state_that_is_true_of_it() {
        render(library(synced, pending, offline, failed)) {
            onNodeWithTag("library-bookmarks").performScrollTo().assertIsDisplayed()
            for (row in listOf(synced, pending, offline, failed)) onNodeWithTag("library-bookmark-${row.id}-open").performScrollTo().assertIsDisplayed()
            assertEquals(0, count("library-bookmark-b-synced-status"), "a synced bookmark carries no badge")
            onNodeWithTag("library-bookmark-b-pending-status", useUnmergedTree = true).assert(hasText(vi.text(CopyKey.SyncPending)))
            onNodeWithTag("library-bookmark-b-offline-status", useUnmergedTree = true).assert(hasText(vi.text(CopyKey.BookmarkOffline)))
            onNodeWithTag("library-bookmark-b-failed-status", useUnmergedTree = true).assert(hasText(vi.text(CopyKey.BookmarkFailed)))
            // A failed bookmark is discarded, not "deleted": the server never had it.
            onNodeWithTag("library-bookmark-b-failed-delete").assertContentDescriptionContains("Bỏ dấu trang", substring = true)
            onNodeWithText(vi.text(CopyKey.DiscardBookmark)).assertExists()
            onNodeWithTag("library-bookmark-b-synced-delete").assertContentDescriptionContains("Xóa dấu trang", substring = true)
            onNodeWithText("Người giữ tiếng chuông · 12:48").assertExists()
        }
    }

    @Test
    fun a_replaced_release_is_explained_and_keeps_its_own_titles_and_a_row_sends_exactly_its_own_events() {
        val events = mutableListOf<ShellEvent>()
        render(library(replaced, synced), catalog = ShellCatalogView(works = sampleWorks, referencedEpisodes = listOf(
            com.cantos.theatre.presentation.model.ShellEpisode(replaced.episodeId, replaced.episodeId.workId, 1, "Tập bản thu cũ", 90))), events = events) {
            onNodeWithTag("library-bookmark-b-replaced-reason", useUnmergedTree = true).performScrollTo().assert(hasText(vi.text(CopyKey.ReleaseUnavailable)))
            onNodeWithText("Tập bản thu cũ · 0:40").assertExists()
            assertTrue(events.isEmpty(), "rendering sends nothing and plays nothing")
            onNodeWithTag("library-bookmark-b-replaced-open").performScrollTo().performClick()
            assertEquals<List<ShellEvent>>(listOf(ShellEvent.OpenBookmark(replaced)), events, "the row only asks to open; the owner decides")
            onNodeWithTag("library-bookmark-b-synced-delete").performScrollTo().performClick()
            assertEquals(ShellEvent.DeleteBookmark("b-synced"), events.last())
        }
    }

    @Test
    fun an_unread_empty_failed_and_more_section_look_different_and_page_through_their_own_events() {
        val events = mutableListOf<ShellEvent>()
        fun view(phase: ShellLibraryPhase) = ShellCatalogView(works = sampleWorks, library = ShellLibraryView(bookmarks = phase))
        render(library(), view(ShellLibraryPhase.Loading), events = events) {
            onNodeWithTag("library-bookmarks-loading").performScrollTo().assertIsDisplayed()
            onNodeWithText(vi.text(CopyKey.NoBookmarks)).assertDoesNotExist()
        }
        render(library(), view(ShellLibraryPhase.Complete), events = events) {
            onNodeWithText(vi.text(CopyKey.NoBookmarks)).performScrollTo().assertIsDisplayed()
            onNodeWithText(vi.text(CopyKey.NoBookmarksHelp)).assertIsDisplayed()
        }
        render(library(synced), view(ShellLibraryPhase.More), events = events) {
            onNodeWithTag("library-bookmarks-more").performScrollTo().performClick()
        }
        render(library(synced), view(ShellLibraryPhase.Stale), events = events) {
            onNodeWithTag("library-bookmarks-failed").performScrollTo().assertIsDisplayed()
            onNodeWithText(vi.text(CopyKey.LibrarySectionStale)).assertIsDisplayed()
            onNodeWithTag("library-bookmarks-retry").performScrollTo().performClick()
            onNodeWithTag("library-bookmark-b-synced-open").assertExists()
        }
        assertEquals<List<ShellEvent>>(listOf(ShellEvent.LoadMoreLibrary(ShellLibrarySection.Bookmarks), ShellEvent.RetryLibrary(ShellLibrarySection.Bookmarks)), events)
    }

    @Test
    fun the_player_lists_the_same_rows_and_offers_the_control_only_when_a_real_owner_exists() {
        val player = sampleState(stack = listOf(ShellRoute.Discovery, ShellRoute.Player), bookmarks = listOf(synced, offline))
        render(player) {
            onNodeWithTag("player-bookmarks").performScrollTo().assertIsDisplayed()
            onNodeWithTag("player-bookmark-b-synced-open").performScrollTo().assertIsDisplayed()
            onNodeWithTag("player-bookmark-b-offline-status", useUnmergedTree = true).assertExists()
            onNodeWithTag("bookmark").performScrollTo().assertIsDisplayed()
        }
        render(player, bookmarksOffered = false) {
            assertEquals(0, count("bookmark"), "no capability, no control: never a simulated save")
        }
    }

    @Test
    fun a_guest_projection_shows_no_bookmark_section_loading_or_rows() {
        render(sampleState(stack = listOf(ShellRoute.Discovery, ShellRoute.Library), bookmarks = emptyList()), ShellCatalogView(works = sampleWorks)) {
            assertEquals(0, count("library-bookmarks-loading"))
            onNodeWithText(vi.text(CopyKey.NoBookmarks)).performScrollTo().assertExists()
        }
    }

    @Test
    fun english_copy_and_large_text_keep_every_status_readable_and_every_control_reachable() {
        runDesktopComposeUiTest(width = 390, height = 3000) {
            setContent {
                Box(Modifier.requiredSize(390.dp, 3000.dp)) {
                    CantosPresentation(library(offline, failed).copy(locale = ShellLocale.English), ShellCatalogView(works = sampleWorks), { }, textScale = 2f)
                }
            }
            onNodeWithTag("library-bookmark-b-offline-status", useUnmergedTree = true).performScrollTo().assert(hasText(en.text(CopyKey.BookmarkOffline)))
            onNodeWithTag("library-bookmark-b-failed-delete").performScrollTo().assertContentDescriptionContains("Discard bookmark", substring = true)
            onNodeWithText(en.text(CopyKey.DiscardBookmark)).assertExists()
            onNodeWithTag("library-bookmark-b-offline-open").performScrollTo().assertIsDisplayed()
        }
    }
}
