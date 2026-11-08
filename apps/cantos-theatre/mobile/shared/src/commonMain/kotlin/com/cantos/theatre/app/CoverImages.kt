package com.cantos.theatre.app

import androidx.compose.ui.graphics.ImageBitmap
import com.cantos.theatre.adapters.http.CoverBytes
import com.cantos.theatre.adapters.http.CoverBytesSource
import com.cantos.theatre.adapters.http.CoverRequest
import com.cantos.theatre.adapters.image.CoverDecoder
import com.cantos.theatre.domain.catalog.CoverVariant
import com.cantos.theatre.features.shell.presentation.CoverImageSource
import com.cantos.theatre.features.shell.presentation.CoverLoad
import com.cantos.theatre.presentation.model.ShellCover
import com.cantos.theatre.presentation.model.ShellCoverVariant
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.CoroutineStart
import kotlinx.coroutines.Deferred
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.async
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.Semaphore
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.sync.withPermit
import kotlinx.coroutines.withContext
import kotlin.math.ceil

/** Anonymous cover loader. A bitmap is reusable only after the current request admits it. */
class CoverImages(
    private val scope: CoroutineScope,
    private val source: CoverBytesSource,
    private val decoder: CoverDecoder,
    private val budgetBytes: Long = 32L * 1024 * 1024,
    maxConcurrent: Int = 3,
) : CoverImageSource {
    private data class Key(val request: CoverRequest, val edge: Int)
    private class Entry(val etag: String, val bitmap: ImageBitmap) {
        val bytes: Long get() = bitmap.width.toLong() * bitmap.height * 4
    }
    private class Admission(var invalidated: Boolean = false)
    private class Flight(val admission: Admission, val result: Deferred<CoverLoad>, var waiters: Int = 1)

    private val gate = Semaphore(maxConcurrent)
    private val decodeDispatcher = Dispatchers.Default.limitedParallelism(2)
    private val lock = Mutex()
    private val flights = mutableMapOf<Key, Flight>()
    private val decoded = LinkedHashMap<Key, Entry>()
    private var used = 0L

    override suspend fun load(cover: ShellCover, widthPx: Int, heightPx: Int): CoverLoad {
        if (widthPx <= 0 || heightPx <= 0 || cover.widthPx <= 0 || cover.heightPx <= 0) return CoverLoad.Unavailable
        val variant = when (cover.variant) {
            ShellCoverVariant.Thumbnail -> CoverVariant.Thumbnail
            ShellCoverVariant.Full -> CoverVariant.Full
        }
        val request = CoverRequest(cover.url, cover.sha256, cover.byteLength, cover.mime, variant, cover.widthPx, cover.heightPx)
        // Crop uses the larger scale. Clamp it to the source and bucket physical edges so
        // small layout changes during IME/viewport movement do not start another decode.
        val scale = maxOf(widthPx.toDouble() / cover.widthPx, heightPx.toDouble() / cover.heightPx).coerceAtMost(1.0)
        val sourceEdge = maxOf(cover.widthPx, cover.heightPx)
        val edge = (ceil(scale * sourceEdge / 64).toInt() * 64).coerceIn(1, sourceEdge)
        val key = Key(request, edge)
        val flight = lock.withLock {
            val inProgress = flights[key]?.takeUnless { it.result.isCompleted }
            if (inProgress != null) {
                inProgress.waiters++
                inProgress
            } else {
                val admission = Admission()
                val result = scope.async(start = CoroutineStart.LAZY) {
                    gate.withPermit { revalidate(key, admission) }
                }
                Flight(admission, result).also { flights[key] = it }
            }
        }
        try {
            return flight.result.await()
        } finally {
            withContext(NonCancellable) {
                lock.withLock {
                    if (--flight.waiters == 0) {
                        if (flights[key] === flight) flights.remove(key)
                        flight.result.cancel()
                    }
                }
            }
        }
    }

    private suspend fun revalidate(key: Key, admission: Admission): CoverLoad {
        val before = lock.withLock {
            admission.invalidated = false
            decoded[key]
        }
        return when (val reply = source.fetch(key.request.copy(ifNoneMatch = before?.etag))) {
            is CoverBytes.NotModified304 -> lock.withLock {
                val entry = decoded[key]
                if (entry == null || entry !== before || entry.etag != reply.etag) CoverLoad.Unavailable
                else {
                    decoded.remove(key)
                    decoded[key] = entry
                    CoverLoad.Ready(entry.bitmap)
                }
            }
            is CoverBytes.Verified200 -> {
                val bitmap = withContext(decodeDispatcher) { decoder.decode(reply.bytes, key.edge) }
                if (bitmap == null) invalidate(key.request)
                else remember(key, admission, Entry(reply.etag, bitmap))
            }
            CoverBytes.Denied, CoverBytes.Refused -> invalidate(key.request)
            CoverBytes.Unavailable -> CoverLoad.Unavailable
        }
    }

    private suspend fun invalidate(request: CoverRequest): CoverLoad = lock.withLock {
        // Other pixel sizes may still be fetching or decoding a response from before this denial.
        for ((key, flight) in flights) {
            if (key.request == request) flight.admission.invalidated = true
        }
        val iterator = decoded.entries.iterator()
        while (iterator.hasNext()) {
            val entry = iterator.next()
            if (entry.key.request == request) {
                used -= entry.value.bytes
                iterator.remove()
            }
        }
        CoverLoad.Unavailable
    }

    private suspend fun remember(key: Key, admission: Admission, entry: Entry): CoverLoad = lock.withLock {
        if (admission.invalidated || flights[key]?.admission !== admission) return@withLock CoverLoad.Unavailable
        decoded.remove(key)?.let { used -= it.bytes }
        if (entry.bytes <= budgetBytes) {
            decoded[key] = entry
            used += entry.bytes
            val oldest = decoded.entries.iterator()
            while (used > budgetBytes && oldest.hasNext()) {
                used -= oldest.next().value.bytes
                oldest.remove()
            }
        }
        CoverLoad.Ready(entry.bitmap)
    }
}
