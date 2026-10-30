package com.cantos.theatre.domain.library

import com.cantos.theatre.domain.playback.AudioIdentity

enum class LibrarySection(val path: String) { Continue("continue"), Saved("saved"), History("history"), Bookmarks("bookmarks") }
enum class LibrarySync { Pending, Confirmed, Failed }
/** One bookmark's own sync truth: [Offline] is a pending change the last attempt could not deliver. */
/** [Failed] is a create the server refused for good; [DeleteFailed] is a deletion that could not be completed (the bookmark still exists). */
enum class BookmarkSync { Synced, Pending, Offline, Failed, DeleteFailed }
enum class LibraryResume { ExactAvailable, ReleaseReplaced, AccessUnavailable, NeedsRefresh }
enum class ProgressIntent(val wire: String) { Checkpoint("checkpoint"), ExplicitSeek("explicit_seek"), Restart("restart"), ResolveHere("resolve_here"), ResolveOther("resolve_other"), MarkCompleted("mark_completed") }
enum class LibraryWorkKind { Novel, Story, Podcast }

/** Released Work metadata as the API confirmed it for [publicationId]; never an editable head title. */
data class LibraryWorkInfo(val workId: String, val publicationId: String, val title: String, val kind: LibraryWorkKind, val language: String)

/** Released unit metadata for the exact stored publication. [work] names the same release. */
data class LibraryUnitInfo(val work: LibraryWorkInfo, val unitId: String, val unitTitle: String)

/**
 * What the API said about a Library row's metadata. [Unread] means no presentation read has
 * covered it yet (an older local row), [Unavailable] is the API's explicit null for a private,
 * replaced or withdrawn release; neither is a title, and neither may borrow another release's.
 */
sealed interface LibraryPresentation<out T> {
    data object Unread : LibraryPresentation<Nothing>
    data object Unavailable : LibraryPresentation<Nothing>
    data class Available<T>(val value: T) : LibraryPresentation<T>
}
val <T> LibraryPresentation<T>.valueOrNull: T? get() = (this as? LibraryPresentation.Available<T>)?.value

data class SavedEntry(
    val workId: String, val saved: Boolean, val revision: Long, val sync: LibrarySync,
    val presentation: LibraryPresentation<LibraryWorkInfo> = LibraryPresentation.Unread,
    val confirmedSaved: Boolean = sync == LibrarySync.Confirmed && saved,
    /** A rejected Saved intent can be retired and explicitly submitted again. */
    val retryableFailure: Boolean = false,
)
data class ContinueEntry(
    val audio: AudioIdentity, val positionMs: Long, val revision: Long, val resume: LibraryResume, val sync: LibrarySync,
    val presentation: LibraryPresentation<LibraryUnitInfo> = LibraryPresentation.Unread,
)
data class HistoryEntry(
    val audio: AudioIdentity, val startedAtMs: Long, val resume: LibraryResume,
    val presentation: LibraryPresentation<LibraryUnitInfo> = LibraryPresentation.Unread,
)

/**
 * A release-bound bookmark: the exact [audio] it was made on and a position inside it. [resume] says whether
 * that exact release can still be opened; a bookmark never names another release, and owning one grants no access.
 */
data class BookmarkEntry(
    val id: String, val audio: AudioIdentity, val positionMs: Long, val resume: LibraryResume, val sync: BookmarkSync,
    val presentation: LibraryPresentation<LibraryUnitInfo> = LibraryPresentation.Unread,
)

/** Where one section's own paging stands. A read in one section never changes another's. */
data class LibrarySectionState(
    /** The first page of this account/session has been applied (so an empty list is truly empty). */
    val loaded: Boolean = false,
    val loading: LibraryLoad = LibraryLoad.Idle,
    /** The last read of this section failed; rows already shown stay. */
    val failure: LibraryReadFailure? = null,
)
enum class LibraryLoad { Idle, Refreshing, NextPage }
data class LibraryReadFailure(val code: String, val retryable: Boolean, val nextPage: Boolean)
data class PausedCheckpoint(val audio: AudioIdentity, val positionMs: Long, val speedPermille: Int = 1000)
data class LibraryConflict(val resourceKey: String, val operation: String, val localPositionMs: Long?, val serverPositionMs: Long?, val serverRevision: Long)
data class LibraryState(
    val account: String? = null,
    val saved: List<SavedEntry> = emptyList(),
    val continueListening: List<ContinueEntry> = emptyList(),
    val history: List<HistoryEntry> = emptyList(),
    /** Newest first; a change not yet delivered is shown at once and marked, a deleted one is gone. */
    val bookmarks: List<BookmarkEntry> = emptyList(),
    val cursors: Map<LibrarySection, String?> = emptyMap(),
    val sections: Map<LibrarySection, LibrarySectionState> = emptyMap(),
    val pending: Int = 0,
    val failures: Map<String, String> = emptyMap(),
    val conflicts: List<LibraryConflict> = emptyList(),
    val restoredPaused: PausedCheckpoint? = null,
    val failure: String? = null,
    /** The last delivery attempt failed in transport: pending changes wait, they are not lost. */
    val offline: Boolean = false,
) {
    fun section(section: LibrarySection): LibrarySectionState = sections[section] ?: LibrarySectionState()
    /** Any section read in flight; each section's own state is in [sections]. */
    val refreshing: Boolean get() = sections.values.any { it.loading != LibraryLoad.Idle }
}
