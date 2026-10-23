package com.cantos.theatre.app

import com.cantos.theatre.adapters.http.ApiOrigin
import com.sun.net.httpserver.HttpExchange
import com.sun.net.httpserver.HttpServer
import java.net.InetAddress
import java.net.InetSocketAddress
import java.util.concurrent.CopyOnWriteArrayList
import java.util.concurrent.CountDownLatch
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit

/** What the fixture answers for one Library request, in place of the normal stamped 200. */
internal class LoopbackReply(val status: Int, val body: String, val stamp: String?, val dropAfterCommit: Boolean = false)

internal class LoopbackSeen(val method: String, val route: String, val authorization: String?, val session: String?, val cookie: String?)

/**
 * A loopback protocol fixture for the live Desktop Preview tests: Identity v1 login/session plus the closed
 * Library routes, each response stamped with the session family. It is mocked server evidence, not the Cantos
 * Server; [override] injects one-off faults and [gate] holds Library responses.
 */
internal class LoopbackServer(
    private val actor: String,
    private val family: String,
    private val work: String,
    private val access: String,
    private val refresh: String,
) {
    val seen = CopyOnWriteArrayList<LoopbackSeen>()
    val committed = CopyOnWriteArrayList<String>()
    @Volatile var override: ((LoopbackSeen) -> LoopbackReply?)? = null
    @Volatile var gate: CountDownLatch? = null
    private val executor = Executors.newCachedThreadPool { task -> Thread(task, "loopback-server").also { it.isDaemon = true } }
    private val server = HttpServer.create(InetSocketAddress(InetAddress.getLoopbackAddress(), 0), 0).also {
        it.executor = executor
        it.createContext("/", ::handle)
        it.start()
    }
    val origin: ApiOrigin = ApiOrigin.parse("http://127.0.0.1:${server.address.port}")
    fun stop() { server.stop(0); executor.shutdownNow() }

    /** The session's facts are fixed at its first login, as a real session's are: a changed fact is a protocol violation. */
    private val issuedAt: Long by lazy { System.currentTimeMillis() / 1000 }

    private fun pair(now: Long) = """{"token_type":"Bearer","access_token":"$access","refresh_token":"$refresh","actor_id":"$actor","session_id":"$family","expires_at":${now + 120},"refresh_expires_at":${now + 600},"absolute_expires_at":${now + 900},"authenticated_at":$now}"""
    private fun session(now: Long) = """{"actor_id":"$actor","session_id":"$family","expires_at":${now + 120},"absolute_expires_at":${now + 900},"authenticated_at":$now}"""

    private fun send(exchange: HttpExchange, status: Int, body: String, stamp: String? = null) {
        val bytes = body.toByteArray()
        exchange.responseHeaders.add("Content-Type", "application/json")
        stamp?.let { exchange.responseHeaders.add("X-Cantos-Theatre-Session", it) }
        if (status == 204) exchange.sendResponseHeaders(204, -1) else exchange.sendResponseHeaders(status, bytes.size.toLong())
        if (status != 204) exchange.responseBody.use { it.write(bytes) }
        exchange.close()
    }

    private fun handle(exchange: HttpExchange) {
        val route = exchange.requestURI.rawPath.removePrefix("/api/v1/theatre") + (exchange.requestURI.rawQuery?.let { "?$it" } ?: "")
        val body = exchange.requestBody.readBytes().decodeToString()
        val request = LoopbackSeen(exchange.requestMethod, route, exchange.requestHeaders.getFirst("Authorization"),
            exchange.requestHeaders.getFirst("X-Cantos-Theatre-Session"), exchange.requestHeaders.getFirst("Cookie"))
        seen += request
        when {
            route == "/native/identity/login" -> return send(exchange, 200, pair(issuedAt))
            route == "/native/identity/session" && request.method == "GET" -> return send(exchange, 200, session(issuedAt))
            route == "/native/identity/session" && request.method == "DELETE" -> return send(exchange, 204, "")
            route == "/native/identity/revoke" -> return send(exchange, 204, "")
            !route.startsWith("/library") && !route.startsWith("/operations") -> return send(exchange, 404, """{"code":"not_found","retryable":false}""")
        }
        gate?.await(10, TimeUnit.SECONDS)
        val operation = Regex("\"operation_id\":\"([a-f0-9-]{36})\"").find(body)?.groupValues?.get(1)
        val reply = override?.invoke(request) ?: normal(request, route, body, operation)
        if (reply.dropAfterCommit) {
            normal(request, route, body, operation)
            return exchange.close()
        }
        send(exchange, reply.status, reply.body, reply.stamp)
    }

    private fun receipt(operation: String, saved: Boolean) =
        """{"operation_id":"$operation","outcome":"accepted","resolution":null,"state":{"resource":"saved","current":{"work_id":"$work","saved":$saved,"revision":1,"listener_order":1,"accepted_at_ms":1000}}}"""

    private fun normal(request: LoopbackSeen, route: String, body: String, operation: String?): LoopbackReply = when {
        request.method == "GET" && route.startsWith("/library/history?") ->
            LoopbackReply(200, """{"state":{"revision":1,"epoch":0},"items":[],"next_cursor":null}""", family)
        request.method == "GET" && route.startsWith("/library/") -> LoopbackReply(200, """{"items":[],"next_cursor":null}""", family)
        request.method == "PUT" && route.startsWith("/library/saved/") && operation != null -> {
            committed += operation
            LoopbackReply(200, receipt(operation, body.contains("\"saved\":true")), family)
        }
        request.method == "GET" && route.startsWith("/operations/") -> {
            val id = route.substringAfterLast('/')
            if (id in committed) LoopbackReply(200, receipt(id, true), family) else LoopbackReply(404, """{"code":"not_found","retryable":false}""", family)
        }
        else -> LoopbackReply(404, """{"code":"not_found","retryable":false}""", family)
    }
}
