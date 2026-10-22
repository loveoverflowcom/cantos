package com.cantos.theatre.app

import com.cantos.theatre.application.identity.NativeAccountState
import com.cantos.theatre.adapters.http.ApiOrigin
import com.cantos.theatre.domain.identity.RegistrationEmail
import com.cantos.theatre.domain.identity.RegistrationPhase
import com.cantos.theatre.domain.identity.RegistrationResult
import com.cantos.theatre.ports.NativeHttpPort
import com.cantos.theatre.ports.NativeHttpRequest
import com.cantos.theatre.ports.NativeHttpResponse
import com.cantos.theatre.ports.NativeRequestHandle
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs
import kotlin.test.assertTrue

@OptIn(kotlinx.coroutines.ExperimentalCoroutinesApi::class)
class LiveIdentityV1CompositionTest {
    @Test fun explicit_live_preview_uses_v1_signup_and_login_only() = runTest {
        val paths = mutableListOf<String>()
        val http = object : NativeHttpPort {
            override fun send(request: NativeHttpRequest, completion: (NativeHttpResponse) -> Unit): NativeRequestHandle {
                paths += request.url
                completion(if (request.url.endsWith("/signup")) NativeHttpResponse(202, "application/json", """{"accepted":true,"proof_ttl_seconds":900}""", null)
                    else NativeHttpResponse(401, "application/json", "{}", null))
                return object : NativeRequestHandle { override fun cancel() = Unit }
            }
        }
        val owners = LiveEnrollmentOwners(backgroundScope, ApiOrigin.parse("https://api.invalid"), http) { 1_790_900_000 }
        runCurrent()
        owners.registration.editEmail("listener@example.test"); owners.registration.submit(); runCurrent()
        assertEquals(RegistrationPhase.Waiting, owners.registration.state.value.phase)
        assertTrue(owners.shell.openLogin())
        val email = (RegistrationEmail.parse("listener@example.test") as RegistrationResult.Accepted).value
        assertTrue(owners.shell.login(email, "fixture-only")); runCurrent()
        assertIs<NativeAccountState.Failed>(owners.account.state.value)
        assertEquals(listOf("https://api.invalid/api/v1/theatre/native/identity/signup", "https://api.invalid/api/v1/theatre/native/identity/login"), paths)
        owners.close()
    }
}
