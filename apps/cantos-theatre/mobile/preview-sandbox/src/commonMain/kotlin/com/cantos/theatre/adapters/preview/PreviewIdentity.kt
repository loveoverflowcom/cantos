package com.cantos.theatre.adapters.preview

import com.cantos.theatre.application.identity.NativeAccountController
import com.cantos.theatre.domain.identity.NativeGrant
import com.cantos.theatre.ports.CredentialFailure
import com.cantos.theatre.ports.LogoutBarrier
import com.cantos.theatre.ports.LogoutBarrierRead
import com.cantos.theatre.ports.NativeCredentialJournal
import com.cantos.theatre.ports.NativeCredentialRecord
import com.cantos.theatre.ports.NativeJournalRead
import com.cantos.theatre.ports.NativeSession
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.delay

/** A synthetic listener. Account switching in the preview is logout followed by a login as the other one. */
class PreviewAccount(val actor: String, val session: String) {
    override fun toString(): String = "PreviewAccount([synthetic])"

    companion object {
        val Linh = PreviewAccount("00000000-0000-4000-8000-0000000000a1", "00000000-0000-4000-8000-0000000000b1")
        val Minh = PreviewAccount("00000000-0000-4000-8000-0000000000a2", "00000000-0000-4000-8000-0000000000b2")
        val entries: List<PreviewAccount> = listOf(Linh, Minh)
    }
}

/** Scripted Identity HTTP outcomes, not a browser or another session engine. */
enum class PreviewLoginMode { Approve, Hold, Deny }

/** Production v1 account owner; only the remote API, journal and barrier are mocks. */
class PreviewIdentity(
    private val scope: CoroutineScope,
    private val nowSeconds: () -> Long,
    signedInAs: PreviewAccount? = null,
) {
    val configuration = (com.cantos.theatre.domain.identity.NativeIdentityV1Configuration.resolve("https://api.invalid") as
        com.cantos.theatre.domain.identity.NativeIdentityV1ConfigurationResult.Available).configuration
    var nextAccount: PreviewAccount = signedInAs?.let { other -> PreviewAccount.entries.first { it !== other } } ?: PreviewAccount.Linh
    var mode = PreviewLoginMode.Approve
    var loginFailure: com.cantos.theatre.ports.IdentityV1Failure? = null
    var refreshRefused = false
    var loginRequests = 0
        private set
    var admissions = 0
        private set
    var clearFails = false
    private var pending: kotlinx.coroutines.CompletableDeferred<Boolean>? = null
    private var sequence = 1
    private val grants = HashMap<String, NativeGrant>()
    private inner class InMemoryJournal(first: NativeCredentialRecord?) : NativeCredentialJournal {
        private var record = first
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
        override fun read() = if (pending) LogoutBarrierRead.Pending else LogoutBarrierRead.Clear
        override fun mark(): CredentialFailure? { pending = true; return null }
        override fun clear(): CredentialFailure? { pending = false; return null }
    }
    val api = object : com.cantos.theatre.ports.NativeIdentityV1Api {
        override suspend fun signup(email: com.cantos.theatre.domain.identity.RegistrationEmail): com.cantos.theatre.ports.IdentityV1Result<com.cantos.theatre.ports.IdentitySignupAccepted> =
            com.cantos.theatre.adapters.http.NativeIdentityV1Client(configuration, object : com.cantos.theatre.ports.NativeHttpPort {
                override fun send(request: com.cantos.theatre.ports.NativeHttpRequest, completion: (com.cantos.theatre.ports.NativeHttpResponse) -> Unit): com.cantos.theatre.ports.NativeRequestHandle {
                    completion(com.cantos.theatre.ports.NativeHttpResponse(202, "application/json", """{"accepted":true,"proof_ttl_seconds":900}""", null))
                    return object : com.cantos.theatre.ports.NativeRequestHandle { override fun cancel() = Unit }
                }
            }, nowSeconds).signup(email)
        override suspend fun login(email: com.cantos.theatre.domain.identity.RegistrationEmail, password: String): com.cantos.theatre.ports.IdentityV1Result<NativeGrant> {
            loginRequests += 1
            val approved = when (mode) {
                PreviewLoginMode.Hold -> kotlinx.coroutines.CompletableDeferred<Boolean>().also { pending = it }.await()
                PreviewLoginMode.Approve -> { delay(50); true }
                PreviewLoginMode.Deny -> { delay(50); false }
            }
            val failure = loginFailure
            return if (failure != null) com.cantos.theatre.ports.IdentityV1Result.Refused(failure)
            else if (!approved) com.cantos.theatre.ports.IdentityV1Result.Refused(com.cantos.theatre.ports.IdentityV1Failure.Unauthorized)
            else com.cantos.theatre.ports.IdentityV1Result.Accepted(issue(nextAccount, null))
        }
        override suspend fun refresh(previous: NativeGrant): com.cantos.theatre.ports.IdentityV1Result<NativeGrant> =
            if (refreshRefused) com.cantos.theatre.ports.IdentityV1Result.Refused(com.cantos.theatre.ports.IdentityV1Failure.Unauthorized)
            else com.cantos.theatre.ports.IdentityV1Result.Accepted(issue(PreviewAccount.entries.first { it.session == previous.view.session }, previous))
        override suspend fun revokeFamily(previous: NativeGrant): com.cantos.theatre.ports.IdentityV1Result<Unit> {
            grants.values.removeAll { it.view.session == previous.view.session }
            return com.cantos.theatre.ports.IdentityV1Result.Accepted(Unit)
        }
        override suspend fun session(previous: NativeGrant): com.cantos.theatre.ports.IdentityV1Result<NativeSession> {
            admissions += 1
            return grants[previous.accessToken]?.let { com.cantos.theatre.ports.IdentityV1Result.Accepted(it.view) }
                ?: com.cantos.theatre.ports.IdentityV1Result.Refused(com.cantos.theatre.ports.IdentityV1Failure.Unauthorized)
        }
        override suspend fun revoke(previous: NativeGrant) = revokeFamily(previous)
    }
    val account = NativeAccountController(scope, configuration, api, journal, barrier, nowSeconds)
    fun completeLogin(approve: Boolean) { pending?.complete(approve); pending = null }
    fun persistSession(account: PreviewAccount = PreviewAccount.Linh) { journal.write(NativeCredentialRecord.Active(issue(account, null))) }
    private fun issue(account: PreviewAccount, previous: NativeGrant?): NativeGrant {
        val now = nowSeconds()
        val grant = (NativeGrant.identityV1("ci1_n_" + hex(sequence++), "ci1_r_" + hex(sequence++),
            account.actor, account.session, now + 120, now + 600, previous?.absoluteExpiresAt ?: (now + 900),
            previous?.authenticatedAt ?: now, now, previous) as com.cantos.theatre.domain.identity.IdentityV1Validation.Accepted).value
        grants.values.removeAll { it.view.session == account.session }
        grants[grant.accessToken] = grant
        return grant
    }
    private fun hex(number: Int) = number.toString(16).padStart(64, '0')
}
