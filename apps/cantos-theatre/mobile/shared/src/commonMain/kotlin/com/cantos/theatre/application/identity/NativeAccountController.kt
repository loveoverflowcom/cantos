package com.cantos.theatre.application.identity

import com.cantos.theatre.ports.NativeApiResult
import com.cantos.theatre.ports.NativeApiFailure
import com.cantos.theatre.domain.identity.NativeClientConfiguration
import com.cantos.theatre.ports.NativeIdentityApi
import com.cantos.theatre.ports.NativeSession
import com.cantos.theatre.domain.identity.CallbackOutcome
import com.cantos.theatre.domain.identity.NativeCallback
import com.cantos.theatre.ports.CredentialRead
import com.cantos.theatre.ports.CredentialStore
import com.cantos.theatre.ports.IdentityCrypto
import com.cantos.theatre.ports.LogoutBarrier
import com.cantos.theatre.ports.LogoutBarrierRead
import com.cantos.theatre.ports.SystemBrowser
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

enum class NativeAccountFailure { Configuration, Entropy, Login, Storage, Cleanup, Expired, Api, Protocol, Exhausted }
sealed interface NativeAccountState {
    data object SignedOut : NativeAccountState
    data object Pending : NativeAccountState
    data class SignedIn(val actor: String, val expiresAt: Long, val savedCount: Int? = null) : NativeAccountState
    data class Failed(val reason: NativeAccountFailure) : NativeAccountState
    data object CleanupRequired : NativeAccountState
}

/** One process/main-dispatcher owner. No automatic restore; every foreign result is fenced. */
class NativeAccountController(
    private val scope: CoroutineScope,
    private val configuration: NativeClientConfiguration,
    private val api: NativeIdentityApi,
    crypto: IdentityCrypto,
    browser: SystemBrowser,
    private val store: CredentialStore,
    private val barrier: LogoutBarrier,
    private val now: () -> Long,
) {
    private val login = NativeLoginController(crypto, browser)
    private val accounts = AccountLifecycle()
    private val mutableState = MutableStateFlow<NativeAccountState>(NativeAccountState.SignedOut)
    val state: StateFlow<NativeAccountState> = mutableState.asStateFlow()
    private var generation = 0L
    private var refreshGeneration = 0L
    private var job: Job? = null
    private var flow: String? = null
    private var credential: String? = null
    private var cleanupCredential: String? = null
    private var closed = false
    init {
        if (barrier.read() != LogoutBarrierRead.Clear) mutableState.value = NativeAccountState.CleanupRequired
    }
    private fun advance(): Long? {
        if (closed || generation == Long.MAX_VALUE) {
            mutableState.value = NativeAccountState.Failed(NativeAccountFailure.Exhausted); return null
        }
        generation += 1
        job?.cancel(); job = null
        login.cancel(); flow = null
        accounts.leave()
        credential?.let { cleanupCredential = it }
        credential = null
        return generation
    }
    private fun current(issued: Long): Boolean = !closed && generation == issued
    private fun fail(reason: NativeAccountFailure) { mutableState.value = NativeAccountState.Failed(reason) }
    /** Marker precedes the first effect; unknown revoke outcomes retain the exact credential. */
    private suspend fun cleanup(issued: Long): Boolean {
        if (barrier.mark() != null) { if (current(issued)) mutableState.value = NativeAccountState.CleanupRequired; return false }
        var token = cleanupCredential
        if (token == null) {
            when (val read = store.read()) {
                is CredentialRead.Found -> if (Regex("[0-9a-f]{64}").matches(read.value)) token = read.value
                is CredentialRead.Failed -> {
                    if (read.reason != com.cantos.theatre.ports.CredentialFailure.Corrupt) {
                        if (current(issued)) mutableState.value = NativeAccountState.CleanupRequired
                        return false
                    }
                }
                CredentialRead.Missing -> Unit
            }
        }
        if (token != null) {
            cleanupCredential = token
            when (api.revoke(token)) {
                is NativeApiResult.Refused -> { if (current(issued)) mutableState.value = NativeAccountState.CleanupRequired; return false }
                is NativeApiResult.Accepted -> Unit
            }
        }
        if (!current(issued)) return false
        if (store.clear() != null || barrier.clear() != null) { mutableState.value = NativeAccountState.CleanupRequired; return false }
        cleanupCredential = null
        return true
    }
    fun beginLogin() {
        val issued = advance() ?: return
        mutableState.value = NativeAccountState.Pending
        job = scope.launch {
            if (!cleanup(issued) || !current(issued)) return@launch
            if (login.prepare(configuration.identity, now()) != null) { fail(NativeAccountFailure.Entropy); return@launch }
            val attempt = login.preparedAttempt() ?: run { fail(NativeAccountFailure.Protocol); return@launch }
            when (val result = api.begin(attempt.state.value, attempt.nonce.value, attempt.challenge)) {
                is NativeApiResult.Refused -> if (current(issued)) { login.cancel(); fail(NativeAccountFailure.Api) }
                is NativeApiResult.Accepted -> if (current(issued)) {
                    flow = result.value.id
                    if (login.openPrepared(result.value.authorizationUrl) != null) { flow = null; fail(NativeAccountFailure.Configuration) }
                }
            }
        }
    }
    fun callback(callback: NativeCallback) {
        val issued = generation
        val pendingFlow = flow ?: return
        when (val outcome = login.callback(callback, now())) {
            is CallbackOutcome.Refused -> if (outcome.reason != com.cantos.theatre.domain.identity.LoginFailure.InvalidCallback &&
                outcome.reason != com.cantos.theatre.domain.identity.LoginFailure.Replay) { flow = null; fail(NativeAccountFailure.Login) }
            is CallbackOutcome.Exchange -> {
                flow = null // Consume before the one exchange; unknown outcomes never replay a code.
                job = scope.launch {
                    when (val result = api.exchange(pendingFlow, outcome.request.attempt.state.value, outcome.request.codeForExchange(), outcome.request.verifierForExchange())) {
                        is NativeApiResult.Refused -> if (current(issued)) { login.cancel(); fail(NativeAccountFailure.Api) }
                        is NativeApiResult.Accepted -> {
                            val token = result.value.credential
                            if (!current(issued) || !login.acceptsExchange(outcome.request.generation)) {
                                if (token != null) api.revoke(token)
                                return@launch
                            }
                            if (token == null || !Regex("[0-9a-f]{64}").matches(token)) {
                                if (token != null) api.revoke(token)
                                fail(NativeAccountFailure.Protocol); return@launch
                            }
                            if (result.value.expiresAt <= now()) { api.revoke(token); fail(NativeAccountFailure.Expired); return@launch }
                            if (barrier.mark() != null || store.write(token) != null || barrier.clear() != null) {
                                api.revoke(token)
                                if (current(issued)) mutableState.value = NativeAccountState.CleanupRequired
                                return@launch
                            }
                            establish(result.value, token, issued)
                        }
                    }
                }
            }
        }
    }
    private fun establish(session: NativeSession, token: String, issued: Long) {
        if (!current(issued)) return
        val account = AccountSession.fromServer(session.actor, session.session)
        if (account == null) { fail(NativeAccountFailure.Protocol); return }
        if (session.expiresAt <= now()) { fail(NativeAccountFailure.Expired); return }
        if (accounts.enter(account) != null) { fail(NativeAccountFailure.Exhausted); return }
        credential = token
        mutableState.value = NativeAccountState.SignedIn(session.actor, session.expiresAt)
    }
    /** Explicit user action; a local credential alone never establishes an account. */
    fun restore() {
        val issued = advance() ?: return
        if (barrier.read() != LogoutBarrierRead.Clear) { mutableState.value = NativeAccountState.CleanupRequired; return }
        val read = store.read()
        if (read is CredentialRead.Failed) { fail(NativeAccountFailure.Storage); return }
        if (read !is CredentialRead.Found) { mutableState.value = NativeAccountState.SignedOut; return }
        if (!Regex("[0-9a-f]{64}").matches(read.value)) { mutableState.value = NativeAccountState.CleanupRequired; return }
        mutableState.value = NativeAccountState.Pending
        job = scope.launch {
            when (val result = api.session(read.value)) {
                is NativeApiResult.Refused -> if (current(issued)) fail(NativeAccountFailure.Api)
                is NativeApiResult.Accepted -> establish(result.value, read.value, issued)
            }
        }
    }
    fun refreshSaved() {
        val token = credential ?: return
        val fence = accounts.fence() ?: return
        val signedIn = mutableState.value as? NativeAccountState.SignedIn ?: return
        val issued = generation
        if (refreshGeneration == Long.MAX_VALUE) { fail(NativeAccountFailure.Exhausted); return }
        refreshGeneration += 1
        val requested = refreshGeneration
        job?.cancel()
        job = scope.launch {
            when (val result = api.saved(token, fence.session.session)) {
                is NativeApiResult.Refused -> if (current(issued) && refreshGeneration == requested && accounts.accepts(fence)) {
                    accounts.leave(); credential = null
                    if (result.reason == NativeApiFailure.SessionChanged) {
                        mutableState.value = NativeAccountState.Pending
                        when (val fresh = api.session(token)) {
                            is NativeApiResult.Refused -> if (current(issued)) fail(NativeAccountFailure.Api)
                            is NativeApiResult.Accepted -> if (current(issued)) {
                                if (fresh.value.actor == signedIn.actor) establish(fresh.value, token, issued)
                                else fail(NativeAccountFailure.Protocol)
                            }
                        }
                    } else fail(NativeAccountFailure.Api)
                }
                is NativeApiResult.Accepted -> if (current(issued) && refreshGeneration == requested && accounts.accepts(fence)) mutableState.value = signedIn.copy(savedCount = result.value)
            }
        }
    }
    fun logout() {
        val issued = advance() ?: return
        mutableState.value = NativeAccountState.CleanupRequired // Hide account data before any IO.
        job = scope.launch { if (cleanup(issued) && current(issued)) mutableState.value = NativeAccountState.SignedOut }
    }
    fun cancelLogin() { advance()?.let { mutableState.value = if (barrier.read() == LogoutBarrierRead.Clear) NativeAccountState.SignedOut else NativeAccountState.CleanupRequired } }
    fun close() { if (!closed) { advance(); closed = true } }
}
