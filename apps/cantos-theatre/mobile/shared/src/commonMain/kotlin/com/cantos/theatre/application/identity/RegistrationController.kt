package com.cantos.theatre.application.identity

import com.cantos.theatre.domain.identity.RegistrationCommand
import com.cantos.theatre.domain.identity.RegistrationEmail
import com.cantos.theatre.domain.identity.RegistrationFailure
import com.cantos.theatre.domain.identity.RegistrationPhase
import com.cantos.theatre.domain.identity.RegistrationResult
import com.cantos.theatre.domain.identity.RegistrationState
import com.cantos.theatre.domain.identity.allows
import com.cantos.theatre.domain.identity.suspendRegistration
import com.cantos.theatre.ports.IdentityV1Failure
import com.cantos.theatre.ports.IdentityV1Result
import com.cantos.theatre.ports.NativeIdentityV1Api
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

/**
 * Route-scoped RAM state for the email-only signup initiation; confined to the host dispatcher.
 * It holds no credential, proof or account authority: the mailbox proof and the password are
 * completed on Web, and the listener then signs in explicitly.
 */
class RegistrationController(
    private val scope: CoroutineScope,
    private val api: NativeIdentityV1Api,
) {
    private val mutableState = MutableStateFlow(RegistrationState())
    val state = mutableState.asStateFlow()
    private var request: Job? = null
    private var epoch = 0L
    private var closed = false

    private fun phase(next: RegistrationPhase) { mutableState.value = RegistrationState(state.value.email, next) }
    private fun failed(reason: RegistrationFailure) = phase(RegistrationPhase.Failed(reason))

    fun enter() {
        if (closed || state.value.phase != RegistrationPhase.Idle) return
        phase(RegistrationPhase.Editing)
    }

    fun editEmail(value: String) {
        if (closed || state.value.phase != RegistrationPhase.Editing &&
            state.value.phase != RegistrationPhase.Failed(RegistrationFailure.InvalidEmail)) return
        mutableState.value = RegistrationState(value.take(254), RegistrationPhase.Editing)
    }

    fun submit() {
        if (closed || !state.value.allows(RegistrationCommand.Submit)) return
        val email = when (val checked = RegistrationEmail.parse(state.value.email)) {
            is RegistrationResult.Accepted -> checked.value
            is RegistrationResult.Refused -> { failed(checked.reason); return }
        }
        phase(RegistrationPhase.Submitting)
        val captured = epoch
        request = scope.launch {
            val result = api.signup(email)
            if (closed || captured != epoch) return@launch
            when (result) {
                is IdentityV1Result.Accepted -> phase(RegistrationPhase.Waiting)
                is IdentityV1Result.Refused -> failed(result.reason.toRegistrationFailure())
            }
        }
    }

    /** Stops waiting here; a mail already sent cannot be recalled and its link stays Web-owned. */
    fun cancel() {
        if (closed) return
        val previous = state.value.phase
        leave()
        if (previous != RegistrationPhase.Submitting) phase(RegistrationPhase.Cancelled)
    }

    /** Explicit new attempt; never an automatic resend or replay of an acknowledged initiation. */
    fun restart() {
        if (closed || state.value.phase == RegistrationPhase.Submitting) return
        leave()
        phase(RegistrationPhase.Idle)
        enter()
    }

    /** Cleanup forgets the pending request only. It cannot revoke an already dispatched initiation. */
    fun leave() {
        if (closed) return
        epoch += 1
        request?.cancel()
        request = null
        mutableState.value = suspendRegistration(state.value)
    }

    fun close() { leave(); closed = true; mutableState.value = RegistrationState() }

    fun reset() { if (!closed) { leave(); mutableState.value = RegistrationState() } }
}

private fun IdentityV1Failure.toRegistrationFailure(): RegistrationFailure = when (this) {
    IdentityV1Failure.NotConfigured -> RegistrationFailure.Disabled
    IdentityV1Failure.Protocol -> RegistrationFailure.Protocol
    IdentityV1Failure.Unauthorized, IdentityV1Failure.Denied -> RegistrationFailure.Denied
    is IdentityV1Failure.RateLimited -> RegistrationFailure.RateLimited
    // A 409 fence cannot occur for an anonymous signup; treat it, 503, timeouts and loss as unknown.
    IdentityV1Failure.SessionChanged, IdentityV1Failure.ServiceUnavailable,
    IdentityV1Failure.Transport, IdentityV1Failure.UnknownOutcome -> RegistrationFailure.StartUnknownOutcome
}
