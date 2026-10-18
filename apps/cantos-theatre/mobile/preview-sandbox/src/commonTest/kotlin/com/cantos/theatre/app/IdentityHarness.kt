package com.cantos.theatre.app

import com.cantos.theatre.adapters.mock.sandbox.sessions.MockStorageSession
import com.cantos.theatre.adapters.mock.sandbox.sessions.seed
import com.cantos.theatre.application.identity.NativeAccountController
import com.cantos.theatre.adapters.identity.SecureNativeCredentialJournal
import com.cantos.theatre.presentation.model.ShellState
import com.cantos.theatre.domain.identity.LoginSecret
import com.cantos.theatre.domain.identity.NativeCallback
import com.cantos.theatre.domain.identity.NativeClientConfiguration
import com.cantos.theatre.domain.identity.NativeGrant
import com.cantos.theatre.ports.CredentialFailure
import com.cantos.theatre.ports.CredentialRead
import com.cantos.theatre.ports.CredentialStore
import com.cantos.theatre.ports.IdentityCrypto
import com.cantos.theatre.ports.LogoutBarrier
import com.cantos.theatre.ports.LogoutBarrierRead
import com.cantos.theatre.ports.NativeApiResult
import com.cantos.theatre.ports.NativeFlow
import com.cantos.theatre.ports.NativeIdentityApi
import com.cantos.theatre.ports.NativeCredentialRecord
import com.cantos.theatre.ports.NativeSession
import com.cantos.theatre.ports.SystemBrowser
import kotlinx.coroutines.CoroutineScope

/**
 * The real account owner and app container over deterministic identity ports. Nothing here is a
 * provider: the "server" accepts the first attempt's state, so it proves the app's lifecycle and
 * fencing, never live identity, secure storage or system-browser behavior.
 */
internal class IdentityHarness(scope: CoroutineScope, seed: ShellState = MockStorageSession.Demo.seed()) {
    val actor = "00000000-0000-4000-8000-000000000001"
    val session = "00000000-0000-4000-8000-000000000002"
    val token = "ca1_n_" + "a".repeat(64)
    val grant = requireNotNull(NativeGrant.create(token, "cr1_" + "b".repeat(64), actor, session,
        200, 700, 1000, 100, now = 100))
    val configuration = NativeClientConfiguration.create(
        "https://api.example", "https://idm.example/oauth2/openid/cantos-theatre-native",
    )!!

    class Crypto : IdentityCrypto {
        var count = 0
        override fun secret(): LoginSecret = LoginSecret.parse(('a' + count++).toString().repeat(64))!!
        override fun challenge(verifier: LoginSecret): String = "x".repeat(43)
    }

    class Browser : SystemBrowser {
        val opened = mutableListOf<String>()
        override fun open(url: String): Boolean { opened += url; return true }
        override fun cancel() = Unit
    }

    class Store : CredentialStore {
        var value: String? = null
        var clearFails = false
        override fun read(): CredentialRead = value?.let { CredentialRead.Found(it) } ?: CredentialRead.Missing
        override fun write(value: String): CredentialFailure? { this.value = value; return null }
        override fun clear(): CredentialFailure? {
            if (clearFails) return CredentialFailure.Denied
            value = null
            return null
        }
    }

    class Barrier : LogoutBarrier {
        var pending = false
        override fun read(): LogoutBarrierRead = if (pending) LogoutBarrierRead.Pending else LogoutBarrierRead.Clear
        override fun mark(): CredentialFailure? { pending = true; return null }
        override fun clear(): CredentialFailure? { pending = false; return null }
    }

    inner class Api : NativeIdentityApi {
        var begins = 0
        var exchanges = 0
        override suspend fun begin(state: String, nonce: String, challenge: String): NativeApiResult<NativeFlow> {
            begins += 1
            return NativeApiResult.Accepted(NativeFlow(session,
                "https://idm.example/ui/oauth2?scope=openid&response_type=code&client_id=cantos-theatre-native" +
                    "&redirect_uri=com.cantos.theatre%3A%2Foauth2%2Fcallback&state=$state&nonce=$nonce" +
                    "&code_challenge_method=S256&code_challenge=$challenge"))
        }
        override suspend fun exchange(flow: String, state: String, code: String, verifier: String): NativeApiResult<NativeGrant> {
            exchanges += 1
            return NativeApiResult.Accepted(grant)
        }
        override suspend fun refresh(refreshToken: String, session: String): NativeApiResult<NativeGrant> =
            NativeApiResult.Refused(com.cantos.theatre.ports.NativeApiFailure.Unavailable)
        override suspend fun revokeFamily(refreshToken: String, session: String): NativeApiResult<Unit> = NativeApiResult.Accepted(Unit)
        override suspend fun cancel(flow: String, state: String, verifier: String): NativeApiResult<Unit> = NativeApiResult.Accepted(Unit)
        override suspend fun session(credential: String): NativeApiResult<NativeSession> =
            NativeApiResult.Accepted(grant.view)
        override suspend fun revoke(credential: String): NativeApiResult<Unit> = NativeApiResult.Accepted(Unit)
        override suspend fun saved(credential: String, session: String): NativeApiResult<Int> = NativeApiResult.Accepted(0)
    }

    val browser = Browser()
    val store = Store()
    private val journal = SecureNativeCredentialJournal(store, configuration.apiOrigin, configuration.identity.issuer)
    val barrier = Barrier()
    val api = Api()
    val account = NativeAccountController(scope, configuration, api, Crypto(), browser, journal, barrier) { 100 }
    val app = mockAppContainer(scope, seed, account = account)

    /** The provider return for the first attempt: its secret state is the first generated secret. */
    fun first(hasError: Boolean = false) = NativeCallback(
        configuration.identity.redirect, "a".repeat(64), if (hasError) "" else "code", hasError,
    )

    fun persistSession() { check(journal.write(NativeCredentialRecord.Active(grant)) == null) }

    fun close() { app.close(); account.close() }
}
