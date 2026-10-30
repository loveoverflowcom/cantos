package com.cantos.theatre.application.identity

/** Display-only failure facts; feature UI never depends on transport ports. */
sealed interface AccountLoginFailure {
    data object InvalidCredentials : AccountLoginFailure
    data object RetryLater : AccountLoginFailure
    data object AccessDenied : AccountLoginFailure
    data object SessionChanged : AccountLoginFailure
    data object UnknownOutcome : AccountLoginFailure
    data object Protocol : AccountLoginFailure
    data class RateLimited(val retryAfterSeconds: Long?) : AccountLoginFailure
}

private fun com.cantos.theatre.ports.IdentityV1Failure.toAccountFailure(credentialAttempt: Boolean): AccountLoginFailure = when (this) {
    com.cantos.theatre.ports.IdentityV1Failure.Unauthorized, com.cantos.theatre.ports.IdentityV1Failure.Denied -> if (credentialAttempt) AccountLoginFailure.InvalidCredentials else AccountLoginFailure.AccessDenied
    com.cantos.theatre.ports.IdentityV1Failure.Protocol -> AccountLoginFailure.Protocol
    is com.cantos.theatre.ports.IdentityV1Failure.RateLimited -> AccountLoginFailure.RateLimited(retryAfterSeconds)
    com.cantos.theatre.ports.IdentityV1Failure.NotConfigured, com.cantos.theatre.ports.IdentityV1Failure.ServiceUnavailable,
    com.cantos.theatre.ports.IdentityV1Failure.Transport -> AccountLoginFailure.RetryLater
    com.cantos.theatre.ports.IdentityV1Failure.UnknownOutcome -> AccountLoginFailure.UnknownOutcome
    com.cantos.theatre.ports.IdentityV1Failure.SessionChanged -> AccountLoginFailure.SessionChanged
}

/**
 * Immutable presentation of the account boundary. It carries no actor, session, credential,
 * expiry or page count. Only Server-projected pseudonyms may supply the displayed name.
 */
sealed interface AccountView {
    /** No identity boundary is configured. The guest shell stays complete; this is not a failure. */
    data object Unavailable : AccountView
    data object SignedOut : AccountView
    data object Pending : AccountView
    data class SignedIn(val presentation: com.cantos.theatre.domain.identity.AccountPresentation? = null) : AccountView
    /** An earlier credential is not yet acknowledged as removed; login stays blocked. */
    data object CleanupRequired : AccountView
    data class Failed(val reason: NativeAccountFailure, val identityFailure: AccountLoginFailure? = null,
        val sessionLoss: ConfirmedSessionLoss? = null) : AccountView

    /** Only a resting guest state may start a login; the owner still refuses anything else. */
    val canBeginLogin: Boolean get() = this == SignedOut || this is Failed
}

fun NativeAccountState?.toAccountView(): AccountView = when (this) {
    null -> AccountView.Unavailable
    NativeAccountState.SignedOut -> AccountView.SignedOut
    NativeAccountState.Pending -> AccountView.Pending
    is NativeAccountState.SignedIn -> AccountView.SignedIn(presentation)
    NativeAccountState.CleanupRequired -> AccountView.CleanupRequired
    is NativeAccountState.Failed -> AccountView.Failed(reason, identityFailure?.toAccountFailure(credentialAttempt), sessionLoss)
}
