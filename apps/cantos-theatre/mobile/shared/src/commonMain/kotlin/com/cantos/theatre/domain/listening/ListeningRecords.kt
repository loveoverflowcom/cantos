package com.cantos.theatre.domain.listening

import com.cantos.theatre.domain.publication.EpisodeKey
import com.cantos.theatre.domain.publication.WorkKey

data class SavedWork(val work: WorkKey)

/** A memory-only point. Neither end position nor engine end means completed listening. */
data class ProgressRecord(val episode: EpisodeKey, val positionMs: Long, val durationMs: Long) {
    init {
        require(durationMs > 0 && positionMs in 0..durationMs) { "Progress must be within its episode" }
    }
}

data class Bookmark(val episode: EpisodeKey, val positionMs: Long, val durationMs: Long) {
    init {
        require(durationMs > 0 && positionMs in 0..durationMs) { "Bookmark must be within its episode" }
    }
}

/** Independent memory records. History is most recent accepted simulated start first, per work. */
class ListeningSnapshot(
    savedWorks: Set<SavedWork> = emptySet(),
    bookmarks: List<Bookmark> = emptyList(),
    progress: List<ProgressRecord> = emptyList(),
    history: List<ProgressRecord> = emptyList(),
) {
    private val savedSnapshot = savedWorks.toSet()
    private val bookmarkSnapshot = bookmarks.toList()
    private val progressSnapshot = progress.toList()
    private val historySnapshot = history.toList()
    val savedWorks: Set<SavedWork> get() = savedSnapshot.toSet()
    val bookmarks: List<Bookmark> get() = bookmarkSnapshot.toList()
    /** The last entry is the most recently changed episode; a rewind remains a valid change. */
    val progress: List<ProgressRecord> get() = progressSnapshot.toList()
    val history: List<ProgressRecord> get() = historySnapshot.toList()

    init {
        require(bookmarkSnapshot.map { it.episode to it.positionMs }.distinct().size == bookmarkSnapshot.size) {
            "Bookmark points must be unique"
        }
        require(progressSnapshot.map { it.episode }.distinct().size == progressSnapshot.size) {
            "Each episode has one memory progress record"
        }
        require(historySnapshot.map { it.episode.work }.distinct().size == historySnapshot.size) {
            "History keeps one latest episode per work"
        }
    }

    override fun equals(other: Any?): Boolean = other is ListeningSnapshot &&
        savedSnapshot == other.savedSnapshot && bookmarkSnapshot == other.bookmarkSnapshot &&
        progressSnapshot == other.progressSnapshot && historySnapshot == other.historySnapshot

    override fun hashCode(): Int {
        var result = savedSnapshot.hashCode()
        result = 31 * result + bookmarkSnapshot.hashCode()
        result = 31 * result + progressSnapshot.hashCode()
        return 31 * result + historySnapshot.hashCode()
    }

    override fun toString(): String = "ListeningSnapshot(saved=${savedSnapshot.size}, " +
        "bookmarks=${bookmarkSnapshot.size}, progress=${progressSnapshot.size}, " +
        "history=${historySnapshot.size}, memoryOnly=true)"
}

sealed interface ListeningEvent {
    data class SetSaved(val work: WorkKey, val saved: Boolean) : ListeningEvent
    data class ToggleSaved(val work: WorkKey) : ListeningEvent
    data class AddBookmark(val bookmark: Bookmark) : ListeningEvent
    data class RecordProgress(val record: ProgressRecord) : ListeningEvent
    /** The application emits this only after a committed transition into simulated playback. */
    data class RecordStarted(val record: ProgressRecord) : ListeningEvent
    data object ClearHistory : ListeningEvent
    data class Reset(val snapshot: ListeningSnapshot) : ListeningEvent
}

/** Pure local decisions only; no account, reconciliation, persistence or authorization policy. */
fun reduceListening(state: ListeningSnapshot, event: ListeningEvent): ListeningSnapshot = when (event) {
    is ListeningEvent.SetSaved -> {
        val saved = SavedWork(event.work)
        if ((saved in state.savedWorks) == event.saved) {
            state
        } else {
            ListeningSnapshot(
                savedWorks = if (event.saved) state.savedWorks + saved else state.savedWorks - saved,
                bookmarks = state.bookmarks,
                progress = state.progress,
                history = state.history,
            )
        }
    }
    is ListeningEvent.ToggleSaved -> reduceListening(
        state, ListeningEvent.SetSaved(event.work, SavedWork(event.work) !in state.savedWorks),
    )
    is ListeningEvent.AddBookmark -> if (state.bookmarks.any {
        it.episode == event.bookmark.episode && it.positionMs == event.bookmark.positionMs
    }) {
        state
    } else {
        ListeningSnapshot(state.savedWorks, state.bookmarks + event.bookmark, state.progress, state.history)
    }
    is ListeningEvent.RecordProgress -> {
        val progress = if (event.record in state.progress) state.progress else {
            state.progress.filterNot { it.episode == event.record.episode } + event.record
        }
        val history = state.history.map { if (it.episode == event.record.episode) event.record else it }
        if (progress == state.progress && history == state.history) state else {
            ListeningSnapshot(state.savedWorks, state.bookmarks, progress, history)
        }
    }
    is ListeningEvent.RecordStarted -> {
        val progress = state.progress.filterNot { it.episode == event.record.episode } + event.record
        val history = listOf(event.record) + state.history.filterNot { it.episode.work == event.record.episode.work }
        if (progress == state.progress && history == state.history) state else {
            ListeningSnapshot(state.savedWorks, state.bookmarks, progress, history)
        }
    }
    ListeningEvent.ClearHistory -> if (state.history.isEmpty()) state else {
        ListeningSnapshot(state.savedWorks, state.bookmarks, state.progress)
    }
    is ListeningEvent.Reset -> event.snapshot
}
