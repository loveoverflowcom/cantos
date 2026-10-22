package com.cantos.theatre.adapters.library

import com.cantos.theatre.adapters.library.wire.AudioPresentation
import com.cantos.theatre.adapters.library.wire.BookmarkDelete
import com.cantos.theatre.adapters.library.wire.BookmarkRecord
import com.cantos.theatre.adapters.library.wire.BookmarkWrite
import com.cantos.theatre.adapters.library.wire.ContinueItem
import com.cantos.theatre.adapters.library.wire.HistoryClear
import com.cantos.theatre.adapters.library.wire.HistoryItem
import com.cantos.theatre.adapters.library.wire.HistoryStart
import com.cantos.theatre.adapters.library.wire.HistoryState
import com.cantos.theatre.adapters.library.LibraryCodec
import com.cantos.theatre.adapters.library.wire.MutationState
import com.cantos.theatre.adapters.library.wire.Operation
import com.cantos.theatre.adapters.library.wire.ProgressKind
import com.cantos.theatre.adapters.library.wire.ProgressRecord
import com.cantos.theatre.adapters.library.wire.ProgressWrite
import com.cantos.theatre.adapters.library.wire.Receipt
import com.cantos.theatre.adapters.library.wire.SavedRecord
import com.cantos.theatre.adapters.library.wire.SavedWrite
import com.cantos.theatre.adapters.library.wire.AudioIdentity as WireAudio
import com.cantos.theatre.domain.playback.AudioIdentity
import com.cantos.theatre.domain.playback.isSupportedPlaybackSpeed
import com.cantos.theatre.domain.library.BookmarkSync
import com.cantos.theatre.domain.library.LibraryLoad
import com.cantos.theatre.domain.library.LibraryReadFailure
import com.cantos.theatre.domain.library.LibrarySection
import com.cantos.theatre.domain.library.LibrarySectionState
import com.cantos.theatre.domain.library.LibraryState
import com.cantos.theatre.domain.library.ProgressIntent
import com.cantos.theatre.ports.LibraryLocalPort
import com.cantos.theatre.ports.LibraryTransaction
import com.cantos.theatre.ports.LocalLibraryIntent
import com.cantos.theatre.ports.LocalLibraryRecord
import com.cantos.theatre.ports.LibrarySessionPort
import com.cantos.theatre.application.library.NativeLibraryController
import com.cantos.theatre.ports.NativeSession
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.delay
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.longOrNull

/** One main-dispatcher owner; no credentials in SQLite and no playback effects from server reads. */
internal class NativeLibrarySync(
    private val scope: CoroutineScope,
    private val api: LibraryApi,
    private val store: LibraryLocalPort,
    private val newOperation: () -> String,
    private val onSessionInvalidated: (String) -> Unit = {},
    private val ownsAuthorization: (LibraryAuthorization) -> Boolean = { true },
    private val nowMs: () -> Long,
) : LibrarySessionPort {
    private val json = LibraryCodec.json
    private val mutableState = MutableStateFlow(LibraryState())
    override val state: StateFlow<LibraryState> = mutableState.asStateFlow()
    private var auth: LibraryAuthorization? = null
    private var generation = 0L
    private val jobs = mutableListOf<Job>()
    private var draining = false
    private var retryJob: Job? = null
    private var retryAttempt = 0
    private val reads = mutableMapOf<LibrarySection, Long>()
    private val pageCursors = mutableMapOf<LibrarySection, String?>()
    private var offline = false
    private var lastCheckpointAt = Long.MIN_VALUE
    private var lastCheckpointPublication: String? = null
    companion object { const val CHECKPOINT_INTERVAL_MS = 30_000L }
    override fun enter(session: NativeSession, bearer: String) {
        val candidate = LibraryAuthorization(session.actor, session.session, bearer)
        if (!ownsAuthorization(candidate)) { leave(); return }
        if (auth?.let { it.account == session.actor && it.session == session.session && it.bearer == bearer } == true) return
        leave()
        auth = candidate
        publish()
        refresh()
        drain()
    }
    override fun leave() {
        jobs.forEach { it.cancel() }; jobs.clear()
        auth = null; generation += 1; draining = false
        retryJob?.cancel(); retryJob = null; retryAttempt = 0; offline = false
        lastCheckpointAt = Long.MIN_VALUE; lastCheckpointPublication = null
        pageCursors.clear()
        mutableState.value = LibraryState()
    }
    private fun current(issued: Long, owner: LibraryAuthorization) =
        generation == issued && auth === owner && ownsAuthorization(owner)
    private fun liveOwner(): LibraryAuthorization? {
        val owner = auth ?: return null
        if (!ownsAuthorization(owner)) { leave(); return null }
        return owner
    }
    private fun wire(audio: AudioIdentity) = WireAudio(audio.workId, audio.unitId, audio.publicationId, audio.renditionId, audio.sha256, audio.durationMs)
    private fun operation(tx: LibraryTransaction, key: String): Operation {
        val (device, sequence) = tx.nextSequence(key, newOperation)
        val id = newOperation()
        check(Regex("[a-f0-9]{8}(-[a-f0-9]{4}){3}-[a-f0-9]{12}").matches(id))
        return Operation(id, device, sequence, nowMs().coerceAtLeast(0))
    }
    private fun <T> local(block: LibraryTransaction.() -> T): T? {
        val owner = liveOwner() ?: return null
        return try { store.transaction(owner.account, block) } catch (_: Exception) {
            mutableState.value = mutableState.value.copy(failure = "storage")
            null
        }
    }
    private fun historyState(tx: LibraryTransaction): HistoryState = tx.record("history_state", "history")?.let { LibraryCodec.decode<HistoryState>("HistoryState", it.body) } ?: HistoryState(0, 0)
    private fun reconciled(tx: LibraryTransaction, key: String) = tx.record("reconciled", key)?.revision ?: 0L
    override fun setSaved(workId: String, saved: Boolean) {
        if (!Regex("[a-f0-9]{8}(-[a-f0-9]{4}){3}-[a-f0-9]{12}").matches(workId)) return
        local {
            val key = "saved:$workId"
            retireRejected(this, key)
            put(LocalLibraryRecord("conflict", key, "", 0))
            val op = operation(this, key)
            val body = json.encodeToString(SavedWrite(op, reconciled(this, key), saved))
            enqueue(LocalLibraryIntent(op.operation_id, key, "PUT", "/library/saved/$workId", body))
        }
        publish(); drain()
    }
    /** Local-only native playback setting. It is never part of the progress mutation wire body. */
    override fun savePlaybackSettings(audio: AudioIdentity, speedPermille: Int) {
        if (!isSupportedPlaybackSpeed(speedPermille)) return
        local { put(LocalLibraryRecord("playback_speed", audio.publicationId, speedPermille.toString(), 0)) }
        publish()
    }
    /** Only call from acknowledged engine observations. Ordinary ticks are bounded to 30 seconds. */
    override fun progressCheckpoint(audio: AudioIdentity, positionMs: Long, kind: ProgressIntent, force: Boolean) {
        if (positionMs !in 0..audio.durationMs || (kind == ProgressIntent.Restart && positionMs != 0L) || (kind == ProgressIntent.MarkCompleted && positionMs != audio.durationMs)) return
        val now = nowMs()
        if (!force && kind == ProgressIntent.Checkpoint && lastCheckpointPublication == audio.publicationId && lastCheckpointAt != Long.MIN_VALUE && now >= lastCheckpointAt && now - lastCheckpointAt < CHECKPOINT_INTERVAL_MS) return
        val recorded = local {
            val key = "progress:${audio.publicationId}"
            val op = operation(this, key)
            val body = json.encodeToString(ProgressWrite(op, wire(audio), reconciled(this, key), positionMs, ProgressKind.valueOf(kind.wire)))
            enqueue(LocalLibraryIntent(op.operation_id, key, "PUT", "/progress/${audio.publicationId}", body))
            put(LocalLibraryRecord("checkpoint", "latest", body, 0))
            true
        } == true
        if (recorded) { lastCheckpointAt = now; lastCheckpointPublication = audio.publicationId }
        publish(); drain()
    }
    /** The caller must fence engine/source generation and call only after actual native start. */
    override fun engineStarted(audio: AudioIdentity, engineSession: String) {
        if (!Regex("[a-f0-9]{8}(-[a-f0-9]{4}){3}-[a-f0-9]{12}").matches(engineSession)) return
        local {
            if (record("engine_session", engineSession) != null) return@local
            val op = operation(this, "history")
            val body = json.encodeToString(HistoryStart(op, wire(audio), engineSession, "playback_started", historyState(this).epoch))
            enqueue(LocalLibraryIntent(op.operation_id, "history", "POST", "/library/history/playback-starts", body))
            put(LocalLibraryRecord("engine_session", engineSession, "started", 0))
        }
        publish(); drain()
    }
    /** Call only after explicit listener confirmation; leaves Saved and progress intact. */
    override fun clearHistory() {
        local {
            retireRejected(this, "history")
            put(LocalLibraryRecord("conflict", "history", "", 0))
            val history = historyState(this)
            val op = operation(this, "history")
            val body = json.encodeToString(HistoryClear(op, history.revision, history.epoch, "clear_history"))
            enqueue(LocalLibraryIntent(op.operation_id, "history", "POST", "/library/history/clear", body))
        }
        publish(); drain()
    }
    /**
     * The bookmark and its create intent are one local transaction: nothing waits for the network. The same release
     * position already bookmarked, confirmed or pending, is the same bookmark; a failed one is not, so it can be retried.
     */
    override fun createBookmark(audio: AudioIdentity, positionMs: Long): String? {
        if (positionMs !in 0..audio.durationMs) return null
        val id = local {
            val existing = projectBookmarks(records(), intents(), offline)
                .firstOrNull { it.audio.publicationId == audio.publicationId && it.positionMs == positionMs && it.sync != BookmarkSync.Failed }
            if (existing != null) return@local existing.id
            val fresh = newOperation()
            check(Regex("[a-f0-9]{8}(-[a-f0-9]{4}){3}-[a-f0-9]{12}").matches(fresh))
            val key = "bookmark:$fresh"
            val op = operation(this, key)
            val body = json.encodeToString(BookmarkWrite(op, fresh, wire(audio), positionMs))
            enqueue(LocalLibraryIntent(op.operation_id, key, "PUT", "/library/bookmarks/$fresh", body))
            fresh
        }
        publish(); drain()
        return id
    }
    override fun deleteBookmark(id: String) {
        if (!Regex("[a-f0-9]{8}(-[a-f0-9]{4}){3}-[a-f0-9]{12}").matches(id)) return
        local {
            val key = "bookmark:$id"
            // A failed intent of this bookmark is the listener's to discard or retry; it must not wedge the key.
            val failedCreate = intents().any { it.key == key && it.method == "PUT" && it.failure != null }
            intents().filter { it.key == key && it.failure != null }.forEach { retire(this, it) }
            val live = intents().filter { it.key == key }
            if (live.any { it.method == "POST" }) return@local
            val unsentCreate = live.firstOrNull { it.method == "PUT" && !it.sent }
            val serverKnows = record(LibraryResources.Bookmark, id) != null || live.any { it.method == "PUT" && it.sent }
            when {
                // Never sent: the server cannot know it, so cancelling is the whole deletion.
                unsentCreate != null -> { retire(this, unsentCreate); markBookmarkGone(this, id) }
                // Refused for good: the server never accepted it, so discarding is the whole deletion.
                !serverKnows && failedCreate -> markBookmarkGone(this, id)
                !serverKnows -> Unit
                else -> {
                    val op = operation(this, key)
                    enqueue(LocalLibraryIntent(op.operation_id, key, "POST", "/library/bookmarks/$id/delete", json.encodeToString(BookmarkDelete(op))))
                }
            }
        }
        publish(); drain()
    }
    private fun markBookmarkGone(tx: LibraryTransaction, id: String, order: Long = 0) {
        tx.put(LocalLibraryRecord(LibraryResources.BookmarkGone, id, "", order))
        tx.delete(LibraryResources.Bookmark, id)
        tx.delete(LibraryResources.BookmarkResume, id)
    }
    override fun refresh() { mutableState.value = mutableState.value.copy(failure = null); LibrarySection.entries.forEach { loadPage(it, false) } }
    override fun refresh(section: LibrarySection) { loadPage(section, false) }
    override fun nextPage(section: LibrarySection) { loadPage(section, true) }
    private fun updateSection(section: LibrarySection, change: (LibrarySectionState) -> LibrarySectionState) {
        val state = mutableState.value
        mutableState.value = state.copy(sections = state.sections + (section to change(state.section(section))))
    }
    private fun loadPage(section: LibrarySection, next: Boolean) {
        val owner = liveOwner() ?: return
        val issued = generation
        // A next-page tap while this section is already reading is the same request, not a second one.
        if (next && mutableState.value.section(section).loading != LibraryLoad.Idle) return
        val cursor = if (next) mutableState.value.cursors[section] ?: return else null
        val read = (reads[section] ?: 0L) + 1
        reads[section] = read
        fun accepts() = current(issued, owner) && reads[section] == read
        jobs += scope.launch {
            if (!accepts()) return@launch
            updateSection(section) { it.copy(loading = if (next) LibraryLoad.NextPage else LibraryLoad.Refreshing, failure = null) }
            val failed: LibraryResult.Failed? = when (section) {
                LibrarySection.Continue -> when (val result = api.continuePage(owner, cursor)) {
                    is LibraryResult.Failed -> result
                    is LibraryResult.Value -> if (accepts()) apply(section, next, result.value.next_cursor) {
                        if (!next) deleteResource(LibraryResources.Continue)
                        result.value.items.forEach { item ->
                            put(LocalLibraryRecord(LibraryResources.Continue, item.progress.audio.work_id, json.encodeToString(ContinueItem(item.progress, item.resume)), item.progress.revision))
                            putUnitPresentation(this, item.progress.audio.publication_id, item.presentation)
                            adoptProgress(this, item.progress)
                        }
                        if (!next) pruneUnitPresentations(this)
                    } else null
                }
                LibrarySection.Saved -> when (val result = api.savedPage(owner, cursor)) {
                    is LibraryResult.Failed -> result
                    is LibraryResult.Value -> if (accepts()) apply(section, next, result.value.next_cursor) {
                        if (!next) { deleteResource(LibraryResources.Saved); deleteResource(LibraryResources.SavedPresentation) }
                        result.value.items.forEach { item ->
                            adoptSaved(this, item.saved)
                            put(LocalLibraryRecord(LibraryResources.Saved, item.saved.work_id, json.encodeToString(item.saved), item.saved.revision))
                            put(LocalLibraryRecord(LibraryResources.SavedPresentation, item.saved.work_id,
                                item.presentation?.let { json.encodeToString(it) } ?: LibraryResources.NullPresentation, item.saved.revision))
                        }
                    } else null
                }
                LibrarySection.Bookmarks -> when (val result = api.bookmarkPage(owner, cursor)) {
                    is LibraryResult.Failed -> result
                    is LibraryResult.Value -> if (accepts()) apply(section, next, result.value.next_cursor) {
                        if (!next) { deleteResource(LibraryResources.Bookmark); deleteResource(LibraryResources.BookmarkResume) }
                        result.value.items.forEach { item ->
                            val id = item.bookmark.bookmark_id
                            // This account already deleted it: a stale page never brings it back.
                            if (record(LibraryResources.BookmarkGone, id) != null) return@forEach
                            put(LocalLibraryRecord(LibraryResources.Bookmark, id, json.encodeToString(item.bookmark), item.bookmark.listener_order))
                            put(LocalLibraryRecord(LibraryResources.BookmarkResume, id, item.resume.name, 0))
                            putUnitPresentation(this, item.bookmark.audio.publication_id, item.presentation)
                        }
                        if (!next) pruneUnitPresentations(this)
                    } else null
                }
                LibrarySection.History -> when (val result = api.historyPage(owner, cursor)) {
                    is LibraryResult.Failed -> result
                    is LibraryResult.Value -> if (accepts()) apply(section, next, result.value.next_cursor) {
                        if (!next) deleteResource(LibraryResources.History)
                        result.value.items.forEach { item ->
                            put(LocalLibraryRecord(LibraryResources.History, item.audio.work_id,
                                json.encodeToString(HistoryItem(item.audio, item.started_at_ms, item.listener_order, item.progress, item.resume)), item.listener_order))
                            putUnitPresentation(this, item.audio.publication_id, item.presentation)
                            item.progress?.let { adoptProgress(this, it) }
                        }
                        if (!next) pruneUnitPresentations(this)
                        val old = historyState(this)
                        if (result.value.state.revision >= old.revision) put(LocalLibraryRecord("history_state", "history", json.encodeToString(result.value.state), result.value.state.revision))
                    } else null
                }
            }
            if (!accepts()) return@launch
            offline = failed != null && failed.retryable && failed.code != "storage"
            if (failed != null) {
                failedRead(issued, owner, section, next, failed)
                // An expired cursor is the server's contract; the first page is the only honest recovery.
                if (next && failed.code == "invalid_cursor" && accepts()) loadPage(section, false)
            }
            publish()
        }
    }
    /** The API's answer for one exact publication, null included, replaces any earlier answer for it. */
    private fun putUnitPresentation(tx: LibraryTransaction, publication: String, presentation: AudioPresentation?) {
        tx.put(LocalLibraryRecord(LibraryResources.UnitPresentation, publication, presentation?.let { json.encodeToString(it) } ?: LibraryResources.NullPresentation, 0))
    }
    /**
     * A first-page read replaces its rows, so metadata no stored row, History entry or acknowledged
     * register still names is dropped; what remains is bounded by what the account can display.
     */
    private fun pruneUnitPresentations(tx: LibraryTransaction) {
        val records = tx.records()
        val live = HashSet<String>()
        records.forEach { stored ->
            when (stored.resource) {
                LibraryResources.Continue -> LibraryCodec.decode<ContinueItem>("ContinueItem", stored.body)?.let { live += it.progress.audio.publication_id }
                LibraryResources.History -> LibraryCodec.decode<HistoryItem>("HistoryItem", stored.body)?.let { live += it.audio.publication_id }
                LibraryResources.Bookmark -> LibraryCodec.decode<BookmarkRecord>("BookmarkRecord", stored.body)?.let { live += it.audio.publication_id }
                "reconciled" -> if (stored.key.startsWith("progress:")) live += stored.key.removePrefix("progress:")
            }
        }
        val kept = records.filter { it.resource == LibraryResources.UnitPresentation && it.key in live }
        tx.deleteResource(LibraryResources.UnitPresentation)
        kept.forEach { tx.put(it) }
    }
    /** Applies one verified page in a single local transaction; its failure is the section's, rows stay. */
    private fun apply(section: LibrarySection, next: Boolean, cursor: String?, write: LibraryTransaction.() -> Unit): LibraryResult.Failed? {
        val stored = local { write(); true } == true
        if (!stored) return LibraryResult.Failed("storage", true)
        pageCursors[section] = cursor
        updateSection(section) { it.copy(loaded = true, loading = LibraryLoad.Idle, failure = null) }
        return null
    }
    private fun failedRead(issued: Long, owner: LibraryAuthorization, section: LibrarySection, next: Boolean, failure: LibraryResult.Failed) {
        if (!current(issued, owner)) return
        if (failure.code == "unauthenticated" || failure.code == "session_changed") invalidateSession(owner)
        else updateSection(section) { it.copy(loading = LibraryLoad.Idle, failure = LibraryReadFailure(failure.code, failure.retryable, next)) }
    }
    /** One-record reads (Saved register, progress) carry no section state; only authority loss is acted on. */
    private fun failedSingleRead(issued: Long, owner: LibraryAuthorization, failure: LibraryResult.Failed) {
        if (!current(issued, owner)) return
        if (failure.code == "unauthenticated" || failure.code == "session_changed") invalidateSession(owner)
        else mutableState.value = mutableState.value.copy(failure = failure.code)
    }
    private fun invalidateSession(owner: LibraryAuthorization) {
        if (auth !== owner) return
        leave() // Hide projections and cancel/fence effects before authority is revalidated.
        onSessionInvalidated(owner.session)
    }
    override fun refreshSaved(workId: String) {
        val owner = liveOwner() ?: return; val issued = generation
        jobs += scope.launch {
            if (!current(issued, owner)) return@launch
            when (val result = api.saved(owner, workId)) {
                is LibraryResult.Failed -> failedSingleRead(issued, owner, result)
                is LibraryResult.Value -> if (current(issued, owner)) {
                    local { adoptSaved(this, result.value); put(LocalLibraryRecord("saved", workId, json.encodeToString(result.value), result.value.revision)) }
                    publish()
                }
            }
        }
    }
    override fun refreshProgress(publicationId: String) {
        val owner = liveOwner() ?: return; val issued = generation
        jobs += scope.launch {
            if (!current(issued, owner)) return@launch
            when (val result = api.progress(owner, publicationId)) {
                is LibraryResult.Failed -> failedSingleRead(issued, owner, result)
                is LibraryResult.Value -> if (current(issued, owner)) {
                    result.value.current?.let { record -> local { adoptProgress(this, record) } }
                    publish()
                }
            }
        }
    }
    private fun adoptSaved(tx: LibraryTransaction, value: SavedRecord, receipt: Boolean = false) {
        val key = "saved:${value.work_id}"
        if (value.revision < (tx.record("reconciled", key)?.revision ?: 0)) return
        if (receipt || tx.intents().none { it.key == key }) tx.put(LocalLibraryRecord("reconciled", key, json.encodeToString(value), value.revision))
    }
    private fun adoptProgress(tx: LibraryTransaction, value: ProgressRecord, receipt: Boolean = false) {
        val key = "progress:${value.audio.publication_id}"
        if (value.revision < (tx.record("reconciled", key)?.revision ?: 0)) return
        if (receipt || tx.intents().none { it.key == key }) tx.put(LocalLibraryRecord("reconciled", key, json.encodeToString(value), value.revision))
    }
    /** At most one dispatch loop; sent rows always read receipt before byte-identical replay. */
    override fun drain() {
        // An external opportunity after exhaustion opens a fresh bounded episode. Duplicate
        // signals during a drain share that drain; an existing backoff keeps its spent budget.
        if (!draining && retryJob == null) retryAttempt = 0
        drainPending()
    }
    private fun drainPending() {
        val owner = liveOwner() ?: return
        if (draining) return
        retryJob?.cancel(); retryJob = null
        val issued = generation; draining = true
        jobs += scope.launch {
            var retryNeeded = false
            var continueBatch = false
            try {
                var budget = 20
                var resolved = false
                val blocked = mutableSetOf<String>()
                while (current(issued, owner) && budget-- > 0) {
                    val next = local { intents().firstOrNull { it.failure == null && it.key !in blocked && intents().none { older -> older.key == it.key && older.failure != null } } } ?: break
                    val frozen = if (next.sent) next else local {
                        val value = json.parseToJsonElement(next.body).jsonObject
                        val base = value["base_revision"]?.jsonPrimitive?.longOrNull
                        val ownAccepted = record("own_ack", next.key)?.revision
                        val mayChainOwnAcknowledgement = next.key.startsWith("saved:") || value["kind"]?.jsonPrimitive?.content == "checkpoint"
                        // Only our accepted FIFO predecessor may advance an unsent checkpoint or
                        // Saved desire. A GET, already-current or conflict receipt never advances
                        // this watermark. Explicit seeks, completion/restart and choices stay pinned.
                        val body = if (mayChainOwnAcknowledgement && base != null && ownAccepted != null && ownAccepted > base) JsonObject(value + ("base_revision" to JsonPrimitive(ownAccepted))).toString() else next.body
                        freeze(next.operation, body)
                        LocalLibraryIntent(next.operation, next.key, next.method, next.path, body, true)
                    } ?: break
                    val result = if (next.sent) when (val receipt = api.receipt(owner, frozen.operation)) {
                        is LibraryResult.Value -> receipt
                        is LibraryResult.Failed -> if (receipt.code == "not_found") api.mutate(owner, frozen) else receipt
                    } else api.mutate(owner, frozen)
                    if (!current(issued, owner)) break
                    when (result) {
                        is LibraryResult.Failed -> {
                            if (result.code == "unauthenticated" || result.code == "session_changed") {
                                invalidateSession(owner)
                                break
                            }
                            offline = result.retryable
                            if (result.code == "not_found" && frozen.method == "POST" && frozen.key.startsWith("bookmark:")) {
                                // The server never accepted this bookmark, so its deletion is already true.
                                local { markBookmarkGone(this, frozen.key.removePrefix("bookmark:")); retire(this, frozen) }
                                resolved = true
                            } else {
                                if (!result.retryable && !result.unknownOutcome) local {
                                    fail(frozen.operation, result.code)
                                    put(LocalLibraryRecord("receipt_archive", frozen.operation, json.encodeToString(com.cantos.theatre.adapters.library.wire.LibraryError(result.code, false)), 0))
                                    // A create the server refused for good, followed by this account's own deletion of it:
                                    // nothing was ever accepted, so both end here and the bookmark stays gone.
                                    if (frozen.method == "PUT" && frozen.key.startsWith("bookmark:") && intents().any { it.key == frozen.key && it.method == "POST" }) {
                                        intents().filter { it.key == frozen.key }.forEach { retire(this, it) }
                                        markBookmarkGone(this, frozen.key.removePrefix("bookmark:"))
                                    }
                                }
                                else retryNeeded = true
                                blocked += frozen.key
                            }
                        }
                        is LibraryResult.Value -> {
                            val receipt = result.value
                            offline = false
                            if (receipt.operation_id != frozen.operation || !receiptMatches(frozen, receipt.state)) { local { fail(frozen.operation, "protocol") }; blocked += frozen.key }
                            else { local { reconcile(this, frozen, receipt) }; resolved = true }
                        }
                    }
                    publish()
                }
                if (current(issued, owner) && budget < 0) continueBatch = local { intents().any { it.failure == null && it.key !in blocked } } == true
                if (resolved && current(issued, owner)) refresh()
            } finally {
                if (current(issued, owner)) {
                    draining = false
                    if (continueBatch) {
                        retryJob = scope.launch { delay(250); retryJob = null; if (current(issued, owner)) drainPending() }
                        jobs += requireNotNull(retryJob)
                    } else if (retryNeeded && retryAttempt < 6) {
                        val waitMs = (2_000L shl retryAttempt) + (nowMs().coerceAtLeast(0) % 500)
                        retryAttempt += 1
                        retryJob = scope.launch { delay(waitMs); retryJob = null; if (current(issued, owner)) drainPending() }
                        jobs += requireNotNull(retryJob)
                    } else if (!retryNeeded) retryAttempt = 0
                }
            }
        }
    }
    private fun receiptMatches(intent: LocalLibraryIntent, state: MutationState): Boolean = when (state) {
        is MutationState.Progress -> intent.key.startsWith("progress:") && (state.current == null || (intent.key == "progress:${state.current.audio.publication_id}" && LibraryCodec.decode<ProgressWrite>("ProgressWrite", intent.body)?.audio == state.current.audio))
        is MutationState.Saved -> intent.key == "saved:${state.current.work_id}"
        is MutationState.History -> intent.key == "history"
        is MutationState.Bookmark -> intent.key == "bookmark:${state.current.bookmark_id}"
    }
    private fun reconcile(tx: LibraryTransaction, intent: LocalLibraryIntent, receipt: Receipt) {
        tx.put(LocalLibraryRecord("receipt_archive", intent.operation, json.encodeToString(receipt), 0))
        when (val value = receipt.state) {
            is MutationState.Saved -> {
                if (receipt.outcome == "accepted") tx.put(LocalLibraryRecord("own_ack", intent.key, intent.operation, value.current.revision))
                adoptSaved(tx, value.current, true); tx.put(LocalLibraryRecord("saved", value.current.work_id, json.encodeToString(value.current), value.current.revision)) }
            is MutationState.Progress -> value.current?.let {
                if (receipt.outcome == "accepted") tx.put(LocalLibraryRecord("own_ack", intent.key, intent.operation, it.revision))
                adoptProgress(tx, it, true)
            }
            is MutationState.Bookmark -> {
                val record = value.current
                if (record.deleted) markBookmarkGone(tx, record.bookmark_id, record.listener_order)
                else if (tx.record(LibraryResources.BookmarkGone, record.bookmark_id) == null) {
                    tx.put(LocalLibraryRecord(LibraryResources.Bookmark, record.bookmark_id, json.encodeToString(record), record.listener_order))
                }
            }
            is MutationState.History -> {
                if (value.current.revision >= historyState(tx).revision) tx.put(LocalLibraryRecord("history_state", "history", json.encodeToString(value.current), value.current.revision))
                if (receipt.outcome == "accepted" && intent.path == "/library/history/clear") tx.deleteResource("history")
            }
        }
        if (receipt.outcome == "revision_conflict" && receipt.resolution == "listener_chooses") {
            tx.fail(intent.operation, "listener_chooses")
            tx.put(LocalLibraryRecord("conflict", intent.key, json.encodeToString(receipt), 0))
        } else retire(tx, intent)
    }
    private fun retire(tx: LibraryTransaction, intent: LocalLibraryIntent) {
        tx.put(LocalLibraryRecord("intent_archive", intent.operation, intent.body, 0))
        tx.resolve(intent.operation)
    }
    private fun retireRejected(tx: LibraryTransaction, key: String) {
        val terminal = setOf("listener_chooses", "invalid_request", "invalid_position", "idempotency_key_reused", "stale_device_sequence", "revision_exhausted", "corrupt_state", "not_found", "unauthenticated", "denied")
        tx.intents().filter { it.key == key && it.failure in terminal }.forEach { retire(tx, it) }
    }
    /** Explicit conflict choice creates a new intent; never seeks or starts the engine itself. */
    override fun resolveProgress(publicationId: String, useLocal: Boolean) {
        local {
            val key = "progress:$publicationId"
            val receipt = record("conflict", key)?.let { LibraryCodec.decode<Receipt>("Receipt", it.body) } ?: return@local
            val server = (receipt.state as? MutationState.Progress)?.current ?: return@local
            val previous = intents().firstOrNull { it.operation == receipt.operation_id } ?: return@local
            val candidate = LibraryCodec.decode<ProgressWrite>("ProgressWrite", previous.body) ?: return@local
            retire(this, previous)
            val op = operation(this, key)
            val write = candidate.copy(operation = op, base_revision = server.revision, position_ms = if (useLocal) candidate.position_ms else server.position_ms, kind = if (useLocal) ProgressKind.resolve_here else ProgressKind.resolve_other)
            enqueue(LocalLibraryIntent(op.operation_id, key, "PUT", previous.path, json.encodeToString(write)))
            put(LocalLibraryRecord("conflict", key, "", 0))
        }
        publish(); drain()
    }
    private fun publish() {
        val owner = liveOwner() ?: return
        val (projected, pending) = local {
            val outbox = intents()
            projectLibraryRows(records(), outbox, offline) { resource, key -> record(resource, key) } to outbox
        } ?: return
        val previous = mutableState.value
        mutableState.value = LibraryState(owner.account, projected.saved, projected.continueListening, projected.history, projected.bookmarks,
            pageCursors.toMap(), previous.sections,
            pending.count { it.failure == null }, pending.mapNotNull { it.failure?.let { reason -> it.operation to reason } }.toMap(), projected.conflicts, projected.checkpoint,
            previous.failure, offline)
    }
    override fun close() { leave(); store.close() }
}

/** Native composition constructs the transport/SQLite shell behind the domain-only session port.
 * Production supplies [ownsAuthorization] from the process identity owner to fence same-turn
 * logout. Its permissive default is for isolated storage/controller fixtures without identity. */
fun nativeLibraryController(
    scope: CoroutineScope,
    api: LibraryApi,
    store: LibraryLocalPort,
    newOperation: () -> String,
    onSessionInvalidated: (String) -> Unit = {},
    ownsAuthorization: (LibraryAuthorization) -> Boolean = { true },
    nowMs: () -> Long,
): NativeLibraryController = NativeLibraryController(
    NativeLibrarySync(scope, api, store, newOperation, onSessionInvalidated, ownsAuthorization, nowMs),
)
