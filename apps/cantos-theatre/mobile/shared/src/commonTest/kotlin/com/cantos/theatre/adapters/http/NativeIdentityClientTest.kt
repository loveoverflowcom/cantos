package com.cantos.theatre.adapters.http

import com.cantos.theatre.domain.identity.NativeClientConfiguration
import com.cantos.theatre.ports.NativeApiFailure
import com.cantos.theatre.ports.NativeApiResult
import com.cantos.theatre.ports.NativeSession

import com.cantos.theatre.ports.NativeHttpPort
import com.cantos.theatre.ports.NativeHttpRequest
import com.cantos.theatre.ports.NativeHttpResponse
import com.cantos.theatre.ports.NativeRequestHandle
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs
import kotlin.test.assertNull
import kotlin.test.assertTrue

class NativeIdentityClientTest {
    private val config = NativeClientConfiguration.create("https://api.example", "https://idm.example/oauth2/openid/cantos-theatre-native")!!
    private val session = "00000000-0000-4000-8000-000000000002"
    private class Http : NativeHttpPort {
        val requests = mutableListOf<NativeHttpRequest>()
        var result = NativeHttpResponse(0, null, null, "transport")
        override fun send(request: NativeHttpRequest, completion: (NativeHttpResponse) -> Unit): NativeRequestHandle {
            requests += request; completion(result)
            return object : NativeRequestHandle { override fun cancel() = Unit }
        }
    }
    @Test fun native_client_configuration_rejects_credentials_paths_http_and_wrong_surface() {
        for (api in listOf("http://localhost", "https://api.example/path", "https://user@api.example", "https://api.example:65536", "https://api..example")) {
            assertNull(NativeClientConfiguration.create(api, config.identity.issuer))
        }
        assertNull(NativeClientConfiguration.create(config.apiOrigin, "https://idm.example/oauth2/openid/cantos-studio-web-bff"))
    }
    @Test fun native_session_decoder_consumes_the_same_sha_pinned_contract_fixture_as_rust() = runTest {
        val http = Http(); val client = NativeIdentityClient(config, http)
        http.result = NativeHttpResponse(200, "application/json", CanonicalNativeIdentity.json, null)
        val accepted = assertIs<NativeApiResult.Accepted<NativeSession>>(client.session("a".repeat(64)))
        assertEquals("00000000-0000-4000-8000-000000000001", accepted.value.actor)
        assertEquals("00000000-0000-4000-8000-000000000002", accepted.value.session)
        assertEquals(1900000900L, accepted.value.expiresAt)
        assertNull(accepted.value.credential)
        assertEquals("GET", http.requests.single().method)
        assertEquals("https://api.example/api/v1/theatre/native/session", http.requests.single().url)
    }
    @Test fun native_session_decoder_rejects_wrong_keys_token_purpose_and_expiry_types() = runTest {
        val http = Http(); val client = NativeIdentityClient(config, http)
        val actor = "00000000-0000-4000-8000-000000000001"; val session = "00000000-0000-4000-8000-000000000002"
        val body = "{\"actor_id\":\"$actor\",\"session_id\":\"$session\",\"expires_at\":200,\"access_token\":\"${"a".repeat(64)}\"}"
        http.result = NativeHttpResponse(200, "application/json", body, null)
        val accepted = assertIs<NativeApiResult.Accepted<NativeSession>>(client.exchange(session, "state", "code", "verifier"))
        assertEquals(actor, accepted.value.actor)
        http.result = NativeHttpResponse(200, "application/json", body.replace(actor, "owner-a"), null)
        assertEquals("owner-a", assertIs<NativeApiResult.Accepted<NativeSession>>(client.exchange(session, "state", "code", "verifier")).value.actor)
        for (invalid in listOf(body.replace("expires_at\":200", "expires_at\":\"200\""), body.replace("\"access_token\"", "\"id_token\""), body.replace("a".repeat(64), "opaque-web-cookie"))) {
            http.result = NativeHttpResponse(200, "application/json", invalid, null)
            assertEquals(NativeApiResult.Refused(NativeApiFailure.Protocol), client.exchange(session, "state", "code", "verifier"))
        }
        assertTrue(http.requests.all { it.bearer == null && it.url == "https://api.example/api/v1/theatre/native/auth/flows/$session/exchange" })
    }
    @Test fun ambiguous_exchange_is_a_closed_outcome_with_exactly_one_http_effect() = runTest {
        val http = Http(); val client = NativeIdentityClient(config, http)
        assertEquals(NativeApiResult.Refused(NativeApiFailure.UnknownOutcome), client.exchange("00000000-0000-4000-8000-000000000002", "state", "code", "verifier"))
        assertEquals(1, http.requests.size)
        assertEquals("NativeHttpRequest([redacted])", http.requests.single().toString())
    }
    @Test fun saved_page_uses_real_library_contract_and_refuses_invalid_projection() = runTest {
        val http = Http(); val client = NativeIdentityClient(config, http)
        http.result = NativeHttpResponse(200, "application/json", "{\"items\":[],\"next_cursor\":null}", null, session)
        assertEquals(0, assertIs<NativeApiResult.Accepted<Int>>(client.saved("a".repeat(64), session)).value)
        http.result = NativeHttpResponse(200, "application/json", "{\"items\":[{}],\"next_cursor\":null}", null, session)
        assertEquals(NativeApiResult.Refused(NativeApiFailure.Protocol), client.saved("a".repeat(64), session))
        val valid = "{\"items\":[{\"saved\":{\"work_id\":\"$session\",\"saved\":true,\"revision\":1,\"listener_order\":1,\"accepted_at_ms\":100}}],\"next_cursor\":null}"
        http.result = NativeHttpResponse(200, "application/json", valid, null, session)
        assertEquals(1, assertIs<NativeApiResult.Accepted<Int>>(client.saved("a".repeat(64), session)).value)
        for (invalid in listOf(valid.replace("\"revision\":1", "\"revision\":\"1\""), valid.replace("\"saved\":true", "\"saved\":\"true\""))) {
            http.result = NativeHttpResponse(200, "application/json", invalid, null, session)
            assertEquals(NativeApiResult.Refused(NativeApiFailure.Protocol), client.saved("a".repeat(64), session))
        }
        assertEquals("a".repeat(64), http.requests.first().bearer)
        assertEquals("https://api.example/api/v1/theatre/library/saved", http.requests.first().url)
        assertEquals(session, http.requests.first().expectedSession)
    }
    @Test fun native_resource_rejects_body_before_decoding_on_missing_or_mismatched_session_stamp() = runTest {
        val http = Http(); val client = NativeIdentityClient(config, http)
        for (stamp in listOf(null, "00000000-0000-4000-8000-000000000009")) {
            http.result = NativeHttpResponse(200, "application/json", "{\"items\":[],\"next_cursor\":null}", null, stamp)
            assertEquals(NativeApiResult.Refused(NativeApiFailure.SessionChanged), client.saved("a".repeat(64), session))
        }
    }
    @OptIn(ExperimentalCoroutinesApi::class)
    @Test fun whole_exchange_deadline_cancels_owned_transport_and_reports_unknown_without_retry() = runTest {
        var calls = 0; var cancelled = false
        val http = object : NativeHttpPort {
            override fun send(request: NativeHttpRequest, completion: (NativeHttpResponse) -> Unit): NativeRequestHandle {
                calls += 1
                return object : NativeRequestHandle { override fun cancel() { cancelled = true } }
            }
        }
        val client = NativeIdentityClient(config, http)
        assertEquals(NativeApiResult.Refused(NativeApiFailure.UnknownOutcome), client.exchange(session, "state", "code", "verifier"))
        assertEquals(15000L, testScheduler.currentTime); assertEquals(1, calls); assertTrue(cancelled)
    }
}
