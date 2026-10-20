package com.cantos.theatre.adapters.library

import com.cantos.theatre.adapters.http.ApiOrigin
import com.cantos.theatre.adapters.library.wire.AudioIdentity as WireAudio
import com.cantos.theatre.adapters.library.wire.BookmarkWrite
import com.cantos.theatre.adapters.library.wire.Operation
import com.cantos.theatre.application.library.NativeLibraryController
import com.cantos.theatre.domain.library.BookmarkSync
import com.cantos.theatre.domain.library.LibrarySection
import com.cantos.theatre.domain.library.LibraryState
import com.cantos.theatre.domain.playback.AudioIdentity
import com.cantos.theatre.ports.LocalLibraryIntent
import com.cantos.theatre.ports.NativeHttpPort
import com.cantos.theatre.ports.NativeHttpRequest
import com.cantos.theatre.ports.NativeHttpResponse
import com.cantos.theatre.ports.NativeRequestHandle
import com.cantos.theatre.ports.NativeSession
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.asCoroutineDispatcher
import kotlinx.coroutines.delay
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withContext
import kotlinx.serialization.encodeToString
import org.junit.Assume
import org.junit.Test
import java.net.URI
import java.net.http.HttpClient
import java.net.http.HttpRequest
import java.net.http.HttpResponse
import java.nio.file.Files
import java.util.UUID
import java.util.concurrent.Executors
import kotlin.test.assertEquals
import kotlin.test.assertIs
import kotlin.test.assertNotNull
import kotlin.test.assertTrue

/**
 * The production Library sync adapter, codec, SQLite store and HTTP client against a **real** running Cantos API
 * and PostgreSQL. It runs only when `CANTOS_LIVE_API_ORIGIN` names a loopback API (the Rust harness
 * `cmp_production_client_against_the_real_api` starts one and checks the database afterwards); otherwise it is
 * skipped, never passed. The identity is the API test harness's synthetic bearer, and no audio decoder is involved.
 */
class LiveBookmarkApiTest {
    private val origin = System.getenv("CANTOS_LIVE_API_ORIGIN")
    private val alice = "synthetic-listener-alice"
    private val bob = "synthetic-listener-bob"

    /** A JDK HTTP port with two faults the real network has: no connectivity, and a response lost after commit. */
    private class LiveHttp : NativeHttpPort {
        @Volatile var offline = false
        @Volatile var loseMutationResponses = 0
        val sent = java.util.concurrent.CopyOnWriteArrayList<String>()
        private val client = HttpClient.newHttpClient()
        override fun send(request: NativeHttpRequest, completion: (NativeHttpResponse) -> Unit): NativeRequestHandle {
            if (offline) { completion(NativeHttpResponse(0, null, null, "offline")); return Handle {} }
            sent += "${request.method} ${request.url.substringAfter("/api/v1/theatre")}"
            val builder = HttpRequest.newBuilder(URI(request.url)).header("Accept", "application/json")
            request.bearer?.let { builder.header("Authorization", "Bearer $it") }
            if (request.body != null) builder.header("Content-Type", "application/json").method(request.method, HttpRequest.BodyPublishers.ofString(request.body))
            else builder.method(request.method, HttpRequest.BodyPublishers.noBody())
            val future = client.sendAsync(builder.build(), HttpResponse.BodyHandlers.ofString()).whenComplete { response, error ->
                when {
                    error != null -> completion(NativeHttpResponse(0, null, null, "timeout"))
                    request.method != "GET" && loseMutationResponses > 0 -> { loseMutationResponses -= 1; completion(NativeHttpResponse(0, null, null, "timeout")) }
                    else -> completion(NativeHttpResponse(response.statusCode(), response.headers().firstValue("content-type").orElse(null), response.body(), null, request.expectedSession))
                }
            }
            return Handle { future.cancel(true) }
        }
        private class Handle(val onCancel: () -> Unit) : NativeRequestHandle { override fun cancel() = onCancel() }
    }

    private class Device(origin: String, bearer: String, actor: String, folder: java.io.File, file: String, val http: LiveHttp = LiveHttp()) {
        private val executor = Executors.newSingleThreadExecutor()
        val dispatcher = executor.asCoroutineDispatcher()
        private val scope = CoroutineScope(SupervisorJob() + dispatcher)
        private val path = folder.resolve(file).absolutePath
        private var current: NativeLibraryController? = null
        val library: NativeLibraryController get() = checkNotNull(current)
        val api = NativeLibraryClient(ApiOrigin.parse(origin), http)
        val authorization = LibraryAuthorization(actor, SESSION, bearer)
        val bearer0 = bearer
        val actor0 = actor
        fun open() {
            current = nativeLibraryController(scope, api, desktopLibraryStore(path), { UUID.randomUUID().toString() }, nowMs = { System.currentTimeMillis() })
        }
        fun enter() = on { library.enter(NativeSession(actor0, SESSION, Long.MAX_VALUE, null), bearer0) }
        fun <T> on(block: () -> T): T = runBlocking { withContext(dispatcher) { block() } }
        fun close() { on { library.close() }; current = null }
        fun shutdown() { executor.shutdownNow() }
        val state: LibraryState get() = library.state.value
        companion object { const val SESSION = "00000000-0000-0000-0000-000000000001" }
    }

    private fun await(what: String, timeoutMs: Long = 20_000, condition: () -> Boolean) = runBlocking {
        val end = System.currentTimeMillis() + timeoutMs
        while (!condition()) {
            check(System.currentTimeMillis() < end) { "timed out waiting for: $what" }
            delay(25)
        }
    }

    @Test
    fun the_production_client_creates_lists_pages_restarts_syncs_and_deletes_against_the_real_api() {
        Assume.assumeTrue("CANTOS_LIVE_API_ORIGIN is not set: not a live run", origin != null)
        val live = requireNotNull(origin)
        val audio = Regex("\"(\\w+)\":\"?([^\",}]+)\"?").findAll(requireNotNull(System.getenv("CANTOS_LIVE_AUDIO"))).associate { it.groupValues[1] to it.groupValues[2] }
        val identity = requireNotNull(AudioIdentity.of(audio.getValue("work_id"), audio.getValue("unit_id"), audio.getValue("publication_id"),
            audio.getValue("rendition_id"), audio.getValue("sha256"), audio.getValue("duration_ms").toLong()))
        val d = identity.durationMs
        assertTrue(d >= 100, "the live fixture audio is too short for distinct positions")
        val seeded = requireNotNull(System.getenv("CANTOS_LIVE_SEEDED")).toInt()
        val folder = Files.createTempDirectory("cantos-live-bookmarks").toFile()
        val phone = Device(live, alice, "alice", folder, "phone.sqlite")
        val tablet = Device(live, alice, "alice", folder, "tablet.sqlite")
        val stranger = Device(live, bob, "bob", folder, "stranger.sqlite")
        try {
            listOf(phone, tablet, stranger).forEach { it.open() }
            // 1. The account's server-held bookmarks are read in bounded pages, newest first, each exactly once.
            phone.enter(); stranger.enter()
            await("first page") { phone.state.section(LibrarySection.Bookmarks).loaded && phone.state.bookmarks.size == 20 }
            assertNotNull(phone.state.cursors[LibrarySection.Bookmarks], "more pages exist")
            phone.on { phone.library.nextPage(LibrarySection.Bookmarks) }
            await("second page") { phone.state.bookmarks.size == seeded }
            assertEquals(seeded, phone.state.bookmarks.map { it.id }.toSet().size, "no duplicate across pages")
            assertEquals(phone.state.bookmarks.map { it.positionMs }.sortedDescending(), phone.state.bookmarks.map { it.positionMs }, "newest first")
            assertTrue(phone.state.bookmarks.all { it.audio == identity && it.sync == BookmarkSync.Synced })
            await("bob loaded") { stranger.state.section(LibrarySection.Bookmarks).loaded }
            assertTrue(stranger.state.bookmarks.isEmpty(), "another account sees none of them")

            // 2. A created bookmark is local at once, then confirmed by the server and read back from it.
            val first = phone.on { phone.library.createBookmark(identity, d - 1) }!!
            assertEquals(BookmarkSync.Pending, phone.state.bookmarks.first { it.id == first }.sync)
            await("synced") { phone.state.bookmarks.firstOrNull { it.id == first }?.sync == BookmarkSync.Synced && phone.state.pending == 0 }

            // 3. Offline: durable and visible as such; delivered exactly once on reconnect.
            phone.http.offline = true
            val offlineId = phone.on { phone.library.createBookmark(identity, d - 2) }!!
            await("offline marked") { phone.state.bookmarks.firstOrNull { it.id == offlineId }?.sync == BookmarkSync.Offline }
            phone.http.offline = false
            phone.on { phone.library.drain() }
            await("delivered after reconnect") { phone.state.bookmarks.firstOrNull { it.id == offlineId }?.sync == BookmarkSync.Synced && phone.state.pending == 0 }

            // 4. A response lost after the server committed: the client reads the receipt, never duplicates.
            phone.http.loseMutationResponses = 1
            val lost = phone.on { phone.library.createBookmark(identity, d - 3) }!!
            await("lost response reconciled", 40_000) { phone.state.bookmarks.firstOrNull { it.id == lost }?.sync == BookmarkSync.Synced && phone.state.pending == 0 }
            assertEquals(1, phone.state.bookmarks.count { it.positionMs == d - 3 })

            // 5. A second device of the same account converges; the stranger still sees nothing.
            tablet.enter()
            await("tablet synced") { tablet.state.section(LibrarySection.Bookmarks).loaded && tablet.state.bookmarks.any { it.id == lost } }
            assertEquals(setOf(first, offlineId, lost), tablet.state.bookmarks.map { it.id }.filter { it in setOf(first, offlineId, lost) }.toSet())
            stranger.on { stranger.library.refresh() }
            await("bob reread") { stranger.state.section(LibrarySection.Bookmarks).loaded }
            assertTrue(stranger.state.bookmarks.isEmpty())

            // 6. Delete on the tablet is synced; the phone sees it gone after its next read and never gets it back.
            tablet.on { tablet.library.deleteBookmark(first) }
            assertTrue(tablet.state.bookmarks.none { it.id == first }, "gone at once")
            await("delete delivered") { tablet.state.pending == 0 }
            phone.on { phone.library.refresh() }
            await("phone converged") { phone.state.bookmarks.none { it.id == first } && phone.state.section(LibrarySection.Bookmarks).loaded }

            // 7. A restart keeps the cache with no network, and the server's truth returns with it.
            phone.close()
            phone.http.offline = true
            phone.open(); phone.enter()
            assertTrue(phone.state.bookmarks.any { it.id == offlineId } && phone.state.bookmarks.none { it.id == first }, "durable rows survive a process restart")
            phone.http.offline = false
            phone.on { phone.library.refresh() }
            await("restart reread") { phone.state.section(LibrarySection.Bookmarks).loaded && phone.state.bookmarks.any { it.id == lost } }
            assertTrue(phone.state.bookmarks.none { it.id == first })

            // 8. Wire-level refusals from the real server: a deleted ID is never resurrected, positions are bounded,
            //    and a stranger's credential never reaches this account.
            val operation = { Operation(UUID.randomUUID().toString(), UUID.randomUUID().toString(), 1, null) }
            val wire = WireAudio(identity.workId, identity.unitId, identity.publicationId, identity.renditionId, identity.sha256, identity.durationMs)
            fun body(id: String, position: Long) = LibraryCodec.json.encodeToString(BookmarkWrite(operation(), id, wire, position))
            fun put(id: String, position: Long): LibraryResult<com.cantos.theatre.adapters.library.wire.Receipt> {
                val text = body(id, position)
                val op = LibraryCodec.decode<BookmarkWrite>("BookmarkWrite", text)!!.operation.operation_id
                return runBlocking { phone.api.mutate(phone.authorization, LocalLibraryIntent(op, "bookmark:$id", "PUT", "/library/bookmarks/$id", text)) }
            }
            val resurrected = assertIs<LibraryResult.Value<com.cantos.theatre.adapters.library.wire.Receipt>>(put(first, d - 1))
            assertEquals("bookmark_deleted", resurrected.value.outcome)
            assertEquals("keep_server", resurrected.value.resolution)
            assertEquals("invalid_position", assertIs<LibraryResult.Failed>(put(UUID.randomUUID().toString(), d + 1)).code)
            val foreign = LibraryAuthorization("alice", Device.SESSION, "synthetic-listener-mallory")
            val text = body(UUID.randomUUID().toString(), 1)
            val op = LibraryCodec.decode<BookmarkWrite>("BookmarkWrite", text)!!
            assertEquals("unauthenticated", assertIs<LibraryResult.Failed>(runBlocking {
                phone.api.mutate(foreign, LocalLibraryIntent(op.operation.operation_id, "bookmark:${op.bookmark_id}", "PUT", "/library/bookmarks/${op.bookmark_id}", text))
            }).code)
            println("LIVE-BOOKMARK-OK first=$first offline=$offlineId lost=$lost requests=${phone.http.sent.size + tablet.http.sent.size + stranger.http.sent.size}")
        } finally {
            listOf(phone, tablet, stranger).forEach { runCatching { it.close() }; it.shutdown() }
            folder.deleteRecursively()
        }
    }
}
