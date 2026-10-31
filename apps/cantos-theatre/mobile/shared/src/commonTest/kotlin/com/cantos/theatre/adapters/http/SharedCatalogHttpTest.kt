package com.cantos.theatre.adapters.http

import com.cantos.theatre.application.identity.NativeVerifiedSession
import com.cantos.theatre.domain.catalog.PublicCatalogSamples
import com.cantos.theatre.ports.NativeHttpPort
import com.cantos.theatre.ports.NativeHttpRequest
import com.cantos.theatre.ports.NativeHttpResponse
import com.cantos.theatre.ports.NativeRequestHandle
import com.cantos.theatre.ports.NativeSession
import com.cantos.theatre.ports.SharedCatalogFailure
import com.cantos.theatre.ports.SharedCatalogRead
import kotlinx.coroutines.async
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs
import kotlin.test.assertTrue

class SharedCatalogHttpTest {
    private val session = "00000000-0000-4000-8000-000000000001"
    private val origin = ApiOrigin.parse("https://api.example.org")
    private val lease = NativeVerifiedSession(NativeSession("actor-a", session, 121), "sentinel-bearer", 7)
    private val work = PublicCatalogSamples.work(1)
    private val workJson = """{"work_id":"${work.id.value}","kind":"story","title":"Sơn Hà Ký","description":"Lời hẹn","language":"vi-VN","first_visible_at_ms":5000,"unit_count":1,"cover":null}"""

    private class Transport(var reply: NativeHttpResponse) : NativeHttpPort {
        val requests = mutableListOf<NativeHttpRequest>()
        var beforeReply: () -> Unit = {}
        override fun send(request: NativeHttpRequest, completion: (NativeHttpResponse) -> Unit): NativeRequestHandle {
            requests += request
            beforeReply()
            completion(reply)
            return object : NativeRequestHandle { override fun cancel() = Unit }
        }
    }

    private fun ready(body: String) = NativeHttpResponse(200, "application/json", body, null, session)

    @OptIn(kotlinx.coroutines.ExperimentalCoroutinesApi::class)
    @Test fun request_deadline_cancels_native_transport_and_returns_no_stale_metadata() = runTest {
        var cancelled = false
        val http = object : NativeHttpPort {
            override fun send(request: NativeHttpRequest, completion: (NativeHttpResponse) -> Unit): NativeRequestHandle =
                object : NativeRequestHandle { override fun cancel() { cancelled = true } }
        }
        val adapter = SharedCatalogHttp(origin, http) { lease }
        val result = backgroundScope.async { adapter.works(lease.sharedKey(), null) }
        runCurrent()
        advanceTimeBy(15_001)
        runCurrent()
        assertEquals(SharedCatalogRead.Refused(SharedCatalogFailure.Unavailable), result.await())
        assertTrue(cancelled)
    }

    @Test fun shared_get_has_exact_bearer_session_and_route_but_no_public_cache_authority() = runTest {
        val http = Transport(ready("""{"items":[$workJson],"next_cursor":null}"""))
        val adapter = SharedCatalogHttp(origin, http) { lease }
        val page = assertIs<SharedCatalogRead.Ready<*>>(adapter.works(lease.sharedKey(), null))
        assertTrue(page.value is com.cantos.theatre.domain.catalog.PublicWorkPage)
        val request = http.requests.single()
        assertEquals("https://api.example.org/api/v1/theatre/shared/works?limit=20", request.url)
        assertEquals("GET", request.method)
        assertEquals("Bearer sentinel-bearer", request.headers["Authorization"])
        assertEquals(session, request.headers["X-Cantos-Theatre-Session"])
        assertEquals(4 * 1024 * 1024, request.maxBodyBytes)
        assertEquals("NativeHttpRequest([redacted])", request.toString())
    }

    @Test fun missing_session_and_account_switch_never_accept_response_or_retry_anonymous() = runTest {
        var current: NativeVerifiedSession? = null
        val http = Transport(ready(workJson))
        val adapter = SharedCatalogHttp(origin, http) { current }
        assertEquals(SharedCatalogRead.Refused(SharedCatalogFailure.SignInRequired), adapter.work(lease.sharedKey(), work.id))
        assertTrue(http.requests.isEmpty())
        current = lease
        http.beforeReply = { current = NativeVerifiedSession(NativeSession("actor-b", session, 121), "other-bearer", 8) }
        assertEquals(SharedCatalogRead.Refused(SharedCatalogFailure.SignInRequired), adapter.work(lease.sharedKey(), work.id))
        assertEquals(1, http.requests.size)
    }

    @Test fun wrong_or_missing_response_family_and_wrong_work_id_are_protocol_refusals() = runTest {
        for (actual in listOf(null, "different-session")) {
            val http = Transport(NativeHttpResponse(200, "application/json", workJson, null, actual))
            val adapter = SharedCatalogHttp(origin, http) { lease }
            assertEquals(SharedCatalogRead.Refused(SharedCatalogFailure.Protocol), adapter.work(lease.sharedKey(), work.id))
        }
        val http = Transport(ready(workJson.replace(work.id.value, PublicCatalogSamples.workId(2).value)))
        assertEquals(SharedCatalogRead.Refused(SharedCatalogFailure.Protocol), SharedCatalogHttp(origin, http) { lease }.work(lease.sharedKey(), work.id))
    }

    @Test fun opaque_cursor_is_kept_on_shared_page_and_invalid_cursor_requires_restart() = runTest {
        val cursor = PublicCatalogSamples.cursor(1)
        val http = Transport(NativeHttpResponse(400, "application/json", """{"code":"invalid_cursor","retryable":false}""", null))
        val adapter = SharedCatalogHttp(origin, http) { lease }
        assertEquals(SharedCatalogRead.Refused(SharedCatalogFailure.Restart), adapter.works(lease.sharedKey(), cursor))
        assertEquals("https://api.example.org/api/v1/theatre/shared/works?limit=20&cursor=${cursor.value}", http.requests.single().url)
        assertTrue(sharedCatalogReadPath("/shared/works?limit=20&cursor=${cursor.value}"))
        assertTrue(sharedCatalogReadPath("/shared/works/${work.id.value}/units?limit=20"))
        for (route in listOf("/works?limit=20", "/shared/claim", "/shared/media/token", "/shared/works?limit=51",
            "/shared/works?limit=20&actor=other", "/shared/works/${work.id.value}/../identity/session")) {
            assertEquals(false, sharedCatalogReadPath(route), route)
        }
    }

    @Test fun denied_unavailable_redirect_and_unknown_fields_are_distinct_and_never_public_retry() = runTest {
        val cases = listOf(
            NativeHttpResponse(404, "application/json", "{}", null) to SharedCatalogFailure.Denied,
            NativeHttpResponse(503, null, null, null) to SharedCatalogFailure.Unavailable,
            NativeHttpResponse(302, null, null, null) to SharedCatalogFailure.Protocol,
            ready(workJson.dropLast(1) + ",\"owner\":\"private\"}") to SharedCatalogFailure.Protocol,
            ready(workJson.replace("\"unit_count\":1", "\"unit_count\":\"1\"")) to SharedCatalogFailure.Protocol,
        )
        for ((reply, failure) in cases) {
            val http = Transport(reply)
            assertEquals(SharedCatalogRead.Refused(failure), SharedCatalogHttp(origin, http) { lease }.work(lease.sharedKey(), work.id))
            assertEquals(1, http.requests.size)
        }
    }

    @Test fun metadata_only_units_keep_null_playback_and_work_bound_identity() = runTest {
        val unit = PublicCatalogSamples.unitId(1).value
        val http = Transport(ready("""{"work_id":"${work.id.value}","items":[{"unit_id":"$unit","kind":"chapter","position":0,"title":"Một lời hẹn","description":"","ordinal":1,"playback":null}],"next_cursor":null}"""))
        val result = assertIs<SharedCatalogRead.Ready<com.cantos.theatre.domain.catalog.PublicUnitPage>>(
            SharedCatalogHttp(origin, http) { lease }.units(lease.sharedKey(), work, null))
        assertEquals(null, result.value.items.single().publicationId)
        assertEquals(work.id, result.value.work)
    }
}
