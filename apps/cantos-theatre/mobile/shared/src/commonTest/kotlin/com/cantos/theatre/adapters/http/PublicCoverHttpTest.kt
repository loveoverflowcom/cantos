package com.cantos.theatre.adapters.http

import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.test.runTest
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertIs
import kotlin.test.assertTrue

class PublicCoverHttpTest {
    private val bytes = ByteArray(300) { (it * 13 + 1).toByte() }
    private val sha = sha256Hex(bytes)
    private val request = CoverRequest(
        "https://api.cantos.example/api/v1/theatre/works/10000000-0000-4000-8000-000000000001/cover/$sha",
        sha, bytes.size, "image/png",
    )

    private class Scripted(val respond: (CatalogHttpRequest) -> CatalogHttpResponse) : CatalogHttpTransport {
        val requests = mutableListOf<CatalogHttpRequest>()
        override suspend fun get(request: CatalogHttpRequest): CatalogHttpResponse = respond(request).also { requests += request }
    }

    private fun reply(status: Int, body: ByteArray = bytes, type: String? = "image/png") = CatalogHttpResponse(status, type, body)

    private suspend fun fetch(response: CatalogHttpResponse): CoverBytes =
        PublicCoverHttp(CatalogHttpTransport { response }).fetch(request)

    @Test
    fun a_verified_cover_is_read_from_exactly_the_given_url_with_a_body_bound_of_its_disclosed_length() = runTest {
        val transport = Scripted { reply(200) }
        val result = PublicCoverHttp(transport).fetch(request)
        assertTrue(bytes.contentEquals(assertIs<CoverBytes.Verified>(result).bytes))
        val sent = transport.requests.single()
        assertEquals(request.url, sent.url)
        assertEquals(bytes.size, sent.maxBodyBytes)
        assertEquals("image/png", sent.accept)
        // A parameterised content type is the same type.
        assertIs<CoverBytes.Verified>(fetch(reply(200, type = "image/png; charset=binary")))
        val avifRequest = CoverRequest(request.url, sha, bytes.size, "image/avif")
        assertIs<CoverBytes.Verified>(PublicCoverHttp(CatalogHttpTransport { reply(200, type = "image/avif") }).fetch(avifRequest))
    }

    @Test
    fun the_contract_status_table_maps_to_typed_outcomes() = runTest {
        assertEquals(CoverBytes.NotFound, fetch(reply(404, """{"code":"not_found","retryable":false}""".encodeToByteArray(), "application/json")))
        listOf(500, 502, 503, 504, 408).forEach { assertEquals(CoverBytes.Unavailable, fetch(reply(it, ByteArray(0), "application/json")), "$it") }
        // No redirect is followed, and no other status is part of this route.
        listOf(301, 302, 304, 401, 403, 206).forEach { assertEquals(CoverBytes.Refused, fetch(reply(it)), "$it") }
    }

    @Test
    fun bytes_that_contradict_the_pinned_facts_are_refused_before_decoding() = runTest {
        assertEquals(CoverBytes.Refused, fetch(reply(200, type = "image/jpeg")))
        assertEquals(CoverBytes.Refused, fetch(reply(200, type = "text/html")))
        assertEquals(CoverBytes.Refused, fetch(reply(200, type = null)))
        assertEquals(CoverBytes.Refused, fetch(reply(200, body = bytes.copyOf(bytes.size - 1))))
        assertEquals(CoverBytes.Refused, fetch(reply(200, body = bytes + 0)))
        // Same length and type, different bytes: the digest is the authority.
        assertEquals(CoverBytes.Refused, fetch(reply(200, body = bytes.copyOf().also { it[0] = (it[0] + 1).toByte() })))
        assertEquals(CoverBytes.Refused, fetch(reply(200, body = ByteArray(0))))
    }

    @Test
    fun transport_failures_are_typed_and_cancellation_propagates() = runTest {
        suspend fun outcome(failure: Exception) = PublicCoverHttp(CatalogHttpTransport { throw failure }).fetch(request)
        assertEquals(CoverBytes.Refused, outcome(CatalogResponseTooLargeException()))
        assertEquals(CoverBytes.Unavailable, outcome(CatalogTimeoutException()))
        assertEquals(CoverBytes.Unavailable, outcome(IllegalStateException("io")))
        assertEquals(CoverBytes.Unavailable, outcome(CookieRuntimeException()))
        assertFailsWith<CancellationException> { outcome(CancellationException("cancelled")) }
    }
}
