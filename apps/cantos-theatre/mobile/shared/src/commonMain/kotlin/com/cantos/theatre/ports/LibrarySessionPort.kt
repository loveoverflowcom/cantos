package com.cantos.theatre.ports

import com.cantos.theatre.domain.library.LibrarySection
import com.cantos.theatre.domain.library.LibraryState
import com.cantos.theatre.domain.library.ProgressIntent
import com.cantos.theatre.domain.playback.AudioIdentity
import kotlinx.coroutines.flow.StateFlow

/** Account-fenced native Library capability. Wire DTOs, JSON and SQL stay in its adapter. */
interface LibrarySessionPort {
    val state: StateFlow<LibraryState>
    fun enter(session: NativeSession, bearer: String)
    fun leave()
    fun setSaved(workId: String, saved: Boolean)
    fun savePlaybackSettings(audio: AudioIdentity, speedPermille: Int)
    fun progressCheckpoint(audio: AudioIdentity, positionMs: Long, kind: ProgressIntent = ProgressIntent.Checkpoint, force: Boolean = false)
    fun engineStarted(audio: AudioIdentity, engineSession: String)
    fun clearHistory()
    /**
     * Records one release-bound bookmark at [positionMs] of exactly [audio], durably, before any network. The caller
     * takes both from one snapshot the player owner confirmed. Returns the bookmark's ID, or the existing one when
     * the same release position is already bookmarked; null when nothing could be recorded.
     */
    fun createBookmark(audio: AudioIdentity, positionMs: Long): String?
    /** Deletes (or, for a bookmark the server never accepted, discards) one bookmark; it never returns. */
    fun deleteBookmark(id: String)
    fun refresh()
    /** Re-reads one section from its first page; the other sections and their rows are untouched. */
    fun refresh(section: LibrarySection)
    fun nextPage(section: LibrarySection)
    fun refreshSaved(workId: String)
    fun refreshProgress(publicationId: String)
    fun drain()
    fun resolveProgress(publicationId: String, useLocal: Boolean)
    fun close()
}
