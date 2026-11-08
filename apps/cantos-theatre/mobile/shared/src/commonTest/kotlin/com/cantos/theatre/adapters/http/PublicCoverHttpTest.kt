package com.cantos.theatre.adapters.http

import com.cantos.theatre.domain.catalog.CoverVariant
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.test.runTest
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertIs
import kotlin.test.assertNull
import kotlin.test.assertTrue

class PublicCoverHttpTest {
    private val bytes = thumbnailWebp()
    private val sha = sha256Hex(bytes)
    private val etag = "\"$sha\""
    private val request = CoverRequest(
        "https://api.cantos.example/api/v1/theatre/works/10000000-0000-4000-8000-000000000001/cover/thumbnail/$sha",
        sha, bytes.size, "image/webp", CoverVariant.Thumbnail, 2, 3,
    )

    private class Scripted(val respond: (CatalogHttpRequest) -> CatalogHttpResponse) : CatalogHttpTransport {
        val requests = mutableListOf<CatalogHttpRequest>()
        override suspend fun get(request: CatalogHttpRequest): CatalogHttpResponse =
            respond(request).also { requests += request }
    }

    private fun reply(status: Int, body: ByteArray = bytes, type: String? = "image/webp", validator: String? = etag) =
        CatalogHttpResponse(status, type, body, validator)

    private suspend fun fetch(response: CatalogHttpResponse, sent: CoverRequest = request): CoverBytes =
        PublicCoverHttp(CatalogHttpTransport { response }).fetch(sent)

    @Test
    fun exact_opaque_webp_is_admitted_with_its_strong_validator() = runTest {
        val transport = Scripted { reply(200) }
        val verified = assertIs<CoverBytes.Verified200>(PublicCoverHttp(transport).fetch(request))
        assertTrue(bytes.contentEquals(verified.bytes))
        assertEquals(etag, verified.etag)
        val sent = transport.requests.single()
        assertEquals(request.url, sent.url)
        assertEquals(bytes.size, sent.maxBodyBytes)
        assertEquals("image/webp", sent.accept)
        assertNull(sent.ifNoneMatch)
        assertIs<CoverBytes.Verified200>(fetch(reply(200, type = "image/webp; charset=binary")))
    }

    @Test
    fun only_matching_empty_304_on_a_conditional_request_reuses_the_validator() = runTest {
        val transport = Scripted { reply(304, body = byteArrayOf(), type = null) }
        val http = PublicCoverHttp(transport)
        val conditional = request.copy(ifNoneMatch = etag)
        val result = assertIs<CoverBytes.NotModified304>(http.fetch(conditional))
        assertEquals(etag, result.etag)
        assertEquals(etag, transport.requests.single().ifNoneMatch)
        assertIs<CoverBytes.Refused>(http.fetch(request))
        for (unexpected in listOf(reply(304), reply(304, byteArrayOf(), validator = null),
            reply(304, byteArrayOf(), validator = "W/$etag"), reply(304, byteArrayOf(), validator = "\"other\""))) {
            assertIs<CoverBytes.Refused>(fetch(unexpected, conditional))
        }
        assertIs<CoverBytes.Denied>(fetch(reply(404, byteArrayOf(), type = "application/json"), conditional))
        assertIs<CoverBytes.Verified200>(fetch(reply(200), conditional))
    }

    @Test
    fun status_and_transport_failures_are_closed_and_cancellation_propagates() = runTest {
        for (status in listOf(408, 500, 502, 503, 504)) assertIs<CoverBytes.Unavailable>(fetch(reply(status)))
        for (status in listOf(301, 302, 401, 403, 206)) assertIs<CoverBytes.Refused>(fetch(reply(status)))
        suspend fun outcome(failure: Exception) = PublicCoverHttp(CatalogHttpTransport { throw failure }).fetch(request)
        assertIs<CoverBytes.Refused>(outcome(CatalogResponseTooLargeException()))
        assertIs<CoverBytes.Unavailable>(outcome(CatalogTimeoutException()))
        assertIs<CoverBytes.Unavailable>(outcome(CookieRuntimeException()))
        assertFailsWith<CancellationException> { outcome(CancellationException("cancelled")) }
    }

    @Test
    fun refuses_mime_length_digest_geometry_role_and_invalid_webp_before_decode() = runTest {
        for (bad in listOf(
            reply(200, type = "image/jpeg"), reply(200, type = "text/html"), reply(200, type = null),
            reply(200, body = bytes.copyOf(bytes.size - 1)), reply(200, body = bytes + 0),
            reply(200, body = bytes.copyOf().also { it[32]++ }), reply(200, validator = null),
        )) assertIs<CoverBytes.Refused>(fetch(bad))
        assertIs<CoverBytes.Refused>(fetch(reply(200), request.copy(widthPx = 3)))
        assertIs<CoverBytes.Refused>(fetch(reply(200), request.copy(variant = CoverVariant.Full)))

        // A forged digest/ETag covering the wrong container is still refused.
        val spoof = bytes.copyOf().also { it[12] = 'X'.code.toByte() }
        val spoofSha = sha256Hex(spoof)
        assertIs<CoverBytes.Refused>(fetch(reply(200, spoof, validator = "\"$spoofSha\""), request.copy(sha256 = spoofSha)))
        val transport = Scripted { error("An inadmissible MIME or validator must not make a request") }
        for (type in listOf("image/png", "image/jpeg", "image/avif")) {
            assertIs<CoverBytes.Refused>(PublicCoverHttp(transport).fetch(request.copy(contentType = type)))
        }
        assertIs<CoverBytes.Refused>(PublicCoverHttp(transport).fetch(request.copy(ifNoneMatch = "\"other\"")))
        assertTrue(transport.requests.isEmpty())
    }
}

/** Independently encoded opaque 2 × 3 VP8 plus the documented Thumbnail profile chunk. */
@OptIn(ExperimentalStdlibApi::class)
internal fun thumbnailWebp(): ByteArray = "524946464200000057454250565038202c0000007001009d012a0200030001001e25a0027401400000fef4beee599c2bd97ffe32a7ffe32a7ffe32a7fc5cc00043545652020000000154".hexToByteArray()
