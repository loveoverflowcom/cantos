package com.cantos.theatre.application.identity

import com.cantos.theatre.domain.identity.NativeGrant
import com.cantos.theatre.domain.identity.RegistrationEmail
import com.cantos.theatre.ports.NativeIdentityV1Api
import com.cantos.theatre.ports.IdentityV1Failure
import com.cantos.theatre.ports.IdentityV1Result
import com.cantos.theatre.ports.LogoutBarrier
import com.cantos.theatre.ports.LogoutBarrierRead
import com.cantos.theatre.ports.NativeApiFailure
import com.cantos.theatre.ports.NativeApiResult
import com.cantos.theatre.ports.NativeCredentialJournal
import com.cantos.theatre.ports.NativeCredentialRecord
import com.cantos.theatre.ports.NativeJournalRead
import com.cantos.theatre.ports.NativeSession
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.withContext
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlin.time.TimeSource

enum class NativeAccountFailure { Storage, Expired, Api, Protocol, Exhausted }
sealed interface NativeAccountState {
    data object SignedOut : NativeAccountState
    data object Pending : NativeAccountState
    data class SignedIn(val actor: String, val expiresAt: Long) : NativeAccountState
    data class Failed(val reason: NativeAccountFailure, val identityFailure: IdentityV1Failure? = null) : NativeAccountState
    data object CleanupRequired : NativeAccountState
}

/** API-confirmed authority, bound to one local intent and one rotation. Never saved in UI state. */
class NativeVerifiedSession internal constructor(val session: NativeSession, val bearer: String, internal val intent: Long = 0) {
    override fun toString(): String = "NativeVerifiedSession([redacted])"
}

/** One process/application-dispatcher owner. Secure journal precedes every consuming effect.
 * Network cancellation never proves noncommit: a Refreshing record is cleanup-only on restart. */
class NativeAccountController internal constructor(
    private val scope: CoroutineScope,
    private val api: NativeIdentityV1Api,
    private val store: NativeCredentialJournal,
    private val barrier: LogoutBarrier,
    timeSource: TimeSource,
    private val wallNow: () -> Long,
) {
    constructor(scope: CoroutineScope, api: NativeIdentityV1Api, store: NativeCredentialJournal,
        barrier: LogoutBarrier, now: () -> Long) : this(scope, api, store, barrier, TimeSource.Monotonic, now)

    private val clockOrigin = timeSource.markNow()
    private var observedElapsed = 0L
    private var observedTime = wallNow()

    // Keep server UNIX deadlines, but never extend their remaining duration when the device
    // clock moves backwards. This process-wide floor survives refresh/revalidation/new intents;
    // a restart still requires API admission. OS suspend/time-source behavior needs device evidence.
    private fun now(): Long {
        val elapsed = clockOrigin.elapsedNow().inWholeSeconds.coerceAtLeast(observedElapsed)
        val delta = elapsed - observedElapsed
        val advanced = if (observedTime > Long.MAX_VALUE - delta) Long.MAX_VALUE else observedTime + delta
        observedTime = maxOf(wallNow(), advanced)
        observedElapsed = elapsed
        return observedTime
    }

    private fun <T> native(result: IdentityV1Result<T>): NativeApiResult<T> = when (result) {
        is IdentityV1Result.Accepted -> NativeApiResult.Accepted(result.value)
        is IdentityV1Result.Refused -> NativeApiResult.Refused(when (result.reason) {
            IdentityV1Failure.Unauthorized -> NativeApiFailure.Unauthorized
            IdentityV1Failure.SessionChanged -> NativeApiFailure.SessionChanged
            IdentityV1Failure.Denied -> NativeApiFailure.Denied
            IdentityV1Failure.Protocol -> NativeApiFailure.Protocol
            IdentityV1Failure.UnknownOutcome -> NativeApiFailure.UnknownOutcome
            IdentityV1Failure.NotConfigured, is IdentityV1Failure.RateLimited, IdentityV1Failure.ServiceUnavailable,
            IdentityV1Failure.Transport -> NativeApiFailure.Unavailable
        }, result.reason)
    }

    private suspend fun session(value: NativeGrant): NativeApiResult<NativeSession> = native(api.session(value))
    private suspend fun refresh(value: NativeGrant): NativeApiResult<NativeGrant> = native(api.refresh(value))
    private suspend fun revokeFamily(value: NativeGrant): NativeApiResult<Unit> = native(api.revokeFamily(value))
    private val mutableState = MutableStateFlow<NativeAccountState>(NativeAccountState.SignedOut)
    val state: StateFlow<NativeAccountState> = mutableState.asStateFlow()
    private val mutableVerifiedSession = MutableStateFlow<NativeVerifiedSession?>(null)
    val verifiedSession: StateFlow<NativeVerifiedSession?> = mutableVerifiedSession.asStateFlow()
    private var generation = 0L
    // Separate from IO generation: revalidation breaks playback continuity even if StateFlow
    // conflates the intermediate null with an immediate successful session response.
    private var admissionEpoch = 0L
    private var operation: Job? = null
    private var expiry: Job? = null
    private var rotation: Job? = null
    private var flight: CompletableDeferred<NativeApiResult<NativeVerifiedSession>>? = null
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
        return generation
    }
    private fun fail(reason: NativeAccountFailure, identityFailure: IdentityV1Failure? = null) {
        hide()
        mutableState.value = NativeAccountState.Failed(reason, identityFailure)
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
            is NativeCredentialRecord.Active -> revokeFamily(record.grant)
            is NativeCredentialRecord.Refreshing -> revokeFamily(record.grant)
            // Records of a retired protocol have no Identity v1 binding. Never disclose them to the
            // API; remove locally and require login. Their original server authority expired
            // within the pre-migration 900-second bound.
            NativeCredentialRecord.Retired -> NativeApiResult.Accepted(Unit)
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

    /** Explicit single attempt: password exists only in this call and its in-flight transport. */
    fun login(email: RegistrationEmail, password: String) {
        val issued = advance() ?: return
        if (!mark()) return
        mutableState.value = NativeAccountState.Pending
        operation = scope.launch {
            if (!cleanup(issued) || !current(issued) || !mark()) return@launch
            when (val result = api.login(email, password)) {
                is IdentityV1Result.Refused -> if (current(issued)) {
                    // No pair means no family identity to revoke. Never invent one or replay a password.
                    if (cleanup(issued) && current(issued)) fail(NativeAccountFailure.Api, result.reason)
                }
                is IdentityV1Result.Accepted -> {
                    val value = result.value
                    if (!current(issued)) {
                        withContext(NonCancellable) { revokeFamily(value) }
                        return@launch
                    }
                    if (value.admit(now()) == null || value.expiresAt <= now()) {
                        retireKnown(value, issued, NativeAccountFailure.Protocol)
                        return@launch
                    }
                    admitLogin(value, issued)
                }
            }
        }
    }

    private suspend fun retireKnown(value: NativeGrant, issued: Long, reason: NativeAccountFailure) {
        if (!current(issued)) return
        cleanupRequired()
        // Readback may have failed after commit; the response still identifies this exact family.
        // Keep the denial barrier even if both the secure store and network are unavailable.
        val read = store.read()
        if (read is NativeJournalRead.Found) {
            // Preserve a known unadmitted family for offline cleanup when the active write failed.
            // A corrupt/foreign record is never overwritten to manufacture a recovery path.
            store.write(NativeCredentialRecord.Refreshing(value))
        }
        val result = revokeFamily(value)
        if (!current(issued)) return
        if (result is NativeApiResult.Accepted && cleanup(issued) && current(issued)) fail(reason)
        else if (current(issued)) cleanupRequired()
    }

    private suspend fun admitLogin(value: NativeGrant, issued: Long) {
        if (!current(issued)) return
        if (!mark() || store.write(NativeCredentialRecord.Active(value)) != null) {
            retireKnown(value, issued, NativeAccountFailure.Storage)
            return
        }
        val response = session(value)
        if (!current(issued)) return
        if (response is NativeApiResult.Refused && response.reason == NativeApiFailure.Unavailable) {
            // A complete read-back pair is safe to check again after an outage. It has not been
            // consumed by this GET, and remains a candidate with no published private authority.
            if (barrier.clear() != null) cleanupRequired()
            else fail(NativeAccountFailure.Api, response.identityFailure)
            return
        }
        if (response !is NativeApiResult.Accepted || !matches(value, response.value)) {
            retireKnown(value, issued, NativeAccountFailure.Protocol)
            return
        }
        if (value.admit(now()) == null || value.expiresAt <= now() || barrier.clear() != null) {
            retireKnown(value, issued, NativeAccountFailure.Expired)
            return
        }
        establish(value, issued)
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
        mutableState.value = NativeAccountState.SignedIn(admitted.view.actor, admitted.view.expiresAt)
        expiry?.cancel()
        // Active apps renew before expiry; the original absolute authority stays fixed.
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
        if (read is NativeJournalRead.Failed) {
            if (barrierState == LogoutBarrierRead.Pending) cleanupRequired() else fail(NativeAccountFailure.Storage)
            return
        }
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
            } else when (val result = session(candidate)) {
                is NativeApiResult.Refused -> if (current(issued)) {
                    if (result.reason == NativeApiFailure.Unauthorized || result.reason == NativeApiFailure.Protocol) {
                        if (cleanup(issued) && current(issued)) fail(NativeAccountFailure.Expired)
                    } else fail(NativeAccountFailure.Api, result.identityFailure)
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
        val result = refresh(previous)
        if (!current(issued)) {
            // Cleanup is exact to A, even if the late response arrives after B was established.
            withContext(NonCancellable) { revokeFamily(previous) }
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
        if (!cleaned && current(issued)) withContext(NonCancellable) { revokeFamily(previous) }
        if (cleaned && current(issued)) fail(if (result is NativeApiResult.Accepted) NativeAccountFailure.Protocol else NativeAccountFailure.Expired, (result as? NativeApiResult.Refused)?.identityFailure)
        return if (result is NativeApiResult.Refused) result else NativeApiResult.Refused(NativeApiFailure.Protocol)
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
            when (val response = session(candidate)) {
                is NativeApiResult.Refused -> if (current(issued) && grant === candidate) {
                    if (response.reason == NativeApiFailure.Unavailable || response.reason == NativeApiFailure.UnknownOutcome) {
                        fail(NativeAccountFailure.Api, response.identityFailure)
                    } else {
                        val next = advance() ?: return@launch
                        cleanupRequired()
                        if (mark()) operation = scope.launch { if (cleanup(next) && current(next)) fail(NativeAccountFailure.Expired) }
                    }
                }
                is NativeApiResult.Accepted -> if (current(issued) && grant === candidate) {
                    if (!matches(candidate, response.value)) logout()
                    else establish(candidate, issued)
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
