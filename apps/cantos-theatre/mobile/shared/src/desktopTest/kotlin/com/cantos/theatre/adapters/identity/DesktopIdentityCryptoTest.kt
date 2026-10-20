package com.cantos.theatre.adapters.identity

import com.cantos.theatre.domain.identity.LoginSecret
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertTrue

class DesktopIdentityCryptoTest {
    @Test fun real_desktop_s256_matches_the_independent_rfc7636_appendix_b_vector() {
        // https://www.rfc-editor.org/rfc/rfc7636#appendix-B
        val verifier = assertNotNull(LoginSecret.parse("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"))
        assertEquals("E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM", DesktopIdentityCrypto().challenge(verifier))
    }

    @Test fun desktop_entropy_returns_a_valid_32_byte_url_safe_secret_without_printing_it() {
        val secret = assertNotNull(DesktopIdentityCrypto().secret())
        assertEquals(43, secret.value.length)
        assertTrue(secret.value.all { it.isLetterOrDigit() || it in "-_" })
        assertEquals("LoginSecret([redacted])", secret.toString())
    }
}
