package com.cantos.theatre.presentation.model

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue

class ShellStateTest {
    @Test
    fun neutral_projection_accepts_non_fixture_identities_without_manufacturing_listening_records() {
        val work = ShellWorkId("published-work-42")
        val episode = ShellEpisodeId("published-episode-8", work)
        val state = ShellState(selectedEpisode = episode)
        assertEquals(episode, state.selectedEpisode)
        assertEquals(PlaybackUiStatus.Idle, state.playbackStatus)
        assertTrue(state.savedWorks.isEmpty())
        assertTrue(state.bookmarks.isEmpty())
        assertTrue(state.progress.isEmpty())
        assertTrue(state.history.isEmpty())
        val view = ShellCatalogView(listOf(ShellWork(work, "Ánh đèn cuối sân khấu", "Truyện", "", listOf(
            ShellEpisode(episode, work, 8, "Người dẫn chuyện", 60),
        ))))
        assertEquals(episode, view.episode(episode)?.id)
        assertEquals(work, filteredWorks(state, view).single().id)
    }
    @Test
    fun playback_projection_refuses_negative_fences_and_pending_targets_on_construction_or_copy() {
        val episode = ShellEpisodeId("episode", ShellWorkId("work"))
        assertFailsWith<IllegalArgumentException> { ShellState(selectedEpisode = episode, playbackGeneration = -1) }
        assertFailsWith<IllegalArgumentException> { ShellState(selectedEpisode = episode, pendingSeekSeconds = -1) }
        val valid = ShellState(selectedEpisode = episode, playbackGeneration = 2, pendingSeekSeconds = 12)
        assertEquals(valid, valid.copy())
        assertFailsWith<IllegalArgumentException> { valid.copy(playbackGeneration = -1) }
        assertFailsWith<IllegalArgumentException> { valid.copy(pendingSeekSeconds = -1) }
    }

}
