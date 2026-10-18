package com.cantos.theatre.adapters.library

import com.cantos.theatre.adapters.library.wire.ContinuePage
import com.cantos.theatre.adapters.library.wire.HistoryClear
import com.cantos.theatre.adapters.library.wire.HistoryStart
import com.cantos.theatre.adapters.library.wire.ProgressWrite
import com.cantos.theatre.adapters.library.wire.SavedWrite
import com.cantos.theatre.adapters.library.wire.HistoryPage
import com.cantos.theatre.adapters.library.LibraryCodec
import com.cantos.theatre.adapters.library.wire.LibraryError
import com.cantos.theatre.adapters.library.wire.MutationState
import com.cantos.theatre.adapters.library.wire.Receipt
import com.cantos.theatre.adapters.library.wire.SavedPage
import com.cantos.theatre.adapters.library.wire.SavedRecord
import com.cantos.theatre.domain.identity.NativeClientConfiguration
import com.cantos.theatre.adapters.http.ApiOrigin
import com.cantos.theatre.ports.LocalLibraryIntent
import com.cantos.theatre.ports.NativeHttpPort
import com.cantos.theatre.ports.NativeHttpRequest
import com.cantos.theatre.ports.NativeHttpResponse
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.withTimeoutOrNull
import kotlin.coroutines.resume

/** JSON metadata only. Audio bytes use the native media engine's separate transport. */
class NativeLibraryClient(private val apiOrigin: ApiOrigin, private val http: NativeHttpPort) : LibraryApi {
    constructor(configuration: NativeClientConfiguration, http: NativeHttpPort) : this(ApiOrigin.parse(configuration.apiOrigin), http)
    private val uuid = Regex("[a-f0-9]{8}(-[a-f0-9]{4}){3}-[a-f0-9]{12}")
    private suspend inline fun <reified T> request(auth: LibraryAuthorization, method: String, path: String, type: String, body: String? = null): LibraryResult<T> {
        val response = withTimeoutOrNull(15_000) {
            suspendCancellableCoroutine { continuation ->
                val handle = http.send(NativeHttpRequest(method, "${apiOrigin.probeUrl}$path", body, auth.bearer, auth.session)) {
                    if (continuation.isActive) continuation.resume(it)
                }
                continuation.invokeOnCancellation { handle.cancel() }
            }
        } ?: NativeHttpResponse(0, null, null, "timeout")
        val mutation = method != "GET"
        if (response.failure == "authority_changed") return LibraryResult.Failed("authority_changed", true, mutation)
        if (response.failure == "session_changed") return LibraryResult.Failed("session_changed", false, mutation)
        if (response.failure == "unauthenticated") return LibraryResult.Failed("unauthenticated", false, mutation)
        if (response.failure == "denied") return LibraryResult.Failed("denied", false, mutation)
        if (response.failure == "protocol") return LibraryResult.Failed("protocol", false, mutation)
        if (response.failure != null) return LibraryResult.Failed("unavailable", true, mutation)
        if (response.status == 401) return LibraryResult.Failed("unauthenticated", false)
        if (response.actualSession != auth.session) return LibraryResult.Failed("session_changed", false, mutation)
        if (response.contentType?.substringBefore(';')?.trim()?.lowercase() != "application/json" || (response.body?.length ?: Int.MAX_VALUE) > 1_048_576) return LibraryResult.Failed("protocol", false, mutation)
        val raw = response.body ?: return LibraryResult.Failed("protocol", false, mutation)
        if (response.status in 200..299 || (response.status == 409 && type == "Receipt")) {
            LibraryCodec.decode<T>(type, raw)?.let { return LibraryResult.Value(it) }
        }
        LibraryCodec.decode<LibraryError>("LibraryError", raw)?.let { return LibraryResult.Failed(it.code, it.retryable) }
        return LibraryResult.Failed("protocol", false, mutation)
    }
    private fun page(section: String, cursor: String?): String? {
        if (cursor != null && !Regex("[a-f0-9]{64}").matches(cursor)) return null
        return "/library/$section?limit=20" + (cursor?.let { "&cursor=$it" } ?: "")
    }
    override suspend fun continuePage(auth: LibraryAuthorization, cursor: String?): LibraryResult<ContinuePage> = page("continue", cursor)?.let { request(auth, "GET", it, "ContinuePage") } ?: LibraryResult.Failed("invalid_cursor", false)
    override suspend fun savedPage(auth: LibraryAuthorization, cursor: String?): LibraryResult<SavedPage> = page("saved", cursor)?.let { request(auth, "GET", it, "SavedPage") } ?: LibraryResult.Failed("invalid_cursor", false)
    override suspend fun historyPage(auth: LibraryAuthorization, cursor: String?): LibraryResult<HistoryPage> = page("history", cursor)?.let { request(auth, "GET", it, "HistoryPage") } ?: LibraryResult.Failed("invalid_cursor", false)
    override suspend fun saved(auth: LibraryAuthorization, work: String): LibraryResult<SavedRecord> {
        if (!uuid.matches(work)) return LibraryResult.Failed("invalid_request", false)
        return when (val result = request<SavedRecord>(auth, "GET", "/library/saved/$work", "SavedRecord")) {
            is LibraryResult.Value -> if (result.value.work_id == work) result else LibraryResult.Failed("protocol", false)
            is LibraryResult.Failed -> result
        }
    }
    override suspend fun progress(auth: LibraryAuthorization, publication: String): LibraryResult<MutationState.Progress> = if (uuid.matches(publication)) when (val result = request<MutationState>(auth, "GET", "/progress/$publication", "MutationState")) { is LibraryResult.Value -> (result.value as? MutationState.Progress)?.takeIf { it.current == null || it.current.audio.publication_id == publication }?.let { LibraryResult.Value(it) } ?: LibraryResult.Failed("protocol", false); is LibraryResult.Failed -> result } else LibraryResult.Failed("invalid_request", false)
    override suspend fun mutate(auth: LibraryAuthorization, intent: LocalLibraryIntent): LibraryResult<Receipt> {
        val operation = when {
            intent.method == "PUT" && intent.path.startsWith("/library/saved/") -> {
                val work = intent.path.removePrefix("/library/saved/")
                if (!uuid.matches(work) || intent.key != "saved:$work") return LibraryResult.Failed("invalid_request", false)
                LibraryCodec.decode<SavedWrite>("SavedWrite", intent.body)?.operation
            }
            intent.method == "PUT" && intent.path.startsWith("/progress/") -> {
                val publication = intent.path.removePrefix("/progress/")
                val write = LibraryCodec.decode<ProgressWrite>("ProgressWrite", intent.body)
                if (!uuid.matches(publication) || intent.key != "progress:$publication" || write?.audio?.publication_id != publication) return LibraryResult.Failed("invalid_request", false)
                write.operation
            }
            intent.method == "POST" && intent.path == "/library/history/playback-starts" && intent.key == "history" -> LibraryCodec.decode<HistoryStart>("HistoryStart", intent.body)?.operation
            intent.method == "POST" && intent.path == "/library/history/clear" && intent.key == "history" -> LibraryCodec.decode<HistoryClear>("HistoryClear", intent.body)?.operation
            else -> null
        }
        if (operation?.operation_id != intent.operation) return LibraryResult.Failed("invalid_request", false)
        return request(auth, intent.method, intent.path, "Receipt", intent.body)
    }
    override suspend fun receipt(auth: LibraryAuthorization, operation: String): LibraryResult<Receipt> = if (uuid.matches(operation)) request(auth, "GET", "/operations/$operation", "Receipt") else LibraryResult.Failed("invalid_request", false)
}
