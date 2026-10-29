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
import kotlin.test.assertTrue

class PublicCatalogHttpAdapterTest {
    private val corpus = CanonicalDiscoveryCorpus.fixtureJsonByFile
    /** Two covered Works (a shown cover, then null) and one covered detail: the v1.2 reads. */
    private val works = corpus.getValue("metadata/covered_works.json")
    private val detail = corpus.getValue("metadata/covered_work.json")
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
        val transport = Scripted { json(200, works) }
        val adapter = PublicCatalogHttpAdapter(origin, transport)
        assertIs<PublicCatalogRead.Ready<*>>(adapter.works(null))
        adapter.works(cursor(1))
        transport.respond = { json(200, detail) }
        val story = work(1, PublicWorkKind.Story)
        assertIs<PublicCatalogRead.Ready<*>>(adapter.work(workId(3)))
        transport.respond = { json(200, corpus.getValue("metadata/presented_units.json")) }
        assertIs<PublicCatalogRead.Ready<*>>(adapter.units(story, null))
        adapter.units(story, cursor(2))
        assertEquals(
            listOf(
                // Work reads opt in to the cover (presentation=3); unit reads stay on default v1.
                "https://api.cantos.example/api/v1/theatre/works?limit=20&presentation=3",
                "https://api.cantos.example/api/v1/theatre/works?limit=20&cursor=${cursor(1).value}&presentation=3",
                "https://api.cantos.example/api/v1/theatre/works/${workId(3).value}?presentation=3",
                "https://api.cantos.example/api/v1/theatre/works/${story.id.value}/units?limit=20&presentation=3",
                "https://api.cantos.example/api/v1/theatre/works/${story.id.value}/units?limit=20&cursor=${cursor(2).value}&presentation=3",
            ),
            transport.urls,
        )
        assertEquals(setOf(CatalogBodyLimitBytes), transport.bounds.toSet())
    }

    @Test
    fun search_and_filter_go_to_server_and_encode_unicode_and_query_delimiters() = runTest {
        val transport = Scripted { json(200, works) }
        val adapter = PublicCatalogHttpAdapter(origin, transport)
        val query = assertIs<CatalogCheck.Valid<PublicCatalogQuery>>(PublicCatalogQuery.of("á &?#", PublicWorkKind.Podcast)).value
        adapter.works(null, query)
        adapter.works(cursor(2), query)
        adapter.works(null, assertIs<CatalogCheck.Valid<PublicCatalogQuery>>(PublicCatalogQuery.of("", PublicWorkKind.Story)).value)
        assertEquals(listOf(
            "https://api.cantos.example/api/v1/theatre/search?limit=20&presentation=3&q=%C3%A1%20%26%3F%23&kind=podcast",
            "https://api.cantos.example/api/v1/theatre/search?limit=20&cursor=${cursor(2).value}&presentation=3&q=%C3%A1%20%26%3F%23&kind=podcast",
            "https://api.cantos.example/api/v1/theatre/works?limit=20&presentation=3&kind=story",
        ), transport.urls)
    }

    @Test
    fun work_detail_must_be_the_requested_identity() = runTest {
        val adapter = PublicCatalogHttpAdapter(origin, Scripted { json(200, detail) })
        assertEquals(PublicCatalogRead.Protocol(CatalogProtocolFault.Invalid), adapter.work(workId(2)))
        val ready = assertIs<PublicCatalogRead.Ready<com.cantos.theatre.domain.catalog.PublicWork>>(adapter.work(workId(3)))
        assertEquals(workId(3), ready.value.cover?.work)
    }

    @Test
    fun unit_pages_are_validated_against_the_requested_work() = runTest {
        val adapter = PublicCatalogHttpAdapter(origin, Scripted { json(200, corpus.getValue("metadata/presented_units.json")) })
        // Fixture units belong to work 1 and are chapters.
        assertEquals(PublicCatalogRead.Protocol(CatalogProtocolFault.Invalid), adapter.units(work(2), null))
        assertEquals(PublicCatalogRead.Protocol(CatalogProtocolFault.Invalid), adapter.units(work(1, PublicWorkKind.Podcast), null))
        assertIs<PublicCatalogRead.Ready<*>>(adapter.units(work(1, PublicWorkKind.Novel), null))
    }

    @Test
    fun status_code_and_envelope_follow_the_exact_v1_error_table() = runTest {
        fun outcome(response: CatalogHttpResponse, sentCursor: Boolean = true) =
            classifyCatalogResponse(response, sentCursor, MetadataWorkPageWire.serializer()) { it.toDomain().acceptedOrNull() }
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
        assertEquals(PublicCatalogRead.AccessFailure(com.cantos.theatre.ports.CatalogAccessFault.Unauthenticated), outcome(json(401, "", type = null)))
        assertEquals(PublicCatalogRead.Unavailable(retryable = true), outcome(json(502, "<html>Bad gateway</html>", type = "text/html")))
        assertEquals(PublicCatalogRead.AccessFailure(com.cantos.theatre.ports.CatalogAccessFault.Timeout), outcome(json(504, "", type = null)))
    }

    @Test
    fun success_requires_json_utf8_strict_shape_and_valid_values() = runTest {
        fun outcome(response: CatalogHttpResponse) =
            classifyCatalogResponse(response, false, MetadataWorkPageWire.serializer()) { it.toDomain().acceptedOrNull() }
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
            outcome(json(200, works.replace("lc1_0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef", "lc1_not-hex"))))
        assertEquals(PublicCatalogRead.Protocol(CatalogProtocolFault.Oversized),
            outcome(CatalogHttpResponse(200, "application/json", ByteArray(CatalogBodyLimitBytes + 1))))
    }

    private fun covered(body: String) =
        classifyCatalogResponse(json(200, body), false, MetadataWorkPageWire.serializer()) { it.toDomain().acceptedOrNull() }

    @Test
    fun a_default_or_presentation_one_body_is_refused_never_read_as_no_cover() {
        assertEquals(PublicCatalogRead.Protocol(CatalogProtocolFault.Malformed), covered(corpus.getValue("works.json")))
        assertEquals(PublicCatalogRead.Protocol(CatalogProtocolFault.Malformed), covered(corpus.getValue("presented_works.json")))
        // Even a v1.2 body that merely drops the required-nullable key is a protocol fault.
        assertEquals(PublicCatalogRead.Protocol(CatalogProtocolFault.Malformed), covered(Regex(""",\s*"cover": null""").replace(works, "")))
        assertIs<PublicCatalogRead.Ready<*>>(covered(works))
    }

    @Test
    fun an_unusable_cover_refuses_the_whole_page_instead_of_hiding_the_fault() {
        val sha = "a3c1f0e2b4d6a8c0e2f4a6b8c0d2e4f6a8b0c2d4e6f8a0b2c4d6e8f0a2b4c6d8"
        val workUrl = "https://api.cantos.example/api/v1/theatre/works/10000000-0000-4000-8000-000000000001/cover/$sha"
        fun mutated(from: String, to: String): String {
            assertTrue(works.contains(from), from)
            return works.replace(from, to)
        }
        val invalid = listOf(
            mutated("\"sha256\": \"$sha\"", "\"sha256\": \"${sha.uppercase()}\""),
            mutated("\"sha256\": \"$sha\"", "\"sha256\": \"${sha.drop(1)}\""),
            // The url must name this summary's own Work and the disclosed digest, exactly.
            mutated(workUrl, workUrl.replace("000000000001/", "000000000002/")),
            mutated(workUrl, workUrl.dropLast(1) + "0"),
            mutated(workUrl, workUrl.replace("https://", "http://")),
            mutated(workUrl, workUrl.replace("api.cantos.example", "user@api.cantos.example")),
            mutated(workUrl, "$workUrl?x=1"),
            mutated(workUrl, "$workUrl#top"),
            mutated(workUrl, workUrl.replace("/api/v1/theatre/works/", "/api/v1/studio/works/")),
            mutated(workUrl, workUrl.replace("api.cantos.example", " ")),
            mutated(workUrl, workUrl.replace("https://api.cantos.example", "https://")),
            mutated("\"width_px\": 1200", "\"width_px\": 4097"),
            mutated("\"height_px\": 1800", "\"height_px\": 0"),
            mutated("\"byte_length\": 245760", "\"byte_length\": 8388609"),
            mutated("\"byte_length\": 245760", "\"byte_length\": 0"),
            mutated("\"alt_text\": \"Sân khấu tối, một ngọn đèn vàng chiếu xuống chiếc ghế trống.\"", "\"alt_text\": \"   \""),
            mutated("\"alt_text\": \"Sân khấu tối, một ngọn đèn vàng chiếu xuống chiếc ghế trống.\"", "\"alt_text\": \"${"ế".repeat(342)}\""),
        )
        invalid.forEach { body -> assertEquals(PublicCatalogRead.Protocol(CatalogProtocolFault.Invalid), covered(body), body) }
        // The type is a closed wire enum: an unknown one is malformed before the domain sees it.
        assertEquals(PublicCatalogRead.Protocol(CatalogProtocolFault.Malformed), covered(mutated("image/avif", "image/gif")))
        // AVIF is admitted under the exact same contract rules as PNG and JPEG.
        assertIs<PublicCatalogRead.Ready<*>>(covered(mutated("image/avif", "image/jpeg")))
        // The same facts at the schema's limits are accepted: 4096 px, 8 MiB and 1024 alt-text bytes.
        assertIs<PublicCatalogRead.Ready<*>>(covered(
            mutated("\"width_px\": 1200", "\"width_px\": 4096")
                .replace("\"byte_length\": 245760", "\"byte_length\": 8388608")
                .replace("\"alt_text\": \"Sân khấu tối, một ngọn đèn vàng chiếu xuống chiếc ghế trống.\"", "\"alt_text\": \"${"ế".repeat(341)}a\""),
        ))
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
    fun an_absent_override_selects_the_production_default_and_never_an_empty_one() {
        assertEquals(CatalogOriginSelection.Configured(ApiOrigin.parse("https://api.cantos.cc"), OriginSource.Default),
            selectCatalogOrigin(null, allowLoopbackHttp = false))
        assertEquals(selectCatalogOrigin(null, allowLoopbackHttp = false), selectCatalogOrigin(null, allowLoopbackHttp = true))
        assertEquals("https://api.cantos.cc", DefaultProductionApiOrigin)
        // Packaging preserves absent versus empty instead of silently switching environments.
        assertEquals(selectCatalogOrigin(null, allowLoopbackHttp = false), selectPackagedApiOrigin(PackagedDefaultApiOrigin, allowLoopbackHttp = false))
        assertEquals(selectCatalogOrigin(null, allowLoopbackHttp = false), selectPackagedApiOrigin(null, allowLoopbackHttp = true))
        assertEquals(CatalogOriginSelection.Rejected(ApiOriginRefusal.Empty), selectCatalogOrigin("", allowLoopbackHttp = true))
        assertEquals(CatalogOriginSelection.Rejected(ApiOriginRefusal.Empty), selectCatalogOrigin("   ", allowLoopbackHttp = false))
        assertEquals(CatalogOriginSelection.Rejected(ApiOriginRefusal.NotHttps), selectPackagedApiOrigin("http://127.0.0.1:8080", allowLoopbackHttp = false))
        assertEquals(CatalogOriginSelection.Configured(ApiOrigin.parse("https://api.cantos.example"), OriginSource.Override),
            selectPackagedApiOrigin("https://api.cantos.example", allowLoopbackHttp = false))
    }

    @Test
    fun release_admits_only_https_and_debug_additionally_admits_explicit_loopback() {
        assertEquals(CatalogOriginSelection.Configured(ApiOrigin.parse("https://api.cantos.example")),
            selectCatalogOrigin("https://api.cantos.example", allowLoopbackHttp = false))
        assertEquals(CatalogOriginSelection.Rejected(ApiOriginRefusal.NotHttps), selectCatalogOrigin("http://127.0.0.1:8080", allowLoopbackHttp = false))
        assertEquals(CatalogOriginSelection.Configured(ApiOrigin.parse("http://127.0.0.1:8080")),
            selectCatalogOrigin("http://127.0.0.1:8080", allowLoopbackHttp = true))
        assertEquals(CatalogOriginSelection.Configured(ApiOrigin.parse("http://localhost:8090")),
            selectCatalogOrigin("http://localhost:8090", allowLoopbackHttp = true))
        // ApiOrigin parses [::1], but the debug network security config permits no cleartext there.
        val refusals = mapOf(
            "http://[::1]:8080" to ApiOriginRefusal.CleartextHostNotAllowed,
            "http://192.168.1.20:8080" to ApiOriginRefusal.CleartextHostNotAllowed,
            "http://api.cantos.example" to ApiOriginRefusal.CleartextHostNotAllowed,
            "https://api.cantos.example/" to ApiOriginRefusal.Malformed,
            "https://user@api.cantos.example" to ApiOriginRefusal.Malformed,
            "https://api.cantos.example/api/v1/studio" to ApiOriginRefusal.Malformed,
            " https://api.cantos.example" to ApiOriginRefusal.Malformed,
            "api.cantos.cc" to ApiOriginRefusal.Malformed,
            "ftp://127.0.0.1" to ApiOriginRefusal.NotHttps,
            "https://api.cantos.example:0" to ApiOriginRefusal.Malformed,
            "https://api.cantos.example:70000" to ApiOriginRefusal.Malformed,
        )
        for ((raw, refusal) in refusals) {
            // Never the default, never a mock: a refusal is typed and identical for every build type's input.
            assertEquals(CatalogOriginSelection.Rejected(refusal), selectCatalogOrigin(raw, allowLoopbackHttp = true), raw)
        }
    }
}
