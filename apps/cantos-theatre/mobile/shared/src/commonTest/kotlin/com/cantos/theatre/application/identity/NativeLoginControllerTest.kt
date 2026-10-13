package com.cantos.theatre.application.identity

import com.cantos.theatre.domain.identity.*
import com.cantos.theatre.ports.*
import kotlin.test.*

class NativeLoginControllerTest {
    private class Browser : SystemBrowser {
        var opened: String? = null
        override fun open(url: String): Boolean { opened = url; return true }
        override fun cancel() { opened = null }
    }
    private class Crypto : IdentityCrypto {
        var count = 0
        override fun secret(): LoginSecret = LoginSecret.parse(('a' + count++).toString().repeat(64))!!
        override fun challenge(verifier: LoginSecret): String = "x".repeat(43)
    }
    private val configuration = NativeConfiguration.create("https://idm.example/oauth2/openid/cantos-theatre-native", "com.cantos.theatre:/oauth2/callback")!!
    @Test fun exactCallbackIsConsumedOnceAndOldExchangeCannotSurviveCancelOrNewLogin() {
        val browser = Browser()
        val login = NativeLoginController(Crypto(), browser)
        assertNull(login.begin(configuration, "https://idm.example/ui/oauth2", 100))
        assertTrue(browser.opened!!.contains("scope=openid"))
        assertTrue(browser.opened!!.contains("code_challenge_method=S256"))
        val result = login.callback(NativeCallback(configuration.redirect, "a".repeat(64), "code", false), 101) as CallbackOutcome.Exchange
        assertEquals("cantos-theatre-native", result.request.client)
        assertEquals("c".repeat(64), result.request.verifierForExchange())
        assertEquals(CallbackOutcome.Refused(LoginFailure.Replay), login.callback(NativeCallback(configuration.redirect, "a".repeat(64), "code", false), 102))
        assertTrue(login.acceptsExchange(result.request.generation))
        login.cancel()
        assertFalse(login.acceptsExchange(result.request.generation))
        assertNull(login.begin(configuration, "https://idm.example/ui/oauth2", 200))
        assertFalse(login.acceptsExchange(result.request.generation))
        assertEquals(CallbackOutcome.Refused(LoginFailure.InvalidCallback), login.callback(NativeCallback(configuration.redirect, "a".repeat(64), "old-code", false), 201))
        assertTrue(login.state is NativeLoginState.Waiting)
        assertTrue(login.callback(NativeCallback(configuration.redirect, "d".repeat(64), "new-code", false), 201) is CallbackOutcome.Exchange)
    }
    @Test fun callbackPartitionsRejectBeforeAnyCodeExchange() {
        for ((callback, now, failure) in listOf(
            Triple(NativeCallback("com.cantos.studio:/oauth2/callback", "a".repeat(64), "code", false), 101L, LoginFailure.InvalidCallback),
            Triple(NativeCallback(configuration.redirect, "wrong", "code", false), 101L, LoginFailure.InvalidCallback),
            Triple(NativeCallback(configuration.redirect, "a".repeat(64), "", false), 101L, LoginFailure.InvalidCallback),
            Triple(NativeCallback(configuration.redirect, "a".repeat(64), "code", false), 400L, LoginFailure.Expired),
            Triple(NativeCallback(configuration.redirect, "a".repeat(64), "code", false), 99L, LoginFailure.Expired),
            Triple(NativeCallback(configuration.redirect, "a".repeat(64), "code", true), 101L, LoginFailure.Cancelled),
        )) {
            val login = NativeLoginController(Crypto(), Browser())
            login.begin(configuration, "https://idm.example/ui/oauth2", 100)
            assertEquals(CallbackOutcome.Refused(failure), login.callback(callback, now))
        }
    }
    @Test fun configurationAndSecretsAreBoundedAndDiagnosticsRedacted() {
        assertNull(NativeConfiguration.create("https://idm.example/oauth2/openid/cantos-studio-web-bff", configuration.redirect))
        for (size in listOf(0, 42, 129)) assertNull(LoginSecret.parse("a".repeat(size)))
        assertNull(LoginSecret.parse("ệ".repeat(64)))
        assertEquals("LoginSecret([redacted])", LoginSecret.parse("a".repeat(64)).toString())
        for (now in 100L..399L) {
            val login = NativeLoginController(Crypto(), Browser())
            login.begin(configuration, "https://idm.example/ui/oauth2", 100)
            assertTrue(login.callback(NativeCallback(configuration.redirect, "a".repeat(64), "code", false), now) is CallbackOutcome.Exchange)
        }
    }
}
