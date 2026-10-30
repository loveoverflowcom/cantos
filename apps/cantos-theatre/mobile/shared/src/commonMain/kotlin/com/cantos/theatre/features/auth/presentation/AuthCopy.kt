package com.cantos.theatre.features.auth.presentation

import com.cantos.theatre.application.identity.AccountLoginFailure
import com.cantos.theatre.application.identity.NativeAccountFailure
import com.cantos.theatre.domain.identity.RegistrationFailure
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.theatre.presentation.model.generated.AuthKey

internal class AuthCopy(private val locale: ShellLocale) {
    fun text(key: AuthText): String = key.text(locale)

    fun registrationFailure(reason: RegistrationFailure): String = text(when (reason) {
        RegistrationFailure.InvalidEmail -> AuthText.RegisterInvalidEmail
        RegistrationFailure.Disabled -> AuthText.RegisterDisabled
        RegistrationFailure.Denied -> AuthText.RegisterDenied
        RegistrationFailure.Protocol -> AuthText.FailureProtocol
        RegistrationFailure.RateLimited -> AuthText.RegisterRateLimited
        RegistrationFailure.StartUnknownOutcome -> AuthText.RegisterSendUnknown
    })

    fun failure(reason: NativeAccountFailure, identity: AccountLoginFailure? = null): String {
        if (identity is AccountLoginFailure.RateLimited) {
            val seconds = identity.retryAfterSeconds
            return if (seconds == null) text(AuthText.RetryLater) else legacyRetryAfter(locale, seconds)
        }
        if (identity != null) return text(when (identity) {
            AccountLoginFailure.InvalidCredentials -> AuthText.InvalidCredentials
            AccountLoginFailure.AccessDenied -> AuthText.AccessDenied
            AccountLoginFailure.SessionChanged -> AuthText.SessionChanged
            AccountLoginFailure.UnknownOutcome -> AuthText.UnknownOutcome
            AccountLoginFailure.Protocol -> AuthText.FailureProtocol
            AccountLoginFailure.RetryLater, is AccountLoginFailure.RateLimited -> AuthText.RetryLater
        })
        return text(when (reason) {
            NativeAccountFailure.Storage -> AuthText.FailureStorage
            NativeAccountFailure.Expired -> AuthText.FailureExpired
            NativeAccountFailure.Api -> AuthText.FailureApi
            NativeAccountFailure.Protocol -> AuthText.FailureProtocol
            NativeAccountFailure.Exhausted -> AuthText.FailureExhausted
        })
    }
}

/**
 * Listener-facing resources for the optional account flow. The brand is only **Cantos**.
 * Nothing here is a name, count or entitlement: those need a real contract before display.
 */
internal typealias AuthText = AuthKey
