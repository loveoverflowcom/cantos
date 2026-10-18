package com.cantos.theatre.adapters.http

import com.cantos.theatre.domain.catalog.PublicCatalogSamples.cursor
import com.cantos.theatre.domain.catalog.PublicCatalogSamples.work
import com.cantos.theatre.domain.catalog.PublicCatalogSamples.workId
import com.cantos.theatre.domain.catalog.PublicCatalogQuery
import com.cantos.theatre.domain.catalog.CatalogCheck
import com.cantos.theatre.domain.catalog.PublicWorkKind
import com.cantos.theatre.ports.CatalogProtocolFault
import com.cantos.theatre.ports.PublicCatalogRead
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.test.runTest
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertIs

class PublicCatalogHttpAdapterTest {
    private val corpus = CanonicalDiscoveryCorpus.fixtureJsonByFile
    private val origin = ApiOrigin.parse("https://api.cantos.example")

    private class Scripted(var respond: (CatalogHttpRequest) -> CatalogHttpResponse) : CatalogHttpTransport {
        val urls = mutableListOf<String>()
        val bounds = mutableListOf<Int>()
        override suspend fun get(request: CatalogHttpRequest): CatalogHttpResponse {
            urls += request.url
            bounds += request.maxBodyBytes
            return respond(request)
        }
    }

    private fun json(status: Int, body: String, type: String? = "application/json") =
        CatalogHttpResponse(status, type, body.encodeToByteArray())

    @Test
    fun requests_only_anonymous_theatre_routes_with_explicit_limit_and_opaque_cursor() = runTest {
        val transport = Scripted { json(200, corpus.getValue("works.json")) }
        val adapter = PublicCatalogHttpAdapter(origin, transport)
        assertIs<PublicCatalogRead.Ready<*>>(adapter.works(null))
        adapter.works(cursor(1))
        transport.respond = { json(200, corpus.getValue("works.json").let { page -> page.substringAfter("[").substringBeforeLast("]").trim() }) }
        val story = work(1, PublicWorkKind.Story)
        assertIs<PublicCatalogRead.Ready<*>>(adapter.work(story.id))
        transport.respond = { json(200, corpus.getValue("units.json")) }
        assertIs<PublicCatalogRead.Ready<*>>(adapter.units(story, null))
        adapter.units(story, cursor(2))
        assertEquals(
            listOf(
                "https://api.cantos.example/api/v1/theatre/works?limit=20",
                "https://api.cantos.example/api/v1/theatre/works?limit=20&cursor=${cursor(1).value}",
                "https://api.cantos.example/api/v1/theatre/works/${story.id.value}",
                "https://api.cantos.example/api/v1/theatre/works/${story.id.value}/units?limit=20",
                "https://api.cantos.example/api/v1/theatre/works/${story.id.value}/units?limit=20&cursor=${cursor(2).value}",
            ),
            transport.urls,
        )
        assertEquals(setOf(CatalogBodyLimitBytes), transport.bounds.toSet())
    }

    @Test
    fun search_and_filter_go_to_server_and_encode_unicode_and_query_delimiters() = runTest {
        val transport = Scripted { json(200, corpus.getValue("works.json")) }
        val adapter = PublicCatalogHttpAdapter(origin, transport)
        val query = assertIs<CatalogCheck.Valid<PublicCatalogQuery>>(PublicCatalogQuery.of("á &?#", PublicWorkKind.Podcast)).value
        adapter.works(null, query)
        adapter.works(cursor(2), query)
        adapter.works(null, assertIs<CatalogCheck.Valid<PublicCatalogQuery>>(PublicCatalogQuery.of("", PublicWorkKind.Story)).value)
        assertEquals(listOf(
            "https://api.cantos.example/api/v1/theatre/search?limit=20&q=%C3%A1%20%26%3F%23&kind=podcast",
            "https://api.cantos.example/api/v1/theatre/search?limit=20&cursor=${cursor(2).value}&q=%C3%A1%20%26%3F%23&kind=podcast",
            "https://api.cantos.example/api/v1/theatre/works?limit=20&kind=story",
        ), transport.urls)
    }

    @Test
    fun work_detail_must_be_the_requested_identity() = runTest {
        val body = corpus.getValue("works.json").substringAfter("[").substringBeforeLast("]").trim()
        val adapter = PublicCatalogHttpAdapter(origin, Scripted { json(200, body) })
        assertEquals(PublicCatalogRead.Protocol(CatalogProtocolFault.Invalid), adapter.work(workId(2)))
    }

    @Test
    fun unit_pages_are_validated_against_the_requested_work() = runTest {
        val adapter = PublicCatalogHttpAdapter(origin, Scripted { json(200, corpus.getValue("units.json")) })
        // Fixture units belong to work 1 and are chapters.
        assertEquals(PublicCatalogRead.Protocol(CatalogProtocolFault.Invalid), adapter.units(work(2), null))
        assertEquals(PublicCatalogRead.Protocol(CatalogProtocolFault.Invalid), adapter.units(work(1, PublicWorkKind.Podcast), null))
        assertIs<PublicCatalogRead.Ready<*>>(adapter.units(work(1, PublicWorkKind.Novel), null))
    }

    @Test
    fun status_code_and_envelope_follow_the_exact_v1_error_table() = runTest {
        fun outcome(response: CatalogHttpResponse, sentCursor: Boolean = true) =
            classifyCatalogResponse(response, sentCursor, WorkPageWire.serializer()) { it.toDomain().acceptedOrNull() }
        val error = { name: String -> corpus.getValue(name) }
        assertEquals(PublicCatalogRead.NotFound, outcome(json(404, error("not_found.json"))))
        assertEquals(PublicCatalogRead.InvalidCursor, outcome(json(400, error("invalid_cursor.json"))))
        // The server cannot reject a cursor that was never sent.
        assertEquals(PublicCatalogRead.Protocol(CatalogProtocolFault.UnexpectedStatus), outcome(json(400, error("invalid_cursor.json")), sentCursor = false))
        assertEquals(PublicCatalogRead.Protocol(CatalogProtocolFault.UnexpectedStatus), outcome(json(400, error("invalid_request.json"))))
        assertEquals(PublicCatalogRead.Unavailable(retryable = true), outcome(json(503, error("unavailable.json"))))
        assertEquals(PublicCatalogRead.Unavailable(retryable = true), outcome(json(503, error("rendition_unavailable.json"))))
        assertEquals(PublicCatalogRead.Unavailable(retryable = false), outcome(json(500, """{"code":"unavailable","retryable":false}""")))
        // Mismatched pairs are not trusted.
        assertEquals(PublicCatalogRead.Protocol(CatalogProtocolFault.UnexpectedStatus), outcome(json(503, error("not_found.json"))))
        assertEquals(PublicCatalogRead.Protocol(CatalogProtocolFault.UnexpectedStatus), outcome(json(404, """{"code":"not_found","retryable":true}""")))
        assertEquals(PublicCatalogRead.Protocol(CatalogProtocolFault.UnexpectedStatus), outcome(json(200 + 101, "")))
        // The probe's Studio-shaped envelope is not a Theatre error.
        assertEquals(PublicCatalogRead.Protocol(CatalogProtocolFault.Malformed),
            outcome(json(404, """{"code":"not_found","current_revision":null,"issues":[]}""")))
        assertEquals(PublicCatalogRead.Protocol(CatalogProtocolFault.UnexpectedStatus), outcome(json(302, "", type = null)))
        assertEquals(PublicCatalogRead.Protocol(CatalogProtocolFault.UnexpectedStatus), outcome(json(401, "", type = null)))
        assertEquals(PublicCatalogRead.Unavailable(retryable = true), outcome(json(502, "<html>Bad gateway</html>", type = "text/html")))
        assertEquals(PublicCatalogRead.Unavailable(retryable = true), outcome(json(504, "", type = null)))
    }

    @Test
    fun success_requires_json_utf8_strict_shape_and_valid_values() = runTest {
        fun outcome(response: CatalogHttpResponse) =
            classifyCatalogResponse(response, false, WorkPageWire.serializer()) { it.toDomain().acceptedOrNull() }
        val works = corpus.getValue("works.json")
        assertIs<PublicCatalogRead.Ready<*>>(outcome(json(200, works, "application/json; charset=utf-8")))
        assertEquals(PublicCatalogRead.Protocol(CatalogProtocolFault.ContentType), outcome(json(200, works, "text/plain")))
        assertEquals(PublicCatalogRead.Protocol(CatalogProtocolFault.ContentType), outcome(json(200, works, null)))
        assertEquals(PublicCatalogRead.Protocol(CatalogProtocolFault.Malformed),
            outcome(CatalogHttpResponse(200, "application/json", byteArrayOf(0x7b, 0xc3.toByte(), 0x28, 0x7d))))
        assertEquals(PublicCatalogRead.Protocol(CatalogProtocolFault.Malformed), outcome(json(200, works.replace("\"story\"", "\"drama\""))))
        assertEquals(PublicCatalogRead.Protocol(CatalogProtocolFault.Malformed), outcome(json(200, "[]")))
        assertEquals(PublicCatalogRead.Protocol(CatalogProtocolFault.Invalid),
            outcome(json(200, works.replace("10000000-0000-4000-8000-000000000001", "10000000-0000-4000-8000-00000000000A"))))
        assertEquals(PublicCatalogRead.Protocol(CatalogProtocolFault.Invalid),
            outcome(json(200, works.replace("\"next_cursor\": null", "\"next_cursor\": \"lc1_not-hex\""))))
        assertEquals(PublicCatalogRead.Protocol(CatalogProtocolFault.Oversized),
            outcome(CatalogHttpResponse(200, "application/json", ByteArray(CatalogBodyLimitBytes + 1))))
    }

    @Test
    fun transport_failures_are_typed_and_cancellation_propagates() = runTest {
        val adapter = { failure: Exception -> PublicCatalogHttpAdapter(origin, CatalogHttpTransport { throw failure }) }
        assertEquals(PublicCatalogRead.Transport, adapter(IllegalStateException("io")).works(null))
        assertEquals(PublicCatalogRead.Transport, adapter(CookieRuntimeException()).works(null))
        assertEquals(PublicCatalogRead.Protocol(CatalogProtocolFault.Oversized), adapter(CatalogResponseTooLargeException()).works(null))
        assertFailsWith<CancellationException> { adapter(CancellationException("cancelled")).works(null) }
    }

    @Test
    fun release_admits_only_https_and_debug_additionally_admits_explicit_loopback() {
        assertEquals(CatalogOriginSelection.NotConfigured, selectCatalogOrigin(null, allowLoopbackHttp = false))
        assertEquals(CatalogOriginSelection.NotConfigured, selectCatalogOrigin("", allowLoopbackHttp = true))
        assertEquals(CatalogOriginSelection.Configured(ApiOrigin.parse("https://api.cantos.example")),
            selectCatalogOrigin("https://api.cantos.example", allowLoopbackHttp = false))
        assertEquals(CatalogOriginSelection.Rejected, selectCatalogOrigin("http://127.0.0.1:8080", allowLoopbackHttp = false))
        assertEquals(CatalogOriginSelection.Configured(ApiOrigin.parse("http://127.0.0.1:8080")),
            selectCatalogOrigin("http://127.0.0.1:8080", allowLoopbackHttp = true))
        assertEquals(CatalogOriginSelection.Configured(ApiOrigin.parse("http://localhost:8090")),
            selectCatalogOrigin("http://localhost:8090", allowLoopbackHttp = true))
        // ApiOrigin parses [::1], but the debug network security config permits no cleartext there.
        for (raw in listOf("http://[::1]:8080", "http://192.168.1.20:8080", "http://api.cantos.example", "https://api.cantos.example/",
            "https://user@api.cantos.example", "https://api.cantos.example/api/v1/studio", " https://api.cantos.example",
            "ftp://127.0.0.1", "https://api.cantos.example:0", "https://api.cantos.example:70000")) {
            assertEquals(CatalogOriginSelection.Rejected, selectCatalogOrigin(raw, allowLoopbackHttp = true), raw)
        }
    }
}
