package com.cantos.theatre.app

import com.cantos.theatre.adapters.http.*
import com.sun.net.httpserver.HttpServer
import java.net.CookieHandler
import java.net.CookieManager
import java.net.InetSocketAddress
import java.util.concurrent.atomic.AtomicInteger
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlinx.coroutines.runBlocking

class ListenerHttpTransportTest {
    @Test
    fun realLoopbackHttpUsesReservedRouteAndOmitsCookieAndBearer() = runBlocking {
        val server = HttpServer.create(InetSocketAddress("127.0.0.1", 0), 0)
        val calls = AtomicInteger()
        server.createContext("/api/v1/theatre") { exchange ->
            calls.incrementAndGet()
            assertEquals("GET", exchange.requestMethod)
            assertEquals(null, exchange.requestHeaders.getFirst("Authorization"))
            assertEquals("", exchange.requestHeaders.getFirst("Cookie") ?: "")
            val body = """{"code":"not_found","current_revision":null,"issues":[]}""".encodeToByteArray()
            exchange.responseHeaders.add("Content-Type", "application/json")
            exchange.sendResponseHeaders(404, body.size.toLong())
            exchange.responseBody.use { it.write(body) }
        }
        server.start()
        try {
            val client = ListenerClient(ApiOrigin.parse("http://127.0.0.1:${server.address.port}"), JvmHttpTransport())
            assertEquals(ListenerOutcome.Unsupported(UnsupportedCapability.ListenerApi), client.probe())
            assertEquals(1, calls.get())
            val original = CookieHandler.getDefault()
            try {
                CookieHandler.setDefault(CookieManager())
                assertEquals(ListenerOutcome.Failed(HttpFailure.CookieRuntime), client.probe())
                assertEquals(1, calls.get())
            } finally { CookieHandler.setDefault(original) }
        } finally { server.stop(0) }
    }
    @Test
    fun largeResponseAndRedirectNeverBecomeSuccessOrFollowAnotherSurface() = runBlocking {
        val server = HttpServer.create(InetSocketAddress("127.0.0.1", 0), 0)
        val privateCalls = AtomicInteger()
        var large = true
        server.createContext("/api/v1/theatre") { exchange ->
            if (large) {
                val bytes = ByteArray(8193) { 'a'.code.toByte() }
                exchange.sendResponseHeaders(404, bytes.size.toLong())
                exchange.responseBody.use { it.write(bytes) }
            } else {
                exchange.responseHeaders.add("Location", "/api/v1/studio/works")
                exchange.sendResponseHeaders(302, -1)
                exchange.close()
            }
        }
        server.createContext("/api/v1/studio/works") { privateCalls.incrementAndGet(); it.sendResponseHeaders(200, -1); it.close() }
        server.start()
        try {
            val client = ListenerClient(ApiOrigin.parse("http://127.0.0.1:${server.address.port}"), JvmHttpTransport())
            assertEquals(ListenerOutcome.Failed(HttpFailure.Transport), client.probe())
            large = false
            assertEquals(ListenerOutcome.Failed(HttpFailure.Protocol), client.probe())
            assertEquals(0, privateCalls.get())
        } finally { server.stop(0) }
    }
}
