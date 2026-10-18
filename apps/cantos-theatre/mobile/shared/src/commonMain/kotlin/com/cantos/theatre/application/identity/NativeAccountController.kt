package com.cantos.theatre.application.identity

import com.cantos.theatre.domain.identity.CallbackOutcome
import com.cantos.theatre.domain.identity.LoginFailure
import com.cantos.theatre.domain.identity.NativeCallback
import com.cantos.theatre.domain.identity.NativeClientConfiguration
import com.cantos.theatre.domain.identity.NativeGrant
import com.cantos.theatre.ports.IdentityCrypto
import com.cantos.theatre.ports.LogoutBarrier
import com.cantos.theatre.ports.LogoutBarrierRead
import com.cantos.theatre.ports.NativeApiFailure
import com.cantos.theatre.ports.NativeApiResult
import com.cantos.theatre.ports.NativeCredentialJournal
import com.cantos.theatre.ports.NativeCredentialRecord
import com.cantos.theatre.ports.NativeIdentityApi
import com.cantos.theatre.ports.NativeJournalRead
import com.cantos.theatre.ports.NativeSession
import com.cantos.theatre.ports.SystemBrowser
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
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

/** API-confirmed authority, bound to one local intent and one rotation. Never saved in UI state. */
class NativeVerifiedSession internal constructor(val session: NativeSession, val bearer: String, internal val intent: Long = 0) {
    override fun toString(): String = "NativeVerifiedSession([redacted])"
}

/** One process/application-dispatcher owner. Secure journal precedes every consuming effect.
 * Network cancellation never proves noncommit: Flow/Refreshing records are cleanup-only on restart. */
class NativeAccountController(
    private val scope: CoroutineScope,
    private val configuration: NativeClientConfiguration,
    private val api: NativeIdentityApi,
    crypto: IdentityCrypto,
    browser: SystemBrowser,
    private val store: NativeCredentialJournal,
    private val barrier: LogoutBarrier,
    private val now: () -> Long,
) {
    private val login = NativeLoginController(crypto, browser)
    private val mutableState = MutableStateFlow<NativeAccountState>(NativeAccountState.SignedOut)
    val state: StateFlow<NativeAccountState> = mutableState.asStateFlow()
    private val mutableVerifiedSession = MutableStateFlow<NativeVerifiedSession?>(null)
    val verifiedSession: StateFlow<NativeVerifiedSession?> = mutableVerifiedSession.asStateFlow()
    private var generation = 0L
    // Separate from IO generation: revalidation breaks playback continuity even if StateFlow
    // conflates the intermediate null with an immediate successful session response.
    private var admissionEpoch = 0L
    private var savedGeneration = 0L
    private var operation: Job? = null
    private var expiry: Job? = null
    private var rotation: Job? = null
    private var flight: CompletableDeferred<NativeApiResult<NativeVerifiedSession>>? = null
    private var pendingFlow: NativeCredentialRecord.Flow? = null
    private var grant: NativeGrant? = null
    private val acceptedAccess = mutableSetOf<String>()
    private var closed = false

    init {
        if (barrier.read() != LogoutBarrierRead.Clear) mutableState.value = NativeAccountState.CleanupRequired
    }

    private fun current(issued: Long): Boolean = !closed && generation == issued
    private fun hide() {
        mutableVerifiedSession.value = null
        grant = null
        acceptedAccess.clear()
        expiry?.cancel(); expiry = null
    }
    private fun advance(): Long? {
        hide()
        if (closed || generation == Long.MAX_VALUE || admissionEpoch == Long.MAX_VALUE) {
            mutableState.value = NativeAccountState.Failed(NativeAccountFailure.Exhausted)
            return null
        }
        generation += 1
        admissionEpoch += 1
        operation?.cancel(); operation = null
        rotation?.cancel(); rotation = null
        flight?.complete(NativeApiResult.Refused(NativeApiFailure.SessionChanged)); flight = null
        login.cancel(); pendingFlow = null
        return generation
    }
    private fun fail(reason: NativeAccountFailure) {
        hide()
        mutableState.value = NativeAccountState.Failed(reason)
    }
    private fun cleanupRequired() {
        hide()
        mutableState.value = NativeAccountState.CleanupRequired
    }
    private fun mark(): Boolean {
        // Establish durable emptiness before marking a first login/logout. A restart between
        // these writes can finish cleanup without a RAM-only exception to the barrier.
        if (barrier.read() == LogoutBarrierRead.Clear && store.read() == NativeJournalRead.Missing &&
            store.write(NativeCredentialRecord.Cleared) != null) {
            cleanupRequired()
            return false
        }
        if (barrier.mark() == null) return true
        cleanupRequired()
        return false
    }

    /** Cleared is durable before deleting the marker: a crash between the two can finish safely. */
    private suspend fun cleanup(issued: Long): Boolean {
        val previous = barrier.read()
        if (previous == LogoutBarrierRead.Unavailable) { if (current(issued)) cleanupRequired(); return false }
        val read = store.read()
        if (read is NativeJournalRead.Failed || (read == NativeJournalRead.Missing && previous == LogoutBarrierRead.Pending)) {
            if (current(issued)) cleanupRequired()
            return false
        }
        if (!current(issued) || !mark()) return false
        val record = (read as? NativeJournalRead.Found)?.record
        val result = when (record) {
            is NativeCredentialRecord.Active -> api.revokeFamily(record.grant.refreshToken, record.grant.view.session)
            is NativeCredentialRecord.Refreshing -> api.revokeFamily(record.grant.refreshToken, record.grant.view.session)
            is NativeCredentialRecord.Flow -> api.cancel(record.flow, record.state, record.verifier)
            // Old single-token records have no API/issuer binding. Never disclose them to a
            // possibly changed origin; remove locally and require authorization. Their original
            // server authority expires within the pre-migration 900-second bound.
            is NativeCredentialRecord.Legacy -> NativeApiResult.Accepted(Unit)
            NativeCredentialRecord.Cleared, null -> NativeApiResult.Accepted(Unit)
        }
        if (!current(issued)) return false
        if (result is NativeApiResult.Refused || store.write(NativeCredentialRecord.Cleared) != null ||
            barrier.clear() != null || store.clear() != null) {
            cleanupRequired()
            return false
        }
        return true
    }

    fun beginLogin() {
        val issued = advance() ?: return
        if (!mark()) return
        mutableState.value = NativeAccountState.Pending
        operation = scope.launch {
            if (!cleanup(issued) || !current(issued)) return@launch
            if (login.prepare(configuration.identity, now()) != null) { fail(NativeAccountFailure.Entropy); return@launch }
            val attempt = login.preparedAttempt() ?: run { fail(NativeAccountFailure.Protocol); return@launch }
            when (val result = api.begin(attempt.state.value, attempt.nonce.value, attempt.challenge)) {
                is NativeApiResult.Refused -> if (current(issued)) { login.cancel(); fail(NativeAccountFailure.Api) }
                is NativeApiResult.Accepted -> if (current(issued)) {
                    val flow = NativeCredentialRecord.Flow(result.value.id, attempt.state.value, attempt.verifier.value)
                    if (!mark() || store.write(flow) != null) { cleanupRequired(); return@launch }
                    pendingFlow = flow
                    if (login.openPrepared(result.value.authorizationUrl) != null) {
                        if (cleanup(issued) && current(issued)) fail(NativeAccountFailure.Configuration)
                    }
                }
            }
        }
    }

    fun callback(callback: NativeCallback) {
        val flow = pendingFlow ?: return
        val issued = generation
        when (val outcome = login.callback(callback, now())) {
            is CallbackOutcome.Refused -> if (outcome.reason !in setOf(LoginFailure.InvalidCallback, LoginFailure.Replay)) {
                val failedIntent = advance() ?: return
                cleanupRequired()
                if (mark()) operation = scope.launch {
                    if (cleanup(failedIntent) && current(failedIntent)) fail(NativeAccountFailure.Login)
                }
            }
            is CallbackOutcome.Exchange -> {
                pendingFlow = null // One exchange. Its durable Flow remains until full pair replacement.
                operation = scope.launch {
                    when (val result = api.exchange(flow.flow, flow.state, outcome.request.codeForExchange(), flow.verifier)) {
                        is NativeApiResult.Refused -> if (current(issued)) {
                            login.cancel()
                            if (cleanup(issued) && current(issued)) fail(NativeAccountFailure.Api)
                        }
                        is NativeApiResult.Accepted -> {
                            if (!current(issued) || !login.acceptsExchange(outcome.request.generation)) {
                                api.cancel(flow.flow, flow.state, flow.verifier)
                                return@launch
                            }
                            val admitted = result.value.admit(now())
                            if (admitted == null || admitted.view.expiresAt <= now()) {
                                if (cleanup(issued) && current(issued)) fail(NativeAccountFailure.Protocol)
                            } else if (!persistAndEstablish(admitted, issued) && current(issued)) {
                                // OS persistence/readback can fail while the server already issued.
                                // The known flow can still cancel that family without reading storage.
                                val cleaned = cleanup(issued)
                                if (!cleaned && current(issued)) api.cancel(flow.flow, flow.state, flow.verifier)
                                if (cleaned && current(issued)) fail(NativeAccountFailure.Storage)
                            }
                        }
                    }
                }
            }
        }
    }

    private fun persistAndEstablish(value: NativeGrant, issued: Long): Boolean {
        if (!current(issued)) return false
        if (!mark() || store.write(NativeCredentialRecord.Active(value)) != null || barrier.clear() != null) {
            cleanupRequired()
            return false
        }
        establish(value, issued)
        return true
    }

    private fun establish(value: NativeGrant, issued: Long) {
        if (!current(issued)) return
        val admitted = value.admit(now())
        if (admitted == null || admitted.view.expiresAt <= now()) { fail(NativeAccountFailure.Expired); return }
        grant = admitted
        acceptedAccess += admitted.accessToken
        mutableVerifiedSession.value = NativeVerifiedSession(admitted.view, admitted.accessToken, admissionEpoch)
        val saved = (mutableState.value as? NativeAccountState.SignedIn)?.takeIf { it.actor == admitted.view.actor }?.savedCount
        mutableState.value = NativeAccountState.SignedIn(admitted.view.actor, admitted.view.expiresAt, saved)
        expiry?.cancel()
        // Active tabs/apps renew before expiry. The original provider/absolute authority stays fixed.
        expiry = scope.launch {
            val margin = if (admitted.expiresAt < admitted.absoluteExpiresAt) 15 else 0
            val seconds = (admitted.view.expiresAt - now() - margin).coerceAtLeast(1)
            delay(seconds * 1000)
            if (current(issued) && grant === admitted) authorization(admitted.view.session)
        }
    }

    /** Local storage is a candidate only; no account data is read until API confirmation. */
    fun restore() {
        val issued = advance() ?: return
        val barrierState = barrier.read()
        if (barrierState == LogoutBarrierRead.Unavailable) { cleanupRequired(); return }
        val read = store.read()
        if (read is NativeJournalRead.Failed) { fail(NativeAccountFailure.Storage); return }
        if (read == NativeJournalRead.Missing && barrierState == LogoutBarrierRead.Clear) { mutableState.value = NativeAccountState.SignedOut; return }
        val record = (read as? NativeJournalRead.Found)?.record
        mutableState.value = NativeAccountState.Pending
        operation = scope.launch {
            if (barrierState != LogoutBarrierRead.Clear || record !is NativeCredentialRecord.Active) {
                if (cleanup(issued) && current(issued)) mutableState.value = NativeAccountState.SignedOut
                return@launch
            }
            val candidate = record.grant.admit(now())
            if (candidate == null) {
                if (cleanup(issued) && current(issued)) fail(NativeAccountFailure.Expired)
            } else if (needsRotation(candidate)) {
                rotate(candidate, issued)
            } else when (val result = api.session(candidate.accessToken)) {
                is NativeApiResult.Refused -> if (current(issued)) {
                    if (result.reason == NativeApiFailure.Unauthorized || result.reason == NativeApiFailure.Protocol) {
                        if (cleanup(issued) && current(issued)) fail(NativeAccountFailure.Expired)
                    } else fail(NativeAccountFailure.Api)
                }
                is NativeApiResult.Accepted -> if (current(issued)) {
                    if (!matches(candidate, result.value)) {
                        if (cleanup(issued) && current(issued)) fail(NativeAccountFailure.Protocol)
                    } else establish(candidate, issued)
                }
            }
        }
    }

    private fun matches(value: NativeGrant, session: NativeSession): Boolean =
        value.view.actor == session.actor && value.view.session == session.session && value.view.expiresAt == session.expiresAt &&
            value.view.absoluteExpiresAt == session.absoluteExpiresAt && value.view.authenticatedAt == session.authenticatedAt

    fun belongsToCurrent(bearer: String, expectedSession: String): Boolean = !closed &&
        grant?.view?.session == expectedSession && bearer in acceptedAccess && mutableVerifiedSession.value != null

    fun accepts(lease: NativeVerifiedSession): Boolean = !closed && lease.intent == admissionEpoch &&
        mutableVerifiedSession.value === lease && lease.session.expiresAt > now()

    // Once access reaches the immutable authority ceiling, rotation cannot extend it. Keep the
    // still-valid lease until that deadline and then cleanup, rather than rotating every second.
    private fun needsRotation(value: NativeGrant): Boolean =
        value.expiresAt - now() <= 15 && value.expiresAt < value.absoluteExpiresAt

    /** All private effects and the expiry timer share this flight; a cancelled waiter cannot spend twice. */
    suspend fun authorization(expectedSession: String? = null): NativeApiResult<NativeVerifiedSession> {
        val value = grant ?: return NativeApiResult.Refused(NativeApiFailure.Unauthorized)
        if (expectedSession != null && expectedSession != value.view.session) return NativeApiResult.Refused(NativeApiFailure.SessionChanged)
        val lease = mutableVerifiedSession.value ?: return NativeApiResult.Refused(NativeApiFailure.Unauthorized)
        if (value.admit(now()) == null) {
            val issued = advance() ?: return NativeApiResult.Refused(NativeApiFailure.Unauthorized)
            cleanupRequired()
            if (mark()) operation = scope.launch { if (cleanup(issued) && current(issued)) fail(NativeAccountFailure.Expired) }
            return NativeApiResult.Refused(NativeApiFailure.Unauthorized)
        }
        if (!needsRotation(value) && accepts(lease)) return NativeApiResult.Accepted(lease)
        flight?.let { return it.await() }
        val shared = CompletableDeferred<NativeApiResult<NativeVerifiedSession>>()
        flight = shared
        val issued = generation
        rotation = scope.launch {
            try {
                val result = rotate(value, issued)
                shared.complete(result)
            } finally {
                shared.complete(NativeApiResult.Refused(NativeApiFailure.UnknownOutcome))
                if (flight === shared) { flight = null; rotation = null }
            }
        }
        return shared.await()
    }

    private suspend fun rotate(previous: NativeGrant, issued: Long): NativeApiResult<NativeVerifiedSession> {
        if (!current(issued)) return NativeApiResult.Refused(NativeApiFailure.SessionChanged)
        if (!mark() || store.write(NativeCredentialRecord.Refreshing(previous)) != null) {
            cleanupRequired()
            return NativeApiResult.Refused(NativeApiFailure.UnknownOutcome)
        }
        val result = api.refresh(previous.refreshToken, previous.view.session)
        if (!current(issued)) {
            // Cleanup is exact to A, even if the late response arrives after B was established.
            api.revokeFamily(previous.refreshToken, previous.view.session)
            return NativeApiResult.Refused(NativeApiFailure.SessionChanged)
        }
        if (result is NativeApiResult.Accepted) {
            val admitted = result.value.admit(now(), previous)
            if (admitted != null && admitted.view.expiresAt > now() && persistAndEstablish(admitted, issued)) {
                val lease = mutableVerifiedSession.value
                if (lease != null) return NativeApiResult.Accepted(lease)
            }
        }
        // This path also covers unknown commit and failed atomic replacement. Never retry refresh.
        cleanupRequired()
        val cleaned = cleanup(issued)
        if (!cleaned && current(issued)) api.revokeFamily(previous.refreshToken, previous.view.session)
        if (cleaned && current(issued)) fail(if (result is NativeApiResult.Accepted) NativeAccountFailure.Protocol else NativeAccountFailure.Expired)
        return NativeApiResult.Refused(if (result is NativeApiResult.Refused) result.reason else NativeApiFailure.Protocol)
    }

    /** Quarantine rejected authority synchronously. Reconfirmation may only re-admit this family. */
    fun revalidate(expectedSession: String) {
        val lease = mutableVerifiedSession.value ?: return
        val candidate = grant ?: return
        if (lease.session.session != expectedSession) return
        val issued = generation
        val pendingRotation = flight
        if (admissionEpoch == Long.MAX_VALUE) { advance(); return }
        admissionEpoch += 1
        mutableVerifiedSession.value = null
        mutableState.value = NativeAccountState.Pending
        expiry?.cancel(); expiry = null
        operation = scope.launch {
            if (pendingRotation != null) {
                // That exact in-flight refresh is already an API confirmation. Never spend again.
                val result = pendingRotation.await()
                val active = grant
                if (result is NativeApiResult.Accepted && current(issued) && active != null &&
                    active.view.actor == candidate.view.actor && active.view.session == expectedSession &&
                    mutableVerifiedSession.value == null) establish(active, issued)
                return@launch
            }
            if (!current(issued) || grant !== candidate) return@launch
            if (needsRotation(candidate)) {
                rotate(candidate, issued)
                return@launch
            }
            when (val response = api.session(candidate.accessToken)) {
                is NativeApiResult.Refused -> if (current(issued) && grant === candidate) {
                    val next = advance() ?: return@launch
                    cleanupRequired()
                    if (mark()) operation = scope.launch { if (cleanup(next) && current(next)) fail(NativeAccountFailure.Expired) }
                }
                is NativeApiResult.Accepted -> if (current(issued) && grant === candidate) {
                    if (!matches(candidate, response.value)) logout()
                    else establish(candidate, issued)
                }
            }
        }
    }

    fun refreshSaved() {
        val expected = mutableVerifiedSession.value ?: return
        if (savedGeneration == Long.MAX_VALUE) { fail(NativeAccountFailure.Exhausted); return }
        savedGeneration += 1
        val requested = savedGeneration
        scope.launch {
            when (val result = authorization(expected.session.session)) {
                is NativeApiResult.Refused -> Unit
                is NativeApiResult.Accepted -> when (val saved = api.saved(result.value.bearer, result.value.session.session)) {
                    is NativeApiResult.Refused -> if (accepts(result.value) && requested == savedGeneration &&
                        saved.reason in setOf(NativeApiFailure.Unauthorized, NativeApiFailure.SessionChanged)) revalidate(result.value.session.session)
                    is NativeApiResult.Accepted -> if (accepts(result.value) && requested == savedGeneration) {
                        val signed = mutableState.value as? NativeAccountState.SignedIn
                        if (signed != null) mutableState.value = signed.copy(savedCount = saved.value)
                    }
                }
            }
        }
    }

    fun logout() {
        val issued = advance() ?: return
        cleanupRequired()
        if (!mark()) return
        operation = scope.launch { if (cleanup(issued) && current(issued)) mutableState.value = NativeAccountState.SignedOut }
    }
    fun cancelLogin() = logout()
    fun close() {
        if (!closed) { advance(); closed = true }
    }
}
