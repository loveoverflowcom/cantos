package com.cantos.theatre.adapters.http

import com.cantos.theatre.domain.identity.RegistrationAction
import com.cantos.theatre.domain.identity.RegistrationEmail
import com.cantos.theatre.domain.identity.RegistrationFailure
import com.cantos.theatre.ports.NativeHttpPort
import com.cantos.theatre.ports.NativeHttpRequest
import com.cantos.theatre.ports.NativeHttpResponse
import com.cantos.theatre.ports.NativeRequestHandle
import com.cantos.theatre.domain.identity.RegistrationResult
import kotlinx.coroutines.test.runTest
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertIs
import kotlin.test.assertNull

class NativeRegistrationHttpTest {
    private class MockHttp : NativeHttpPort {
        var reply = NativeHttpResponse(200, "application/json", "", null)
        val requests = mutableListOf<NativeHttpRequest>()
        override fun send(request: NativeHttpRequest, completion: (NativeHttpResponse) -> Unit): NativeRequestHandle {
            requests += request; completion(reply)
            return object : NativeRequestHandle { override fun cancel() = Unit }
        }
    }
    private val action = assertIs<RegistrationResult.Accepted<RegistrationAction>>(RegistrationAction.parse("https://cantos.example/?mode=verifyEmail&oobCode=private-code&continueUrl=https%3A%2F%2Fcantos.example%2F%3FfirebaseEnrollment%3D00000000-0000-4000-8000-000000000001", "https://cantos.example")).value

    @Test fun native_mutations_use_strict_contract_bodies_without_cookies_bearer_or_an_origin() = runTest {
        val http = MockHttp()
        val api = NativeRegistrationHttp(ApiOrigin.parse("https://api.example"), "https://idm.example", http)
        http.reply = NativeHttpResponse(202, "application/json", """{"status":"accepted"}""", null)
        assertIs<RegistrationResult.Accepted<Unit>>(api.start(assertIs<RegistrationResult.Accepted<RegistrationEmail>>(RegistrationEmail.parse("a@example.test")).value))
        val request = http.requests.single()
        assertEquals("https://api.example/api/v1/theatre/native/auth/registration", request.url)
        assertEquals("""{"email":"a@example.test"}""", request.body)
        assertNull(request.bearer)
        assertFalse(request.toString().contains("a@example.test"))
        http.reply = NativeHttpResponse(200, "application/json", """{"status":"setup_ready","login":"listener_1","setup_url":"https://idm.example/ui/reset?token=private-setup"}""", null)
        assertIs<RegistrationResult.Accepted<*>>(api.confirm(action))
        assertEquals("""{"operation_id":"00000000-0000-4000-8000-000000000001","oob_code":"private-code"}""", http.requests.last().body)
    }

    @Test fun malformed_or_lost_consuming_responses_never_become_success_or_automatic_retries() = runTest {
        val http = MockHttp()
        val api = NativeRegistrationHttp(ApiOrigin.parse("https://api.example"), "https://idm.example", http)
        val bad = listOf(
            """{"status":"setup_ready","login":"x","setup_url":"https://evil.test/ui/reset?token=x"}""",
            """{"status":"setup_ready","login":"x","setup_url":"https://idm.example/ui/reset?token=x","session_id":"forged"}""",
            """{"status":"unavailable","status":"setup_ready"}""",
            """{"status":"setup_ready","login":4,"setup_url":"https://idm.example/ui/reset?token=x"}""",
            """{"status":"complete"}""",
        )
        for (body in bad) {
            http.reply = NativeHttpResponse(200, "application/json", body, null)
            assertEquals(RegistrationResult.Refused(RegistrationFailure.UnknownOutcome), api.confirm(action))
        }
        http.reply = NativeHttpResponse(0, null, null, "timeout")
        assertEquals(RegistrationResult.Refused(RegistrationFailure.UnknownOutcome), api.confirm(action))
        assertEquals(bad.size + 1, http.requests.size)
        http.reply = NativeHttpResponse(200, "application/json", """{"status":"unavailable"}""", null)
        assertEquals(RegistrationResult.Refused(RegistrationFailure.ProofUnavailable), api.confirm(action))
    }

    @Test fun capability_and_api_down_are_independent_from_sign_in() = runTest {
        val http = MockHttp()
        val api = NativeRegistrationHttp(ApiOrigin.parse("https://api.example"), "https://idm.example", http)
        http.reply = NativeHttpResponse(0, null, null, "transport")
        assertEquals(RegistrationResult.Refused(RegistrationFailure.Unavailable), api.configuration())
        http.reply = NativeHttpResponse(200, "application/json", """{"status":"firebase_email_verification","action_origin":"https://cantos.example/path"}""", null)
        assertEquals(RegistrationResult.Refused(RegistrationFailure.Protocol), api.configuration())
    }

    @Test fun a_consuming_timeout_cancels_the_handle_once_without_retrying() = runTest {
        var sends = 0
        var cancellations = 0
        val http = object : NativeHttpPort {
            override fun send(request: NativeHttpRequest, completion: (NativeHttpResponse) -> Unit): NativeRequestHandle {
                sends += 1
                return object : NativeRequestHandle { override fun cancel() { cancellations += 1 } }
            }
        }
        val api = NativeRegistrationHttp(ApiOrigin.parse("https://api.example"), "https://idm.example", http)
        assertEquals(RegistrationResult.Refused(RegistrationFailure.UnknownOutcome), api.confirm(action))
        assertEquals(1, sends)
        assertEquals(1, cancellations)
    }
}
