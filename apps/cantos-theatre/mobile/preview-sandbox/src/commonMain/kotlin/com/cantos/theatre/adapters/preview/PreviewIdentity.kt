package com.cantos.theatre.adapters.preview

import com.cantos.theatre.application.identity.NativeAccountController
import com.cantos.theatre.domain.identity.LoginSecret
import com.cantos.theatre.domain.identity.NativeCallback
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
import com.cantos.theatre.ports.NativeIdentityApi
import com.cantos.theatre.ports.NativeJournalRead
import com.cantos.theatre.ports.NativeSession
import com.cantos.theatre.ports.SystemBrowser
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

/** A synthetic listener. Account switching in the preview is logout followed by a login as the other one. */
class PreviewAccount(val actor: String, val session: String) {
    override fun toString(): String = "PreviewAccount([synthetic])"

    companion object {
        val Linh = PreviewAccount("00000000-0000-4000-8000-0000000000a1", "00000000-0000-4000-8000-0000000000b1")
        val Minh = PreviewAccount("00000000-0000-4000-8000-0000000000a2", "00000000-0000-4000-8000-0000000000b2")
        val entries: List<PreviewAccount> = listOf(Linh, Minh)
    }
}

/** What the stand-in identity provider does when the listener returns from "the browser". */
enum class PreviewLoginMode {
    /** The callback arrives with a code and the exchange succeeds. */
    Approve,
    /** The browser stays open: the attempt remains pending until the listener cancels. */
    Hold,
    /** The provider answers with an error callback. */
    Deny,
}

/**
 * The real [NativeAccountController] over deterministic identity ports. Every port here is an
 * external boundary (identity API, secure credential journal, logout barrier, entropy, system
 * browser); the lifecycle, fencing and cleanup are the production owner's. Nothing proves live
 * identity, secure storage or system-browser behavior.
 */
class PreviewIdentity(
    private val scope: CoroutineScope,
    /** Seconds. Held constant by default so a long-running preview never expires on its own. */
    private val nowSeconds: () -> Long,
    signedInAs: PreviewAccount? = null,
) {
    val configuration: NativeClientConfiguration = requireNotNull(
        NativeClientConfiguration.create("https://api.invalid", "https://identity.invalid/oauth2/openid/cantos-theatre-native"),
    )

    /** Who the next completed login authenticates as. */
    var nextAccount: PreviewAccount = signedInAs?.let { other -> PreviewAccount.entries.first { it !== other } } ?: PreviewAccount.Linh
    var mode: PreviewLoginMode = PreviewLoginMode.Approve
    /** When true the provider refuses to rotate credentials, which ends the session like an expiry. */
    var refreshRefused: Boolean = false
    var browserOpens: Int = 0
        private set
    var begins: Int = 0
        private set
    var exchanges: Int = 0
        private set
    /** When true the secure store refuses to clear, which owes the account a visible cleanup. */
    var clearFails: Boolean = false
    /** The `state` of the authorization URL last handed to the browser; the provider return must echo it. */
    private var pendingState: String? = null

    private var sequence = 1
    private val grants = HashMap<String, NativeGrant>()
    private lateinit var controller: NativeAccountController

    private inner class InMemoryJournal(first: NativeCredentialRecord?) : NativeCredentialJournal {
        private var record: NativeCredentialRecord? = first
        override fun read(): NativeJournalRead = record?.let(NativeJournalRead::Found) ?: NativeJournalRead.Missing
        override fun write(record: NativeCredentialRecord): CredentialFailure? { this.record = record; return null }
        override fun clear(): CredentialFailure? {
            if (clearFails) return CredentialFailure.Denied
            record = null
            return null
        }
    }

    private val journal = InMemoryJournal(signedInAs?.let { NativeCredentialRecord.Active(issue(it, null)) })

    private val barrier = object : LogoutBarrier {
        private var pending = false
        override fun read(): LogoutBarrierRead = if (pending) LogoutBarrierRead.Pending else LogoutBarrierRead.Clear
        override fun mark(): CredentialFailure? { pending = true; return null }
        override fun clear(): CredentialFailure? { pending = false; return null }
    }

    private val crypto = object : IdentityCrypto {
        private var count = 0
        override fun secret(): LoginSecret = requireNotNull(LoginSecret.parse(hex(1_000 + count++))) { "Preview entropy" }
        override fun challenge(verifier: LoginSecret): String = "x".repeat(43)
    }

    private val browser = object : SystemBrowser {
        override fun open(url: String): Boolean {
            browserOpens += 1
            pendingState = Regex("[?&]state=([A-Za-z0-9._~-]+)").find(url)?.groupValues?.get(1) ?: return false
            when (mode) {
                PreviewLoginMode.Hold -> Unit
                // The provider returns after the listener leaves the app; never within `open`.
                PreviewLoginMode.Approve -> scope.launch { delay(CallbackDelayMs); returnFromBrowser(approve = true) }
                PreviewLoginMode.Deny -> scope.launch { delay(CallbackDelayMs); returnFromBrowser(approve = false) }
            }
            return true
        }
        override fun cancel() = Unit
    }

    private val api = object : NativeIdentityApi {
        override suspend fun begin(state: String, nonce: String, challenge: String): NativeApiResult<NativeFlow> {
            begins += 1
            return NativeApiResult.Accepted(
                NativeFlow(
                    "preview-flow-${sequence++}",
                    "https://identity.invalid/ui/oauth2?scope=openid&response_type=code&client_id=cantos-theatre-native" +
                        "&redirect_uri=com.cantos.theatre%3A%2Foauth2%2Fcallback&state=$state&nonce=$nonce" +
                        "&code_challenge_method=S256&code_challenge=$challenge",
                ),
            )
        }

        override suspend fun exchange(flow: String, state: String, code: String, verifier: String): NativeApiResult<NativeGrant> {
            exchanges += 1
            return NativeApiResult.Accepted(issue(nextAccount, null))
        }

        override suspend fun refresh(refreshToken: String, session: String): NativeApiResult<NativeGrant> {
            val previous = grants.values.firstOrNull { it.refreshToken == refreshToken && it.view.session == session }
            if (refreshRefused || previous == null) return NativeApiResult.Refused(NativeApiFailure.Unauthorized)
            val account = PreviewAccount.entries.first { it.session == session }
            return NativeApiResult.Accepted(issue(account, previous))
        }

        override suspend fun revokeFamily(refreshToken: String, session: String): NativeApiResult<Unit> {
            grants.values.removeAll { it.view.session == session }
            return NativeApiResult.Accepted(Unit)
        }

        override suspend fun cancel(flow: String, state: String, verifier: String): NativeApiResult<Unit> = NativeApiResult.Accepted(Unit)

        override suspend fun session(credential: String): NativeApiResult<NativeSession> =
            grants.values.firstOrNull { it.accessToken == credential }?.let { NativeApiResult.Accepted(it.view) }
                ?: NativeApiResult.Refused(NativeApiFailure.Unauthorized)

        override suspend fun revoke(credential: String): NativeApiResult<Unit> = NativeApiResult.Accepted(Unit)

        override suspend fun saved(credential: String, session: String): NativeApiResult<Int> = NativeApiResult.Accepted(0)
    }

    val account: NativeAccountController =
        NativeAccountController(scope, configuration, api, crypto, browser, journal, barrier, nowSeconds)
            .also { controller = it }

    /**
     * The provider's return to the app for the attempt the browser last opened. With [PreviewLoginMode.Hold]
     * the workbench (or a test) calls this to approve or refuse; a stale or absent attempt is ignored by the owner.
     */
    fun returnFromBrowser(approve: Boolean) {
        val state = pendingState ?: return
        controller.callback(
            NativeCallback(configuration.identity.redirect, state, if (approve) "preview-code" else "", hasError = !approve),
        )
    }

    /** Puts [account]'s credential on this device without signing in; an explicit restore may then resume it. */
    fun persistSession(account: PreviewAccount = PreviewAccount.Linh) {
        journal.write(NativeCredentialRecord.Active(issue(account, null)))
    }

    /** A bounded pair as the API issues it: access <= 120 s, refresh <= 600 s, absolute <= 900 s. */
    private fun issue(account: PreviewAccount, previous: NativeGrant?): NativeGrant {
        val now = nowSeconds()
        val access = "ca1_n_" + hex(sequence++)
        val refresh = "cr1_" + hex(sequence++)
        val grant = requireNotNull(
            NativeGrant.create(access, refresh, account.actor, account.session, now + 120, now + 600, now + 900, now, now, previous),
        ) { "Preview grant must satisfy the production admission bounds" }
        grants.values.removeAll { it.view.session == account.session }
        grants[access] = grant
        return grant
    }

    private fun hex(number: Int): String = number.toString(16).padStart(64, '0')

    private companion object {
        const val CallbackDelayMs = 50L
    }
}
