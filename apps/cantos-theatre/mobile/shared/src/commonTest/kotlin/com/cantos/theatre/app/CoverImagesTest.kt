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
    fun every_use_revalidates_the_exact_url_while_fresh_decoded_pixels_may_be_reused() = runTest {
        val source = Source()
        var decodes = 0
        val loader = images(source, decode = { _, _ -> decodes += 1; ImageBitmap(2, 2) })
        val first = assertIs<CoverLoad.Ready>(loader.load(cover(1)))
        assertSame(first.bitmap, assertIs<CoverLoad.Ready>(loader.load(cover(1))).bitmap)
        assertEquals(2, source.requests.size, "no-cache requires a new read before every use")
        assertEquals(1, decodes, "fresh verified bytes may reuse the decoded pixels")
        // The same digest under another Work is another authorization: it is read, not borrowed.
        assertIs<CoverLoad.Ready>(loader.load(cover(1, work = 2)))
        assertEquals(3, source.requests.size)
        assertEquals(2, decodes)
        // The local freshness window controls decoding only, never whether the Server is asked.
        now += 100
        assertIs<CoverLoad.Ready>(loader.load(cover(1)))
        assertEquals(4, source.requests.size)
        assertEquals(3, decodes)
    }

    @Test
    fun revocation_or_refusal_is_observed_before_reusing_a_cached_bitmap() = runTest {
        for (failure in listOf(CoverBytes.NotFound, CoverBytes.Refused, CoverBytes.Unavailable)) {
            val source = Source()
            val loader = images(source)
            assertIs<CoverLoad.Ready>(loader.load(cover(1)))
            source.outcome = { failure }
            assertEquals(CoverLoad.Unavailable, loader.load(cover(1)), "the prior image grants no new use")
            assertEquals(2, source.requests.size, "revalidation cannot wait for a local freshness window")
        }
    }

    @Test
    fun a_not_found_response_is_not_retained_when_the_same_url_becomes_available() = runTest {
        val source = Source { CoverBytes.NotFound }
        val loader = images(source)
        assertEquals(CoverLoad.Unavailable, loader.load(cover(1)))
        source.outcome = { CoverBytes.Verified(byteArrayOf(1, 2, 3, 4)) }
        assertIs<CoverLoad.Ready>(loader.load(cover(1)))
        assertEquals(2, source.requests.size, "errors are no-store, not permanent URL decisions")
    }

    @Test
    fun a_cached_url_still_checks_the_current_disclosed_request_facts() = runTest {
        val source = Source { request ->
            if (request.byteLength == 4 && request.contentType == "image/png") CoverBytes.Verified(byteArrayOf(1, 2, 3, 4))
            else CoverBytes.Refused
        }
        val loader = images(source)
        assertIs<CoverLoad.Ready>(loader.load(cover(1)))
        val changed = cover(1).copy(byteLength = 6, mime = "image/jpeg")
        assertEquals(CoverLoad.Unavailable, loader.load(changed))
        assertEquals(2, source.requests.size)
        val sent = source.requests.last()
        assertEquals(changed.url, sent.url)
        assertEquals(changed.sha256, sent.sha256)
        assertEquals(6, sent.byteLength)
        assertEquals("image/jpeg", sent.contentType)
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
        var decodes = 0
        // Each 2x2 bitmap is 16 bytes: room for two.
        val loader = images(source, decode = { _, _ -> decodes += 1; ImageBitmap(2, 2) }, budget = 32)
        loader.load(cover(1)); loader.load(cover(2))
        loader.load(cover(1)) // touch 1 so 2 is the oldest
        loader.load(cover(3)) // evicts 2
        assertEquals(4, source.requests.size)
        assertEquals(3, decodes)
        loader.load(cover(1)); loader.load(cover(3))
        assertEquals(6, source.requests.size, "retained pixels still require revalidation")
        assertEquals(3, decodes, "1 and 3 are retained")
        loader.load(cover(2))
        assertEquals(7, source.requests.size)
        assertEquals(4, decodes, "2 was evicted")
        // An image larger than the whole budget is shown but never retained.
        var tinyDecodes = 0
        val tiny = images(Source(), decode = { _, _ -> tinyDecodes += 1; ImageBitmap(2, 2) }, budget = 8)
        assertIs<CoverLoad.Ready>(tiny.load(cover(1)))
        assertIs<CoverLoad.Ready>(tiny.load(cover(1)))
        assertEquals(2, tinyDecodes)
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
