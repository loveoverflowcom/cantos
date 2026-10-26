package com.cantos.theatre.app

import com.cantos.theatre.adapters.http.CoverBytes
import com.cantos.theatre.adapters.http.CoverBytesSource
import com.cantos.theatre.adapters.http.CoverRequest
import com.cantos.theatre.adapters.image.CoverDecodeMaxEdgePx
import com.cantos.theatre.adapters.image.CoverDecoder
import com.cantos.theatre.features.shell.presentation.CoverImageSource
import com.cantos.theatre.features.shell.presentation.CoverLoad
import com.cantos.theatre.presentation.model.ShellCover
import androidx.compose.ui.graphics.ImageBitmap
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.Semaphore
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.sync.withPermit
import kotlinx.coroutines.withContext

/**
 * The process's cover pipeline: verified bytes -> decoded image, with at most [maxConcurrent]
 * reads in flight and a small in-memory cache. Nothing is written to disk.
 *
 * The Server serves covers `public, no-cache` so a revoked image stops at the next request.
 * A decoded image is therefore reused for at most [freshForMs] and only for the exact URL it
 * was read from (the URL names the Work and the digest, so one Work's authorization is never
 * another's), then read again. A 404 on a digest-pinned URL is stable for that URL and is
 * remembered for the process: a refreshed summary carries a different URL or no cover. Any other
 * failure is not remembered, so a later composition may retry. Memory is bounded by
 * [budgetBytes] of decoded pixels, least recently used first.
 */
class CoverImages(
    private val source: CoverBytesSource,
    private val decoder: CoverDecoder,
    private val nowMs: () -> Long,
    private val budgetBytes: Long = 32L * 1024 * 1024,
    private val freshForMs: Long = 120_000,
    maxConcurrent: Int = 3,
) : CoverImageSource {
    private class Entry(val bitmap: ImageBitmap, val loadedAtMs: Long, val bytes: Long)

    private val gate = Semaphore(maxConcurrent)
    private val lock = Mutex()
    private val decoded = LinkedHashMap<String, Entry>()
    private val missing = HashSet<String>()
    private var used = 0L

    override suspend fun load(cover: ShellCover): CoverLoad {
        lookup(cover.url)?.let { return it }
        return gate.withPermit {
            lookup(cover.url)?.let { return@withPermit it }
            val request = CoverRequest(cover.url, cover.sha256, cover.byteLength, cover.mime)
            when (val bytes = source.fetch(request)) {
                is CoverBytes.Verified -> {
                    val bitmap = withContext(Dispatchers.Default) { decoder.decode(bytes.bytes, CoverDecodeMaxEdgePx) }
                    if (bitmap == null) CoverLoad.Unavailable else remember(cover.url, bitmap)
                }
                CoverBytes.NotFound -> {
                    lock.withLock { missing += cover.url }
                    CoverLoad.Unavailable
                }
                CoverBytes.Unavailable, CoverBytes.Refused -> CoverLoad.Unavailable
            }
        }
    }

    private suspend fun lookup(url: String): CoverLoad? = lock.withLock {
        if (url in missing) return@withLock CoverLoad.Unavailable
        val entry = decoded.remove(url) ?: return@withLock null
        if (nowMs() - entry.loadedAtMs >= freshForMs) {
            used -= entry.bytes
            return@withLock null
        }
        decoded[url] = entry
        CoverLoad.Ready(entry.bitmap)
    }

    private suspend fun remember(url: String, bitmap: ImageBitmap): CoverLoad = lock.withLock {
        val size = bitmap.width.toLong() * bitmap.height * 4
        decoded.remove(url)?.let { used -= it.bytes }
        if (size <= budgetBytes) {
            decoded[url] = Entry(bitmap, nowMs(), size)
            used += size
            val oldest = decoded.entries.iterator()
            while (used > budgetBytes && oldest.hasNext()) {
                val victim = oldest.next()
                used -= victim.value.bytes
                oldest.remove()
            }
        }
        CoverLoad.Ready(bitmap)
    }
}
