package com.cantos.theatre.app

import androidx.compose.ui.graphics.ImageBitmap
import com.cantos.theatre.adapters.http.CoverBytes
import com.cantos.theatre.adapters.http.CoverBytesSource
import com.cantos.theatre.adapters.http.CoverRequest
import com.cantos.theatre.adapters.image.CoverDecoder
import com.cantos.theatre.features.shell.presentation.CoverLoad
import com.cantos.theatre.presentation.model.ShellCover
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.async
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runTest
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertIs
import kotlin.test.assertSame
import kotlin.test.assertTrue

@OptIn(ExperimentalCoroutinesApi::class)
class CoverImagesTest {
    private fun cover(n: Int, work: Int = 1) = ShellCover(
        url = "https://api.cantos.example/api/v1/theatre/works/10000000-0000-4000-8000-00000000000$work/cover/${n.toString().padStart(64, '0')}",
        sha256 = n.toString().padStart(64, '0'), byteLength = 4, mime = "image/png", widthPx = 2, heightPx = 2, altText = "Bìa $n",
    )

    private class Source(var outcome: (CoverRequest) -> CoverBytes = { CoverBytes.Verified(byteArrayOf(1, 2, 3, 4)) }) : CoverBytesSource {
        val requests = mutableListOf<CoverRequest>()
        var inFlight = 0
        var peak = 0
        var gate: CompletableDeferred<Unit>? = null
        override suspend fun fetch(request: CoverRequest): CoverBytes {
            requests += request
            inFlight += 1
            peak = maxOf(peak, inFlight)
            try {
                gate?.await()
                return outcome(request)
            } finally {
                inFlight -= 1
            }
        }
    }

    private val decoder = CoverDecoder { _, _ -> ImageBitmap(2, 2) }
    private var now = 1_000L

    private fun images(source: Source, decode: CoverDecoder = decoder, budget: Long = 1_000_000, concurrent: Int = 3) =
        CoverImages(source, decode, { now }, budgetBytes = budget, freshForMs = 100, maxConcurrent = concurrent)

    @Test
    fun a_verified_cover_decodes_and_is_reused_only_while_fresh_for_the_exact_url() = runTest {
        val source = Source()
        val loader = images(source)
        val first = assertIs<CoverLoad.Ready>(loader.load(cover(1)))
        assertSame(first.bitmap, assertIs<CoverLoad.Ready>(loader.load(cover(1))).bitmap)
        assertEquals(1, source.requests.size)
        // The same digest under another Work is another authorization: it is read, not borrowed.
        assertIs<CoverLoad.Ready>(loader.load(cover(1, work = 2)))
        assertEquals(2, source.requests.size)
        // no-cache: past the freshness window the Server is asked again, so revocation takes effect.
        now += 100
        assertIs<CoverLoad.Ready>(loader.load(cover(1)))
        assertEquals(3, source.requests.size)
    }

    @Test
    fun a_revoked_cover_is_unavailable_and_stable_but_other_failures_may_retry() = runTest {
        val source = Source { CoverBytes.NotFound }
        val loader = images(source)
        assertEquals(CoverLoad.Unavailable, loader.load(cover(1)))
        assertEquals(CoverLoad.Unavailable, loader.load(cover(1)))
        assertEquals(1, source.requests.size, "a 404 on a digest-pinned URL is remembered")
        source.outcome = { CoverBytes.Unavailable }
        assertEquals(CoverLoad.Unavailable, loader.load(cover(2)))
        source.outcome = { CoverBytes.Verified(byteArrayOf(1, 2, 3, 4)) }
        assertIs<CoverLoad.Ready>(loader.load(cover(2)))
        assertEquals(3, source.requests.size, "an unavailable cover is read again later")
        source.outcome = { CoverBytes.Refused }
        assertEquals(CoverLoad.Unavailable, loader.load(cover(3)))
    }

    @Test
    fun undecodable_bytes_show_the_fallback_without_an_exception() = runTest {
        val loader = images(Source(), decode = { _, _ -> null })
        assertEquals(CoverLoad.Unavailable, loader.load(cover(1)))
    }

    @Test
    fun the_request_carries_exactly_the_disclosed_facts() = runTest {
        val source = Source()
        images(source).load(cover(7))
        val sent = source.requests.single()
        assertEquals(cover(7).url, sent.url)
        assertEquals(cover(7).sha256, sent.sha256)
        assertEquals(4, sent.byteLength)
        assertEquals("image/png", sent.contentType)
    }

    @Test
    fun decoded_memory_is_bounded_and_least_recently_used_goes_first() = runTest {
        val source = Source()
        // Each 2x2 bitmap is 16 bytes: room for two.
        val loader = images(source, budget = 32)
        loader.load(cover(1)); loader.load(cover(2))
        loader.load(cover(1)) // touch 1 so 2 is the oldest
        loader.load(cover(3)) // evicts 2
        assertEquals(3, source.requests.size)
        loader.load(cover(1)); loader.load(cover(3))
        assertEquals(3, source.requests.size, "1 and 3 are retained")
        loader.load(cover(2))
        assertEquals(4, source.requests.size, "2 was evicted")
        // An image larger than the whole budget is shown but never retained.
        val tiny = images(Source(), budget = 8)
        assertIs<CoverLoad.Ready>(tiny.load(cover(1)))
    }

    @Test
    fun reads_are_concurrency_bounded() = runTest {
        val source = Source().also { it.gate = CompletableDeferred() }
        val loader = images(source, concurrent = 2)
        val loads = (1..5).map { async { loader.load(cover(it)) } }
        advanceUntilIdle()
        assertEquals(2, source.peak)
        source.gate!!.complete(Unit)
        loads.forEach { assertIs<CoverLoad.Ready>(it.await()) }
        assertEquals(2, source.peak)
        assertEquals(5, source.requests.size)
    }

    @Test
    fun cancelling_a_load_cancels_the_read_and_remembers_nothing() = runTest {
        val source = Source().also { it.gate = CompletableDeferred() }
        val loader = images(source)
        val pending = async { loader.load(cover(1)) }
        advanceUntilIdle()
        assertEquals(1, source.inFlight)
        pending.cancel()
        advanceUntilIdle()
        assertEquals(0, source.inFlight)
        assertTrue(pending.isCancelled)
        assertFailsWith<CancellationException> { pending.await() }
        source.gate = null
        assertIs<CoverLoad.Ready>(loader.load(cover(1)))
    }
}
