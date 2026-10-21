package com.cantos.theatre.application.identity

import com.cantos.theatre.domain.identity.RegistrationAction
import com.cantos.theatre.domain.identity.RegistrationCommand
import com.cantos.theatre.domain.identity.RegistrationEmail
import com.cantos.theatre.domain.identity.RegistrationFailure
import com.cantos.theatre.domain.identity.RegistrationPhase
import com.cantos.theatre.domain.identity.RegistrationState
import com.cantos.theatre.domain.identity.allows
import com.cantos.theatre.domain.identity.suspendRegistration
import com.cantos.theatre.ports.RegistrationBrowser
import com.cantos.theatre.ports.RegistrationCapability
import com.cantos.theatre.ports.RegistrationPort
import com.cantos.theatre.domain.identity.RegistrationResult
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

/** Route-scoped RAM state; confined to the host dispatcher. No credentials or account authority. */
class RegistrationController private constructor(
    private val scope: CoroutineScope,
    private val api: RegistrationPort?,
    private val browser: RegistrationBrowser?,
    private val nativeApi: com.cantos.theatre.ports.NativeIdentityV1Api?,
) {
    constructor(scope: CoroutineScope, api: RegistrationPort, browser: RegistrationBrowser) : this(scope, api, browser, null)
    constructor(scope: CoroutineScope, api: com.cantos.theatre.ports.NativeIdentityV1Api) : this(scope, null, null, api)
    private fun initial() = RegistrationState(firstParty = nativeApi != null)
    private val mutableState = MutableStateFlow(initial())
    val state = mutableState.asStateFlow()
    private var actionOrigin: String? = null
    private var request: Job? = null
    private var epoch = 0L
    private var closed = false

    private fun phase(next: RegistrationPhase) { mutableState.value = RegistrationState(state.value.email, next, nativeApi != null) }
    private fun failed(reason: RegistrationFailure) = phase(RegistrationPhase.Failed(reason))

    fun enter() {
        if (closed || state.value.phase != RegistrationPhase.Idle) return
        if (nativeApi != null) { phase(RegistrationPhase.Editing); return }
        phase(RegistrationPhase.Checking)
        val captured = epoch
        request = scope.launch {
            val result = api?.configuration() ?: return@launch
            if (closed || captured != epoch) return@launch
            when (result) {
                is RegistrationResult.Accepted -> when (val capability = result.value) {
                    is RegistrationCapability.Available -> { actionOrigin = capability.actionOrigin; phase(RegistrationPhase.Editing) }
                    RegistrationCapability.Disabled -> failed(RegistrationFailure.Disabled)
                }
                is RegistrationResult.Refused -> failed(result.reason)
            }
        }
    }

    fun editEmail(value: String) {
        if (closed || state.value.phase != RegistrationPhase.Editing &&
            state.value.phase != RegistrationPhase.Failed(RegistrationFailure.InvalidEmail)) return
        mutableState.value = RegistrationState(value.take(254), RegistrationPhase.Editing, nativeApi != null)
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
            val result = if (nativeApi != null) when (val started = nativeApi.signup(email)) {
                is com.cantos.theatre.ports.IdentityV1Result.Accepted -> RegistrationResult.Accepted(Unit)
                is com.cantos.theatre.ports.IdentityV1Result.Refused -> RegistrationResult.Refused(when (started.reason) {
                    com.cantos.theatre.ports.IdentityV1Failure.NotConfigured -> RegistrationFailure.Disabled
                    com.cantos.theatre.ports.IdentityV1Failure.Protocol -> RegistrationFailure.Protocol
                    com.cantos.theatre.ports.IdentityV1Failure.Unauthorized, com.cantos.theatre.ports.IdentityV1Failure.Denied -> RegistrationFailure.Denied
                    com.cantos.theatre.ports.IdentityV1Failure.SessionChanged,
                    is com.cantos.theatre.ports.IdentityV1Failure.RateLimited,
                    com.cantos.theatre.ports.IdentityV1Failure.ServiceUnavailable,
                    com.cantos.theatre.ports.IdentityV1Failure.Transport,
                    com.cantos.theatre.ports.IdentityV1Failure.UnknownOutcome -> RegistrationFailure.StartUnknownOutcome
                })
            } else api?.start(email) ?: return@launch
            if (closed || captured != epoch) return@launch
            when (result) {
                is RegistrationResult.Accepted -> phase(RegistrationPhase.Waiting)
                is RegistrationResult.Refused -> failed(if (result.reason == RegistrationFailure.UnknownOutcome)
                    RegistrationFailure.StartUnknownOutcome else result.reason)
            }
        }
    }

    fun importAction(link: String) {
        if (closed || nativeApi != null || state.value.phase !in listOf(RegistrationPhase.Waiting,
                RegistrationPhase.Failed(RegistrationFailure.InvalidAction),
                RegistrationPhase.Failed(RegistrationFailure.StartUnknownOutcome))) return
        val origin = actionOrigin ?: return
        when (val checked = RegistrationAction.parse(link, origin)) {
            is RegistrationResult.Accepted -> phase(RegistrationPhase.Confirmable(checked.value))
            is RegistrationResult.Refused -> failed(checked.reason)
        }
    }

    fun confirm() {
        if (closed || !state.value.allows(RegistrationCommand.Confirm)) return
        val pending = state.value.phase as? RegistrationPhase.Confirmable ?: return
        phase(RegistrationPhase.Verifying(pending.action))
        val captured = epoch
        request = scope.launch {
            val result = api?.confirm(pending.action) ?: return@launch
            if (closed || captured != epoch) return@launch
            when (result) {
                is RegistrationResult.Accepted -> phase(RegistrationPhase.SetupReady(result.value))
                is RegistrationResult.Refused -> failed(result.reason)
            }
        }
    }

    fun cancel() {
        if (closed) return
        val pending = state.value.phase as? RegistrationPhase.Confirmable
        val previous = state.value.phase
        leave()
        if (pending == null) {
            if (previous != RegistrationPhase.Submitting && previous !is RegistrationPhase.Verifying &&
                previous != RegistrationPhase.Cancelling && previous !is RegistrationPhase.SetupReady) phase(RegistrationPhase.Cancelled)
            return
        }
        phase(RegistrationPhase.Cancelling)
        val captured = epoch
        request = scope.launch {
            val result = api?.cancel(pending.action) ?: return@launch
            if (closed || captured != epoch) return@launch
            when (result) {
                is RegistrationResult.Accepted -> phase(RegistrationPhase.Cancelled)
                is RegistrationResult.Refused -> failed(result.reason)
            }
        }
    }

    fun openSetup() {
        if (closed || !state.value.allows(RegistrationCommand.OpenSetup)) return
        val ready = state.value.phase as? RegistrationPhase.SetupReady ?: return
        // Consume before platform I/O. A failed/lost open cannot make the secret replayable.
        phase(RegistrationPhase.CredentialSetup)
        val captured = epoch
        browser?.openSetup(ready.setup.url) { opened ->
            scope.launch { if (!closed && captured == epoch && !opened) failed(RegistrationFailure.Browser) }
        }
    }

    /** Explicit new attempt; never an automatic resend or replay of a consumed action. */
    fun restart() {
        if (closed || state.value.phase == RegistrationPhase.Checking || state.value.phase == RegistrationPhase.Submitting ||
            state.value.phase is RegistrationPhase.Verifying || state.value.phase == RegistrationPhase.Cancelling) return
        leave()
        phase(RegistrationPhase.Idle)
        enter()
    }

    /** Cleanup forgets capabilities only. It cannot revoke an already dispatched provider effect. */
    fun leave() {
        if (closed) return
        epoch += 1
        request?.cancel()
        request = null
        mutableState.value = suspendRegistration(state.value)
    }

    fun close() { leave(); closed = true; mutableState.value = initial(); actionOrigin = null }

    fun reset() { if (!closed) { leave(); mutableState.value = initial(); actionOrigin = null } }
}
