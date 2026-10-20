package com.cantos.theatre.adapters.http

import com.cantos.theatre.application.identity.NativeAccountController
import com.cantos.theatre.domain.identity.LoginSecret
import com.cantos.theatre.domain.identity.NativeClientConfiguration
import com.cantos.theatre.domain.identity.NativeGrant
import com.cantos.theatre.ports.CredentialFailure
import com.cantos.theatre.ports.IdentityCrypto
import com.cantos.theatre.ports.LogoutBarrier
import com.cantos.theatre.ports.LogoutBarrierRead
import com.cantos.theatre.ports.NativeApiFailure
import com.cantos.theatre.ports.NativeApiResult
import com.cantos.theatre.ports.NativeCredentialJournal
import com.cantos.theatre.ports.NativeCredentialRecord
import com.cantos.theatre.ports.NativeFlow
import com.cantos.theatre.ports.NativeHttpPort
import com.cantos.theatre.ports.NativeHttpRequest
import com.cantos.theatre.ports.NativeHttpResponse
import com.cantos.theatre.ports.NativeIdentityApi
import com.cantos.theatre.ports.NativeJournalRead
import com.cantos.theatre.ports.NativeRequestHandle
import com.cantos.theatre.ports.NativeSession
import com.cantos.theatre.ports.SystemBrowser
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.ExperimentalCoroutinesApi
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
    private class Fixture(scope: CoroutineScope) {
        val family = "00000000-0000-4000-8000-000000000001"
        val initialBearer = "ca1_n_" + "a".repeat(64)
        var now = 100L
        var remote = requireNotNull(NativeGrant.create(initialBearer, "cr1_" + "a".repeat(64),
            "actor-a", family, 220, 700, 1000, 100, now))
        var refreshes = 0
        var refreshGate: CompletableDeferred<Unit>? = null
        val configuration = requireNotNull(NativeClientConfiguration.create("https://api.invalid",
            "https://identity.invalid/oauth2/openid/cantos-theatre-native"))
        private val api = object : NativeIdentityApi {
            override suspend fun begin(state: String, nonce: String, challenge: String): NativeApiResult<NativeFlow> =
                NativeApiResult.Refused(NativeApiFailure.Unavailable)
            override suspend fun exchange(flow: String, state: String, code: String, verifier: String): NativeApiResult<NativeGrant> =
                NativeApiResult.Refused(NativeApiFailure.Unavailable)
            override suspend fun refresh(refreshToken: String, session: String): NativeApiResult<NativeGrant> {
                assertEquals(remote.refreshToken, refreshToken)
                assertEquals(family, session)
                refreshes += 1
                refreshGate?.await()
                val letter = ('a'.code + refreshes).toChar().toString().repeat(64)
                remote = requireNotNull(NativeGrant.create("ca1_n_$letter", "cr1_$letter", "actor-a", family,
                    now + 120, minOf(now + 600, 1000), 1000, 100, now, previous = remote))
                return NativeApiResult.Accepted(remote)
            }
            override suspend fun session(credential: String): NativeApiResult<NativeSession> =
                if (credential == remote.accessToken) NativeApiResult.Accepted(remote.view)
                else NativeApiResult.Refused(NativeApiFailure.Unauthorized)
            override suspend fun revokeFamily(refreshToken: String, session: String) = NativeApiResult.Accepted(Unit)
            override suspend fun cancel(flow: String, state: String, verifier: String) = NativeApiResult.Accepted(Unit)
            override suspend fun revoke(credential: String) = NativeApiResult.Accepted(Unit)
            override suspend fun saved(credential: String, session: String) = NativeApiResult.Accepted(0)
        }
        val account = NativeAccountController(scope, configuration, api, object : IdentityCrypto {
            override fun secret(): LoginSecret? = null
            override fun challenge(verifier: LoginSecret): String? = null
        }, object : SystemBrowser {
            override fun open(url: String) = false
            override fun cancel() = Unit
        }, object : NativeCredentialJournal {
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
        val http = NativeAuthorizedHttpPort(scope, ApiOrigin.parse(configuration.apiOrigin), account, object : NativeHttpPort {
            override fun send(request: NativeHttpRequest, completion: (NativeHttpResponse) -> Unit): NativeRequestHandle {
                val pending = Pending(request, completion)
                requests += pending
                return object : NativeRequestHandle { override fun cancel() { pending.cancelled = true } }
            }
        })
        fun request(method: String = "GET", route: String = "/library/saved?limit=20&presentation=1", bearer: String = initialBearer,
                    session: String = family) = NativeHttpRequest(method, configuration.apiOrigin + "/api/v1/theatre" + route,
            if (method == "GET") null else "{}", bearer, session)
        fun reply(index: Int = requests.lastIndex, status: Int = 200) {
            requests[index].completion(NativeHttpResponse(status, "application/json", "{}", null, family))
        }
        init { account.restore() }
        fun close() = account.close()
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
    fun anUnknownBearerOrOtherFamilyCannotBorrowTheCurrentAccount() = runTest {
        val f = Fixture(backgroundScope)
        try {
            runCurrent()
            val replies = mutableListOf<NativeHttpResponse>()
            f.http.send(f.request(bearer = "ca1_n_" + "c".repeat(64)), replies::add)
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
