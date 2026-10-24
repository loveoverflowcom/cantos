package com.cantos.theatre.adapters.http

import com.cantos.theatre.domain.identity.CompletionStatus
import com.cantos.theatre.domain.identity.IdentityV1Validation
import com.cantos.theatre.domain.identity.NativeGrant
import com.cantos.theatre.domain.identity.NativeSession
import com.cantos.theatre.domain.identity.RegistrationEmail
import com.cantos.theatre.domain.identity.RegistrationResult
import com.cantos.theatre.domain.identity.SignupCompletionRequest
import com.cantos.theatre.ports.IdentitySignupAccepted
import com.cantos.theatre.ports.IdentityV1Failure
import com.cantos.theatre.ports.IdentityV1Result
import com.cantos.theatre.ports.NativeHttpFailure
import com.cantos.theatre.ports.NativeHttpPort
import com.cantos.theatre.ports.NativeHttpRequest
import com.cantos.theatre.ports.NativeHttpResponse
import com.cantos.theatre.ports.NativeRequestHandle
import kotlinx.coroutines.async
import kotlinx.coroutines.cancelAndJoin
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertIs
import kotlin.test.assertTrue

@OptIn(kotlinx.coroutines.ExperimentalCoroutinesApi::class)
class NativeIdentityV1ClientTest {
    private val origin = "https://api.example:8443"
    private val apiOrigin = ApiOrigin.parse(origin)
    private val email = assertIs<RegistrationResult.Accepted<RegistrationEmail>>(RegistrationEmail.parse("listener@example.org")).value
    private val access = "ci1_n_" + "a".repeat(64)
    private val refresh = "ci1_r_" + "b".repeat(64)
    private val session = "11111111-2222-4333-8444-555555555555"
    private val pair = """{"token_type":"Bearer","access_token":"$access","refresh_token":"$refresh","actor_id":"listener_a","session_id":"$session","expires_at":1120,"refresh_expires_at":1600,"absolute_expires_at":1900,"authenticated_at":1000}"""
    private val sessionBody = """{"actor_id":"listener_a","session_id":"$session","expires_at":1120,"absolute_expires_at":1900,"authenticated_at":1000}"""
    private val unknown = IdentityV1Result.Refused(IdentityV1Failure.UnknownOutcome)
    private val protocol = IdentityV1Result.Refused(IdentityV1Failure.Protocol)

    private class RecordingHttp : NativeHttpPort {
        val requests = mutableListOf<NativeHttpRequest>()
        var response: NativeHttpResponse? = null
        var cancelled = 0
        var completion: ((NativeHttpResponse) -> Unit)? = null
        override fun send(request: NativeHttpRequest, completion: (NativeHttpResponse) -> Unit): NativeRequestHandle {
            requests += request
            this.completion = completion
            response?.let(completion)
            return object : NativeRequestHandle { override fun cancel() { cancelled += 1 } }
        }
    }
    private fun response(body: String = pair, status: Int = 200) = NativeHttpResponse(status, "application/json", body, null)
    private fun client(http: RecordingHttp, at: ApiOrigin = apiOrigin) = NativeIdentityV1Client(at, http) { 1000L }
    private fun grant() = assertIs<IdentityV1Validation.Accepted<NativeGrant>>(NativeGrant.identityV1(
        access, refresh, "listener_a", session, 1120, 1600, 1900, 1000, 1000,
    )).value
    private fun replace(body: String = pair, key: String, value: JsonPrimitive): String =
        JsonObject((Json.parseToJsonElement(body) as JsonObject) + (key to value)).toString()

    private fun request(
        http: RecordingHttp, method: String, path: String, body: String?, bearer: String? = null, fence: String? = null,
    ) {
        val actual = http.requests.last()
        assertEquals(method, actual.method)
        assertEquals("$origin/api/v1/theatre/native/identity$path", actual.url)
        assertEquals(body, actual.body)
        assertEquals(bearer, actual.bearer)
        assertEquals(fence, actual.expectedSession)
        val headers = buildMap {
            put("Accept", "application/json")
            if (body != null) put("Content-Type", "application/json")
            if (bearer != null) put("Authorization", "Bearer $bearer")
            if (fence != null) put("X-Cantos-Theatre-Session", fence)
        }
        assertEquals(headers, actual.headers)
        assertFalse(actual.headers.keys.any { it.equals("Cookie", true) || it.equals("Origin", true) || it.startsWith("Sec-Fetch-", true) })
        assertEquals("NativeHttpRequest([redacted])", actual.toString())
    }

    @Test fun signup_only_sends_initiation_email_and_returns_generic_ttl() = runTest {
        val http = RecordingHttp().apply { response = response("""{"accepted":true,"proof_ttl_seconds":900}""", 202) }
        val accepted = assertIs<IdentityV1Result.Accepted<IdentitySignupAccepted>>(client(http).signup(email)).value
        assertEquals(900L, accepted.proofTtlSeconds)
        request(http, "POST", "/signup", """{"email":"listener@example.org"}""")
        assertEquals(1, http.requests.size)
        for (body in listOf("""{"accepted":false,"proof_ttl_seconds":900}""", """{"accepted":true,"proof_ttl_seconds":0}""",
            """{"accepted":true,"proof_ttl_seconds":4294967296}""", """{"accepted":true,"proof_ttl_seconds":"900"}""",
            """{"accepted":true,"proof_ttl_seconds":900,"session_id":"$session"}""",
            """{"accepted":true,"accepted":true,"proof_ttl_seconds":900}""")) {
            http.response = response(body, 202)
            assertEquals(unknown, client(http).signup(email))
        }
    }

    @Test fun signup_completion_is_anonymous_exact_json_and_never_a_grant() = runTest {
        val op = "0123456789abcdef0123456789abcdef"
        val proof = "fedcba9876543210fedcba9876543210"
        val password = "  Ánh đèn \"cuối\" sân khấu  "
        val completion = SignupCompletionRequest(op, proof, "synthetic-Action_code.1", password)
        val http = RecordingHttp().apply { response = response("""{"status":"completed"}""") }
        assertEquals(CompletionStatus.Completed, assertIs<IdentityV1Result.Accepted<CompletionStatus>>(client(http).completeSignup(completion)).value)
        // No Bearer and no session fence, whoever is signed in; the body is the Web body without a native secret.
        request(http, "POST", "/signup/complete", """{"operation_id":"$op","proof_id":"$proof","firebase_oob_code":"synthetic-Action_code.1","password":"  Ánh đèn \"cuối\" sân khấu  ","explicit_confirmation":true}""")
        val first = http.requests.last().body
        http.response = response("""{"status":"replayed"}""")
        assertEquals(CompletionStatus.Replayed, assertIs<IdentityV1Result.Accepted<CompletionStatus>>(client(http).completeSignup(completion)).value)
        assertEquals(first, http.requests.last().body, "a retained request is repeated byte for byte")
        for (body in listOf("""{"status":"created"}""", """{"status":"completed","session_id":"$session"}""", pair,
            """{"status":"completed","status":"replayed"}""", "")) {
            http.response = response(body)
            assertEquals(unknown, client(http).completeSignup(completion))
        }
        http.response = response("""{"status":"completed"}""", 201)
        assertEquals(unknown, client(http).completeSignup(completion))
        for ((status, reason) in listOf(400 to IdentityV1Failure.Protocol, 401 to IdentityV1Failure.Unauthorized,
            403 to IdentityV1Failure.Denied, 404 to IdentityV1Failure.NotConfigured, 503 to IdentityV1Failure.ServiceUnavailable)) {
            http.response = NativeHttpResponse(status, "application/json", """{"error":"x"}""", null)
            assertEquals(IdentityV1Result.Refused(reason), client(http).completeSignup(completion))
        }
        http.response = NativeHttpResponse(0, null, null, NativeHttpFailure.Timeout)
        assertEquals(unknown, client(http).completeSignup(completion))
    }

    @Test fun login_uses_exact_json_and_anonymous_headers_without_altering_password() = runTest {
        val http = RecordingHttp().apply { response = response() }
        val password = "  quote\"slash\\\n秘密  "
        val result = assertIs<IdentityV1Result.Accepted<NativeGrant>>(client(http).login(email, password)).value
        assertEquals(access, result.accessToken)
        assertEquals(refresh, result.refreshToken)
        assertEquals(1000L, result.authenticatedAt)
        request(http, "POST", "/login", """{"email":"listener@example.org","password":"  quote\"slash\\\n秘密  "}""")
        assertEquals(1, http.requests.size)
        assertEquals("NativeGrant([redacted])", result.toString())
        assertEquals("NativeHttpResponse([redacted])", http.response.toString())
        assertEquals(protocol, client(http).login(email, "密".repeat(4096)))
        assertEquals(1, http.requests.size)
    }

    @Test fun refresh_and_revoke_use_only_refresh_and_session_fields() = runTest {
        val previous = grant()
        val rotated = pair.replace(access, "ci1_n_" + "c".repeat(64)).replace(refresh, "ci1_r_" + "d".repeat(64))
        val http = RecordingHttp().apply { response = response(rotated) }
        assertIs<IdentityV1Result.Accepted<NativeGrant>>(client(http).refresh(previous))
        val body = """{"refresh_token":"$refresh","session_id":"$session"}"""
        request(http, "POST", "/refresh", body, fence = session)
        http.response = response("", 204)
        assertIs<IdentityV1Result.Accepted<Unit>>(client(http).revokeFamily(previous))
        request(http, "POST", "/revoke", body, fence = session)
        assertEquals(2, http.requests.size)
        val expired = assertIs<IdentityV1Validation.Accepted<NativeGrant>>(NativeGrant.restoreIdentityV1(
            access, refresh, "listener_a", session, 120, 600, 900, 1,
        )).value
        assertIs<IdentityV1Result.Accepted<Unit>>(client(http).revokeFamily(expired))
        assertEquals(3, http.requests.size)
        assertEquals(IdentityV1Result.Refused(IdentityV1Failure.Unauthorized), client(http).refresh(expired))
        assertEquals(IdentityV1Result.Refused(IdentityV1Failure.Unauthorized), client(http).session(expired))
        assertEquals(3, http.requests.size)
    }

    @Test fun session_and_logout_send_native_bearer_with_session_fence() = runTest {
        val http = RecordingHttp().apply { response = response(sessionBody) }
        val value = assertIs<IdentityV1Result.Accepted<NativeSession>>(client(http).session(grant())).value
        assertEquals(session, value.session)
        request(http, "GET", "/session", null, access, session)
        http.response = response("", 204)
        assertIs<IdentityV1Result.Accepted<Unit>>(client(http).revoke(grant()))
        request(http, "DELETE", "/session", null, access, session)
    }

    @Test fun legacy_wrong_surface_and_mixed_pairs_are_refused() = runTest {
        val http = RecordingHttp()
        for (body in listOf(pair.replace("ci1_n_", "ca1_n_"), pair.replace("ci1_r_", "cr1_"),
            pair.replace("ci1_n_", "ci1_t_"), pair.replace("ci1_n_", "ci1_s_"),
            pair.replace("ci1_r_", "ci1_n_"), replace(key = "token_type", value = JsonPrimitive("bearer")),
            replace(key = "access_token", value = JsonPrimitive(refresh)))) {
            http.response = response(body)
            assertEquals(unknown, client(http).login(email, "password"))
        }
        // A retired-provider pair cannot even be constructed, so no API call can carry one.
        assertFalse(NativeGrant.isIdentityV1AccessToken("ca1_n_" + "a".repeat(64)))
        assertFalse(NativeGrant.isIdentityV1RefreshToken("cr1_" + "b".repeat(64)))
        assertEquals(IdentityV1Validation.Refused(com.cantos.theatre.domain.identity.IdentityV1Invalid.Credential),
            NativeGrant.restoreIdentityV1("ca1_n_" + "a".repeat(64), "cr1_" + "b".repeat(64), "listener_a", session, 1120, 1600, 1900, 1000))
    }

    @Test fun requests_bind_to_the_admitted_origin_including_the_debug_loopback_exception() = runTest {
        for (admitted in listOf("https://api.example", "http://127.0.0.1:8080", "http://localhost:8443")) {
            val http = RecordingHttp().apply { response = response() }
            assertIs<IdentityV1Result.Accepted<NativeGrant>>(client(http, ApiOrigin.parse(admitted)).login(email, "password"))
            assertEquals("$admitted/api/v1/theatre/native/identity/login", http.requests.single().url)
        }
        // The origin owner (not this client) is what refuses cleartext to any other host.
        assertEquals(CatalogOriginSelection.Rejected(ApiOriginRefusal.CleartextHostNotAllowed),
            selectCatalogOrigin("http://api.example", allowLoopbackHttp = true))
        assertEquals(CatalogOriginSelection.Rejected(ApiOriginRefusal.NotHttps),
            selectCatalogOrigin("http://127.0.0.1:8080", allowLoopbackHttp = false))
    }

    @Test fun strict_shape_rejects_duplicates_escaped_keys_types_and_deadlines() = runTest {
        val http = RecordingHttp()
        val malformed = listOf(
            "[]", "{", pair.dropLast(1) + ",\"actor_id\":\"listener_a\"}",
            pair.dropLast(1) + ",\"\\u0061ctor_id\":\"listener_a\"}",
            pair.dropLast(1) + ",\"unknown\":true}",
            JsonObject((Json.parseToJsonElement(pair) as JsonObject) - "authenticated_at").toString(),
            replace(key = "authenticated_at", value = JsonNull),
            replace(key = "authenticated_at", value = JsonPrimitive(0)),
            replace(key = "authenticated_at", value = JsonPrimitive(1001)),
            replace(key = "expires_at", value = JsonPrimitive("1120")),
            replace(key = "actor_id", value = JsonPrimitive(123)),
            replace(key = "actor_id", value = JsonPrimitive("other actor")),
            replace(key = "session_id", value = JsonPrimitive("invalid")),
            replace(key = "expires_at", value = JsonPrimitive(1000)),
            replace(key = "expires_at", value = JsonPrimitive(1121)),
            replace(key = "refresh_expires_at", value = JsonPrimitive(1119)),
            replace(key = "refresh_expires_at", value = JsonPrimitive(1601)),
            replace(key = "absolute_expires_at", value = JsonPrimitive(1599)),
            replace(key = "absolute_expires_at", value = JsonPrimitive(1901)),
            pair.replace("1120", "1120.0"), pair.replace("1120", "9223372036854775808"),
        )
        for (body in malformed) {
            http.response = response(body)
            assertEquals(unknown, client(http).login(email, "password"), body)
        }
        for (reply in listOf(NativeHttpResponse(200, "text/plain", pair, null), response(" ".repeat(65_537) + pair), response(pair, 201))) {
            http.response = reply
            assertEquals(unknown, client(http).login(email, "password"))
        }
    }

    @Test fun refresh_rejects_changed_family_authentication_deadline_and_reused_secrets() = runTest {
        val rotated = pair.replace(access, "ci1_n_" + "c".repeat(64)).replace(refresh, "ci1_r_" + "d".repeat(64))
        val http = RecordingHttp()
        for (body in listOf(pair, rotated.replace("listener_a", "listener_b"), rotated.replace(session, "22222222-2222-4333-8444-555555555555"),
            rotated.replace("1900", "1899"), rotated.replace("1600", "1599"), rotated.replace("1000", "999"),
            rotated.replace("ci1_n_" + "c".repeat(64), access), rotated.replace("ci1_r_" + "d".repeat(64), refresh))) {
            http.response = response(body)
            assertEquals(unknown, client(http).refresh(grant()))
        }
    }

    @Test fun session_rejects_malformed_changed_and_expired_facts() = runTest {
        val http = RecordingHttp()
        for (body in listOf(sessionBody.replace("listener_a", "listener_b"), sessionBody.replace("1900", "1899"),
            sessionBody.replace("1000", "999"), sessionBody.replace("1120", "1000"), sessionBody.replace("1000", "null"),
            sessionBody.dropLast(1) + ",\"session_id\":\"$session\"}")) {
            http.response = response(body)
            assertEquals(protocol, client(http).session(grant()))
        }
        http.response = NativeHttpResponse(200, "application/json", sessionBody, null, "22222222-2222-4333-8444-555555555555")
        assertEquals(IdentityV1Result.Refused(IdentityV1Failure.SessionChanged), client(http).session(grant()))
    }

    @Test fun status_mapping_preserves_typed_errors_and_never_falls_back() = runTest {
        val http = RecordingHttp()
        for ((status, reason) in listOf(401 to IdentityV1Failure.Unauthorized, 403 to IdentityV1Failure.Denied,
            404 to IdentityV1Failure.NotConfigured, 409 to IdentityV1Failure.SessionChanged,
            429 to IdentityV1Failure.RateLimited(17), 503 to IdentityV1Failure.ServiceUnavailable,
            400 to IdentityV1Failure.Protocol, 302 to IdentityV1Failure.UnknownOutcome, 500 to IdentityV1Failure.UnknownOutcome)) {
            http.response = NativeHttpResponse(status, "text/plain", "password=$access", null, retryAfter = "17")
            val before = http.requests.size
            assertEquals(IdentityV1Result.Refused(reason), client(http).login(email, "password"))
            assertEquals(before + 1, http.requests.size)
            assertFalse(reason.toString().contains(access))
            assertTrue(http.requests.last().url.endsWith("/native/identity/login"))
        }
        for ((raw, seconds) in listOf("0" to 0L, " 60 " to 60L, null to null, "-1" to null, "1.5" to null,
            "Wed, 21 Oct 2026 00:00:00 GMT" to null, "9223372036854775808" to null, "token=$access" to null)) {
            http.response = NativeHttpResponse(429, null, null, null, retryAfter = raw)
            assertEquals(IdentityV1Result.Refused(IdentityV1Failure.RateLimited(seconds)), client(http).refresh(grant()))
        }
        http.response = NativeHttpResponse(0, null, null, "secret-server-detail")
        assertEquals(IdentityV1Result.Refused(IdentityV1Failure.Transport), client(http).session(grant()))
        assertEquals(unknown, client(http).revoke(grant()))
    }

    @Test fun lost_login_and_refresh_responses_make_exactly_one_request() = runTest {
        for (refreshing in listOf(false, true)) {
            for (failure in listOf("timeout", "transport")) {
                val http = RecordingHttp().apply { response = NativeHttpResponse(0, null, null, failure) }
                val result = if (refreshing) client(http).refresh(grant()) else client(http).login(email, "password")
                assertEquals(unknown, result)
                assertEquals(1, http.requests.size)
            }
            val http = RecordingHttp()
            val operation = async { if (refreshing) client(http).refresh(grant()) else client(http).login(email, "password") }
            runCurrent()
            advanceTimeBy(15_000)
            runCurrent()
            assertEquals(unknown, operation.await())
            assertEquals(1, http.requests.size)
            assertEquals(1, http.cancelled)
            http.completion?.invoke(response())
            assertEquals(1, http.requests.size)
        }
    }

    @Test fun owner_cancellation_cancels_the_request_and_propagates() = runTest {
        val http = RecordingHttp()
        val operation = async { client(http).login(email, "password") }
        runCurrent()
        operation.cancelAndJoin()
        assertTrue(operation.isCancelled)
        assertEquals(1, http.cancelled)
        assertEquals(1, http.requests.size)
    }
}
