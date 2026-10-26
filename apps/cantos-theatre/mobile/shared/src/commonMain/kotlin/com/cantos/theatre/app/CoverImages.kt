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
 * The Server serves covers `public, no-cache`: every load revalidates the exact disclosed
 * request before showing pixels, including a previously decoded image. Only after fresh byte
 * verification may decoded pixels be reused for [freshForMs], and only for that exact URL
 * (which names the Work and digest). Refusals discard cached pixels and are never retained,
 * so a later authorized read can recover. Memory is bounded by [budgetBytes] of decoded pixels,
 * least recently used first; the freshness window controls decoding, never authorization.
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
    private var used = 0L

    override suspend fun load(cover: ShellCover): CoverLoad = gate.withPermit {
        val request = CoverRequest(cover.url, cover.sha256, cover.byteLength, cover.mime)
        when (val bytes = source.fetch(request)) {
            is CoverBytes.Verified -> {
                lookup(cover.url)?.let { return@withPermit it }
                val bitmap = withContext(Dispatchers.Default) { decoder.decode(bytes.bytes, CoverDecodeMaxEdgePx) }
                if (bitmap == null) CoverLoad.Unavailable else remember(cover.url, bitmap)
            }
            CoverBytes.NotFound, CoverBytes.Unavailable, CoverBytes.Refused -> {
                lock.withLock { decoded.remove(cover.url)?.let { used -= it.bytes } }
                CoverLoad.Unavailable
            }
        }
    }

    /** Decoded reuse is safe only after this load has freshly verified the disclosed bytes. */
    private suspend fun lookup(url: String): CoverLoad? = lock.withLock {
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
