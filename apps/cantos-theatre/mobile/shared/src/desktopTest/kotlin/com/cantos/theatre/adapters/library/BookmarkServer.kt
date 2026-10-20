package com.cantos.theatre.adapters.library

import com.cantos.theatre.adapters.library.wire.BookmarkDelete
import com.cantos.theatre.adapters.library.wire.Completion
import com.cantos.theatre.adapters.library.wire.HistoryClear
import com.cantos.theatre.adapters.library.wire.HistoryStart
import com.cantos.theatre.adapters.library.wire.ProgressRecord
import com.cantos.theatre.adapters.library.wire.ProgressWrite
import com.cantos.theatre.adapters.library.wire.SavedWrite
import kotlinx.serialization.encodeToString
import com.cantos.theatre.adapters.library.wire.BookmarkItem
import com.cantos.theatre.adapters.library.wire.BookmarkPage
import com.cantos.theatre.adapters.library.wire.BookmarkRecord
import com.cantos.theatre.adapters.library.wire.BookmarkWrite
import com.cantos.theatre.adapters.library.wire.HistoryState
import com.cantos.theatre.adapters.library.wire.MutationState
import com.cantos.theatre.adapters.library.wire.PresentedContinuePage
import com.cantos.theatre.adapters.library.wire.PresentedHistoryPage
import com.cantos.theatre.adapters.library.wire.PresentedSavedPage
import com.cantos.theatre.adapters.library.wire.Receipt
import com.cantos.theatre.adapters.library.wire.ResumeStatus
import com.cantos.theatre.adapters.library.wire.SavedRecord
import com.cantos.theatre.adapters.library.wire.AudioIdentity as WireAudio
import com.cantos.theatre.adapters.library.wire.AudioPresentation
import com.cantos.theatre.ports.LocalLibraryIntent
import kotlinx.coroutines.CompletableDeferred

/**
 * A unit-test double of the account's bookmark authority with the server's own rules: create-once by ID, a
 * final tombstone no replay undoes, not-found for an unknown ID, operation receipts, and per-account isolation.
 * Knobs inject outages, a lost response after commit and held page answers. It is a double, never API evidence.
 */
internal class BookmarkServer : LibraryApi {
    class Account {
        val bookmarks = LinkedHashMap<String, BookmarkRecord>()
        val receipts = HashMap<String, Receipt>()
    }

    val accounts = HashMap<String, Account>()
    private var order = 0L
    val calls = mutableListOf<String>()
    var unavailable = false
    /** Commit the next mutation, then report an unknown outcome as a lost response does. */
    var loseNextResponse = false
    var refuseCreateWith: String? = null
    /** Resume status the listing reports for every bookmark (the server's current-release view). */
    var resume = ResumeStatus.exact_available
    var presentation: (WireAudio) -> AudioPresentation? = { null }
    var pageSize = 20
    /** Rows a lagging or replayed answer still lists as live, though the account already deleted them. */
    var staleRows: List<BookmarkRecord> = emptyList()
    /** Holds a bookmark page answer open after its content was fixed, like a slow response. */
    var pageGate: CompletableDeferred<Unit>? = null
    /** Bookmark mutations only: progress, history and Saved writes ride the same outbox but are not under test. */
    val mutations get() = calls.count { (it.startsWith("PUT") || it.startsWith("POST")) && "/library/bookmarks/" in it }

    fun account(actor: String) = accounts.getOrPut(actor) { Account() }
    fun live(actor: String) = account(actor).bookmarks.values.filter { !it.deleted }
    fun tombstones(actor: String) = account(actor).bookmarks.values.filter { it.deleted }

    /** A bookmark another device of the same account made, directly on the server. */
    fun create(actor: String, id: String, audio: WireAudio, positionMs: Long): BookmarkRecord =
        BookmarkRecord(id, audio, positionMs, false, ++order, 1_000).also { account(actor).bookmarks[id] = it }

    fun delete(actor: String, id: String) {
        val current = account(actor).bookmarks.getValue(id)
        account(actor).bookmarks[id] = current.copy(deleted = true, listener_order = ++order)
    }

    private fun <T> served(value: T): LibraryResult<T> =
        if (unavailable) LibraryResult.Failed("unavailable", true) else LibraryResult.Value(value)
    override suspend fun continuePage(auth: LibraryAuthorization, cursor: String?) = served(PresentedContinuePage(emptyList(), null))
    override suspend fun savedPage(auth: LibraryAuthorization, cursor: String?) = served(PresentedSavedPage(emptyList(), null))
    override suspend fun historyPage(auth: LibraryAuthorization, cursor: String?) = served(PresentedHistoryPage(HistoryState(0, 0), emptyList(), null))
    override suspend fun saved(auth: LibraryAuthorization, work: String) = LibraryResult.Value(SavedRecord(work, false, 0, 0, 0))
    override suspend fun progress(auth: LibraryAuthorization, publication: String) = LibraryResult.Value(MutationState.Progress(null))

    override suspend fun bookmarkPage(auth: LibraryAuthorization, cursor: String?): LibraryResult<BookmarkPage> {
        calls += "GET bookmarks:${cursor?.take(4)}"
        if (unavailable) return LibraryResult.Failed("unavailable", true)
        val rows = (live(auth.account) + staleRows).sortedByDescending { it.listener_order }
        val offset = cursor?.toIntOrNull(16) ?: 0
        val slice = rows.drop(offset).take(pageSize)
        val next = (offset + slice.size).takeIf { it < rows.size }?.let { it.toString(16).padStart(64, '0') }
        val answer = BookmarkPage(slice.map { BookmarkItem(it, resume, presentation(it.audio)) }, next)
        pageGate?.await() // the answer is fixed when the request is made, like a server's
        return LibraryResult.Value(answer)
    }

    override suspend fun mutate(auth: LibraryAuthorization, intent: LocalLibraryIntent): LibraryResult<Receipt> {
        calls += "${intent.method} ${intent.path}"
        if (unavailable) return LibraryResult.Failed("unavailable", true, unknownOutcome = intent.method != "GET")
        val account = account(auth.account)
        if (!intent.path.startsWith("/library/bookmarks/")) return otherwise(account, intent)
        val operation = when {
            intent.path.endsWith("/delete") -> LibraryCodec.decode<BookmarkDelete>("BookmarkDelete", intent.body)!!.operation.operation_id
            else -> LibraryCodec.decode<BookmarkWrite>("BookmarkWrite", intent.body)!!.operation.operation_id
        }
        account.receipts[operation]?.let { return LibraryResult.Value(it) }
        val receipt = if (intent.path.endsWith("/delete")) {
            val id = intent.path.removePrefix("/library/bookmarks/").removeSuffix("/delete")
            val current = account.bookmarks[id] ?: return LibraryResult.Failed("not_found", false)
            if (current.deleted) Receipt(operation, "already_current", null, MutationState.Bookmark(current))
            else {
                val gone = current.copy(deleted = true, listener_order = ++order, accepted_at_ms = 2_000)
                account.bookmarks[id] = gone
                Receipt(operation, "accepted", null, MutationState.Bookmark(gone))
            }
        } else {
            val write = LibraryCodec.decode<BookmarkWrite>("BookmarkWrite", intent.body)!!
            refuseCreateWith?.let { return LibraryResult.Failed(it, false) }
            val current = account.bookmarks[write.bookmark_id]
            when {
                current?.deleted == true -> Receipt(operation, "bookmark_deleted", "keep_server", MutationState.Bookmark(current))
                current != null && current.audio == write.audio && current.position_ms == write.position_ms ->
                    Receipt(operation, "already_current", null, MutationState.Bookmark(current))
                current != null -> return LibraryResult.Failed("idempotency_key_reused", false)
                else -> {
                    val made = BookmarkRecord(write.bookmark_id, write.audio, write.position_ms, false, ++order, 1_000)
                    account.bookmarks[write.bookmark_id] = made
                    Receipt(operation, "accepted", null, MutationState.Bookmark(made))
                }
            }
        }
        account.receipts[operation] = receipt
        if (loseNextResponse) {
            loseNextResponse = false
            return LibraryResult.Failed("unavailable", true, unknownOutcome = true)
        }
        return LibraryResult.Value(receipt)
    }

    /** Progress, history and Saved are accepted verbatim: they only keep the shared outbox draining. */
    private fun otherwise(account: Account, intent: LocalLibraryIntent): LibraryResult<Receipt> {
        val json = LibraryCodec.json
        val receipt = when {
            intent.path.startsWith("/progress/") -> {
                val write = LibraryCodec.decode<ProgressWrite>("ProgressWrite", intent.body)!!
                Receipt(write.operation.operation_id, "accepted", null, MutationState.Progress(
                    ProgressRecord(write.audio, write.position_ms, Completion.in_progress, write.base_revision + 1, 1_000, ++order, write.operation.device_id, write.kind)))
            }
            intent.path == "/library/history/playback-starts" ->
                Receipt(LibraryCodec.decode<HistoryStart>("HistoryStart", intent.body)!!.operation.operation_id, "accepted", null, MutationState.History(HistoryState(1, 0)))
            intent.path == "/library/history/clear" ->
                Receipt(LibraryCodec.decode<HistoryClear>("HistoryClear", intent.body)!!.operation.operation_id, "accepted", null, MutationState.History(HistoryState(2, 1)))
            intent.path.startsWith("/library/saved/") -> {
                val write = LibraryCodec.decode<SavedWrite>("SavedWrite", intent.body)!!
                Receipt(write.operation.operation_id, "accepted", null, MutationState.Saved(
                    SavedRecord(intent.path.removePrefix("/library/saved/"), write.saved, write.base_revision + 1, ++order, 1_000)))
            }
            else -> return LibraryResult.Failed("invalid_request", false)
        }
        account.receipts[receipt.operation_id] = receipt
        check(json.encodeToString(receipt).isNotEmpty())
        return LibraryResult.Value(receipt)
    }

    override suspend fun receipt(auth: LibraryAuthorization, operation: String): LibraryResult<Receipt> {
        calls += "GET receipt"
        if (unavailable) return LibraryResult.Failed("unavailable", true)
        return account(auth.account).receipts[operation]?.let { LibraryResult.Value(it) } ?: LibraryResult.Failed("not_found", false)
    }
}
