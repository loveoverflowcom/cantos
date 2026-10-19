package com.cantos.theatre.adapters.preview

import com.cantos.theatre.adapters.library.LibraryApi
import com.cantos.theatre.adapters.library.LibraryAuthorization
import com.cantos.theatre.adapters.library.LibraryResult
import com.cantos.theatre.adapters.library.wire.AudioIdentity as WireAudio
import com.cantos.theatre.adapters.library.wire.Completion
import com.cantos.theatre.adapters.library.wire.ContinueItem
import com.cantos.theatre.adapters.library.wire.ContinuePage
import com.cantos.theatre.adapters.library.wire.HistoryClear
import com.cantos.theatre.adapters.library.wire.HistoryItem
import com.cantos.theatre.adapters.library.wire.HistoryPage
import com.cantos.theatre.adapters.library.wire.HistoryStart
import com.cantos.theatre.adapters.library.wire.HistoryState
import com.cantos.theatre.adapters.library.wire.MutationState
import com.cantos.theatre.adapters.library.wire.ProgressKind
import com.cantos.theatre.adapters.library.wire.ProgressRecord
import com.cantos.theatre.adapters.library.wire.ProgressWrite
import com.cantos.theatre.adapters.library.wire.Receipt
import com.cantos.theatre.adapters.library.wire.ResumeStatus
import com.cantos.theatre.adapters.library.wire.SavedItem
import com.cantos.theatre.adapters.library.wire.SavedPage
import com.cantos.theatre.adapters.library.wire.SavedRecord
import com.cantos.theatre.adapters.library.wire.SavedWrite
import com.cantos.theatre.domain.playback.AudioIdentity
import com.cantos.theatre.ports.LocalLibraryIntent
import kotlinx.coroutines.delay
import kotlinx.serialization.SerializationException
import kotlinx.serialization.json.Json

/** One synthetic listening position or history entry the Library server starts with. */
class PreviewListeningSeed(val unitId: String, val positionMs: Long)

/**
 * The remote Library as the native sync adapter sees it. Mutations are idempotent by operation ID
 * and per-account, so the production [com.cantos.theatre.adapters.library.NativeLibrarySync] and its
 * SQLite outbox run unchanged: an offline server leaves intents durable, a replayed operation
 * returns its first receipt. Conflict resolution and multi-device ordering are not modelled.
 */
class PreviewLibraryApi(private val nowMs: () -> Long, private val latencyMs: Long = 0) : LibraryApi {
    private class Account {
        val saved = LinkedHashMap<String, SavedRecord>()
        val progress = LinkedHashMap<String, ProgressRecord>()
        var history = emptyList<HistoryItem>()
        var historyState = HistoryState(0, 0)
        val receipts = HashMap<String, Receipt>()
    }

    private val accounts = HashMap<String, Account>()
    private var order = 0L
    private val json = Json { ignoreUnknownKeys = false; explicitNulls = true; classDiscriminator = "resource" }

    /** When set, every call fails as a retryable outage and the client keeps its durable intents. */
    var unavailable: Boolean = false
    var latencyOverrideMs: Long? = null
    var mutations: Int = 0
        private set

    /** Replaces [actor]'s remote data; older entries come first in [progress] and [history]. */
    fun seed(actor: String, saved: Set<String>, progress: List<PreviewListeningSeed>, history: List<PreviewListeningSeed>) {
        val account = account(actor)
        account.saved.clear(); account.progress.clear(); account.receipts.clear()
        saved.forEach { work -> account.saved[work] = SavedRecord(work, true, 1, ++order, nowMs()) }
        progress.forEach { entry ->
            val audio = audioOf(entry.unitId) ?: return@forEach
            account.progress[audio.publication_id] =
                record(audio, entry.positionMs, 1, DeviceId, ProgressKind.checkpoint)
        }
        account.history = history.mapNotNull { entry ->
            val audio = audioOf(entry.unitId) ?: return@mapNotNull null
            HistoryItem(audio, nowMs(), ++order, account.progress[audio.publication_id], ResumeStatus.exact_available)
        }.asReversed()
        account.historyState = HistoryState(if (account.history.isEmpty()) 0 else 1, 0)
    }

    override suspend fun continuePage(auth: LibraryAuthorization, cursor: String?): LibraryResult<ContinuePage> = served {
        val items = account(auth.account).progress.values.sortedByDescending { it.listener_order }
            .map { ContinueItem(it, ResumeStatus.exact_available) }
        ContinuePage(items, null)
    }

    override suspend fun savedPage(auth: LibraryAuthorization, cursor: String?): LibraryResult<SavedPage> = served {
        SavedPage(account(auth.account).saved.values.filter { it.saved }.sortedByDescending { it.listener_order }.map(::SavedItem), null)
    }

    override suspend fun historyPage(auth: LibraryAuthorization, cursor: String?): LibraryResult<HistoryPage> = served {
        val account = account(auth.account)
        HistoryPage(account.historyState, account.history, null)
    }

    override suspend fun saved(auth: LibraryAuthorization, work: String): LibraryResult<SavedRecord> = served {
        account(auth.account).saved[work] ?: SavedRecord(work, false, 0, 0, 0)
    }

    override suspend fun progress(auth: LibraryAuthorization, publication: String): LibraryResult<MutationState.Progress> = served {
        MutationState.Progress(account(auth.account).progress[publication])
    }

    override suspend fun mutate(auth: LibraryAuthorization, intent: LocalLibraryIntent): LibraryResult<Receipt> {
        pause()
        if (unavailable) return LibraryResult.Failed("unavailable", retryable = true, unknownOutcome = false)
        mutations += 1
        val account = account(auth.account)
        return try {
            LibraryResult.Value(apply(account, intent))
        } catch (_: SerializationException) {
            LibraryResult.Failed("invalid_request", retryable = false)
        } catch (_: IllegalArgumentException) {
            LibraryResult.Failed("invalid_request", retryable = false)
        }
    }

    override suspend fun receipt(auth: LibraryAuthorization, operation: String): LibraryResult<Receipt> {
        pause()
        if (unavailable) return LibraryResult.Failed("unavailable", retryable = true)
        return account(auth.account).receipts[operation]?.let { LibraryResult.Value(it) }
            ?: LibraryResult.Failed("not_found", retryable = false)
    }

    private suspend fun <T> served(read: () -> T): LibraryResult<T> {
        pause()
        return if (unavailable) LibraryResult.Failed("unavailable", retryable = true) else LibraryResult.Value(read())
    }

    private suspend fun pause() {
        val wait = latencyOverrideMs ?: latencyMs
        if (wait > 0) delay(wait)
    }

    private fun apply(account: Account, intent: LocalLibraryIntent): Receipt {
        val body = intent.body
        val operation = when {
            intent.path.startsWith("/library/saved/") -> json.decodeFromString<SavedWrite>(body).operation.operation_id
            intent.path.startsWith("/progress/") -> json.decodeFromString<ProgressWrite>(body).operation.operation_id
            intent.path == "/library/history/playback-starts" -> json.decodeFromString<HistoryStart>(body).operation.operation_id
            intent.path == "/library/history/clear" -> json.decodeFromString<HistoryClear>(body).operation.operation_id
            else -> throw IllegalArgumentException("Unknown library route")
        }
        account.receipts[operation]?.let { return it }
        val receipt = when {
            intent.path.startsWith("/library/saved/") -> saved(account, operation, intent.path.removePrefix("/library/saved/"), json.decodeFromString(body))
            intent.path.startsWith("/progress/") -> progress(account, operation, json.decodeFromString(body))
            intent.path == "/library/history/playback-starts" -> historyStart(account, operation, json.decodeFromString(body))
            else -> historyClear(account, operation, json.decodeFromString(body))
        }
        account.receipts[operation] = receipt
        return receipt
    }

    private fun saved(account: Account, operation: String, work: String, write: SavedWrite): Receipt {
        val current = account.saved[work]
        if (current != null && current.saved == write.saved) {
            return Receipt(operation, "already_current", null, MutationState.Saved(current))
        }
        val next = SavedRecord(work, write.saved, (current?.revision ?: 0) + 1, ++order, nowMs())
        account.saved[work] = next
        return Receipt(operation, "accepted", null, MutationState.Saved(next))
    }

    private fun progress(account: Account, operation: String, write: ProgressWrite): Receipt {
        val publication = write.audio.publication_id
        val previous = account.progress[publication]
        val next = record(write.audio, write.position_ms.coerceIn(0, write.audio.duration_ms), (previous?.revision ?: 0) + 1,
            write.operation.device_id, write.kind)
        account.progress[publication] = next
        return Receipt(operation, "accepted", null, MutationState.Progress(next))
    }

    private fun historyStart(account: Account, operation: String, write: HistoryStart): Receipt {
        if (write.history_epoch != account.historyState.epoch) {
            return Receipt(operation, "history_epoch_conflict", "keep_server", MutationState.History(account.historyState))
        }
        val publication = write.audio.publication_id
        val entry = HistoryItem(write.audio, nowMs(), ++order, account.progress[publication], ResumeStatus.exact_available)
        account.history = listOf(entry) + account.history.filter { it.audio.publication_id != publication }
        account.historyState = HistoryState(account.historyState.revision + 1, account.historyState.epoch)
        return Receipt(operation, "accepted", null, MutationState.History(account.historyState))
    }

    private fun historyClear(account: Account, operation: String, write: HistoryClear): Receipt {
        account.history = emptyList()
        account.historyState = HistoryState(account.historyState.revision + 1, write.history_epoch + 1)
        return Receipt(operation, "accepted", null, MutationState.History(account.historyState))
    }

    private fun record(audio: WireAudio, positionMs: Long, revision: Long, device: String, kind: ProgressKind) = ProgressRecord(
        audio, positionMs, if (positionMs >= audio.duration_ms) Completion.completed else Completion.in_progress,
        revision, nowMs(), ++order, device, kind,
    )

    private fun account(actor: String): Account = accounts.getOrPut(actor) { Account() }

    private fun audioOf(unitId: String): WireAudio? = PreviewFixtures.unit(unitId)?.let { (work, unit) ->
        PreviewFixtures.audio(work, unit).toWire()
    }

    private companion object {
        const val DeviceId = "70000000-0000-4000-8000-000000000001"
    }
}

private fun AudioIdentity.toWire() = WireAudio(workId, unitId, publicationId, renditionId, sha256, durationMs)
