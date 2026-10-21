package com.cantos.theatre.application.identity

/**
 * Immutable presentation of the account boundary. It carries no actor, session, credential,
 * expiry or page count: nothing here can be shown as a name or a library total.
 */
enum class IdentityConfigurationFault { ApiOriginMissing, IssuerMissing, BothMissing, ApiOriginInvalid, IssuerInvalid, CallbackInvalid }

fun com.cantos.theatre.domain.identity.NativeIdentityAvailability.diagnostic(): IdentityConfigurationFault? = when (this) {
    is com.cantos.theatre.domain.identity.NativeIdentityAvailability.Available -> null
    is com.cantos.theatre.domain.identity.NativeIdentityAvailability.Missing -> when (fields) {
        com.cantos.theatre.domain.identity.NativeConfigurationFields.ApiOrigin -> IdentityConfigurationFault.ApiOriginMissing
        com.cantos.theatre.domain.identity.NativeConfigurationFields.Issuer -> IdentityConfigurationFault.IssuerMissing
        com.cantos.theatre.domain.identity.NativeConfigurationFields.Both -> IdentityConfigurationFault.BothMissing
    }
    is com.cantos.theatre.domain.identity.NativeIdentityAvailability.Invalid -> when (field) {
        com.cantos.theatre.domain.identity.NativeConfigurationField.ApiOrigin -> IdentityConfigurationFault.ApiOriginInvalid
        com.cantos.theatre.domain.identity.NativeConfigurationField.Issuer -> IdentityConfigurationFault.IssuerInvalid
        com.cantos.theatre.domain.identity.NativeConfigurationField.Callback -> IdentityConfigurationFault.CallbackInvalid
    }
}

/** Display-only failure facts; feature UI never depends on transport ports. */
sealed interface AccountLoginFailure {
    data object InvalidCredentials : AccountLoginFailure
    data object RetryLater : AccountLoginFailure
    data object Protocol : AccountLoginFailure
    data class RateLimited(val retryAfterSeconds: Long?) : AccountLoginFailure
}

private fun com.cantos.theatre.ports.IdentityV1Failure.toAccountFailure(): AccountLoginFailure = when (this) {
    com.cantos.theatre.ports.IdentityV1Failure.Unauthorized, com.cantos.theatre.ports.IdentityV1Failure.Denied -> AccountLoginFailure.InvalidCredentials
    com.cantos.theatre.ports.IdentityV1Failure.Protocol -> AccountLoginFailure.Protocol
    is com.cantos.theatre.ports.IdentityV1Failure.RateLimited -> AccountLoginFailure.RateLimited(retryAfterSeconds)
    com.cantos.theatre.ports.IdentityV1Failure.NotConfigured, com.cantos.theatre.ports.IdentityV1Failure.ServiceUnavailable,
    com.cantos.theatre.ports.IdentityV1Failure.Transport, com.cantos.theatre.ports.IdentityV1Failure.UnknownOutcome,
    com.cantos.theatre.ports.IdentityV1Failure.SessionChanged -> AccountLoginFailure.RetryLater
}

sealed interface AccountView {
    data class ConfigurationUnavailable(val reason: IdentityConfigurationFault) : AccountView
    /** No identity boundary is configured. The guest shell stays complete; this is not a failure. */
    data object Unavailable : AccountView
    data object SignedOut : AccountView
    data object Pending : AccountView
    data object SignedIn : AccountView
    /** An earlier credential is not yet acknowledged as removed; login stays blocked. */
    data object CleanupRequired : AccountView
    data class Failed(val reason: NativeAccountFailure, val identityFailure: AccountLoginFailure? = null) : AccountView

    /** Only a resting guest state may start a login; the owner still refuses anything else. */
    val canBeginLogin: Boolean get() = this == SignedOut || this is Failed
}

fun NativeAccountState?.toAccountView(): AccountView = when (this) {
    null -> AccountView.Unavailable
    NativeAccountState.SignedOut -> AccountView.SignedOut
    NativeAccountState.Pending -> AccountView.Pending
    is NativeAccountState.SignedIn -> AccountView.SignedIn
    NativeAccountState.CleanupRequired -> AccountView.CleanupRequired
    is NativeAccountState.Failed -> AccountView.Failed(reason, identityFailure?.toAccountFailure())
}

/** First-party hosts diagnose only their actual required input; no synthetic issuer/callback. */
fun com.cantos.theatre.domain.identity.NativeIdentityV1ConfigurationResult.diagnostic(): IdentityConfigurationFault? = when (this) {
    is com.cantos.theatre.domain.identity.NativeIdentityV1ConfigurationResult.Available -> null
    com.cantos.theatre.domain.identity.NativeIdentityV1ConfigurationResult.MissingApiOrigin -> IdentityConfigurationFault.ApiOriginMissing
    com.cantos.theatre.domain.identity.NativeIdentityV1ConfigurationResult.InvalidApiOrigin -> IdentityConfigurationFault.ApiOriginInvalid
}
