package com.cantos.theatre.application.identity

import com.cantos.theatre.ports.NativeApiFailure
import com.cantos.theatre.ports.NativeApiResult
import com.cantos.theatre.domain.identity.NativeClientConfiguration
import com.cantos.theatre.ports.NativeFlow
import com.cantos.theatre.ports.NativeIdentityApi
import com.cantos.theatre.ports.NativeSession
import com.cantos.theatre.domain.identity.LoginSecret
import com.cantos.theatre.domain.identity.NativeCallback
import com.cantos.theatre.ports.CredentialFailure
import com.cantos.theatre.ports.CredentialRead
import com.cantos.theatre.ports.CredentialStore
import com.cantos.theatre.ports.IdentityCrypto
import com.cantos.theatre.ports.LogoutBarrier
import com.cantos.theatre.ports.LogoutBarrierRead
import com.cantos.theatre.ports.SystemBrowser
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.withContext
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertIs
import kotlin.test.assertNull
import kotlin.test.assertTrue

@OptIn(ExperimentalCoroutinesApi::class)
class NativeAccountControllerTest {
    private val actor = "00000000-0000-4000-8000-000000000001"
    private val session = "00000000-0000-4000-8000-000000000002"
    private val token = "a".repeat(64)
    private val configuration = NativeClientConfiguration.create("https://api.example", "https://idm.example/oauth2/openid/cantos-theatre-native")!!
    private class Crypto : IdentityCrypto {
        var count = 0
        override fun secret(): LoginSecret = LoginSecret.parse(('a' + count++).toString().repeat(64))!!
        override fun challenge(verifier: LoginSecret): String = "x".repeat(43)
    }
    private class Browser : SystemBrowser {
        var opened: String? = null
        override fun open(url: String): Boolean { opened = url; return true }
        override fun cancel() { opened = null }
    }
    private class Store(private val events: MutableList<String>) : CredentialStore {
        var value: String? = null
        var clearFails = false
        var readDenied = false
        override fun read(): CredentialRead { events += "read"; return if (readDenied) CredentialRead.Failed(CredentialFailure.Denied) else value?.let { CredentialRead.Found(it) } ?: CredentialRead.Missing }
        override fun write(value: String): CredentialFailure? { events += "write"; this.value = value; return null }
        override fun clear(): CredentialFailure? { events += "clear"; if (clearFails) return CredentialFailure.Denied; value = null; return null }
    }
    private class Barrier(private val events: MutableList<String>) : LogoutBarrier {
        var pending = false
        var markFails = false
        override fun read(): LogoutBarrierRead = if (pending) LogoutBarrierRead.Pending else LogoutBarrierRead.Clear
        override fun mark(): CredentialFailure? { events += "mark"; if (markFails) return CredentialFailure.Unavailable; pending = true; return null }
        override fun clear(): CredentialFailure? { events += "unmark"; pending = false; return null }
    }
    private inner class Api(private val events: MutableList<String>) : NativeIdentityApi {
        var exchanges = 0
        var exchangeFailure = false
        var revokeFailure = false
        var observedState = ""
        var savedResult: CompletableDeferred<NativeApiResult<Int>>? = null
        override suspend fun begin(state: String, nonce: String, challenge: String): NativeApiResult<NativeFlow> {
            events += "begin"; observedState = state
            return NativeApiResult.Accepted(NativeFlow(session, "https://idm.example/ui/oauth2?scope=openid&response_type=code&client_id=cantos-theatre-native&redirect_uri=com.cantos.theatre%3A%2Foauth2%2Fcallback&state=$state&nonce=$nonce&code_challenge_method=S256&code_challenge=$challenge"))
        }
        override suspend fun exchange(flow: String, state: String, code: String, verifier: String): NativeApiResult<NativeSession> {
            exchanges += 1; events += "exchange"
            assertEquals(observedState, state); assertEquals("c".repeat(64), verifier); assertEquals("code", code)
            return if (exchangeFailure) NativeApiResult.Refused(NativeApiFailure.UnknownOutcome) else NativeApiResult.Accepted(NativeSession(actor, session, 200, token))
        }
        override suspend fun session(credential: String): NativeApiResult<NativeSession> {
            events += "session"; assertEquals(token, credential)
            return NativeApiResult.Accepted(NativeSession(actor, session, 200, null))
        }
        override suspend fun revoke(credential: String): NativeApiResult<Unit> {
            events += "revoke"; assertEquals(token, credential)
            return if (revokeFailure) NativeApiResult.Refused(NativeApiFailure.UnknownOutcome) else NativeApiResult.Accepted(Unit)
        }
        override suspend fun saved(credential: String, session: String): NativeApiResult<Int> = savedResult?.let { withContext(NonCancellable) { it.await() } } ?: NativeApiResult.Accepted(3)
    }
    @Test fun native_callback_is_single_use_and_credential_is_persisted_only_after_verified_exchange() = runTest {
        val events = mutableListOf<String>(); val store = Store(events); val barrier = Barrier(events); val api = Api(events); val browser = Browser()
        val owner = NativeAccountController(this, configuration, api, Crypto(), browser, store, barrier) { 100 }
        owner.beginLogin(); runCurrent()
        assertEquals(listOf("mark", "read", "clear", "unmark", "begin"), events)
        assertNull(store.value); assertTrue(browser.opened?.startsWith("https://idm.example/ui/oauth2?") == true)
        owner.callback(NativeCallback(configuration.identity.redirect, "wrong", "code", false)); runCurrent()
        assertEquals(0, api.exchanges)
        owner.callback(NativeCallback(configuration.identity.redirect, "a".repeat(64), "code", false)); runCurrent()
        assertIs<NativeAccountState.SignedIn>(owner.state.value)
        assertEquals(token, store.value); assertFalse(barrier.pending)
        owner.callback(NativeCallback(configuration.identity.redirect, "a".repeat(64), "code", false)); runCurrent()
        assertEquals(1, api.exchanges)
        assertEquals(listOf("exchange", "mark", "write", "unmark"), events.takeLast(4))
        owner.close()
    }
    @Test fun unknown_exchange_outcome_never_replays_code_or_creates_account() = runTest {
        val events = mutableListOf<String>(); val api = Api(events); api.exchangeFailure = true
        val store = Store(events); val owner = NativeAccountController(this, configuration, api, Crypto(), Browser(), store, Barrier(events)) { 100 }
        owner.beginLogin(); runCurrent()
        repeat(2) { owner.callback(NativeCallback(configuration.identity.redirect, "a".repeat(64), "code", false)); runCurrent() }
        assertEquals(1, api.exchanges); assertNull(store.value)
        assertEquals(NativeAccountState.Failed(NativeAccountFailure.Api), owner.state.value)
        owner.close()
    }
    @Test fun logout_marker_survives_failed_delete_and_restart_restore_stays_denied() = runTest {
        val events = mutableListOf<String>(); val store = Store(events); store.value = token; store.clearFails = true
        val barrier = Barrier(events); val api = Api(events)
        val owner = NativeAccountController(this, configuration, api, Crypto(), Browser(), store, barrier) { 100 }
        owner.restore(); runCurrent(); assertIs<NativeAccountState.SignedIn>(owner.state.value)
        events.clear(); owner.logout(); assertEquals(NativeAccountState.CleanupRequired, owner.state.value); runCurrent()
        assertEquals(listOf("mark", "revoke", "clear"), events); assertTrue(barrier.pending)
        owner.close()
        val restarted = NativeAccountController(this, configuration, api, Crypto(), Browser(), store, barrier) { 100 }
        restarted.restore(); runCurrent(); assertEquals(NativeAccountState.CleanupRequired, restarted.state.value)
        store.clearFails = false; restarted.logout(); runCurrent()
        assertEquals(NativeAccountState.SignedOut, restarted.state.value); assertNull(store.value); assertFalse(barrier.pending)
        restarted.close()
    }
    @Test fun unknown_revoke_keeps_original_credential_and_barrier_for_idempotent_retry() = runTest {
        val events = mutableListOf<String>(); val store = Store(events); store.value = token; val barrier = Barrier(events); val api = Api(events); api.revokeFailure = true
        val owner = NativeAccountController(this, configuration, api, Crypto(), Browser(), store, barrier) { 100 }
        owner.logout(); runCurrent()
        assertEquals(listOf("mark", "read", "revoke"), events); assertEquals(token, store.value); assertTrue(barrier.pending)
        api.revokeFailure = false; owner.logout(); runCurrent()
        assertNull(store.value); assertFalse(barrier.pending); assertEquals(NativeAccountState.SignedOut, owner.state.value)
        owner.close()
    }
    @Test fun marker_failure_stops_before_revocation_or_browser_and_restart_never_auto_restores() = runTest {
        val events = mutableListOf<String>(); val store = Store(events); store.value = token; val barrier = Barrier(events); barrier.markFails = true
        val api = Api(events)
        val owner = NativeAccountController(this, configuration, api, Crypto(), Browser(), store, barrier) { 100 }
        owner.beginLogin(); runCurrent()
        assertEquals(listOf("mark"), events); assertEquals(token, store.value)
        assertEquals(NativeAccountState.CleanupRequired, owner.state.value); owner.close()
        events.clear()
        val restarted = NativeAccountController(this, configuration, api, Crypto(), Browser(), store, barrier) { 100 }
        runCurrent(); assertEquals(emptyList(), events)
        assertEquals(NativeAccountState.SignedOut, restarted.state.value)
        restarted.close()
    }
    @Test fun signed_in_logout_retains_known_token_when_os_read_becomes_denied() = runTest {
        val events = mutableListOf<String>(); val store = Store(events); store.value = token; val barrier = Barrier(events); val api = Api(events)
        val owner = NativeAccountController(this, configuration, api, Crypto(), Browser(), store, barrier) { 100 }
        owner.restore(); runCurrent(); store.readDenied = true
        events.clear(); owner.logout(); runCurrent()
        assertEquals(listOf("mark", "revoke", "clear", "unmark"), events)
        assertEquals(NativeAccountState.SignedOut, owner.state.value); assertNull(store.value)
        owner.close()
    }
    @Test fun late_old_account_success_and_failure_cannot_restore_signed_out_projection() = runTest {
        for (late in listOf(NativeApiResult.Accepted(99), NativeApiResult.Refused(NativeApiFailure.Unauthorized))) {
            val events = mutableListOf<String>(); val store = Store(events); store.value = token; val api = Api(events); val barrier = Barrier(events)
            val owner = NativeAccountController(this, configuration, api, Crypto(), Browser(), store, barrier) { 100 }
            owner.restore(); runCurrent(); api.savedResult = CompletableDeferred()
            owner.refreshSaved(); runCurrent(); owner.logout(); runCurrent()
            api.savedResult!!.complete(late); runCurrent()
            assertEquals(NativeAccountState.SignedOut, owner.state.value)
            owner.close()
        }
    }
}
