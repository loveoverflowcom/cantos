package com.cantos.theatre.adapters.library

import com.cantos.theatre.adapters.library.wire.AudioPresentation
import com.cantos.theatre.adapters.library.wire.BookmarkRecord
import com.cantos.theatre.adapters.library.wire.BookmarkWrite
import com.cantos.theatre.adapters.library.wire.Completion
import com.cantos.theatre.adapters.library.wire.ContinueItem
import com.cantos.theatre.adapters.library.wire.HistoryItem
import com.cantos.theatre.adapters.library.wire.MutationState
import com.cantos.theatre.adapters.library.wire.ProgressRecord
import com.cantos.theatre.adapters.library.wire.ProgressWrite
import com.cantos.theatre.adapters.library.wire.Receipt
import com.cantos.theatre.adapters.library.wire.ResumeStatus
import com.cantos.theatre.adapters.library.wire.SavedRecord
import com.cantos.theatre.adapters.library.wire.SavedWrite
import com.cantos.theatre.adapters.library.wire.WorkPresentation
import com.cantos.theatre.adapters.library.wire.AudioIdentity as WireAudio
import com.cantos.theatre.domain.library.BookmarkEntry
import com.cantos.theatre.domain.library.BookmarkSync
import com.cantos.theatre.domain.library.ContinueEntry
import com.cantos.theatre.domain.library.HistoryEntry
import com.cantos.theatre.domain.library.LibraryConflict
import com.cantos.theatre.domain.library.LibraryPresentation
import com.cantos.theatre.domain.library.LibraryResume
import com.cantos.theatre.domain.library.LibrarySync
import com.cantos.theatre.domain.library.LibraryUnitInfo
import com.cantos.theatre.domain.library.LibraryWorkInfo
import com.cantos.theatre.domain.library.LibraryWorkKind
import com.cantos.theatre.domain.library.PausedCheckpoint
import com.cantos.theatre.domain.library.SavedEntry
import com.cantos.theatre.domain.playback.AudioIdentity
import com.cantos.theatre.domain.playback.isSupportedPlaybackSpeed
import com.cantos.theatre.ports.LocalLibraryIntent
import com.cantos.theatre.ports.LocalLibraryRecord

/** The Library rows a listener sees, projected from durable local records and the outbox. */
internal class LibraryRows(
    val saved: List<SavedEntry>,
    val continueListening: List<ContinueEntry>,
    val history: List<HistoryEntry>,
    val bookmarks: List<BookmarkEntry>,
    val conflicts: List<LibraryConflict>,
    val checkpoint: PausedCheckpoint?,
)

/** Local record resources that carry API metadata. Bodies are the exact validated wire JSON. */
internal object LibraryResources {
    const val Continue = "continue"
    const val Saved = "saved"
    const val History = "history"
    /**
     * Released metadata of one exact publication, keyed by publication ID: an AudioPresentation, or
     * the literal `null` the API sent. A different release has a different key, so it can never
     * label another release's progress; and a row outside the pages read so far keeps its label.
     */
    const val UnitPresentation = "unit_presentation"
    /** Per-Work presentation of a Saved row (the API's current representative), same encoding. */
    const val SavedPresentation = "saved_presentation"
    const val NullPresentation = "null"
    /** A bookmark the API confirmed (page row or accepted receipt): the exact BookmarkRecord, key = bookmark ID. */
    const val Bookmark = "bookmark"
    /** The API's resume status of a listed bookmark. Absent for a row only a receipt has confirmed. */
    const val BookmarkResume = "bookmark_resume"
    /** A deletion this account made or was told of. It outlives a stale page, so a deleted bookmark never returns. */
    const val BookmarkGone = "bookmark_gone"
}

private fun domain(audio: WireAudio) = AudioIdentity.of(audio.work_id, audio.unit_id, audio.publication_id, audio.rendition_id, audio.sha256, audio.duration_ms)

private fun resume(value: ResumeStatus) = when (value) {
    ResumeStatus.exact_available -> LibraryResume.ExactAvailable
    ResumeStatus.release_replaced -> LibraryResume.ReleaseReplaced
    ResumeStatus.access_unavailable -> LibraryResume.AccessUnavailable
}

private fun kind(value: String): LibraryWorkKind? = when (value) {
    "novel" -> LibraryWorkKind.Novel
    "story" -> LibraryWorkKind.Story
    "podcast" -> LibraryWorkKind.Podcast
    else -> null
}

internal fun WorkPresentation.toDomain(): LibraryWorkInfo? =
    kind(kind)?.let { LibraryWorkInfo(work_id, publication_id, title, it, language) }

private fun AudioPresentation.toDomain(): LibraryUnitInfo? =
    work.toDomain()?.let { LibraryUnitInfo(it, unit_id, unit_title) }

private fun workPresentationOf(record: LocalLibraryRecord?): LibraryPresentation<LibraryWorkInfo> = when {
    record == null -> LibraryPresentation.Unread
    record.body == LibraryResources.NullPresentation -> LibraryPresentation.Unavailable
    else -> LibraryCodec.decode<WorkPresentation>("WorkPresentation", record.body)?.toDomain()
        ?.let { LibraryPresentation.Available(it) } ?: LibraryPresentation.Unavailable
}

/** Stored metadata for exactly [audio]'s publication; one naming another unit or release claims nothing. */
private fun unitPresentationOf(record: LocalLibraryRecord?, audio: WireAudio): LibraryPresentation<LibraryUnitInfo> = when {
    record == null -> LibraryPresentation.Unread
    record.body == LibraryResources.NullPresentation -> LibraryPresentation.Unavailable
    else -> LibraryCodec.decode<AudioPresentation>("AudioPresentation", record.body)
        ?.takeIf { LibraryBinding.exact(audio, it) }?.toDomain()
        ?.let { LibraryPresentation.Available(it) } ?: LibraryPresentation.Unread
}

private class HistoryRow(val audio: WireAudio, val startedAtMs: Long, val resume: ResumeStatus, val order: Long)

/**
 * Pure projection. Server rows keep the API's newest-first order (descending accepted listener
 * order); a local change not yet confirmed has no server order and is newest. Metadata follows
 * the exact stored audio: a different publication never inherits a row's title or duration.
 */
internal fun projectLibraryRows(records: List<LocalLibraryRecord>, pending: List<LocalLibraryIntent>, offline: Boolean = false, record: (String, String) -> LocalLibraryRecord?): LibraryRows {
    val unconfirmed = Long.MAX_VALUE
    val savedRecords = records.filter { it.resource == LibraryResources.Saved }.mapNotNull { LibraryCodec.decode<SavedRecord>("SavedRecord", it.body) }.associateBy { it.work_id }.toMutableMap()
    records.filter { it.resource == "reconciled" && it.key.startsWith("saved:") }.mapNotNull { LibraryCodec.decode<SavedRecord>("SavedRecord", it.body) }
        .forEach { value -> if (value.work_id in savedRecords && value.revision >= (savedRecords[value.work_id]?.revision ?: -1)) savedRecords[value.work_id] = value }
    val savedViews = LinkedHashMap<String, Pair<Long, SavedEntry>>()
    val savedPresentations = records.filter { it.resource == LibraryResources.SavedPresentation }.associateBy { it.key }
    savedRecords.forEach { (work, value) ->
        val presentation = workPresentationOf(savedPresentations[work])
        savedViews[work] = value.listener_order to SavedEntry(work, value.saved, value.revision, LibrarySync.Confirmed, presentation)
    }
    pending.filter { it.key.startsWith("saved:") }.forEach { intent ->
        LibraryCodec.decode<SavedWrite>("SavedWrite", intent.body)?.let { value ->
            val work = intent.key.substringAfter(':')
            val previous = savedViews[work]?.second
            savedViews[work] = unconfirmed to SavedEntry(work, value.saved, value.base_revision, if (intent.failure == null) LibrarySync.Pending else LibrarySync.Failed,
                previous?.presentation ?: LibraryPresentation.Unread)
        }
    }
    val saved = savedViews.values.sortedWith(compareByDescending<Pair<Long, SavedEntry>> { it.first }.thenBy { it.second.workId }).map { it.second }

    val unitPresentations = records.filter { it.resource == LibraryResources.UnitPresentation }.associateBy { it.key }
    fun presentationFor(audio: AudioIdentity, wire: WireAudio) = unitPresentationOf(unitPresentations[audio.publicationId], wire)
    val admitted = records.filter { it.resource == LibraryResources.Continue }.mapNotNull { LibraryCodec.decode<ContinueItem>("ContinueItem", it.body) }
    val order = HashMap<String, Long>()
    val continues = LinkedHashMap<String, ContinueEntry>()
    admitted.forEach { item ->
        val audio = domain(item.progress.audio) ?: return@forEach
        order[audio.workId] = item.progress.listener_order
        continues[audio.workId] = ContinueEntry(audio, item.progress.position_ms, item.progress.revision, resume(item.resume), LibrarySync.Confirmed, presentationFor(audio, item.progress.audio))
    }
    pending.filter { it.key.startsWith("progress:") }.forEach { intent ->
        LibraryCodec.decode<ProgressWrite>("ProgressWrite", intent.body)?.let { value ->
            domain(value.audio)?.let { audio ->
                val same = continues[audio.workId]?.takeIf { it.audio == audio }
                order[audio.workId] = unconfirmed
                continues[audio.workId] = ContinueEntry(audio, value.position_ms, value.base_revision, same?.resume ?: LibraryResume.NeedsRefresh,
                    if (intent.failure == null) LibrarySync.Pending else LibrarySync.Failed, presentationFor(audio, value.audio))
            }
        }
    }
    // Continue is a projection of durable progress, including an acknowledged local register whose
    // subsequent page refresh failed. Match the server's newest-per-Work ordering; a progress
    // receipt itself does not establish present playback access.
    records.filter { it.resource == "reconciled" && it.key.startsWith("progress:") }
        .mapNotNull { LibraryCodec.decode<ProgressRecord>("ProgressRecord", it.body) }
        .sortedWith(compareByDescending<ProgressRecord> { it.listener_order }.thenBy { it.audio.publication_id })
        .distinctBy { it.audio.work_id }
        .forEach { value ->
            val audio = domain(value.audio) ?: return@forEach
            val previous = continues[audio.workId]
            val admittedItem = admitted.firstOrNull { it.progress.audio.work_id == audio.workId }?.progress
            val latest = admittedItem == null || value.listener_order > admittedItem.listener_order ||
                (value.listener_order == admittedItem.listener_order && value.audio.publication_id <= admittedItem.audio.publication_id)
            if (previous?.sync != LibrarySync.Pending && previous?.sync != LibrarySync.Failed &&
                pending.none { it.key == "progress:${audio.publicationId}" } && latest) {
                if (value.completion == Completion.completed) { continues.remove(audio.workId); order.remove(audio.workId) }
                else {
                    val same = previous?.takeIf { it.audio == audio }
                    order[audio.workId] = value.listener_order
                    continues[audio.workId] = ContinueEntry(audio, value.position_ms, value.revision, same?.resume ?: LibraryResume.NeedsRefresh,
                        LibrarySync.Confirmed, presentationFor(audio, value.audio))
                }
            }
        }
    val continueRows = continues.values.sortedWith(compareByDescending<ContinueEntry> { order[it.audio.workId] ?: unconfirmed }.thenBy { it.audio.workId })

    val history = records.filter { it.resource == LibraryResources.History }
        .mapNotNull { stored -> LibraryCodec.decode<HistoryItem>("HistoryItem", stored.body) }
        .sortedByDescending { it.listener_order }
        .mapNotNull { item -> domain(item.audio)?.let { HistoryEntry(it, item.started_at_ms, resume(item.resume), presentationFor(it, item.audio)) } }
    val checkpoint = record("checkpoint", "latest")?.let { LibraryCodec.decode<ProgressWrite>("ProgressWrite", it.body) }?.let { value ->
        domain(value.audio)?.let { PausedCheckpoint(it, value.position_ms, record("playback_speed", it.publicationId)?.body?.toIntOrNull()?.takeIf(::isSupportedPlaybackSpeed) ?: 1000) }
    }
    val conflicts = records.filter { it.resource == "conflict" && it.body.isNotEmpty() }.mapNotNull { stored ->
        val receipt = LibraryCodec.decode<Receipt>("Receipt", stored.body) ?: return@mapNotNull null
        val candidate = pending.firstOrNull { it.operation == receipt.operation_id }?.let { LibraryCodec.decode<ProgressWrite>("ProgressWrite", it.body) }
        val server = (receipt.state as? MutationState.Progress)?.current
        LibraryConflict(stored.key, receipt.operation_id, candidate?.position_ms, server?.position_ms, server?.revision ?: 0)
    }
    return LibraryRows(saved, continueRows, history, projectBookmarks(records, pending, offline), conflicts, checkpoint)
}

private fun bookmarkResume(body: String?): LibraryResume = when (body) {
    "exact_available" -> LibraryResume.ExactAvailable
    "release_replaced" -> LibraryResume.ReleaseReplaced
    "access_unavailable" -> LibraryResume.AccessUnavailable
    else -> LibraryResume.NeedsRefresh
}

/**
 * Bookmarks are server-confirmed rows overlaid with this account's undelivered intents, newest first. A
 * pending create shows at once, a pending delete hides its row, and a remembered deletion hides the row for good,
 * so neither a late page nor a replayed response brings a deleted bookmark back. A row keeps the exact audio it
 * was made on; its metadata is only that release's own, and nothing remaps it to another release.
 */
internal fun projectBookmarks(
    records: List<LocalLibraryRecord>, pending: List<LocalLibraryIntent>, offline: Boolean,
): List<BookmarkEntry> {
    val presentations = records.filter { it.resource == LibraryResources.UnitPresentation }.associateBy { it.key }
    val gone = records.filter { it.resource == LibraryResources.BookmarkGone }.map { it.key }.toSet()
    val resumes = records.filter { it.resource == LibraryResources.BookmarkResume }.associate { it.key to it.body }
    val rows = LinkedHashMap<String, Pair<Long, BookmarkEntry>>()
    fun entry(audio: WireAudio, id: String, position: Long, resume: LibraryResume, sync: BookmarkSync): BookmarkEntry? =
        domain(audio)?.takeIf { position in 0..it.durationMs }
            ?.let { BookmarkEntry(id, it, position, resume, sync, unitPresentationOf(presentations[it.publicationId], audio)) }
    records.filter { it.resource == LibraryResources.Bookmark }
        .mapNotNull { LibraryCodec.decode<BookmarkRecord>("BookmarkRecord", it.body) }
        .filter { !it.deleted && it.bookmark_id !in gone }
        .forEach { value ->
            entry(value.audio, value.bookmark_id, value.position_ms, bookmarkResume(resumes[value.bookmark_id]), BookmarkSync.Synced)
                ?.let { rows[value.bookmark_id] = value.listener_order to it }
        }
    val creates = pending.filter { it.key.startsWith("bookmark:") && it.method == "PUT" }
    creates.forEachIndexed { index, intent ->
        val write = LibraryCodec.decode<BookmarkWrite>("BookmarkWrite", intent.body) ?: return@forEachIndexed
        if (write.bookmark_id in gone || intent.key != "bookmark:${write.bookmark_id}") return@forEachIndexed
        val sync = when {
            intent.failure != null -> BookmarkSync.Failed
            offline -> BookmarkSync.Offline
            else -> BookmarkSync.Pending
        }
        val known = rows[write.bookmark_id]?.second
        entry(write.audio, write.bookmark_id, write.position_ms, known?.resume ?: LibraryResume.NeedsRefresh, sync)
            ?.let { rows[write.bookmark_id] = (Long.MAX_VALUE - (creates.size - index)) to it }
    }
    pending.filter { it.key.startsWith("bookmark:") && it.method == "POST" }.forEach { intent ->
        val id = intent.key.removePrefix("bookmark:")
        if (intent.failure == null) rows.remove(id)
        else rows[id]?.let { (order, row) -> rows[id] = order to row.copy(sync = BookmarkSync.DeleteFailed) }
    }
    return rows.values.sortedWith(compareByDescending<Pair<Long, BookmarkEntry>> { it.first }.thenBy { it.second.id }).map { it.second }
}
