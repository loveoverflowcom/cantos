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
    fun desired_saved_state_is_idempotent_and_preserves_other_memory_records() {
        val point = ProgressRecord(episode, 5_000, 60_000)
        val initial = ListeningSnapshot(
            bookmarks = listOf(Bookmark(episode, 10_000, 60_000)),
            progress = listOf(point), history = listOf(point),
        )
        val saved = reduceListening(initial, ListeningEvent.SetSaved(work, saved = true))
        assertEquals(ListeningSnapshot(setOf(SavedWork(work)), initial.bookmarks, initial.progress, initial.history), saved)
        assertSame(saved, reduceListening(saved, ListeningEvent.SetSaved(work, saved = true)))
        val removed = reduceListening(saved, ListeningEvent.SetSaved(work, saved = false))
        assertEquals(initial, removed)
        assertSame(removed, reduceListening(removed, ListeningEvent.SetSaved(work, saved = false)))
    }

    @Test
    fun history_admission_keeps_the_latest_started_episode_per_work_in_recency_order() {
        val first = ProgressRecord(episode, 0, 60_000)
        val other = ProgressRecord(EpisodeKey(WorkKey("den-khuya"), 0), 0, 60_000)
        val next = ProgressRecord(EpisodeKey(work, 1), 0, 60_000)
        val visited = reduceListening(ListeningSnapshot(), ListeningEvent.RecordProgress(first))
        assertEquals(emptyList(), visited.history)
        val played = reduceListening(visited, ListeningEvent.RecordStarted(first))
        val anotherPlayed = reduceListening(played, ListeningEvent.RecordStarted(other))
        val latest = reduceListening(anotherPlayed, ListeningEvent.RecordStarted(next))
        assertEquals(listOf(next, other), latest.history)
        assertEquals(listOf(first, other, next), latest.progress)
        assertSame(latest, reduceListening(latest, ListeningEvent.RecordStarted(next)))
    }

    @Test
    fun replaying_an_older_episode_at_the_same_point_makes_it_the_latest_progress() {
        val first = ProgressRecord(episode, 5_000, 60_000)
        val next = ProgressRecord(EpisodeKey(work, 1), 10_000, 60_000)
        val initial = ListeningSnapshot(progress = listOf(first, next), history = listOf(next))
        val replayed = reduceListening(initial, ListeningEvent.RecordStarted(first))
        assertEquals(listOf(next, first), replayed.progress)
        assertEquals(listOf(first), replayed.history)
        assertSame(replayed, reduceListening(replayed, ListeningEvent.RecordStarted(first)))
    }

    @Test
    fun progress_updates_only_an_admitted_episode_without_reordering_history_and_clear_preserves_others() {
        val first = ProgressRecord(episode, 30_000, 60_000)
        val other = ProgressRecord(EpisodeKey(WorkKey("den-khuya"), 0), 0, 60_000)
        val initial = ListeningSnapshot(
            setOf(SavedWork(work)), listOf(Bookmark(episode, 10_000, 60_000)), listOf(first), listOf(other, first),
        )
        val rewind = ProgressRecord(episode, 5_000, 60_000)
        val rewound = reduceListening(initial, ListeningEvent.RecordProgress(rewind))
        assertEquals(listOf(other, rewind), rewound.history)
        val unplayed = ProgressRecord(EpisodeKey(work, 1), 0, 60_000)
        val visited = reduceListening(rewound, ListeningEvent.RecordProgress(unplayed))
        assertEquals(rewound.history, visited.history)
        val cleared = reduceListening(visited, ListeningEvent.ClearHistory)
        assertEquals(ListeningSnapshot(visited.savedWorks, visited.bookmarks, visited.progress), cleared)
        assertSame(cleared, reduceListening(cleared, ListeningEvent.ClearHistory))
        assertEquals(emptyList(), reduceListening(cleared, ListeningEvent.RecordProgress(rewind)).history)
    }

    @Test
    fun snapshot_constructor_and_getters_cannot_expose_mutable_authority() {
        val otherWork = WorkKey("den-khuya")
        val otherEpisode = EpisodeKey(otherWork, 0)
        val saved = mutableSetOf(SavedWork(work), SavedWork(otherWork))
        val bookmarks = mutableListOf(Bookmark(episode, 0, 60_000), Bookmark(otherEpisode, 0, 60_000))
        val progress = mutableListOf(ProgressRecord(episode, 0, 60_000), ProgressRecord(otherEpisode, 0, 60_000))
        val history = progress.toMutableList()
        val state = ListeningSnapshot(saved, bookmarks, progress, history)
        val expected = ListeningSnapshot(saved, bookmarks, progress, history)
        saved.clear()
        bookmarks.clear()
        progress.clear()
        history.clear()
        (state.savedWorks as MutableSet<*>).clear()
        (state.bookmarks as MutableList<*>).clear()
        (state.progress as MutableList<*>).clear()
        (state.history as MutableList<*>).clear()
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
        assertFailsWith<IllegalArgumentException> {
            ListeningSnapshot(history = listOf(
                ProgressRecord(episode, 0, 60_000), ProgressRecord(EpisodeKey(work, 1), 0, 60_000),
            ))
        }
    }
}
