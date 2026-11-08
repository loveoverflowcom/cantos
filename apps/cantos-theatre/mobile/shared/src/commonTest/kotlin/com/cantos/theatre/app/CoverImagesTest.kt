package com.cantos.theatre.app

import androidx.compose.ui.graphics.ImageBitmap
import com.cantos.theatre.adapters.http.CoverBytes
import com.cantos.theatre.adapters.http.CoverBytesSource
import com.cantos.theatre.adapters.http.CoverRequest
import com.cantos.theatre.adapters.image.CoverDecoder
import com.cantos.theatre.domain.catalog.CoverVariant
import com.cantos.theatre.features.shell.presentation.CoverLoad
import com.cantos.theatre.presentation.model.ShellCover
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.async
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertIs
import kotlin.test.assertNull
import kotlin.test.assertSame

@OptIn(ExperimentalCoroutinesApi::class)
class CoverImagesTest {
    private fun cover(n: Int, work: Int = 1, variant: CoverVariant = CoverVariant.Thumbnail): ShellCover {
        val digest = n.toString().padStart(64, '0')
        return ShellCover(
            url = "https://api.cantos.example/api/v1/theatre/works/10000000-0000-4000-8000-00000000000$work/cover/${variant.path}/$digest",
            sha256 = digest, byteLength = 74, mime = "image/webp", widthPx = 384, heightPx = 576,
            altText = "Bìa $n", variant = variant,
        )
    }

    private class Source(var outcome: (CoverRequest) -> CoverBytes = { request -> reply(request) }) : CoverBytesSource {
        val requests = mutableListOf<CoverRequest>()
        var inFlight = 0
        var peak = 0
        var gate: CompletableDeferred<Unit>? = null
        override suspend fun fetch(request: CoverRequest): CoverBytes {
            requests += request
            inFlight++
            peak = maxOf(peak, inFlight)
            try {
                gate?.await()
                return outcome(request)
            } finally {
                inFlight--
            }
        }

        companion object {
            fun reply(request: CoverRequest): CoverBytes = request.ifNoneMatch?.let { CoverBytes.NotModified304(it) }
                ?: CoverBytes.Verified200(byteArrayOf(1), request.expectedEtag)
        }
    }

    private fun TestScope.images(
        source: CoverBytesSource,
        decode: CoverDecoder = CoverDecoder { _, _ -> ImageBitmap(2, 2) },
        budget: Long = 1_000_000,
        concurrent: Int = 3,
    ) = CoverImages(this, source, decode, budgetBytes = budget, maxConcurrent = concurrent)

    private suspend fun CoverImages.thumb(cover: ShellCover): CoverLoad = load(cover, 70, 90)

    @Test
    fun matching_304_revalidates_every_use_without_another_decode_or_body() = runTest {
        val source = Source()
        var decodes = 0
        var edge = 0
        val loader = images(source, decode = { _, actual -> edge = actual; decodes++; ImageBitmap(2, 2) })
        val first = assertIs<CoverLoad.Ready>(loader.thumb(cover(1)))
        assertSame(first.bitmap, assertIs<CoverLoad.Ready>(loader.thumb(cover(1))).bitmap)
        assertEquals(2, source.requests.size)
        assertNull(source.requests[0].ifNoneMatch)
        assertEquals(source.requests[0].expectedEtag, source.requests[1].ifNoneMatch)
        assertEquals(1, decodes)
        assertEquals(128, edge)
    }

    @Test
    fun another_work_variant_or_metadata_never_inherits_a_cached_validator() = runTest {
        val source = Source()
        val loader = images(source)
        loader.thumb(cover(1))
        for (other in listOf(cover(1, work = 2), cover(1, variant = CoverVariant.Full), cover(1).copy(byteLength = 75))) {
            assertIs<CoverLoad.Ready>(loader.thumb(other))
            assertNull(source.requests.last().ifNoneMatch)
        }
        assertEquals(4, source.requests.size)
    }

    @Test
    fun denial_drops_cached_pixels_and_network_errors_show_fallback() = runTest {
        for (failure in listOf(CoverBytes.Denied, CoverBytes.Refused, CoverBytes.Unavailable)) {
            val source = Source()
            val loader = images(source)
            assertIs<CoverLoad.Ready>(loader.thumb(cover(1)))
            source.outcome = { failure }
            assertEquals(CoverLoad.Unavailable, loader.thumb(cover(1)))
            assertEquals(2, source.requests.size)
            source.outcome = Source.Companion::reply
            assertIs<CoverLoad.Ready>(loader.thumb(cover(1)))
            if (failure != CoverBytes.Unavailable) assertNull(source.requests.last().ifNoneMatch)
        }
    }

    @Test
    fun a_200_in_flight_before_denial_cannot_restore_pixels_for_another_size() = runTest {
        val started = CompletableDeferred<Unit>()
        val releaseOlder = CompletableDeferred<Unit>()
        val requests = mutableListOf<CoverRequest>()
        val source = CoverBytesSource { request ->
            requests += request
            when (requests.size) {
                1 -> {
                    started.complete(Unit)
                    releaseOlder.await()
                    Source.reply(request)
                }
                2 -> CoverBytes.Denied
                else -> Source.reply(request)
            }
        }
        val loader = images(source)
        val older = async { loader.thumb(cover(1)) }
        started.await()
        assertEquals(CoverLoad.Unavailable, loader.load(cover(1), 160, 240))
        releaseOlder.complete(Unit)
        assertEquals(CoverLoad.Unavailable, older.await())
        assertIs<CoverLoad.Ready>(loader.thumb(cover(1)))
        assertNull(requests.last().ifNoneMatch)
    }

    @Test
    fun missing_cache_mismatched_304_and_decode_failure_are_closed() = runTest {
        val source = Source { CoverBytes.NotModified304(it.expectedEtag) }
        var decodes = 0
        val loader = images(source, decode = { _, _ -> decodes++; null })
        assertEquals(CoverLoad.Unavailable, loader.thumb(cover(1)))
        assertEquals(0, decodes)
        source.outcome = Source.Companion::reply
        assertEquals(CoverLoad.Unavailable, loader.thumb(cover(1)))
        assertEquals(1, decodes)
        assertNull(source.requests.last().ifNoneMatch)

        val cached = images(source)
        assertIs<CoverLoad.Ready>(cached.thumb(cover(2)))
        source.outcome = { CoverBytes.NotModified304("\"wrong\"") }
        assertEquals(CoverLoad.Unavailable, cached.thumb(cover(2)))
    }

    @Test
    fun decoded_memory_is_bounded_and_evicted_entries_get_no_validator() = runTest {
        val source = Source()
        var decodes = 0
        val loader = images(source, decode = { _, _ -> decodes++; ImageBitmap(2, 2) }, budget = 32)
        loader.thumb(cover(1))
        loader.thumb(cover(2))
        loader.thumb(cover(1))
        loader.thumb(cover(3))
        loader.thumb(cover(1))
        loader.thumb(cover(3))
        assertEquals(3, decodes)
        loader.thumb(cover(2))
        assertNull(source.requests.last().ifNoneMatch)
        assertEquals(4, decodes)
        val tiny = images(source, budget = 8)
        assertIs<CoverLoad.Ready>(tiny.thumb(cover(4)))
        assertIs<CoverLoad.Ready>(tiny.thumb(cover(4)))
        assertNull(source.requests.last().ifNoneMatch)
    }

    @Test
    fun concurrent_reads_share_exact_inflight_work_and_stay_bounded() = runTest {
        val source = Source().also { it.gate = CompletableDeferred() }
        val loader = images(source, concurrent = 2)
        val first = async { loader.thumb(cover(1)) }
        val duplicate = async { loader.thumb(cover(1)) }
        val other = (2..5).map { async { loader.thumb(cover(it)) } }
        runCurrent()
        assertEquals(2, source.peak)
        first.cancel()
        runCurrent()
        assertEquals(2, source.inFlight, "the other viewport still needs the exact request")
        source.gate!!.complete(Unit)
        assertIs<CoverLoad.Ready>(duplicate.await())
        other.forEach { assertIs<CoverLoad.Ready>(it.await()) }
        assertEquals(5, source.requests.size)
        assertEquals(2, source.peak)
    }

    @Test
    fun last_waiter_cancellation_releases_the_read_and_does_not_cache() = runTest {
        val source = Source().also { it.gate = CompletableDeferred() }
        val loader = images(source)
        val pending = async { loader.thumb(cover(1)) }
        runCurrent()
        assertEquals(1, source.inFlight)
        pending.cancel()
        runCurrent()
        assertEquals(0, source.inFlight)
        assertFailsWith<CancellationException> { pending.await() }
        source.gate = null
        assertIs<CoverLoad.Ready>(loader.thumb(cover(1)))
        assertNull(source.requests.last().ifNoneMatch)
    }
}
