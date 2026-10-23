package com.cantos.theatre.presentation.model

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class LibraryEmptyViewTest {
    private val work = ShellWorkId("retained-work")
    private val episode = ShellEpisodeId("retained-episode", work)
    private val empty = ShellState(selectedEpisode = episode)
    private val capabilities = ShellCapabilities()

    @Test
    fun only_complete_sections_can_be_empty_and_every_nonterminal_phase_blocks_the_global_hero() {
        ShellLibrarySection.entries.forEach { section ->
            ShellLibraryPhase.entries.forEach { phase ->
                val view = when (section) {
                    ShellLibrarySection.Continue -> ShellLibraryView(continuing = phase)
                    ShellLibrarySection.Saved -> ShellLibraryView(saved = phase)
                    ShellLibrarySection.History -> ShellLibraryView(history = phase)
                    ShellLibrarySection.Bookmarks -> ShellLibraryView(bookmarks = phase)
                }
                val expected = if (phase == ShellLibraryPhase.Complete) ShellLibrarySection.entries.toSet()
                    else ShellLibrarySection.entries.toSet() - section
                assertEquals(expected, emptyLibrarySections(empty, view, capabilities), "$section/$phase")
                assertEquals(phase == ShellLibraryPhase.Complete, isLibraryCompletelyEmpty(empty, view, capabilities), "$section/$phase")
            }
        }
    }

    @Test
    fun retained_rows_prevent_empty_even_without_catalog_metadata() {
        listOf(
            ShellLibrarySection.Continue to empty.copy(progress = listOf(ShellProgress(episode, 0))),
            ShellLibrarySection.Saved to empty.copy(savedWorks = setOf(work)),
            ShellLibrarySection.History to empty.copy(history = listOf(ShellProgress(episode, 0))),
            ShellLibrarySection.Bookmarks to empty.copy(bookmarks = listOf(ShellBookmark(episode, 0))),
        ).forEach { (section, state) ->
            assertEquals(ShellLibrarySection.entries.toSet() - section, emptyLibrarySections(state, ShellLibraryView(), capabilities))
            assertFalse(isLibraryCompletelyEmpty(state, ShellLibraryView(), capabilities))
        }
    }

    @Test
    fun disabled_bookmarks_are_unavailable_rather_than_empty_and_do_not_gate_supported_sections() {
        val disabled = ShellCapabilities(bookmarks = false)
        assertEquals(setOf(ShellLibrarySection.Continue, ShellLibrarySection.Saved, ShellLibrarySection.History),
            emptyLibrarySections(empty, ShellLibraryView(bookmarks = ShellLibraryPhase.Loading), disabled))
        assertTrue(isLibraryCompletelyEmpty(empty, ShellLibraryView(bookmarks = ShellLibraryPhase.Loading), disabled))
        assertFalse(isLibraryCompletelyEmpty(empty, ShellLibraryView(saved = ShellLibraryPhase.Failed), disabled))
    }

    @Test
    fun save_play_clear_and_account_replacement_are_derived_from_each_current_snapshot() {
        val complete = ShellLibraryView()
        assertTrue(isLibraryCompletelyEmpty(empty, complete, capabilities))
        val saved = empty.copy(savedWorks = setOf(work))
        val played = saved.copy(progress = listOf(ShellProgress(episode, 12)), history = listOf(ShellProgress(episode, 12)))
        assertEquals(setOf(ShellLibrarySection.Bookmarks), emptyLibrarySections(played, complete, capabilities))
        val cleared = played.copy(history = emptyList())
        assertEquals(setOf(ShellLibrarySection.History, ShellLibrarySection.Bookmarks), emptyLibrarySections(cleared, complete, capabilities))
        assertFalse(isLibraryCompletelyEmpty(cleared, complete, capabilities))
        assertTrue(isLibraryCompletelyEmpty(empty, complete, capabilities))
    }
}
