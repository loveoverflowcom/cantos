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

/** Saved works, bookmarks and progress have separate identities and collections. */
class ListeningSnapshot(
    savedWorks: Set<SavedWork> = emptySet(),
    bookmarks: List<Bookmark> = emptyList(),
    progress: List<ProgressRecord> = emptyList(),
) {
    private val savedSnapshot = savedWorks.toSet()
    private val bookmarkSnapshot = bookmarks.toList()
    private val progressSnapshot = progress.toList()
    val savedWorks: Set<SavedWork> get() = savedSnapshot.toSet()
    val bookmarks: List<Bookmark> get() = bookmarkSnapshot.toList()
    val progress: List<ProgressRecord> get() = progressSnapshot.toList()

    init {
        require(bookmarkSnapshot.map { it.episode to it.positionMs }.distinct().size == bookmarkSnapshot.size) {
            "Bookmark points must be unique"
        }
        require(progressSnapshot.map { it.episode }.distinct().size == progressSnapshot.size) {
            "Each episode has one memory progress record"
        }
    }

    override fun equals(other: Any?): Boolean = other is ListeningSnapshot &&
        savedSnapshot == other.savedSnapshot && bookmarkSnapshot == other.bookmarkSnapshot &&
        progressSnapshot == other.progressSnapshot

    override fun hashCode(): Int {
        var result = savedSnapshot.hashCode()
        result = 31 * result + bookmarkSnapshot.hashCode()
        return 31 * result + progressSnapshot.hashCode()
    }

    override fun toString(): String = "ListeningSnapshot(saved=${savedSnapshot.size}, " +
        "bookmarks=${bookmarkSnapshot.size}, progress=${progressSnapshot.size}, memoryOnly=true)"
}

sealed interface ListeningEvent {
    data class ToggleSaved(val work: WorkKey) : ListeningEvent
    data class AddBookmark(val bookmark: Bookmark) : ListeningEvent
    data class RecordProgress(val record: ProgressRecord) : ListeningEvent
    data class Reset(val snapshot: ListeningSnapshot) : ListeningEvent
}

/** Pure local decisions only; no account, reconciliation, persistence or authorization policy. */
fun reduceListening(state: ListeningSnapshot, event: ListeningEvent): ListeningSnapshot = when (event) {
    is ListeningEvent.ToggleSaved -> {
        val saved = SavedWork(event.work)
        ListeningSnapshot(
            savedWorks = if (saved in state.savedWorks) state.savedWorks - saved else state.savedWorks + saved,
            bookmarks = state.bookmarks,
            progress = state.progress,
        )
    }
    is ListeningEvent.AddBookmark -> if (state.bookmarks.any {
        it.episode == event.bookmark.episode && it.positionMs == event.bookmark.positionMs
    }) {
        state
    } else {
        ListeningSnapshot(state.savedWorks, state.bookmarks + event.bookmark, state.progress)
    }
    is ListeningEvent.RecordProgress -> if (event.record in state.progress) {
        state
    } else {
        ListeningSnapshot(
            state.savedWorks,
            state.bookmarks,
            state.progress.filterNot { it.episode == event.record.episode } + event.record,
        )
    }
    is ListeningEvent.Reset -> event.snapshot
}
