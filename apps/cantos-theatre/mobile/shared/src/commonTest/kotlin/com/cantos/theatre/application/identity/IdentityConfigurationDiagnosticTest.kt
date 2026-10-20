package com.cantos.theatre.application.identity

import com.cantos.theatre.domain.identity.NativeClientConfiguration
import com.cantos.theatre.domain.identity.NativeIdentityAvailability
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse

class IdentityConfigurationDiagnosticTest {
    @Test
    fun missing_and_invalid_identity_configuration_is_explained_without_minting_an_account() {
        val cases = listOf(
            NativeClientConfiguration.resolve(null, null) to IdentityConfigurationFault.BothMissing,
            NativeClientConfiguration.resolve("https://api.cantos.example", null) to IdentityConfigurationFault.IssuerMissing,
            NativeClientConfiguration.resolve("bad", "https://idm.cantos.example/oauth2/openid/cantos-theatre-native") to IdentityConfigurationFault.ApiOriginInvalid,
            NativeClientConfiguration.resolve("https://api.cantos.example", "bad") to IdentityConfigurationFault.IssuerInvalid,
        )
        for ((availability, expected) in cases) {
            assertFalse(availability is NativeIdentityAvailability.Available)
            assertEquals(expected, availability.diagnostic())
            assertFalse(AccountView.ConfigurationUnavailable(expected).canBeginLogin)
        }
        assertEquals(AccountView.Unavailable, (null as NativeAccountState?).toAccountView())
    }
}
