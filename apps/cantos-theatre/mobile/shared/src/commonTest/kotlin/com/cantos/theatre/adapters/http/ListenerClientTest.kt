package com.cantos.theatre.adapters.http

import kotlinx.serialization.json.*
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.withContext
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.runCurrent

@OptIn(ExperimentalCoroutinesApi::class)
class ListenerClientTest {
    @Test
    fun canonicalErrorEnvelopeCasesAgreeWithRustWithoutInventedListenerDtos() {
        val cases = Json.parseToJsonElement(CanonicalClientBoundary.json).jsonObject.getValue("cases").jsonArray
        assertEquals(6, cases.size)
        for (case in cases) {
            val fields = case.jsonObject
            val response = ProbeResponse(fields.getValue("status").jsonPrimitive.int, fields.getValue("content_type").jsonPrimitive.content, fields.getValue("body").toString())
            val expected = when (fields.getValue("expected").jsonPrimitive.content) {
                "unsupported" -> ListenerOutcome.Unsupported(UnsupportedCapability.ListenerApi)
                "unauthenticated" -> ListenerOutcome.Failed(HttpFailure.Unauthenticated)
                "forbidden" -> ListenerOutcome.Failed(HttpFailure.Forbidden)
                "unavailable" -> ListenerOutcome.Failed(HttpFailure.Unavailable)
                "protocol" -> ListenerOutcome.Failed(HttpFailure.Protocol)
                else -> error("Unknown fixture expectation")
            }
            assertEquals(expected, classifyProbe(response))
        }
    }
    @Test
    fun explicitOriginNeverAllowsCredentialsInsecureRemoteOrAnotherApiSurface() {
        assertEquals("http://127.0.0.1:8080/api/v1/theatre", ApiOrigin.parse("http://127.0.0.1:8080").probeUrl)
        for (origin in listOf("http://example.com", "https://user:secret@example.com", "https://example.com/api/v1/studio", "https://example.com?token=secret", "https://example.com#fragment", "https://example.com:0", "https://example.com:65536")) {
            assertFailsWith<IllegalArgumentException> { ApiOrigin.parse(origin) }
        }
    }
    @Test
    fun reservedBoundaryAndSuccessfulHttpNeverBecomeFakeCatalogOrPlayback() = runTest {
        val requests = mutableListOf<ProbeRequest>()
        val client = ListenerClient(ApiOrigin.parse("https://cantos.example"), HttpTransport { request ->
            requests += request
            ProbeResponse(404, "application/json", """{"code":"not_found","current_revision":null,"issues":[]}""")
        })
        assertEquals(ListenerOutcome.Unsupported(UnsupportedCapability.ListenerApi), client.probe())
        assertEquals("https://cantos.example/api/v1/theatre", requests.single().url)
        assertEquals(ListenerOutcome.Failed(HttpFailure.Protocol), classifyProbe(ProbeResponse(200, "application/json", "{}")))
        assertEquals(ListenerOutcome.Failed(HttpFailure.Protocol), classifyProbe(ProbeResponse(404, "text/html", "{}")))
    }
    @Test
    fun sessionChangeRejectsAnOldNoncooperativeResponse() = runTest {
        val response = CompletableDeferred<ProbeResponse>()
        val client = ListenerClient(ApiOrigin.parse("https://cantos.example"), HttpTransport { withContext(NonCancellable) { response.await() } })
        val controller = ListenerProbeController(backgroundScope, client)
        controller.refresh(); runCurrent()
        controller.sessionChanged(); runCurrent()
        response.complete(ProbeResponse(404, "application/json", """{"code":"not_found","current_revision":null,"issues":[]}""")); runCurrent()
        assertEquals(ListenerOutcome.Idle, controller.state.value.outcome)
        controller.close()
        for (capability in UnsupportedCapability.entries) assertEquals(ListenerOutcome.Unsupported(capability), client.capability(capability))
    }
    @Test
    fun cancellationPropagatesAndNewerRequestOrSessionBoundaryWins() = runTest {
        val first = CompletableDeferred<ProbeResponse>()
        var requests = 0
        val client = ListenerClient(ApiOrigin.parse("https://cantos.example"), HttpTransport {
            requests += 1
            if (requests == 1) withContext(NonCancellable) { first.await() } else ProbeResponse(503, "application/json", "{}")
        })
        val controller = ListenerProbeController(backgroundScope, client)
        controller.refresh(); runCurrent()
        controller.refresh(); runCurrent()
        assertEquals(ListenerOutcome.Failed(HttpFailure.Unavailable), controller.state.value.outcome)
        first.complete(ProbeResponse(404, "application/json", """{"code":"not_found","current_revision":null,"issues":[]}""")); runCurrent()
        assertEquals(ListenerOutcome.Failed(HttpFailure.Unavailable), controller.state.value.outcome)
        controller.sessionChanged(); runCurrent()
        assertEquals(ListenerOutcome.Idle, controller.state.value.outcome)
        controller.close()
        assertFailsWith<CancellationException> { ListenerClient(ApiOrigin.parse("https://cantos.example"), HttpTransport { throw CancellationException() }).probe() }
    }
}
