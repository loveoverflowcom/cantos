package com.cantos.theatre.app

import com.cantos.theatre.adapters.http.ApiOrigin
import com.cantos.theatre.adapters.http.CatalogBodyLimitBytes
import com.cantos.theatre.adapters.http.JvmCatalogTransport
import com.cantos.theatre.adapters.http.PublicCatalogHttpAdapter
import com.cantos.theatre.application.catalog.DetailPhase
import com.cantos.theatre.application.catalog.PublicCatalogController
import com.cantos.theatre.application.catalog.PublicCatalogIntent
import com.cantos.theatre.application.catalog.WalkPhase
import com.cantos.theatre.ports.CatalogProtocolFault
import com.cantos.theatre.ports.PublicCatalogRead
import com.sun.net.httpserver.HttpExchange
import com.sun.net.httpserver.HttpServer
import java.net.CookieHandler
import java.net.CookieManager
import java.net.InetSocketAddress
import java.util.Collections
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.cancelAndJoin
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout

/**
 * Mock-integration evidence: the production adapter and HttpURLConnection transport against a
 * loopback JDK server on an ephemeral port serving contract-shaped JSON. Not the Rust API,
 * PostgreSQL or an Android runtime.
 */
class PublicCatalogHttpTransportTest {
    private val work1 = "10000000-0000-4000-8000-000000000001"
    private val work2 = "10000000-0000-4000-8000-000000000002"
    private val work3 = "10000000-0000-4000-8000-000000000003"
    private val cursor1 = "lc1_" + "a".repeat(64)
    private val unitCursor = "lc1_" + "b".repeat(64)

    private fun workJson(id: String, kind: String, title: String) =
        """{"work_id":"$id","kind":"$kind","title":"$title","description":"Một lời hẹn chưa bao giờ nói ra.","language":"vi-VN","first_published_at_ms":5000}"""

    private fun unitJson(n: Int, kind: String, publication: Int) =
        """{"unit_id":"20000000-0000-4000-8000-00000000000$n","kind":"$kind","position":$n,"title":"Chương $n","description":"","publication_id":"80000000-0000-4000-8000-00000000000$publication","published_at_ms":5000,"duration_ms":${n * 61_000}}"""

    private class Server(handler: (HttpExchange) -> Unit) : AutoCloseable {
        val server: HttpServer = HttpServer.create(InetSocketAddress("127.0.0.1", 0), 0).apply {
            createContext("/") { exchange -> try { handler(exchange) } finally { exchange.close() } }
            start()
        }
        val origin: ApiOrigin get() = ApiOrigin.parse("http://127.0.0.1:${server.address.port}")
        override fun close() = server.stop(0)
    }

    private fun HttpExchange.json(status: Int, body: String) {
        val bytes = body.encodeToByteArray()
        responseHeaders.add("Content-Type", "application/json")
        responseHeaders.add("Cache-Control", "no-store")
        sendResponseHeaders(status, bytes.size.toLong())
        responseBody.write(bytes)
    }

    @Test
    fun controller_walks_the_real_http_catalog_with_server_identities_and_anonymous_gets() = runBlocking {
        val requests = Collections.synchronizedList(mutableListOf<String>())
        val violations = Collections.synchronizedList(mutableListOf<String>())
        Server { exchange ->
            val headers = exchange.requestHeaders
            if (exchange.requestMethod != "GET") violations += "method ${exchange.requestMethod}"
            if (headers.getFirst("Accept") != "application/json") violations += "accept ${headers.getFirst("Accept")}"
            if (headers.getFirst("Authorization") != null) violations += "authorization header"
            if (!(headers.getFirst("Cookie") ?: "").isEmpty()) violations += "cookie header"
            if (headers.getFirst("X-Cantos-Theatre-Session") != null) violations += "session header"
            val path = exchange.requestURI.rawPath
            val query = exchange.requestURI.rawQuery
            requests += "$path?$query"
            when {
                path == "/api/v1/theatre/works" && query == "limit=20" -> exchange.json(200,
                    """{"items":[${workJson(work1, "story", "Ánh đèn cuối sân khấu")},${workJson(work2, "podcast", "Người dẫn chuyện")}],"next_cursor":"$cursor1"}""")
                path == "/api/v1/theatre/works" && query == "limit=20&cursor=$cursor1" -> exchange.json(200,
                    """{"items":[${workJson(work3, "novel", "Tiếng chuông trong sương")}],"next_cursor":null}""")
                path == "/api/v1/theatre/works/$work1" -> exchange.json(200, workJson(work1, "story", "Ánh đèn cuối sân khấu"))
                path == "/api/v1/theatre/works/$work1/units" && query == "limit=20" -> exchange.json(200,
                    """{"work_id":"$work1","items":[${unitJson(1, "chapter", 1)},${unitJson(2, "chapter", 2)}],"next_cursor":"$unitCursor"}""")
                path == "/api/v1/theatre/works/$work1/units" && query == "limit=20&cursor=$unitCursor" -> exchange.json(200,
                    """{"work_id":"$work1","items":[${unitJson(3, "chapter", 3)}],"next_cursor":null}""")
                else -> exchange.json(404, """{"code":"not_found","retryable":false}""")
            }
        }.use { server ->
            val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
            try {
                val controller = PublicCatalogController(scope, PublicCatalogHttpAdapter(server.origin, JvmCatalogTransport()))
                withTimeout(10_000) {
                    controller.dispatch(PublicCatalogIntent.Start)
                    controller.state.first { it.works.phase == WalkPhase.More }
                    controller.dispatch(PublicCatalogIntent.LoadMoreWorks)
                    val listed = controller.state.first { it.works.phase == WalkPhase.Complete }.works.items
                    assertEquals(listOf(work1, work2, work3), listed.map { it.id.value })
                    controller.dispatch(PublicCatalogIntent.OpenWork(listed.first()))
                    controller.state.first { it.detail?.units?.phase == WalkPhase.More }
                    controller.dispatch(PublicCatalogIntent.LoadMoreUnits)
                    val detail = controller.state.first { it.detail?.units?.phase == WalkPhase.Complete }.detail!!
                    assertEquals(work1, (detail.phase as DetailPhase.Ready).work.id.value)
                    val units = detail.units!!.items
                    assertEquals((1..3).map { "20000000-0000-4000-8000-00000000000$it" }, units.map { it.id.value })
                    assertEquals((1..3).map { "80000000-0000-4000-8000-00000000000$it" }, units.map { it.publicationId.value })
                    assertEquals(listOf(61_000L, 122_000L, 183_000L), units.map { it.durationMs })
                }
                controller.close()
            } finally { scope.cancel() }
            assertEquals(
                listOf(
                    "/api/v1/theatre/works?limit=20",
                    "/api/v1/theatre/works?limit=20&cursor=$cursor1",
                    "/api/v1/theatre/works/$work1?null",
                    "/api/v1/theatre/works/$work1/units?limit=20",
                    "/api/v1/theatre/works/$work1/units?limit=20&cursor=$unitCursor",
                ),
                requests.toList(),
            )
            assertEquals(emptyList(), violations.toList())
        }
    }

    @Test
    fun oversized_bodies_are_refused_with_or_without_content_length() = runBlocking {
        var chunked = false
        Server { exchange ->
            exchange.responseHeaders.add("Content-Type", "application/json")
            exchange.sendResponseHeaders(200, if (chunked) 0 else CatalogBodyLimitBytes + 1L)
            val block = ByteArray(64 * 1024) { ' '.code.toByte() }
            var written = 0
            try {
                while (written <= CatalogBodyLimitBytes) {
                    exchange.responseBody.write(block)
                    written += block.size
                }
            } catch (_: java.io.IOException) {
                // The client stopped reading at its bound.
            }
        }.use { server ->
            val adapter = PublicCatalogHttpAdapter(server.origin, JvmCatalogTransport())
            assertEquals(PublicCatalogRead.Protocol(CatalogProtocolFault.Oversized), adapter.works(null))
            chunked = true
            assertEquals(PublicCatalogRead.Protocol(CatalogProtocolFault.Oversized), adapter.works(null))
        }
    }

    @Test
    fun redirects_are_never_followed_and_ambient_cookies_refuse_the_request() = runBlocking {
        val studioCalls = AtomicInteger()
        val calls = AtomicInteger()
        Server { exchange ->
            if (exchange.requestURI.rawPath.startsWith("/api/v1/studio")) {
                studioCalls.incrementAndGet()
                exchange.json(200, "{}")
            } else {
                calls.incrementAndGet()
                exchange.responseHeaders.add("Location", "/api/v1/studio/works")
                exchange.sendResponseHeaders(302, -1)
            }
        }.use { server ->
            val adapter = PublicCatalogHttpAdapter(server.origin, JvmCatalogTransport())
            assertEquals(PublicCatalogRead.Protocol(CatalogProtocolFault.UnexpectedStatus), adapter.works(null))
            assertEquals(0, studioCalls.get())
            val original = CookieHandler.getDefault()
            try {
                CookieHandler.setDefault(CookieManager())
                assertEquals(PublicCatalogRead.Transport, adapter.works(null))
                assertEquals(1, calls.get(), "no request is sent through a cookie runtime")
            } finally {
                CookieHandler.setDefault(original)
            }
        }
    }

    @Test
    fun a_real_socket_deadline_is_a_timeout_not_a_generic_transport_failure() = runBlocking {
        Server { exchange ->
            Thread.sleep(150)
            runCatching { exchange.json(200, """{"items":[],"next_cursor":null}""") }
        }.use { server ->
            val adapter = PublicCatalogHttpAdapter(server.origin, JvmCatalogTransport(readTimeoutMs = 25))
            assertEquals(PublicCatalogRead.AccessFailure(com.cantos.theatre.ports.CatalogAccessFault.Timeout), adapter.works(null))
        }
    }

    @Test
    fun cancellation_disconnects_a_stalled_read_and_a_closed_port_is_a_transport_failure() = runBlocking {
        val received = CountDownLatch(1)
        val release = CountDownLatch(1)
        Server { exchange ->
            received.countDown()
            release.await(10, TimeUnit.SECONDS)
            runCatching { exchange.json(200, """{"items":[],"next_cursor":null}""") }
        }.use { server ->
            val adapter = PublicCatalogHttpAdapter(server.origin, JvmCatalogTransport())
            val read = launch(Dispatchers.Default) { adapter.works(null) }
            assertTrue(received.await(5, TimeUnit.SECONDS))
            val started = System.nanoTime()
            withTimeout(3_000) { read.cancelAndJoin() }
            assertTrue(TimeUnit.NANOSECONDS.toMillis(System.nanoTime() - started) < 3_000)
            release.countDown()
        }
        val closed = HttpServer.create(InetSocketAddress("127.0.0.1", 0), 0)
        val port = closed.address.port
        closed.start()
        closed.stop(0)
        val adapter = PublicCatalogHttpAdapter(ApiOrigin.parse("http://127.0.0.1:$port"), JvmCatalogTransport(connectTimeoutMs = 2_000))
        assertEquals(PublicCatalogRead.Transport, adapter.works(null))
    }
}
