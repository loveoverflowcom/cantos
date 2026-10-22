package com.cantos.theatre.features.auth.presentation

import com.cantos.theatre.application.identity.AccountView
import com.cantos.theatre.application.identity.AccountLoginFailure
import com.cantos.theatre.application.identity.NativeAccountFailure
import com.cantos.theatre.application.identity.NativeAccountState
import com.cantos.theatre.application.identity.toAccountView
import com.cantos.theatre.ports.IdentityV1Failure
import com.cantos.theatre.presentation.model.ShellLocale
import kotlin.test.Test
import kotlin.test.assertEquals

class IdentityV1AuthCopyTest {
    @Test fun failures_preserve_generic_credentials_and_retry_after_in_both_languages() {
        for (locale in ShellLocale.entries) {
            val copy = AuthCopy(locale)
            assertEquals(copy.text(AuthText.InvalidCredentials), copy.failure(NativeAccountFailure.Api, AccountLoginFailure.InvalidCredentials))
            assertEquals(copy.text(AuthText.RetryLater), copy.failure(NativeAccountFailure.Api, AccountLoginFailure.RetryLater))
            assertEquals(copy.text(AuthText.RetryAfter).replace("{seconds}", "17"), copy.failure(NativeAccountFailure.Api, AccountLoginFailure.RateLimited(17)))
            assertEquals(copy.text(AuthText.RetryLater), copy.failure(NativeAccountFailure.Api, AccountLoginFailure.RateLimited(null)))
        }
        assertEquals(AccountView.Failed(NativeAccountFailure.Api, AccountLoginFailure.RateLimited(17)),
            NativeAccountState.Failed(NativeAccountFailure.Api, IdentityV1Failure.RateLimited(17)).toAccountView())
    }
    @Test fun transport_failures_are_projected_to_display_facts_before_entering_features() {
        val expected = mapOf(
            IdentityV1Failure.Unauthorized to AccountLoginFailure.InvalidCredentials,
            IdentityV1Failure.Denied to AccountLoginFailure.InvalidCredentials,
            IdentityV1Failure.Protocol to AccountLoginFailure.Protocol,
            IdentityV1Failure.NotConfigured to AccountLoginFailure.RetryLater,
            IdentityV1Failure.ServiceUnavailable to AccountLoginFailure.RetryLater,
            IdentityV1Failure.Transport to AccountLoginFailure.RetryLater,
            IdentityV1Failure.UnknownOutcome to AccountLoginFailure.RetryLater,
            IdentityV1Failure.SessionChanged to AccountLoginFailure.RetryLater,
            IdentityV1Failure.RateLimited(17) to AccountLoginFailure.RateLimited(17),
        )
        expected.forEach { (failure, display) ->
            assertEquals(AccountView.Failed(NativeAccountFailure.Api, display), NativeAccountState.Failed(NativeAccountFailure.Api, failure).toAccountView())
        }
    }
    @Test fun login_command_diagnostics_never_include_either_form_value() {
        val email = (com.cantos.theatre.domain.identity.RegistrationEmail.parse("private@example.test") as com.cantos.theatre.domain.identity.RegistrationResult.Accepted).value
        assertEquals("AuthIntent.Login([redacted])", AuthIntent.Login(email, "private-password").toString())
    }
}
