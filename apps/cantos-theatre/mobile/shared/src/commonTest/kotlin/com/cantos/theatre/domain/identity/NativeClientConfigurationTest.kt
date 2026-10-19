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

    @Test
    fun profiles_are_a_closed_exact_set_of_callbacks() {
        assertEquals("com.cantos.theatre:/oauth2/callback", NativeCallbackProfile.Production.redirect)
        assertEquals("com.cantos.theatre.dev:/oauth2/callback", NativeCallbackProfile.Development.redirect)
        assertEquals(NativeCallbackProfile.Development, NativeCallbackProfile.forApplicationId("com.cantos.theatre.dev"))
        for (rejected in listOf(
            "com.cantos.theatre.demo", "com.cantos.studio", "com.cantos.theatre.dev.evil", "com.cantos.", "com.cantos.theatre.", "",
            "COM.CANTOS.THEATRE",
        )) {
            assertEquals(null, NativeCallbackProfile.forApplicationId(rejected), rejected)
        }
        for (rejected in listOf(
            "com.cantos.theatre.dev:/oauth2/callback/", "com.cantos.theatre.dev:/oauth2/callback?x=1", "com.cantos.theatre.dev://oauth2/callback",
            "com.cantos.theatre.dev:/oauth2/other", "com.cantos.studio:/oauth2/callback", "https://evil.example/oauth2/callback", "",
        )) {
            assertEquals(null, NativeConfiguration.create(issuer, rejected), rejected)
        }
        assertEquals(NativeCallbackProfile.Development, NativeConfiguration.create(issuer, NativeCallbackProfile.Development.redirect)?.profile)
    }

    @Test
    fun application_id_selects_its_own_callback_and_unknown_ids_never_fall_back_to_production() {
        val dev = assertIs<NativeIdentityAvailability.Available>(
            NativeClientConfiguration.resolveForApplication(api, issuer, "com.cantos.theatre.dev"),
        )
        assertEquals("com.cantos.theatre.dev:/oauth2/callback", dev.configuration.identity.redirect)
        val release = assertIs<NativeIdentityAvailability.Available>(
            NativeClientConfiguration.resolveForApplication(api, issuer, "com.cantos.theatre"),
        )
        assertEquals("com.cantos.theatre:/oauth2/callback", release.configuration.identity.redirect)
        assertEquals(
            NativeIdentityAvailability.Invalid(NativeConfigurationField.Callback),
            NativeClientConfiguration.resolveForApplication(api, issuer, "com.cantos.theatre.demo"),
        )
        // A missing issuer only disables identity; it never reports the callback or blocks guest catalog.
        assertEquals(
            NativeIdentityAvailability.Missing(NativeConfigurationFields.Issuer),
            NativeClientConfiguration.resolveForApplication(api, null, "com.cantos.theatre.demo"),
        )
        assertEquals(
            NativeIdentityAvailability.Invalid(NativeConfigurationField.Issuer),
            NativeClientConfiguration.resolveForApplication(api, "$issuer/", "com.cantos.theatre.dev"),
        )
    }
}
