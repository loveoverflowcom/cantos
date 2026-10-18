package com.cantos.theatre.adapters.http

import com.cantos.theatre.domain.identity.NativeClientConfiguration
import com.cantos.theatre.domain.identity.NativeGrant
import com.cantos.theatre.ports.NativeApiFailure
import com.cantos.theatre.ports.NativeApiResult
import com.cantos.theatre.ports.NativeFlow
import com.cantos.theatre.ports.NativeHttpPort
import com.cantos.theatre.ports.NativeHttpRequest
import com.cantos.theatre.ports.NativeHttpResponse
import com.cantos.theatre.ports.NativeRequestHandle
import com.cantos.theatre.ports.NativeSession
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertIs
import kotlin.test.assertNull
import kotlin.test.assertTrue

class NativeIdentityClientTest {
    private val config = NativeClientConfiguration.create("https://api.example", "https://idm.example/oauth2/openid/cantos-theatre-native")!!
    private val actor = "00000000-0000-4000-8000-000000000001"
    private val session = "00000000-0000-4000-8000-000000000002"
    private val otherSession = "00000000-0000-4000-8000-000000000009"
    private val access = "ca1_n_${"a".repeat(64)}"
    private val refresh = "cr1_${"b".repeat(64)}"
    private val instant = 1900000000L
    // Adapt only the public routing tag for native decoder tests; this proves no server/account linking.
    private val pair get() = CanonicalBearerIdentity.tokenPairJson.replace("ca1_t_", "ca1_n_")
    private val protocol = NativeApiResult.Refused(NativeApiFailure.Protocol)
    private val unknown = NativeApiResult.Refused(NativeApiFailure.UnknownOutcome)

    private class Http : NativeHttpPort {
        val requests = mutableListOf<NativeHttpRequest>()
        var result = NativeHttpResponse(0, null, null, "transport")
        override fun send(request: NativeHttpRequest, completion: (NativeHttpResponse) -> Unit): NativeRequestHandle {
            requests += request
            completion(result)
            return object : NativeRequestHandle { override fun cancel() = Unit }
        }
    }

    private fun client(http: NativeHttpPort) = NativeIdentityClient(config, http) { instant }
    private fun response(body: String, contentType: String = "application/json") = NativeHttpResponse(200, contentType, body, null)
    private fun body(request: NativeHttpRequest): JsonObject = Json.parseToJsonElement(request.body!!) as JsonObject

    @Test fun native_client_configuration_rejects_credentials_paths_http_and_wrong_surface() {
        for (api in listOf("http://localhost", "https://api.example/path", "https://user@api.example", "https://api.example:65536", "https://api..example")) {
            assertNull(NativeClientConfiguration.create(api, config.identity.issuer))
        }
        assertNull(NativeClientConfiguration.create(config.apiOrigin, "https://idm.example/oauth2/openid/cantos-studio-web-bff"))
    }

    @Test fun native_pair_decodes_the_shared_bearer_fixture_only_with_native_purpose() = runTest {
        val http = Http()
        val client = client(http)
        http.result = response(pair)
        val grant = assertIs<NativeApiResult.Accepted<NativeGrant>>(client.exchange(session, "state", "code", "verifier")).value
        assertEquals(actor, grant.view.actor)
        assertEquals(session, grant.view.session)
        assertEquals(access, grant.accessToken)
        assertEquals(refresh, grant.refreshToken)
        assertEquals(1900000120L, grant.expiresAt)
        assertEquals(1900000600L, grant.refreshExpiresAt)
        assertEquals(1900000900L, grant.absoluteExpiresAt)
        assertNull(grant.authenticatedAt)
        assertNull(grant.view.credential)
        assertEquals("POST", http.requests.single().method)
        assertEquals("https://api.example/api/v1/theatre/native/auth/bearer/flows/$session/exchange", http.requests.single().url)
        assertNull(http.requests.single().bearer)
        assertEquals(setOf("state", "code", "code_verifier"), body(http.requests.single()).keys)
        http.result = response(CanonicalBearerIdentity.tokenPairJson)
        assertEquals(unknown, client.exchange(session, "state", "code", "verifier"))
    }

    @Test fun native_session_decodes_the_shared_noncredential_session_fixture() = runTest {
        val http = Http()
        http.result = response(CanonicalBearerIdentity.sessionJson)
        val sessionValue = assertIs<NativeApiResult.Accepted<NativeSession>>(client(http).session(access)).value
        assertEquals(actor, sessionValue.actor)
        assertEquals(session, sessionValue.session)
        assertEquals(1900000120L, sessionValue.expiresAt)
        assertEquals(1900000900L, sessionValue.absoluteExpiresAt)
        assertNull(sessionValue.authenticatedAt)
        assertNull(sessionValue.credential)
        assertEquals("GET", http.requests.single().method)
        assertEquals(access, http.requests.single().bearer)
        assertEquals("https://api.example/api/v1/theatre/native/auth/bearer/session", http.requests.single().url)
        http.result = response(CanonicalNativeIdentity.json)
        assertEquals(protocol, client(http).session(access))
    }

    @Test fun accepted_but_malformed_pairs_require_cleanup_even_when_wrong_shape_or_purpose() = runTest {
        val http = Http()
        val client = client(http)
        val invalid = listOf(
            pair.replace("\"expires_at\": 1900000120", "\"expires_at\": \"1900000120\""),
            pair.replace("\"access_token\"", "\"id_token\""),
            pair.replace("\"Bearer\"", "\"bearer\""),
            pair.replace("ca1_n_", "ca1_s_"),
            pair.replace("cr1_", "ca1_n_"),
            pair.replace("\"authenticated_at\": null", "\"authenticated_at\": \"1900000000\""),
            pair.replace("\"authenticated_at\": null", "\"authenticated_at\": 1900000001"),
            pair.replace("1900000120", "1900000121"),
            pair.replace("1900000120", "1900000000"),
            pair.replace("1900000600", "1900000601"),
            pair.replace("1900000900", "1900000901"),
            pair.replace("\"token_type\":", "\"provider_token\":\"secret\",\"token_type\":"),
            pair.replace("\"actor_id\":", "\"actor_id\":\"other\",\"actor_id\":"),
            pair.replace("\"actor_id\":", "\"act\\u006fr_id\":\"other\",\"actor_id\":"),
            pair.replace("\"authenticated_at\": null", "\"authenticated_at\": 18446744073709551615"),
            "[]", "{", "{}",
        )
        for (value in invalid) {
            http.result = response(value)
            assertEquals(unknown, client.exchange(session, "state", "code", "verifier"))
        }
        for (type in listOf("application/x-protobuf", "text/html", "application/jsonp")) {
            http.result = response(pair, type)
            assertEquals(unknown, client.exchange(session, "state", "code", "verifier"))
        }
        assertTrue(http.requests.all { it.bearer == null })
    }

    @Test fun session_refuses_credentials_in_projection_duplicate_keys_and_invalid_deadlines() = runTest {
        val http = Http()
        val client = client(http)
        for (value in listOf(
            CanonicalBearerIdentity.sessionJson.replace("\"actor_id\":", "\"access_token\":\"secret\",\"actor_id\":"),
            CanonicalBearerIdentity.sessionJson.replace("1900000120", "1900000121"),
            CanonicalBearerIdentity.sessionJson.replace("1900000120", "1900000000"),
            CanonicalBearerIdentity.sessionJson.replace("1900000900", "1900000119"),
            CanonicalBearerIdentity.sessionJson.replace("\"authenticated_at\": null", "\"authenticated_at\": true"),
            CanonicalBearerIdentity.sessionJson.replace("\"actor_id\":", "\"actor_id\":\"other\",\"actor_id\":"),
        )) {
            http.result = response(value)
            assertEquals(protocol, client.session(access))
        }
    }

    @Test fun begin_refresh_revoke_and_cancel_use_exact_bearer_routes_without_authorization() = runTest {
        val http = Http()
        val client = client(http)
        http.result = response("{\"flow_id\":\"$session\",\"authorization_url\":\"https://idm.example/authorize\"}")
        assertIs<NativeApiResult.Accepted<NativeFlow>>(client.begin("state", "nonce", "challenge"))
        assertEquals("https://api.example/api/v1/theatre/native/auth/bearer/flows", http.requests.last().url)
        assertEquals(setOf("state", "nonce", "code_challenge"), body(http.requests.last()).keys)
        http.result = response(pair.replace(refresh, "cr1_${"c".repeat(64)}"))
        assertIs<NativeApiResult.Accepted<NativeGrant>>(client.refresh(refresh, session))
        assertEquals("https://api.example/api/v1/theatre/native/auth/bearer/refresh", http.requests.last().url)
        assertEquals(mapOf("refresh_token" to JsonPrimitive(refresh), "session_id" to JsonPrimitive(session)), body(http.requests.last()).toMap())
        http.result = NativeHttpResponse(204, null, null, null)
        assertIs<NativeApiResult.Accepted<Unit>>(client.revokeFamily(refresh, session))
        assertEquals("https://api.example/api/v1/theatre/native/auth/bearer/revoke", http.requests.last().url)
        assertIs<NativeApiResult.Accepted<Unit>>(client.cancel(session, "state", "verifier"))
        assertEquals("https://api.example/api/v1/theatre/native/auth/bearer/flows/$session/cancel", http.requests.last().url)
        assertEquals(mapOf("state" to JsonPrimitive("state"), "code_verifier" to JsonPrimitive("verifier")), body(http.requests.last()).toMap())
        assertTrue(http.requests.all { it.bearer == null && it.expectedSession == null && it.method == "POST" })
        assertTrue(http.requests.none { it.url.contains(access) || it.url.contains(refresh) })
    }

    @Test fun refresh_rejects_cross_family_or_unrotated_refresh_responses_as_unknown() = runTest {
        val http = Http()
        val client = client(http)
        http.result = response(pair)
        assertEquals(unknown, client.refresh(refresh, session))
        http.result = response(pair.replace(refresh, "cr1_${"c".repeat(64)}").replace(session, otherSession))
        assertEquals(unknown, client.refresh(refresh, session))
        assertEquals(2, http.requests.size)
    }

    @Test fun web_producer_and_legacy_credentials_never_admit_new_native_reads_or_rotation() = runTest {
        val http = Http()
        val client = client(http)
        for (credential in listOf("a".repeat(64), access.replace("ca1_n_", "ca1_t_"), access.replace("ca1_n_", "ca1_s_"), "producer-secret")) {
            assertEquals(protocol, client.session(credential))
            assertEquals(protocol, client.saved(credential, session))
            assertEquals(protocol, client.refresh(credential, session))
        }
        assertEquals(protocol, client.refresh(refresh, "not-a-session"))
        assertEquals(protocol, client.revokeFamily(refresh, "not-a-session"))
        assertEquals(protocol, client.cancel("not-a-flow", "state", "verifier"))
        assertEquals(protocol, client.exchange("not-a-flow", "state", "code", "verifier"))
        assertTrue(http.requests.isEmpty())
    }

    @Test fun legacy_access_can_only_be_deleted_during_migration() = runTest {
        val http = Http()
        val client = client(http)
        http.result = NativeHttpResponse(204, null, null, null)
        assertIs<NativeApiResult.Accepted<Unit>>(client.revoke("a".repeat(64)))
        assertEquals("https://api.example/api/v1/theatre/native/session", http.requests.single().url)
        assertEquals("DELETE", http.requests.single().method)
        assertIs<NativeApiResult.Accepted<Unit>>(client.revoke(access))
        assertEquals("https://api.example/api/v1/theatre/native/auth/bearer/session", http.requests.last().url)
        http.result = NativeHttpResponse(401, "application/json", "{}", null)
        assertIs<NativeApiResult.Accepted<Unit>>(client.revoke("a".repeat(64)))
        assertEquals(NativeApiResult.Refused(NativeApiFailure.Unauthorized), client.revoke(access))
    }

    @Test fun mutation_transport_5xx_and_malformed_success_are_unknown_with_one_effect_each() = runTest {
        val http = Http()
        val client = client(http)
        for (result in listOf(
            NativeHttpResponse(0, null, null, "transport"),
            NativeHttpResponse(503, "application/json", "{}", null),
            NativeHttpResponse(500, "application/json", "{}", null),
            NativeHttpResponse(408, "application/json", "{}", null),
            NativeHttpResponse(200, "application/json", "{}", null),
            NativeHttpResponse(204, null, null, null),
        )) {
            http.result = result
            val before = http.requests.size
            assertEquals(unknown, client.exchange(session, "state", "code", "verifier"))
            assertEquals(before + 1, http.requests.size)
        }
        for ((status, reason) in listOf(401 to NativeApiFailure.Unauthorized, 403 to NativeApiFailure.Denied, 400 to NativeApiFailure.Protocol)) {
            http.result = NativeHttpResponse(status, "application/json", "{}", null)
            assertEquals(NativeApiResult.Refused(reason), client.refresh(refresh, session))
        }
        assertEquals("NativeHttpRequest([redacted])", http.requests.first().toString())
        assertEquals("NativeHttpResponse([redacted])", http.result.toString())
    }

    @Test fun cleanup_requires_definite_204_and_preserves_unknown_outcomes_for_reconciliation() = runTest {
        val http = Http()
        val client = client(http)
        for (result in listOf(response("{}"), NativeHttpResponse(503, "application/json", "{}", null), NativeHttpResponse(0, null, null, "timeout"))) {
            http.result = result
            assertEquals(unknown, client.revokeFamily(refresh, session))
            assertEquals(unknown, client.cancel(session, "state", "verifier"))
        }
        http.result = NativeHttpResponse(401, "application/json", "{}", null)
        assertEquals(NativeApiResult.Refused(NativeApiFailure.Unauthorized), client.revokeFamily(refresh, session))
    }

    @Test fun saved_page_preserves_library_contract_and_exact_session_fence() = runTest {
        val http = Http()
        val client = client(http)
        http.result = NativeHttpResponse(200, "application/json", "{\"items\":[],\"next_cursor\":null}", null, session)
        assertEquals(0, assertIs<NativeApiResult.Accepted<Int>>(client.saved(access, session)).value)
        val valid = "{\"items\":[{\"saved\":{\"work_id\":\"$session\",\"saved\":true,\"revision\":1,\"listener_order\":1,\"accepted_at_ms\":100}}],\"next_cursor\":null}"
        http.result = NativeHttpResponse(200, "application/json", valid, null, session)
        assertEquals(1, assertIs<NativeApiResult.Accepted<Int>>(client.saved(access, session)).value)
        for (invalid in listOf("{\"items\":[{}],\"next_cursor\":null}", valid.replace("\"revision\":1", "\"revision\":\"1\""), valid.replace("\"saved\":true", "\"saved\":\"true\""))) {
            http.result = NativeHttpResponse(200, "application/json", invalid, null, session)
            assertEquals(protocol, client.saved(access, session))
        }
        for (stamp in listOf(null, otherSession)) {
            http.result = NativeHttpResponse(200, "application/json", "{\"items\":[],\"next_cursor\":null}", null, stamp)
            assertEquals(NativeApiResult.Refused(NativeApiFailure.SessionChanged), client.saved(access, session))
        }
        assertEquals(access, http.requests.first().bearer)
        assertEquals("https://api.example/api/v1/theatre/library/saved", http.requests.first().url)
        assertEquals(session, http.requests.first().expectedSession)
    }

    @OptIn(ExperimentalCoroutinesApi::class)
    @Test fun whole_exchange_deadline_cancels_transport_and_reports_unknown_without_retry() = runTest {
        var calls = 0
        var cancelled = false
        val http = object : NativeHttpPort {
            override fun send(request: NativeHttpRequest, completion: (NativeHttpResponse) -> Unit): NativeRequestHandle {
                calls += 1
                return object : NativeRequestHandle { override fun cancel() { cancelled = true } }
            }
        }
        assertEquals(unknown, client(http).exchange(session, "state", "code", "verifier"))
        assertEquals(15000L, testScheduler.currentTime)
        assertEquals(1, calls)
        assertTrue(cancelled)
    }

    @OptIn(ExperimentalCoroutinesApi::class)
    @Test fun owner_cancellation_propagates_and_late_callback_never_completes_or_retries_exchange() = runTest {
        var calls = 0
        var cancelled = false
        var pendingCompletion: ((NativeHttpResponse) -> Unit)? = null
        var completed = false
        val http = object : NativeHttpPort {
            override fun send(request: NativeHttpRequest, completion: (NativeHttpResponse) -> Unit): NativeRequestHandle {
                calls += 1
                pendingCompletion = completion
                return object : NativeRequestHandle { override fun cancel() { cancelled = true } }
            }
        }
        val pending = launch { client(http).exchange(session, "state", "code", "verifier"); completed = true }
        runCurrent()
        pending.cancel()
        runCurrent()
        pendingCompletion?.invoke(response(pair))
        runCurrent()
        assertTrue(pending.isCancelled)
        assertTrue(cancelled)
        assertFalse(completed)
        assertEquals(1, calls)
    }
}
