package com.cantos.theatre.domain.library

import com.cantos.theatre.domain.playback.AudioIdentity

enum class LibrarySection(val path: String) { Continue("continue"), Saved("saved"), History("history") }
enum class LibrarySync { Pending, Confirmed, Failed }
enum class LibraryResume { ExactAvailable, ReleaseReplaced, AccessUnavailable, NeedsRefresh }
enum class ProgressIntent(val wire: String) { Checkpoint("checkpoint"), ExplicitSeek("explicit_seek"), Restart("restart"), ResolveHere("resolve_here"), ResolveOther("resolve_other"), MarkCompleted("mark_completed") }
data class SavedEntry(val workId: String, val saved: Boolean, val revision: Long, val sync: LibrarySync)
data class ContinueEntry(val audio: AudioIdentity, val positionMs: Long, val revision: Long, val resume: LibraryResume, val sync: LibrarySync)
data class HistoryEntry(val audio: AudioIdentity, val startedAtMs: Long, val resume: LibraryResume)
data class PausedCheckpoint(val audio: AudioIdentity, val positionMs: Long, val speedPermille: Int = 1000)
data class LibraryConflict(val resourceKey: String, val operation: String, val localPositionMs: Long?, val serverPositionMs: Long?, val serverRevision: Long)
data class LibraryState(
    val account: String? = null,
    val saved: List<SavedEntry> = emptyList(),
    val continueListening: List<ContinueEntry> = emptyList(),
    val history: List<HistoryEntry> = emptyList(),
    val cursors: Map<LibrarySection, String?> = emptyMap(),
    val pending: Int = 0,
    val failures: Map<String, String> = emptyMap(),
    val conflicts: List<LibraryConflict> = emptyList(),
    val restoredPaused: PausedCheckpoint? = null,
    val refreshing: Boolean = false,
    val failure: String? = null,
)
