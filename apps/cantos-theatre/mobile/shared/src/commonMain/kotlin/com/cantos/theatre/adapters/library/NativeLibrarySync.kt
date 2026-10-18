package com.cantos.theatre.adapters.library

import com.cantos.theatre.adapters.library.wire.Completion
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
import com.cantos.theatre.adapters.library.wire.ResumeStatus
import com.cantos.theatre.adapters.library.wire.SavedRecord
import com.cantos.theatre.adapters.library.wire.SavedWrite
import com.cantos.theatre.adapters.library.wire.AudioIdentity as WireAudio
import com.cantos.theatre.domain.playback.AudioIdentity
import com.cantos.theatre.domain.playback.isSupportedPlaybackSpeed
import com.cantos.theatre.domain.library.ContinueEntry
import com.cantos.theatre.domain.library.HistoryEntry
import com.cantos.theatre.domain.library.LibraryConflict
import com.cantos.theatre.domain.library.LibraryResume
import com.cantos.theatre.domain.library.LibrarySection
import com.cantos.theatre.domain.library.LibraryState
import com.cantos.theatre.domain.library.LibrarySync
import com.cantos.theatre.domain.library.PausedCheckpoint
import com.cantos.theatre.domain.library.ProgressIntent
import com.cantos.theatre.domain.library.SavedEntry
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
    private var lastCheckpointAt = Long.MIN_VALUE
    private var lastCheckpointPublication: String? = null
    companion object { const val CHECKPOINT_INTERVAL_MS = 30_000L }
    override fun enter(session: NativeSession, bearer: String) {
        if (auth?.let { it.account == session.actor && it.session == session.session && it.bearer == bearer } == true) return
        leave()
        auth = LibraryAuthorization(session.actor, session.session, bearer)
        publish()
        refresh()
        drain()
    }
    override fun leave() {
        jobs.forEach { it.cancel() }; jobs.clear()
        auth = null; generation += 1; draining = false
        retryJob?.cancel(); retryJob = null; retryAttempt = 0
        lastCheckpointAt = Long.MIN_VALUE; lastCheckpointPublication = null
        pageCursors.clear()
        mutableState.value = LibraryState()
    }
    private fun current(issued: Long, owner: LibraryAuthorization) = generation == issued && auth === owner
    private fun wire(audio: AudioIdentity) = WireAudio(audio.workId, audio.unitId, audio.publicationId, audio.renditionId, audio.sha256, audio.durationMs)
    private fun domain(audio: WireAudio) = AudioIdentity.of(audio.work_id, audio.unit_id, audio.publication_id, audio.rendition_id, audio.sha256, audio.duration_ms)
    private fun resume(value: ResumeStatus) = when (value) { ResumeStatus.exact_available -> LibraryResume.ExactAvailable; ResumeStatus.release_replaced -> LibraryResume.ReleaseReplaced; ResumeStatus.access_unavailable -> LibraryResume.AccessUnavailable }
    private fun operation(tx: LibraryTransaction, key: String): Operation {
        val (device, sequence) = tx.nextSequence(key, newOperation)
        val id = newOperation()
        check(Regex("[a-f0-9]{8}(-[a-f0-9]{4}){3}-[a-f0-9]{12}").matches(id))
        return Operation(id, device, sequence, nowMs().coerceAtLeast(0))
    }
    private fun <T> local(block: LibraryTransaction.() -> T): T? {
        val owner = auth ?: return null
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
    override fun refresh() { mutableState.value = mutableState.value.copy(failure = null); LibrarySection.entries.forEach { loadPage(it, false) } }
    override fun nextPage(section: LibrarySection) { loadPage(section, true) }
    private fun loadPage(section: LibrarySection, next: Boolean) {
        val owner = auth ?: return
        val issued = generation
        val read = (reads[section] ?: 0L) + 1
        reads[section] = read
        fun accepts() = current(issued, owner) && reads[section] == read
        val cursor = if (next) mutableState.value.cursors[section] ?: return else null
        jobs += scope.launch {
            if (accepts()) mutableState.value = mutableState.value.copy(refreshing = true)
            when (section) {
                LibrarySection.Continue -> when (val result = api.continuePage(owner, cursor)) {
                    is LibraryResult.Failed -> if (accepts()) { failedRead(issued, owner, result); if (next && result.code == "invalid_cursor") loadPage(section, false) }
                    is LibraryResult.Value -> if (accepts()) local {
                        if (!next) deleteResource("continue")
                        result.value.items.forEach { item ->
                            put(LocalLibraryRecord("continue", item.progress.audio.work_id, json.encodeToString(item), item.progress.revision))
                            adoptProgress(this, item.progress)
                        }
                        putCursor(section, result.value.next_cursor)
                    }
                }
                LibrarySection.Saved -> when (val result = api.savedPage(owner, cursor)) {
                    is LibraryResult.Failed -> if (accepts()) { failedRead(issued, owner, result); if (next && result.code == "invalid_cursor") loadPage(section, false) }
                    is LibraryResult.Value -> if (accepts()) local {
                        if (!next) deleteResource("saved")
                        result.value.items.forEach { item -> adoptSaved(this, item.saved); put(LocalLibraryRecord("saved", item.saved.work_id, json.encodeToString(item.saved), item.saved.revision)) }
                        putCursor(section, result.value.next_cursor)
                    }
                }
                LibrarySection.History -> when (val result = api.historyPage(owner, cursor)) {
                    is LibraryResult.Failed -> if (accepts()) { failedRead(issued, owner, result); if (next && result.code == "invalid_cursor") loadPage(section, false) }
                    is LibraryResult.Value -> if (accepts()) local {
                        if (!next) deleteResource("history")
                        result.value.items.forEach { item -> put(LocalLibraryRecord("history", item.audio.work_id, json.encodeToString(item), item.listener_order)); item.progress?.let { adoptProgress(this, it) } }
                        val old = historyState(this)
                        if (result.value.state.revision >= old.revision) put(LocalLibraryRecord("history_state", "history", json.encodeToString(result.value.state), result.value.state.revision))
                        putCursor(section, result.value.next_cursor)
                    }
                }
            }
            if (accepts()) { publish(); mutableState.value = mutableState.value.copy(refreshing = false) }
        }
    }
    private fun LibraryTransaction.putCursor(section: LibrarySection, cursor: String?) { pageCursors[section] = cursor }
    private fun failedRead(issued: Long, owner: LibraryAuthorization, failure: LibraryResult.Failed) {
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
        val owner = auth ?: return; val issued = generation
        jobs += scope.launch { when (val result = api.saved(owner, workId)) {
            is LibraryResult.Failed -> failedRead(issued, owner, result)
            is LibraryResult.Value -> if (current(issued, owner)) { local { adoptSaved(this, result.value); put(LocalLibraryRecord("saved", workId, json.encodeToString(result.value), result.value.revision)) }; publish() }
        } }
    }
    override fun refreshProgress(publicationId: String) {
        val owner = auth ?: return; val issued = generation
        jobs += scope.launch { when (val result = api.progress(owner, publicationId)) {
            is LibraryResult.Failed -> failedRead(issued, owner, result)
            is LibraryResult.Value -> if (current(issued, owner)) { result.value.current?.let { record -> local { adoptProgress(this, record) } }; publish() }
        } }
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
        val owner = auth ?: return
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
                            if (!result.retryable && !result.unknownOutcome) local {
                                fail(frozen.operation, result.code)
                                put(LocalLibraryRecord("receipt_archive", frozen.operation, json.encodeToString(com.cantos.theatre.adapters.library.wire.LibraryError(result.code, false)), 0))
                            }
                            else retryNeeded = true
                            blocked += frozen.key
                        }
                        is LibraryResult.Value -> {
                            val receipt = result.value
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
                        retryJob = scope.launch { delay(250); retryJob = null; if (current(issued, owner)) drain() }
                        jobs += requireNotNull(retryJob)
                    } else if (retryNeeded && retryAttempt < 6) {
                        val waitMs = (2_000L shl retryAttempt) + (nowMs().coerceAtLeast(0) % 500)
                        retryAttempt += 1
                        retryJob = scope.launch { delay(waitMs); retryJob = null; if (current(issued, owner)) drain() }
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
        val owner = auth ?: return
        val snapshot = local {
            val records = records(); val pending = intents()
            val saved = records.filter { it.resource == "saved" }.mapNotNull { LibraryCodec.decode<SavedRecord>("SavedRecord", it.body) }.associateBy { it.work_id }.toMutableMap()
            records.filter { it.resource == "reconciled" && it.key.startsWith("saved:") }.mapNotNull { LibraryCodec.decode<SavedRecord>("SavedRecord", it.body) }.forEach { value -> if (value.work_id in saved && value.revision >= (saved[value.work_id]?.revision ?: -1)) saved[value.work_id] = value }
            val savedViews = saved.mapValues { (_, value) -> SavedEntry(value.work_id, value.saved, value.revision, LibrarySync.Confirmed) }.toMutableMap()
            pending.filter { it.key.startsWith("saved:") }.forEach { intent -> LibraryCodec.decode<SavedWrite>("SavedWrite", intent.body)?.let { value -> val work = intent.key.substringAfter(':'); savedViews[work] = SavedEntry(work, value.saved, value.base_revision, if (intent.failure == null) LibrarySync.Pending else LibrarySync.Failed) } }
            val admittedContinue = records.filter { it.resource == "continue" }.mapNotNull { LibraryCodec.decode<ContinueItem>("ContinueItem", it.body) }
            val continues = admittedContinue.mapNotNull { item -> domain(item.progress.audio)?.let { ContinueEntry(it, item.progress.position_ms, item.progress.revision, resume(item.resume), LibrarySync.Confirmed) } }.associateBy { it.audio.workId }.toMutableMap()
            pending.filter { it.key.startsWith("progress:") }.forEach { intent -> LibraryCodec.decode<ProgressWrite>("ProgressWrite", intent.body)?.let { value -> domain(value.audio)?.let { audio -> continues[audio.workId] = ContinueEntry(audio, value.position_ms, value.base_revision, continues[audio.workId]?.takeIf { it.audio == audio }?.resume ?: LibraryResume.NeedsRefresh, if (intent.failure == null) LibrarySync.Pending else LibrarySync.Failed) } } }
            // Continue is a projection of durable progress, including an acknowledged local
            // register whose subsequent page refresh failed. Match the server's newest-per-Work
            // ordering; a progress receipt itself does not establish present playback access.
            records.filter { it.resource == "reconciled" && it.key.startsWith("progress:") }
                .mapNotNull { LibraryCodec.decode<ProgressRecord>("ProgressRecord", it.body) }
                .sortedWith(compareByDescending<ProgressRecord> { it.listener_order }.thenBy { it.audio.publication_id })
                .distinctBy { it.audio.work_id }
                .forEach { value ->
                    val audio = domain(value.audio) ?: return@forEach
                    val previous = continues[audio.workId]
                    val admitted = admittedContinue.firstOrNull { it.progress.audio.work_id == audio.workId }
                    val latest = admitted == null || value.listener_order > admitted.progress.listener_order ||
                        (value.listener_order == admitted.progress.listener_order && value.audio.publication_id <= admitted.progress.audio.publication_id)
                    if (previous?.sync != LibrarySync.Pending && previous?.sync != LibrarySync.Failed &&
                        pending.none { it.key == "progress:${audio.publicationId}" } && latest) {
                        if (value.completion == Completion.completed) continues.remove(audio.workId)
                        else {
                            val availability = previous?.takeIf { it.audio == audio }?.resume ?: LibraryResume.NeedsRefresh
                            continues[audio.workId] = ContinueEntry(audio, value.position_ms, value.revision, availability, LibrarySync.Confirmed)
                        }
                    }
                }
            val history = records.filter { it.resource == "history" }.sortedByDescending { it.revision }.mapNotNull { LibraryCodec.decode<HistoryItem>("HistoryItem", it.body) }.mapNotNull { value -> domain(value.audio)?.let { HistoryEntry(it, value.started_at_ms, resume(value.resume)) } }
            val checkpoint = record("checkpoint", "latest")?.let { LibraryCodec.decode<ProgressWrite>("ProgressWrite", it.body) }?.let { value -> domain(value.audio)?.let { PausedCheckpoint(it, value.position_ms, record("playback_speed", it.publicationId)?.body?.toIntOrNull()?.takeIf(::isSupportedPlaybackSpeed) ?: 1000) } }
            val conflicts = records.filter { it.resource == "conflict" && it.body.isNotEmpty() }.mapNotNull { record ->
                val receipt = LibraryCodec.decode<Receipt>("Receipt", record.body) ?: return@mapNotNull null
                val candidate = pending.firstOrNull { it.operation == receipt.operation_id }?.let { LibraryCodec.decode<ProgressWrite>("ProgressWrite", it.body) }
                val server = (receipt.state as? MutationState.Progress)?.current
                LibraryConflict(record.key, receipt.operation_id, candidate?.position_ms, server?.position_ms, server?.revision ?: 0)
            }
            LibraryState(owner.account, savedViews.values.toList(), continues.values.toList(), history,
                pageCursors.toMap(),
                pending.count { it.failure == null }, pending.mapNotNull { it.failure?.let { reason -> it.operation to reason } }.toMap(), conflicts, checkpoint)
        } ?: return
        mutableState.value = snapshot.copy(failure = mutableState.value.failure)
    }
    override fun close() { leave(); store.close() }
}

/** Native composition constructs the transport/SQLite shell behind the domain-only session port. */
fun nativeLibraryController(
    scope: CoroutineScope,
    api: LibraryApi,
    store: LibraryLocalPort,
    newOperation: () -> String,
    onSessionInvalidated: (String) -> Unit = {},
    nowMs: () -> Long,
): NativeLibraryController = NativeLibraryController(
    NativeLibrarySync(scope, api, store, newOperation, onSessionInvalidated, nowMs),
)
