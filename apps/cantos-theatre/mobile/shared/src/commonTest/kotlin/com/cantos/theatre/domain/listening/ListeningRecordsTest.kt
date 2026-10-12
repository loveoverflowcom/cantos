package com.cantos.theatre.domain.listening

import com.cantos.theatre.domain.publication.EpisodeKey
import com.cantos.theatre.domain.publication.WorkKey
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertSame

class ListeningRecordsTest {
    private val work = WorkKey("son-ha-ky")
    private val episode = EpisodeKey(work, 0)

    @Test
    fun saving_bookmarking_and_progress_never_change_each_others_records() {
        val initial = ListeningSnapshot()
        val saved = reduceListening(initial, ListeningEvent.ToggleSaved(work))
        assertEquals(ListeningSnapshot(savedWorks = setOf(SavedWork(work))), saved)
        assertEquals(initial, reduceListening(saved, ListeningEvent.ToggleSaved(work)))

        val bookmark = Bookmark(episode, 20_000, 60_000)
        val bookmarked = reduceListening(saved, ListeningEvent.AddBookmark(bookmark))
        assertEquals(ListeningSnapshot(setOf(SavedWork(work)), listOf(bookmark)), bookmarked)
        assertSame(bookmarked, reduceListening(bookmarked, ListeningEvent.AddBookmark(bookmark)))

        val progress = ProgressRecord(episode, 60_000, 60_000)
        val progressed = reduceListening(bookmarked, ListeningEvent.RecordProgress(progress))
        assertEquals(ListeningSnapshot(setOf(SavedWork(work)), listOf(bookmark), listOf(progress)), progressed)
        assertSame(progressed, reduceListening(progressed, ListeningEvent.RecordProgress(progress)))
        val rewound = ProgressRecord(episode, 5_000, 60_000)
        assertEquals(
            ListeningSnapshot(setOf(SavedWork(work)), listOf(bookmark), listOf(rewound)),
            reduceListening(progressed, ListeningEvent.RecordProgress(rewound)),
        )
    }

    @Test
    fun snapshot_constructor_and_getters_cannot_expose_mutable_authority() {
        val otherWork = WorkKey("den-khuya")
        val otherEpisode = EpisodeKey(otherWork, 0)
        val saved = mutableSetOf(SavedWork(work), SavedWork(otherWork))
        val bookmarks = mutableListOf(Bookmark(episode, 0, 60_000), Bookmark(otherEpisode, 0, 60_000))
        val progress = mutableListOf(ProgressRecord(episode, 0, 60_000), ProgressRecord(otherEpisode, 0, 60_000))
        val state = ListeningSnapshot(saved, bookmarks, progress)
        val expected = ListeningSnapshot(saved, bookmarks, progress)
        saved.clear()
        bookmarks.clear()
        progress.clear()
        (state.savedWorks as MutableSet<*>).clear()
        (state.bookmarks as MutableList<*>).clear()
        (state.progress as MutableList<*>).clear()
        assertEquals(expected, state)
        assertEquals(expected.hashCode(), state.hashCode())
    }

    @Test
    fun invalid_points_and_duplicate_identity_are_rejected() {
        assertFailsWith<IllegalArgumentException> { ProgressRecord(episode, -1, 60_000) }
        assertFailsWith<IllegalArgumentException> { ProgressRecord(episode, 60_001, 60_000) }
        assertFailsWith<IllegalArgumentException> { Bookmark(episode, 0, 0) }
        val bookmark = Bookmark(episode, 0, 60_000)
        assertFailsWith<IllegalArgumentException> { ListeningSnapshot(bookmarks = listOf(bookmark, bookmark)) }
        assertFailsWith<IllegalArgumentException> {
            ListeningSnapshot(progress = listOf(ProgressRecord(episode, 0, 60_000), ProgressRecord(episode, 1, 60_000)))
        }
    }
}
