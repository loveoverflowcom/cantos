package com.cantos.theatre.application.identity

import com.cantos.theatre.domain.identity.CompletionFailure
import com.cantos.theatre.domain.identity.CompletionResult
import com.cantos.theatre.domain.identity.EmailVerificationState
import com.cantos.theatre.domain.identity.IssuedCompletion
import com.cantos.theatre.domain.identity.VerificationLink
import com.cantos.theatre.ports.IdentityV1Failure
import com.cantos.theatre.ports.IdentityV1Result
import com.cantos.theatre.ports.SignupCompletionApi
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

/** What an incoming URL meant for this owner. */
enum class LinkReceipt {
    /** Not addressed to the pinned handler: not this app's link, and nothing changed. */
    NotClaimed,
    /** Captured, or refused as unreadable; the completion screen should show it. */
    Shown,
    /** Another completion is in flight; this link was dropped. */
    Busy,
}

/**
 * Process-scoped RAM owner for completing an emailed signup in the app; confined to the host
 * dispatcher. It borrows no account owner and never reads, sends or changes a session: the request
 * is anonymous, and a listener who is already signed in stays signed in as that account.
 */
class EmailVerificationController(
    private val scope: CoroutineScope,
    private val api: SignupCompletionApi,
    private val handler: String,
    private val nowMs: () -> Long,
) {
    private val mutableState = MutableStateFlow(EmailVerificationState.Initial)
    val state: StateFlow<EmailVerificationState> = mutableState.asStateFlow()
    private var request: Job? = null

    /** The URL is read once here and never kept beyond the parsed link. */
    fun receive(url: String): LinkReceipt {
        if (!VerificationLink.claims(url, handler)) return LinkReceipt.NotClaimed
        val before = state.value
        val link = VerificationLink.parse(url, handler)
        val next = if (link != null) before.capture(link) else before.linkRefused()
        if (next === before) return LinkReceipt.Busy
        request?.cancel()
        request = null
        mutableState.value = next
        return LinkReceipt.Shown
    }

    fun editPassword(value: String) { mutableState.value = state.value.editPassword(value) }
    fun editRepeat(value: String) { mutableState.value = state.value.editRepeat(value) }
    fun confirm(value: Boolean) { mutableState.value = state.value.confirm(value) }
    fun editAfterRefusal() { mutableState.value = state.value.editAfterRefusal() }

    /** Whole seconds until the retained request may be sent again, while a 429 holds it. */
    fun retryWaitSeconds(): Long? = state.value.retryWaitSeconds(nowMs())

    fun complete() { state.value.beginComplete()?.let(::send) }

    fun retry() { state.value.beginRetry(nowMs())?.let(::send) }

    private fun send(begun: Pair<EmailVerificationState, IssuedCompletion>) {
        val (next, issued) = begun
        mutableState.value = next
        request = scope.launch {
            val result = when (val answer = api.completeSignup(issued.request)) {
                is IdentityV1Result.Accepted -> CompletionResult.Done(answer.value)
                is IdentityV1Result.Refused -> CompletionResult.Failed(answer.reason.toCompletionFailure())
            }
            mutableState.value = state.value.finish(issued.issued, result, nowMs())
        }
    }

    /**
     * Leaving the screen forgets the link and the password and fences a pending answer. It cannot
     * undo an account the API may already have created; the listener can sign in with it later.
     */
    fun leave() {
        request?.cancel()
        request = null
        mutableState.value = state.value.clear()
    }
}

private fun IdentityV1Failure.toCompletionFailure(): CompletionFailure = when (this) {
    // The client maps 400/413/415/422 here: the request was refused before any effect.
    IdentityV1Failure.Protocol -> CompletionFailure.Invalid
    IdentityV1Failure.Unauthorized -> CompletionFailure.Refused
    IdentityV1Failure.Denied -> CompletionFailure.Forbidden
    IdentityV1Failure.NotConfigured -> CompletionFailure.NotConfigured
    is IdentityV1Failure.RateLimited -> CompletionFailure.RateLimited(retryAfterSeconds)
    IdentityV1Failure.ServiceUnavailable -> CompletionFailure.Unavailable
    // An anonymous request has no session to fence; a 409 is as unexplained as a lost answer.
    IdentityV1Failure.SessionChanged, IdentityV1Failure.Transport,
    IdentityV1Failure.UnknownOutcome -> CompletionFailure.UnknownOutcome
}
