package com.cantos.theatre.adapters.http

import com.cantos.theatre.application.identity.NativeAccountController
import com.cantos.theatre.application.identity.StubIdentityV1Api
import com.cantos.theatre.application.identity.v1Access
import com.cantos.theatre.application.identity.v1Grant
import com.cantos.theatre.domain.identity.NativeGrant
import com.cantos.theatre.ports.CredentialFailure
import com.cantos.theatre.ports.IdentityV1Failure
import com.cantos.theatre.ports.IdentityV1Result
import com.cantos.theatre.ports.LogoutBarrier
import com.cantos.theatre.ports.LogoutBarrierRead
import com.cantos.theatre.ports.NativeCredentialJournal
import com.cantos.theatre.ports.NativeCredentialRecord
import com.cantos.theatre.ports.NativeHttpPort
import com.cantos.theatre.ports.NativeHttpRequest
import com.cantos.theatre.ports.NativeHttpResponse
import com.cantos.theatre.ports.NativeJournalRead
import com.cantos.theatre.ports.NativeRequestHandle
import com.cantos.theatre.ports.NativeSession
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNotEquals
import kotlin.test.assertNull
import kotlin.test.assertTrue

/** Real identity/transport owners over deterministic secure journal, API and HTTP doubles. */
@OptIn(ExperimentalCoroutinesApi::class)
class NativeAuthorizedHttpPortTest {
    private class Fixture(scope: CoroutineScope, val origin: ApiOrigin = ApiOrigin.parse("https://api.invalid")) {
        val family = "00000000-0000-4000-8000-000000000001"
        val initialBearer = v1Access('a')
        var now = 100L
        var remote = v1Grant('a', 'a', "actor-a", family, 220, 700, 1000, 100, now)
        var refreshes = 0
        var refreshGate: CompletableDeferred<Unit>? = null
        private val api = object : StubIdentityV1Api() {
            override suspend fun refresh(previous: NativeGrant): IdentityV1Result<NativeGrant> {
                assertEquals(remote.refreshToken, previous.refreshToken)
                assertEquals(family, previous.view.session)
                refreshes += 1
                refreshGate?.await()
                val letter = ('a'.code + refreshes).toChar()
                remote = v1Grant(letter, letter, "actor-a", family, now + 120, minOf(now + 600, 1000), 1000, 100, now, previous = remote)
                return IdentityV1Result.Accepted(remote)
            }
            override suspend fun session(previous: NativeGrant): IdentityV1Result<NativeSession> =
                if (previous.accessToken == remote.accessToken) IdentityV1Result.Accepted(remote.view)
                else IdentityV1Result.Refused(IdentityV1Failure.Unauthorized)
        }
        val account = NativeAccountController(scope, api, object : NativeCredentialJournal {
            private var record: NativeCredentialRecord? = NativeCredentialRecord.Active(remote)
            override fun read(): NativeJournalRead = record?.let(NativeJournalRead::Found) ?: NativeJournalRead.Missing
            override fun write(record: NativeCredentialRecord): CredentialFailure? { this.record = record; return null }
            override fun clear(): CredentialFailure? { record = null; return null }
        }, object : LogoutBarrier {
            private var pending = false
            override fun read() = if (pending) LogoutBarrierRead.Pending else LogoutBarrierRead.Clear
            override fun mark(): CredentialFailure? { pending = true; return null }
            override fun clear(): CredentialFailure? { pending = false; return null }
        }) { now }
        class Pending(val request: NativeHttpRequest, val completion: (NativeHttpResponse) -> Unit) { var cancelled = false }
        val requests = mutableListOf<Pending>()
        val http = NativeAuthorizedHttpPort(scope, origin, account, object : NativeHttpPort {
            override fun send(request: NativeHttpRequest, completion: (NativeHttpResponse) -> Unit): NativeRequestHandle {
                val pending = Pending(request, completion)
                requests += pending
                return object : NativeRequestHandle { override fun cancel() { pending.cancelled = true } }
            }
        })
        fun request(method: String = "GET", route: String = "/library/saved?limit=20&presentation=1", bearer: String = initialBearer,
                    session: String = family) = NativeHttpRequest(method, origin.toString() + "/api/v1/theatre" + route,
            if (method == "GET") null else "{}", bearer, session)
        fun reply(index: Int = requests.lastIndex, status: Int = 200) {
            requests[index].completion(NativeHttpResponse(status, "application/json", "{}", null, family))
        }
        init { account.restore() }
        fun close() = account.close()
    }

    @Test
    fun hostApprovedDebugLoopbackUsesTheProductionAuthorizedBoundary() = runTest {
        for (text in listOf("http://127.0.0.1:8080", "http://localhost:8081")) {
            val selected = selectCatalogOrigin(text, allowLoopbackHttp = true) as CatalogOriginSelection.Configured
            val f = Fixture(backgroundScope, selected.origin)
            try {
                runCurrent()
                f.http.send(f.request()) {}
                runCurrent()
                assertEquals(1, f.requests.size)
                assertEquals(text + "/api/v1/theatre/library/saved?limit=20&presentation=1", f.requests.single().request.url)
                assertEquals(f.initialBearer, f.requests.single().request.bearer)
                assertEquals(f.family, f.requests.single().request.expectedSession)
                val replies = mutableListOf<NativeHttpResponse>()
                for (foreign in listOf("http://localhost:8080", "http://127.0.0.1:8082", "https://127.0.0.1:8080")) {
                    if (foreign == text) continue
                    f.http.send(NativeHttpRequest("GET", foreign + "/api/v1/theatre/library/saved?limit=20&presentation=1",
                        null, f.initialBearer, f.family), replies::add)
                }
                runCurrent()
                assertEquals(1, f.requests.size, "scheme, host and port must all match")
                assertTrue(replies.all { it.failure == "protocol" })
                assertTrue(selectCatalogOrigin(text, allowLoopbackHttp = false) is CatalogOriginSelection.Rejected)
            } finally { f.close() }
        }
    }

    @Test
    fun retainedLibraryAuthorizationUsesRotatedAccessWithoutRepeatingBusinessRequest() = runTest {
        val f = Fixture(backgroundScope)
        try {
            runCurrent()
            f.now = 221
            var reply: NativeHttpResponse? = null
            f.http.send(f.request()) { reply = it }
            runCurrent()
            assertEquals(1, f.refreshes)
            assertEquals(1, f.requests.size)
            assertEquals(f.remote.accessToken, f.requests.single().request.bearer)
            assertNotEquals(f.initialBearer, f.requests.single().request.bearer)
            f.reply(status = 401); runCurrent()
            assertEquals(401, reply?.status)
            assertEquals(1, f.requests.size, "401 never repeats an account business request")
            assertEquals(1, f.refreshes)
        } finally { f.close() }
    }

    @Test
    fun cancellingOneRefreshWaiterDoesNotCancelSharedRotationOrAnotherRequest() = runTest {
        val f = Fixture(backgroundScope)
        try {
            runCurrent(); f.now = 221
            val gate = CompletableDeferred<Unit>().also { f.refreshGate = it }
            var first: NativeHttpResponse? = null
            var second: NativeHttpResponse? = null
            val cancelled = f.http.send(f.request()) { first = it }
            f.http.send(f.request()) { second = it }
            runCurrent()
            assertEquals(1, f.refreshes)
            cancelled.cancel(); runCurrent()
            gate.complete(Unit); runCurrent()
            assertEquals(1, f.requests.size)
            f.reply(); runCurrent()
            assertNull(first)
            assertEquals(200, second?.status)
            assertEquals(1, f.refreshes)
        } finally { f.close() }
    }

    @Test
    fun protectedRequestsCannotLeakCredentialsToAnotherOriginSurfaceAuthRouteOrTokenUrl() = runTest {
        val f = Fixture(backgroundScope)
        try {
            runCurrent()
            val urls = listOf("https://api.invalid.attacker.test/api/v1/theatre/library/saved?limit=20&presentation=1",
                "https://api.invalid@attacker.test/api/v1/theatre/library/saved?limit=20&presentation=1",
                "https://api.invalid/api/v1/studio/works", "https://api.invalid/api/v1/admin/session",
                "https://api.invalid/api/v1/theatre/session", "https://api.invalid/api/v1/theatre/auth/refresh",
                "https://api.invalid/api/v1/theatre/library/../session",
                "https://api.invalid/api/v1/theatre/library/%2e%2e/session",
                "https://api.invalid/api/v1/theatre/library/saved?limit=20&presentation=1&access_token=${f.initialBearer}",
                "https://api.invalid/api/v1/theatre/library/saved?limit=20&presentation=1#fragment")
            val replies = mutableListOf<NativeHttpResponse>()
            urls.forEach { f.http.send(NativeHttpRequest("GET", it, null, f.initialBearer, f.family), replies::add) }
            runCurrent()
            assertTrue(f.requests.isEmpty())
            assertEquals(urls.size, replies.size)
            assertTrue(replies.all { it.failure == "protocol" && it.body == null })
            assertEquals(0, f.refreshes)
        } finally { f.close() }
    }

    @Test
    fun libraryPagesAreSentOnlyWithTheClosedPresentationOptIn() = runTest {
        val f = Fixture(backgroundScope)
        try {
            runCurrent()
            val cursor = "c".repeat(64)
            val allowed = listOf("continue", "saved", "history").flatMap { section ->
                listOf("/library/$section?limit=20&presentation=1", "/library/$section?limit=20&presentation=1&cursor=$cursor")
            }
            val refused = listOf("/library/saved?limit=20", "/library/saved?limit=20&cursor=$cursor",
                "/library/saved?limit=20&presentation=0", "/library/saved?limit=20&presentation=true",
                "/library/saved?presentation=1&limit=20", "/library/saved?limit=20&presentation=1&presentation=1",
                "/library/saved?limit=20&presentation=1&cursor=${cursor.uppercase()}")
            val replies = mutableListOf<NativeHttpResponse>()
            (allowed + refused).forEach { f.http.send(f.request(route = it), replies::add) }
            runCurrent()
            assertEquals(allowed.size, f.requests.size, "only the opted-in page reads leave the process")
            assertEquals(allowed, f.requests.map { it.request.url.substringAfter("/api/v1/theatre") })
            assertTrue(replies.all { it.failure == "protocol" })
            assertEquals(refused.size, replies.size)
        } finally { f.close() }
    }

    @Test
    fun bookmarkRoutesLeaveTheProcessOnlyInTheirExactShapeAndWithoutThePresentationOptIn() = runTest {
        val f = Fixture(backgroundScope)
        try {
            runCurrent()
            val cursor = "c".repeat(64)
            val id = "b0000000-0000-4000-8000-000000000001"
            val allowed = listOf(
                "GET" to "/library/bookmarks?limit=20", "GET" to "/library/bookmarks?limit=20&cursor=$cursor",
                "PUT" to "/library/bookmarks/$id", "POST" to "/library/bookmarks/$id/delete",
            )
            val refused = listOf(
                "GET" to "/library/bookmarks?limit=20&presentation=1", "GET" to "/library/bookmarks", "GET" to "/library/bookmarks?limit=21",
                "GET" to "/library/bookmarks?limit=20&cursor=${cursor.uppercase()}", "GET" to "/library/bookmarks/$id",
                "PUT" to "/library/bookmarks", "PUT" to "/library/bookmarks/NOT-A-UUID", "PUT" to "/library/bookmarks/$id/delete",
                "POST" to "/library/bookmarks/$id", "POST" to "/library/bookmarks/$id/delete/extra", "POST" to "/library/bookmarks/../session/delete",
                "DELETE" to "/library/bookmarks/$id",
            )
            val replies = mutableListOf<NativeHttpResponse>()
            (allowed + refused).forEach { (method, route) -> f.http.send(f.request(method, route), replies::add) }
            runCurrent()
            assertEquals(allowed.map { it.second }, f.requests.map { it.request.url.substringAfter("/api/v1/theatre") })
            assertEquals(refused.size, replies.size)
            assertTrue(replies.all { it.failure == "protocol" })
        } finally { f.close() }
    }

    @Test
    fun theProductionLibraryClientsBookmarkRequestsPassTheProductionAuthorizedPort() = runTest {
        val f = Fixture(backgroundScope)
        try {
            runCurrent()
            val client = com.cantos.theatre.adapters.library.NativeLibraryClient(f.origin, f.http)
            val auth = com.cantos.theatre.adapters.library.LibraryAuthorization("actor", f.family, f.initialBearer)
            val write = com.cantos.theatre.adapters.library.libraryBookmarkFixtures.getValue("bookmark_write.json")
            val delete = com.cantos.theatre.adapters.library.libraryBookmarkFixtures.getValue("bookmark_delete.json")
            fun operation(body: String) = Regex("\"operation_id\":\\s*\"([^\"]+)\"").find(body)!!.groupValues[1]
            val bookmark = "00000000-0000-4000-8000-0000000000b1"
            val calls = listOf(
                backgroundScope.launch { client.bookmarkPage(auth) },
                backgroundScope.launch { client.bookmarkPage(auth, "c".repeat(64)) },
                backgroundScope.launch { client.mutate(auth, com.cantos.theatre.ports.LocalLibraryIntent(operation(write), "bookmark:$bookmark", "PUT", "/library/bookmarks/$bookmark", write)) },
                backgroundScope.launch { client.mutate(auth, com.cantos.theatre.ports.LocalLibraryIntent(operation(delete), "bookmark:$bookmark", "POST", "/library/bookmarks/$bookmark/delete", delete)) },
            )
            runCurrent()
            assertEquals(4, f.requests.size, "no bookmark request is refused before the network")
            assertEquals(listOf("GET /library/bookmarks?limit=20", "GET /library/bookmarks?limit=20&cursor=${"c".repeat(64)}",
                "PUT /library/bookmarks/$bookmark", "POST /library/bookmarks/$bookmark/delete"),
                f.requests.map { it.request.method + " " + it.request.url.substringAfter("/api/v1/theatre") })
            calls.forEach { it.cancel() }
        } finally { f.close() }
    }

    @Test
    fun anUnknownBearerOrOtherFamilyCannotBorrowTheCurrentAccount() = runTest {
        val f = Fixture(backgroundScope)
        try {
            runCurrent()
            val replies = mutableListOf<NativeHttpResponse>()
            f.http.send(f.request(bearer = v1Access('c')), replies::add)
            f.http.send(f.request(session = "00000000-0000-4000-8000-000000000002"), replies::add)
            runCurrent()
            assertTrue(f.requests.isEmpty())
            assertEquals(listOf("session_changed", "session_changed"), replies.map { it.failure })
        } finally { f.close() }
    }

    @Test
    fun logoutDropsLateAccountBodyAndNeverReplaysAnUncertainMutation() = runTest {
        val f = Fixture(backgroundScope)
        try {
            runCurrent()
            var reply: NativeHttpResponse? = null
            f.http.send(f.request("POST", "/library/history/clear")) { reply = it }
            runCurrent()
            assertEquals(1, f.requests.size)
            f.account.logout(); runCurrent()
            f.reply(); runCurrent()
            assertEquals("authority_changed", reply?.failure)
            assertNull(reply?.body)
            assertEquals(1, f.requests.size)
            assertFalse(f.account.belongsToCurrent(f.initialBearer, f.family))
        } finally { f.close() }
    }

    @Test
    fun cancellationStopsTheOwnedTransportAndSuppressesItsLateCallback() = runTest {
        val f = Fixture(backgroundScope)
        try {
            runCurrent()
            var reply: NativeHttpResponse? = null
            val handle = f.http.send(f.request()) { reply = it }
            runCurrent(); handle.cancel(); runCurrent()
            assertTrue(f.requests.single().cancelled)
            f.reply(); runCurrent()
            assertNull(reply)
        } finally { f.close() }
    }

    @Test
    fun rotationDropsAResponseIssuedWithAnOlderLeaseBeforeTheNewLeaseIsPublished() = runTest {
        val f = Fixture(backgroundScope)
        try {
            runCurrent()
            var reply: NativeHttpResponse? = null
            f.http.send(f.request()) { reply = it }
            runCurrent()
            f.now = 221
            f.account.authorization(f.family)
            runCurrent()
            f.reply(); runCurrent()
            assertEquals("authority_changed", reply?.failure)
            assertNull(reply?.body)
            assertEquals(1, f.requests.size)
        } finally { f.close() }
    }
}
