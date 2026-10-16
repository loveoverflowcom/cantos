package com.cantos.theatre.domain.identity

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs
import kotlin.test.assertFalse

class NativeClientConfigurationTest {
    private val api = "https://api.example:8443"
    private val issuer = "https://idm.example/oauth2/openid/cantos-theatre-native"

    @Test
    fun missing_build_inputs_report_the_exact_missing_fields_and_never_activate_identity() {
        for (empty in listOf(null, "")) {
            assertEquals(
                NativeIdentityAvailability.Missing(NativeConfigurationFields.Both),
                NativeClientConfiguration.resolve(empty, empty),
            )
            assertEquals(
                NativeIdentityAvailability.Missing(NativeConfigurationFields.ApiOrigin),
                NativeClientConfiguration.resolve(empty, issuer),
            )
            assertEquals(
                NativeIdentityAvailability.Missing(NativeConfigurationFields.Issuer),
                NativeClientConfiguration.resolve(api, empty),
            )
        }
    }

    @Test
    fun supplied_but_invalid_inputs_are_distinct_from_absent_configuration() {
        for (invalid in listOf(
            " ", "\$(CANTOS_API_ORIGIN)", "http://api.example", "https://api.example/",
            "https://api.example:0", "https://api.example:65536", "https://api..example",
            "https://user:password@api.example", "https://api.example?token=sentinel-secret",
        )) {
            assertEquals(
                NativeIdentityAvailability.Invalid(NativeConfigurationField.ApiOrigin),
                NativeClientConfiguration.resolve(invalid, issuer),
            )
        }
        for (invalid in listOf(
            " ", "\$(CANTOS_NATIVE_ISSUER)", "http://idm.example/oauth2/openid/cantos-theatre-native",
            "https://idm.example/oauth2/openid/cantos-studio-web-bff", "$issuer/",
        )) {
            assertEquals(
                NativeIdentityAvailability.Invalid(NativeConfigurationField.Issuer),
                NativeClientConfiguration.resolve(api, invalid),
            )
        }
    }

    @Test
    fun valid_configuration_preserves_exact_https_origins_and_the_fixed_public_client() {
        val result = assertIs<NativeIdentityAvailability.Available>(
            NativeClientConfiguration.resolve(api, issuer),
        )
        assertEquals(api, result.configuration.apiOrigin)
        assertEquals(issuer, result.configuration.identity.issuer)
        assertEquals("cantos-theatre-native", result.configuration.identity.client)
        assertEquals("com.cantos.theatre:/oauth2/callback", result.configuration.identity.redirect)
    }

    @Test
    fun configuration_diagnostics_never_copy_untrusted_values() {
        val secret = "sentinel-secret"
        for ((rawApi, rawIssuer) in listOf(
            "https://user:$secret@api.example" to issuer,
            api to "$issuer?credential=$secret",
        )) {
            val result = NativeClientConfiguration.resolve(rawApi, rawIssuer)
            assertIs<NativeIdentityAvailability.Invalid>(result)
            assertFalse(result.toString().contains(secret))
        }
    }
}
